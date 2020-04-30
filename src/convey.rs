#[cfg(test)]
mod tests;

use crate::msg::body::*;
use crate::msg::util::async_io;
use crate::msg::util::decode::{MsgDecode, Problem as DecodeProblem};
use crate::msg::util::encode::{Problem as EncodeProblem};
use crate::msg::util::read::*;
use crate::tls::interface::{TlsClient, TlsServer};

use ::async_trait::async_trait;
use ::core::hint::unreachable_unchecked;
use ::futures::future::{self, Either, Future, FutureExt};
use ::futures::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use ::std::io::{Error as IoError, Result as IoResult};

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

#[derive(Debug, PartialEq)]
pub enum Message<'a> {
    Backend(BackendMsg<'a>),
    Frontend(FrontendMsg<'a>),
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

pub async fn convey<FrontPlain, BackPlain, FrontTlsServer, BackTlsClient, Callback>(
    frontend: FrontPlain,
    backend: BackPlain,
    frontend_tls_server: FrontTlsServer,
    backend_tls_client: BackTlsClient,
    callback: Callback,
) -> ConveyResult<()>
where
    FrontPlain: AsyncRead + AsyncWrite + Send + Unpin,
    BackPlain: AsyncRead + AsyncWrite + Send + Unpin,
    FrontTlsServer: TlsServer<FrontPlain> + Send,
    BackTlsClient: TlsClient<BackPlain> + Send,
    FrontTlsServer::Tls: AsyncRead + AsyncWrite,
    BackTlsClient::Tls: AsyncRead + AsyncWrite,
    Callback: Fn(Message) -> () + Send,
{
    Conveyor::new(
        frontend,
        backend,
        frontend_tls_server,
        backend_tls_client,
        callback,
    ).go().await
}

struct Conveyor<FrontPlain, BackPlain, FrontTlsServer, BackTlsClient, Callback>
where
    FrontPlain: Send + Unpin,
    BackPlain: Send + Unpin,
    FrontTlsServer: TlsServer<FrontPlain>,
    BackTlsClient: TlsClient<BackPlain>,
{
    frontend: StreamWrap<FrontPlain, FrontTlsServer::Tls>,
    backend: StreamWrap<BackPlain, BackTlsClient::Tls>,
    frontend_tls_server: FrontTlsServer,
    backend_tls_client: BackTlsClient,
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

impl<'a, FrontPlain, BackPlain, FrontTlsServer, BackTlsClient, Callback>
Conveyor<FrontPlain, BackPlain, FrontTlsServer, BackTlsClient, Callback>
where
    FrontPlain: ConveyReader + ConveyWriter,
    BackPlain: ConveyReader + ConveyWriter,
    FrontTlsServer: TlsServer<FrontPlain> + Send,
    BackTlsClient: TlsClient<BackPlain> + Send,
    FrontTlsServer::Tls: ConveyReader + ConveyWriter,
    BackTlsClient::Tls: ConveyReader + ConveyWriter,
    Callback: FnMut(Message) -> () + Send,
{
    fn new(
        frontend: FrontPlain,
        backend: BackPlain,
        frontend_tls_server: FrontTlsServer,
        backend_tls_client: BackTlsClient,
        callback: Callback,
    ) -> Self {
        Conveyor {
            frontend: StreamWrap::Plain(frontend),
            backend: StreamWrap::Plain(backend),
            frontend_tls_server,
            backend_tls_client,
            callback,
        }
    }
    #[allow(clippy::cognitive_complexity)]
    async fn go(&mut self) -> ConveyResult<()> {
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
            if cfg!(test) {
                eprintln!("conveyor state is {:?}", state);
            }
            let type_byte = self.read_type_byte_from_both().await?;
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

    async fn process_tls_request(&mut self) -> ConveyResult<()> {
        let tls_response = self.read_backend_type_byte().await?;
        const TLS_SUPPORTED: u8 = b'S';
        const TLS_NOT_SUPPORTED: u8 = b'N';
        match tls_response {
            TLS_NOT_SUPPORTED => {},
            TLS_SUPPORTED => {
                switch_client_to_tls(&mut self.backend, &self.backend_tls_client).await?;
            },
            ErrorResponse::TYPE_BYTE => {
                // "This would only occur if the server predates the addition of SSL support to PostgreSQL"
                read_backend_through!(<ErrorResponse>, self);
                return Ok(())
            }
            _ => {
                return Err(UnknownType(TypeByte::Backend(tls_response)))
            },
        }
        self.write_frontend(&[TLS_SUPPORTED]).await?;
        switch_server_to_tls(&mut self.frontend, &self.frontend_tls_server).await
    }

    async fn process_authentication(&mut self, authentication: Authentication) -> ConveyResult<State> {
        match authentication {
            Authentication::Ok => Ok(State::Authenticated),
            _ => Err(Todo("Authentication::TYPE_BYTE != Ok".into())),
        }
    }

    // util:

    fn callback_backend(&mut self, wrap: BackendMsg<'a>) {
        (self.callback)(Message::Backend(wrap));
    }

    fn callback_frontend(&mut self, wrap: FrontendMsg<'a>) {
        (self.callback)(Message::Frontend(wrap));
    }

    async fn read_backend<Msg>(&mut self) -> ConveyResult<(Vec<u8>, Msg)>
    where Msg: 'static + MsgDecode {
        unwrap_stream!(&mut self.backend, Self::read_msg_mapping_err).await
    }

    async fn read_frontend<Msg>(&mut self) -> ConveyResult<(Vec<u8>, Msg)>
    where Msg: 'static + MsgDecode {
        unwrap_stream!(&mut self.frontend, Self::read_msg_mapping_err).await
    }

    async fn read_msg_mapping_err<R, Msg>(reader: &mut R) -> ConveyResult<(Vec<u8>, Msg)>
    where
        R: ConveyReader,
        Msg: 'static + MsgDecode,
    {
        let ReadData { bytes, msg_result } = reader.read_msg().await.map_err(|read_err|
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

    async fn read_type_byte_from_both(&mut self) -> ConveyResult<TypeByte> {
        let either = future::select(
            unwrap_stream!(&mut self.backend, Self::read_type_byte).boxed(),
            unwrap_stream!(&mut self.frontend, Self::read_type_byte).boxed(),
        ).await;
        // TODO: if both futures are ready, do we loose a result of the second one?
        match either {
            Either::Left((backend, _frontend)) => backend.map(TypeByte::Backend),
            Either::Right((frontend, _backend)) => frontend.map(TypeByte::Frontend),
        }
    }

    async fn read_backend_type_byte(&mut self) -> ConveyResult<u8> {
        unwrap_stream!(&mut self.backend, Self::read_type_byte).await
    }

    async fn read_type_byte(reader: &mut impl ConveyReader) -> ConveyResult<u8> {
        reader.read_type_byte().await.map_err(IoError)
    }

    async fn write_backend(&mut self, bytes: &[u8]) -> ConveyResult<()> {
        unwrap_stream!(&mut self.backend, |wr| Self::write_bytes(wr, bytes)).await
    }

    async fn write_frontend(&mut self, bytes: &[u8]) -> ConveyResult<()> {
        unwrap_stream!(&mut self.frontend, |wr| Self::write_bytes(wr, bytes)).await
    }

    async fn write_bytes(writer: &mut impl ConveyWriter, bytes: &[u8]) -> ConveyResult<()> {
        writer.write_bytes(bytes).await.map_err(IoError)
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

async fn switch_server_to_tls<Plain, TlsServer>(
    wrap: &mut StreamWrap<Plain, TlsServer::Tls>,
    tls_server: &TlsServer,
) -> ConveyResult<()>
where
    Plain: Send + Unpin,
    TlsServer: self::TlsServer<Plain>,
{
    switch_to_tls(wrap, |plain| { tls_server.accept(plain) }).await
}

async fn switch_client_to_tls<Plain, TlsClient>(
    wrap: &mut StreamWrap<Plain, TlsClient::Tls>,
    tls_client: &TlsClient,
) -> ConveyResult<()>
where
    Plain: Send + Unpin,
    TlsClient: self::TlsClient<Plain>,
{
    switch_to_tls(wrap, |plain| { tls_client.connect(plain) }).await
}

async fn switch_to_tls<Plain, FutHandshake, TlsProviderImpl, TlsProviderError>(
    wrap: &mut StreamWrap<Plain, TlsProviderImpl>,
    fn_handshake: impl Fn(Plain) -> FutHandshake,
) -> ConveyResult<()>
where
    Plain: Send + Unpin,
    FutHandshake: Future<Output=Result<TlsProviderImpl, TlsProviderError>>,
    TlsProviderError: std::fmt::Debug,
{
    use StreamWrap::*;
    if let Some(plain_stream) = wrap.replace_plain_with(TlsHandshake) {
        let tls_stream = fn_handshake(plain_stream).await.unwrap();
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
    fn replace_plain_with(&mut self, value: Self) -> Option<Plain> {
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

#[async_trait]
trait ConveyReader : Send + Unpin {
    async fn read_msg<Msg>(&mut self) -> ReadResult<Msg>
    where Msg: 'static + MsgDecode;

    async fn read_type_byte(&mut self) -> IoResult<u8>;
}

#[async_trait]
impl<R> ConveyReader for R
where R: AsyncRead + Send + Unpin {
    async fn read_msg<Msg>(&mut self) -> ReadResult<Msg>
    where Msg: 'static + MsgDecode {
        read_msg(self).await
    }

    async fn read_type_byte(&mut self) -> IoResult<u8> {
        async_io::read_u8(self).await
    }
}

#[async_trait]
trait ConveyWriter : Send + Unpin {
    async fn write_bytes(&mut self, bytes: &[u8]) -> IoResult<()>;
}

#[async_trait]
impl<W> ConveyWriter for W
where W: AsyncWrite + Send + Unpin {
    async fn write_bytes(&mut self, bytes: &[u8]) -> IoResult<()> {
        self.write_all(bytes).await
    }
}
