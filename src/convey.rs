use crate::msg::body::*;
use crate::msg::util::async_io;
use crate::msg::util::decode::{MsgDecode, Problem as DecodeProblem};
use crate::msg::util::encode::{Problem as EncodeProblem};
use crate::msg::util::read::*;
use crate::tls::interface::TlsProvider;

use ::core::hint::unreachable_unchecked;
use ::futures::future::{self, Either, Future, FutureExt};
use ::futures::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use ::std::io::{Error as IoError};

#[derive(Debug)]
pub enum ConveyError {
    DecodeError(DecodeProblem),
    EncodeError(EncodeProblem),
    IoError(IoError),
    TlsError(TlsError),
    LeftUndecoded(usize),
    Todo(String),
    UnexpectedType(TypeByte, State),
    UnknownType(TypeByte),
}

pub type ConveyResult<T> = Result<T, ConveyError>;

#[derive(Debug)]
pub enum TypeByte {
    Backend(u8),
    Frontend(u8),
}

#[derive(Debug)]
pub enum State {
    Startup,
    Authenticated,
    GotAllBackendParams,
    ReadyForQuery,
    GotQuery,
    GotEmptyQueryResponse,
    CommandComplete,
    QueryResponseWithRows,
    QueryAbortedByError,
}

#[derive(Debug)]
pub enum Message<'a> {
    Backend(BackendMsg<'a>),
    Frontend(FrontendMsg<'a>),
}

#[derive(Debug)]
pub enum BackendMsg<'a> {
    Authentication(&'a Authentication),
    BackendKeyData(&'a BackendKeyData),
    CommandComplete(&'a CommandComplete),
    DataRow(&'a DataRow),
    EmptyQueryResponse(&'a EmptyQueryResponse),
    ErrorResponse(&'a ErrorResponse),
    NoticeResponse(&'a NoticeResponse),
    ParameterStatus(&'a ParameterStatus),
    ReadyForQuery(&'a ReadyForQuery),
    RowDescription(&'a RowDescription),
}

#[derive(Debug)]
pub enum FrontendMsg<'a> {
    Query(&'a Query),
    Initial(&'a Initial),
    Terminate(&'a Terminate),
}

#[derive(Debug)]
pub enum TlsError {
    HandshakeDisrupted,
    TlsRequestedInsideTls,
}

pub struct Conveyor<FrontPlain, BackPlain, FrontTlsProvider, BackTlsProvider, Callback>
where
    FrontPlain: AsyncRead + AsyncWrite + Send + Unpin,
    BackPlain: AsyncRead + AsyncWrite + Send + Unpin,
    FrontTlsProvider: TlsProvider<FrontPlain>,
    BackTlsProvider: TlsProvider<BackPlain>,
{
    frontend: StreamWrap<FrontPlain, FrontTlsProvider::Tls>,
    backend: StreamWrap<BackPlain, BackTlsProvider::Tls>,
    frontend_tls_provider: FrontTlsProvider,
    _backend_tls_provider: BackTlsProvider,
    callback: Callback,
}

use ConveyError::*;

macro_rules! read_through {
    (
        <$msg_type:ident>,
        $self:ident,
        $read:ident,
        $callback:ident($cb_wrap:ident),
        $write:ident
    ) => {{
        let (bytes, msg) = $self.$read::<$msg_type>().await?;
        $self.$callback($cb_wrap::$msg_type(&msg));
        $self.$write(&bytes).await?;
        msg
    }};
}

macro_rules! read_backend_through {
    (
        <$msg_type:ident>,
        $self:ident
    ) => {
        read_through!(<$msg_type>, $self, read_backend, callback_backend(BackendMsg), write_frontend)
    }
}

macro_rules! read_frontend_through {
    (
        <$msg_type:ident>,
        $self:ident
    ) => {
        read_through!(<$msg_type>, $self, read_frontend, callback_frontend(FrontendMsg), write_backend)
    }
}

macro_rules! unwrap_stream {
    ($wrap:expr, $func:expr) => { unwrap_stream($wrap, $func, $func) }
}

impl<'a, FrontPlain, BackPlain, FrontTlsProvider, BackTlsProvider, Callback>
Conveyor<FrontPlain, BackPlain, FrontTlsProvider, BackTlsProvider, Callback>
where
    FrontPlain: AsyncRead + AsyncWrite + Send + Unpin,
    BackPlain: AsyncRead + AsyncWrite + Send + Unpin,
    FrontTlsProvider: TlsProvider<FrontPlain> + Send,
    BackTlsProvider: TlsProvider<BackPlain> + Send,
    Callback: Fn(Message) -> () + Send,
{
    pub async fn start(
        frontend: FrontPlain,
        backend: BackPlain,
        frontend_tls_provider: FrontTlsProvider,
        backend_tls_provider: BackTlsProvider,
        callback: Callback,
    ) -> ConveyResult<()> {
        Self {
            frontend: StreamWrap::Plain(frontend),
            backend: StreamWrap::Plain(backend),
            frontend_tls_provider,
            _backend_tls_provider: backend_tls_provider,
            callback,
        }.go().await
    }

    #[allow(clippy::cognitive_complexity)]
    async fn go(self: &mut Self) -> ConveyResult<()> {
        let mut state = match read_frontend_through!(<Initial>, self) {
            Initial::Startup(_) => State::Startup,
            Initial::Cancel(_) => return Ok(()),
            Initial::TLS => {
                self.process_tls_request().await?;
                match read_frontend_through!(<Initial>, self) {
                    Initial::Startup(_) => State::Startup,
                    Initial::Cancel(_) => return Ok(()),
                    _ => return Err(TlsError(TlsError::TlsRequestedInsideTls))
                }
            }
        };
        use TypeByte::*;
        loop {
            let type_byte = self.read_u8_from_both().await?;
            state = match type_byte {
                Backend(Authentication::TYPE_BYTE) => match state {
                    State::Startup => {
                        let authentication = read_backend_through!(<Authentication>, self);
                        self.process_authentication(authentication).await
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                }
                Backend(BackendKeyData::TYPE_BYTE) => match state {
                    State::Authenticated => {
                        read_backend_through!(<BackendKeyData>, self);
                        Ok(State::GotAllBackendParams)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Backend(CommandComplete::TYPE_BYTE) => match state {
                    State::GotQuery |
                    State::CommandComplete |
                    State::QueryResponseWithRows => {
                        read_backend_through!(<CommandComplete>, self);
                        Ok(State::CommandComplete)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Backend(DataRow::TYPE_BYTE) => match state {
                    State::QueryResponseWithRows => {
                        read_backend_through!(<DataRow>, self);
                        Ok(State::QueryResponseWithRows)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Backend(EmptyQueryResponse::TYPE_BYTE) => {
                    match state {
                        State::GotQuery => {
                            read_backend_through!(<EmptyQueryResponse>, self);
                            Ok(State::GotEmptyQueryResponse)
                        },
                        _ => Err(UnexpectedType(type_byte, state)),
                    }
                },
                Backend(ErrorResponse::TYPE_BYTE) => {
                    read_backend_through!(<ErrorResponse>, self);
                    match state {
                        State::GotQuery |
                        State::GotEmptyQueryResponse |
                        State::CommandComplete |
                        State::QueryResponseWithRows |
                        State::QueryAbortedByError => {
                            Ok(State::QueryAbortedByError)
                        },
                        _ => return Ok(())
                    }
                },
                Backend(NoticeResponse::TYPE_BYTE) => {
                   read_backend_through!(<NoticeResponse>, self);
                    Ok(state)
                },
                Backend(ParameterStatus::TYPE_BYTE) => match state {
                    State::Authenticated => {
                        read_backend_through!(<ParameterStatus>, self);
                        Ok(State::Authenticated)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Frontend(Query::TYPE_BYTE) => match state {
                    State::ReadyForQuery => {
                        read_frontend_through!(<Query>, self);
                        Ok(State::GotQuery)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Backend(ReadyForQuery::TYPE_BYTE) => match state {
                    State::GotAllBackendParams |
                    State::GotEmptyQueryResponse |
                    State::CommandComplete |
                    State::QueryAbortedByError => {
                        read_backend_through!(<ReadyForQuery>, self);
                        Ok(State::ReadyForQuery)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Backend(RowDescription::TYPE_BYTE) => match state {
                    State::GotQuery |
                    State::CommandComplete => {
                        read_backend_through!(<RowDescription>, self);
                        Ok(State::QueryResponseWithRows)
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                Frontend(Terminate::TYPE_BYTE) => match state {
                    State::ReadyForQuery => {
                        read_frontend_through!(<Terminate>, self);
                        return Ok(())
                    },
                    _ => Err(UnexpectedType(type_byte, state)),
                },
                _ => Err(UnknownType(type_byte)),
            }?
        }
    }

    async fn process_tls_request(self: &mut Self) -> ConveyResult<()> {
        let tls_response = self.read_backend_u8().await?;
        const TLS_SUPPORTED: u8 = b'S';
        const TLS_NOT_SUPPORTED: u8 = b'N';
        match tls_response {
            TLS_NOT_SUPPORTED => {
                self.write_frontend(&[TLS_SUPPORTED]).await?;
                switch_to_tls(&mut self.frontend, &self.frontend_tls_provider).await
            },
            TLS_SUPPORTED => Err(Todo("backend with TLS".into())),
            ErrorResponse::TYPE_BYTE => {
                // "This would only occur if the server predates the addition of SSL support to PostgreSQL"
                read_backend_through!(<ErrorResponse>, self);
                Ok(())
            }
            _ => Err(UnknownType(TypeByte::Backend(tls_response))),
        }
    }

    async fn process_authentication(self: &mut Self, authentication: Authentication) -> ConveyResult<State> {
        match authentication {
            Authentication::Ok => Ok(State::Authenticated),
            _ => Err(Todo("Authentication::TYPE_BYTE != Ok".into())),
        }
    }

    // util:

    fn callback_backend(self: &Self, wrap: BackendMsg<'a>) {
        (self.callback)(Message::Backend(wrap));
    }

    fn callback_frontend(self: &Self, wrap: FrontendMsg<'a>) {
        (self.callback)(Message::Frontend(wrap));
    }

    async fn read_backend<Msg: MsgDecode>(self: &mut Self) -> ConveyResult<(Vec<u8>, Msg)> {
        unwrap_stream!(&mut self.backend, Self::read_msg_mapping_err).await
    }

    async fn read_frontend<Msg: MsgDecode>(self: &mut Self) -> ConveyResult<(Vec<u8>, Msg)> {
        unwrap_stream!(&mut self.frontend, Self::read_msg_mapping_err).await
    }

    async fn read_msg_mapping_err<R, Msg>(reader: &mut R) -> ConveyResult<(Vec<u8>, Msg)>
    where
        R: AsyncRead + Unpin,
        Msg: MsgDecode,
    {
        let ReadData { bytes, msg_result } = read_msg(reader).await.map_err(|read_err|
            match read_err {
                ReadError::IoError(io_error) => ConveyError::IoError(io_error),
                ReadError::EncodeError(encode_error) => ConveyError::EncodeError(encode_error),
            }
        )?;
        let message = msg_result.map_err(|msg_error|
            match msg_error {
                MsgError::DecodeError(decode_error) => ConveyError::DecodeError(decode_error),
                MsgError::LeftUndecoded(left) => ConveyError::LeftUndecoded(left),
            }
        )?;
        Ok((bytes, message))
    }

    async fn read_u8_from_both(self: &mut Self) -> ConveyResult<TypeByte> {
        let either = future::select(
            unwrap_stream!(&mut self.backend, Self::read_u8).boxed(),
            unwrap_stream!(&mut self.frontend, Self::read_u8).boxed(),
        ).await;
        // TODO: if both futures are ready, do we loose a result of the second one?
        match either {
            Either::Left((backend, _frontend)) => backend.map(TypeByte::Backend),
            Either::Right((frontend, _backend)) => frontend.map(TypeByte::Frontend),
        }
    }

    async fn read_backend_u8(self: &mut Self) -> ConveyResult<u8> {
        unwrap_stream!(&mut self.backend, Self::read_u8).await
    }

    async fn read_u8<R>(reader: &mut R) -> ConveyResult<u8>
    where R: AsyncRead + Unpin {
        async_io::read_u8(reader).await.map_err(IoError)
    }

    async fn write_backend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        unwrap_stream!(&mut self.backend, |wr| Self::write_bytes(wr, bytes)).await
    }

    async fn write_frontend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        unwrap_stream!(&mut self.frontend, |wr| Self::write_bytes(wr, bytes)).await
    }

    async fn write_bytes<W>(writer: &mut W, bytes: &[u8]) -> ConveyResult<()>
    where W: AsyncWrite + Unpin {
        writer.write_all(bytes).await.map_err(IoError)
    }
}

async fn unwrap_stream<'w, Plain, Tls, FnPlain, FnTls, Ok>(
    wrap: &'w mut StreamWrap<Plain, Tls>,
    fn_plain: impl Fn(&'w mut Plain) -> FnPlain,
    fn_tls: impl Fn(&'w mut Tls) -> FnTls
) -> ConveyResult<Ok>
    where
        FnPlain: Future<Output=ConveyResult<Ok>>,
        FnTls: Future<Output=ConveyResult<Ok>>,
{
    use StreamWrap::*;
    match wrap {
        Plain(ref mut plain) => fn_plain(plain).await,
        TlsHandshake => Err(TlsError(TlsError::HandshakeDisrupted)),
        Tls(ref mut tls) => fn_tls(tls).await,
    }
}

async fn switch_to_tls<Plain, TlsProvider>(
    wrap: &mut StreamWrap<Plain, TlsProvider::Tls>,
    tls_provider: &TlsProvider,
) -> ConveyResult<()>
where
    Plain: AsyncRead + AsyncWrite + Send + Unpin,
    TlsProvider: self::TlsProvider<Plain>,
{
    use StreamWrap::*;
    if let Some(plain_stream) = wrap.replace_plain_with(TlsHandshake) {
        let tls_stream = tls_provider.handshake(plain_stream).await.unwrap();
        core::mem::replace(wrap, Tls(tls_stream));
        Ok(())
    } else {
        Err(TlsError(TlsError::HandshakeDisrupted))
    }
}

enum StreamWrap<Plain, Tls> {
    Plain(Plain),
    TlsHandshake,
    Tls(Tls),
}

impl<Plain, Tls> StreamWrap<Plain, Tls> {
    fn replace_plain_with(self: &mut Self, value: Self) -> Option<Plain> {
        use StreamWrap::Plain;
        match self {
            Plain(_) => match core::mem::replace(self, value) {
                Plain(plain) => Some(plain),
                _ => unsafe { unreachable_unchecked() },
            }
            _ => None
        }
    }
}
