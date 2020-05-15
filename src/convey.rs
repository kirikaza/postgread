#[cfg(test)]
mod tests;

use crate::msg::body::*;
use crate::msg::type_byte::TypeByte;
use crate::msg::util::async_io;
use crate::msg::util::decode::{MsgDecode, Problem as DecodeProblem};
use crate::msg::util::encode::{Problem as EncodeProblem};
use crate::msg::util::read::*;
use crate::tls::interface::{TlsClient, TlsServer};

use ::async_trait::async_trait;
use ::core::hint::unreachable_unchecked;
use ::futures::future::{self, Either, Future, FutureExt};
use ::futures::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use ::std::convert::TryFrom;
use ::std::io::{Error as IoError, Result as IoResult};

#[derive(Debug)]
pub enum ConveyError {
    DecodeError(DecodeProblem),
    EncodeError(EncodeProblem),
    IoError(IoError),
    TlsError(TlsError),
    LeftUndecoded(usize),
    Todo(String),
    UnexpectedType(State, Side, TypeByte),
    UnknownType(Side, u8),
    Unsupported(&'static str),
}

pub type ConveyResult<T> = Result<T, ConveyError>;

#[derive(Clone, Copy, Debug)]
pub enum Side {
    Backend,
    Frontend,
}

#[derive(Clone, Copy, Debug)]
pub enum State {
    AskedCleartextPassword,
    AskedGssResponse,
    AskedMd5Password,
    Authenticated,
    CommandComplete,
    GotAllBackendParams,
    GotCleartextPassword,
    GotEmptyQueryResponse,
    GotGssResponse,
    GotMd5Password,
    GotQuery,
    QueryAbortedByError,
    QueryResponseWithRows,
    ReadyForQuery,
    Startup,
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
    NegotiateProtocolVersion(&'a NegotiateProtocolVersion),
    NoticeResponse(&'a NoticeResponse),
    ParameterStatus(&'a ParameterStatus),
    ReadyForQuery(&'a ReadyForQuery),
    RowDescription(&'a RowDescription),
}

#[derive(Debug, PartialEq)]
pub enum FrontendMsg<'a> {
    GssResponse(&'a GssResponse),
    Initial(&'a Initial),
    Password(&'a Password),
    Query(&'a Query),
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
                let tls_response = self.read_backend_type_byte().await?;
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
                        return Err(UnknownType(Side::Backend, tls_response))
                    },
                }
                self.write_frontend(&[TLS_SUPPORTED]).await?;
                switch_server_to_tls(&mut self.frontend, &self.frontend_tls_server).await?;
                match read_frontend_through!(<Initial>, self) {
                    Initial::Startup(_) => State::Startup,
                    Initial::Cancel(_) => return Ok(()),
                    _ => return Err(TlsError(TlsError::TlsRequestedInsideTls))
                }
            }
        };
        loop {
            if cfg!(test) {
                eprintln!("conveyor state is {:?}", state);
            }
            let (side, type_byte) = self.read_type_byte_from_both().await?;
            use Side::*;
            use TypeByte as T;
            state = match (side, type_byte, state) {
                (Backend, T::Authentication, _) => {
                    self.process_backend_authentication(type_byte, state).await
                },
                (Backend, T::BackendKeyData, State::Authenticated) => {
                    read_backend_through!(<BackendKeyData>, self);
                    Ok(State::GotAllBackendParams)
                },
                (Backend, T::CommandComplete, State::GotQuery) |
                (Backend, T::CommandComplete, State::CommandComplete) |
                (Backend, T::CommandComplete, State::QueryResponseWithRows) => {
                    read_backend_through!(<CommandComplete>, self);
                    Ok(State::CommandComplete)
                },
                (Backend, T::DataRow, State::QueryResponseWithRows) => {
                    read_backend_through!(<DataRow>, self);
                    Ok(State::QueryResponseWithRows)
                },
                (Backend, T::EmptyQueryResponse, State::GotQuery) => {
                    read_backend_through!(<EmptyQueryResponse>, self);
                    Ok(State::GotEmptyQueryResponse)
                },
                (Backend, T::ErrorResponse, State::CommandComplete) |
                (Backend, T::ErrorResponse, State::GotEmptyQueryResponse) |
                (Backend, T::ErrorResponse, State::GotQuery) |
                (Backend, T::ErrorResponse, State::QueryAbortedByError) |
                (Backend, T::ErrorResponse, State::QueryResponseWithRows) => {
                    read_backend_through!(<ErrorResponse>, self);
                        Ok(State::QueryAbortedByError)
                },
                (Backend, T::ErrorResponse, _) => {
                    read_backend_through!(<ErrorResponse>, self);
                    return Ok(())
                },
                (Backend, T::NegotiateProtocolVersion, State::Startup) |
                (Backend, T::NegotiateProtocolVersion, State::Authenticated) => {
                    read_backend_through!(<NegotiateProtocolVersion>, self);
                    Ok(state)
                },
                (Backend, T::NoticeResponse, _) => {
                    read_backend_through!(<NoticeResponse>, self);
                    Ok(state)
                },
                (Backend, T::ParameterStatus, State::Authenticated) => {
                    read_backend_through!(<ParameterStatus>, self);
                    Ok(State::Authenticated)
                },
                (Frontend, T::GssResponse_Or_Password, State::AskedCleartextPassword) => {
                    read_frontend_through!(<Password>, self);
                    Ok(State::GotCleartextPassword)
                },
                (Frontend, T::GssResponse_Or_Password, State::AskedGssResponse) => {
                    read_frontend_through!(<GssResponse>, self);
                    Ok(State::GotGssResponse)
                },
                (Frontend, T::GssResponse_Or_Password, State::AskedMd5Password) => {
                    read_frontend_through!(<Password>, self);
                    Ok(State::GotMd5Password)
                },
                (Frontend, T::Query, State::ReadyForQuery) => {
                    read_frontend_through!(<Query>, self);
                    Ok(State::GotQuery)
                },
                (Backend, T::ReadyForQuery, State::GotAllBackendParams) |
                (Backend, T::ReadyForQuery, State::GotEmptyQueryResponse) |
                (Backend, T::ReadyForQuery, State::CommandComplete) |
                (Backend, T::ReadyForQuery, State::QueryAbortedByError) => {
                    read_backend_through!(<ReadyForQuery>, self);
                    Ok(State::ReadyForQuery)
                },
                (Backend, T::RowDescription, State::GotQuery) |
                (Backend, T::RowDescription, State::CommandComplete) => {
                    read_backend_through!(<RowDescription>, self);
                    Ok(State::QueryResponseWithRows)
                },
                (Frontend, T::Terminate, State::ReadyForQuery) => {
                    read_frontend_through!(<Terminate>, self);
                    return Ok(())
                },
                _ => Err(UnexpectedType(state, side, type_byte)),
            }?
        }
    }

    async fn process_backend_authentication(&mut self, type_byte: TypeByte, state: State) -> ConveyResult<State> {
        use Authentication as Auth;
        let authentication = read_backend_through!(<Authentication>, self);
        match (authentication, &state) {
            (Auth::CleartextPassword, State::Startup) =>
                Ok(State::AskedCleartextPassword),
            (Auth::Gss, State::Startup) |
            (Auth::Sspi, State::Startup) =>
                Ok(State::AskedGssResponse),
            (Auth::GssContinue {..}, State::GotGssResponse) =>
                Ok(State::AskedGssResponse),
            (Auth::Md5Password {..}, State::Startup) =>
                Ok(State::AskedMd5Password),
            (Auth::Ok, State::GotCleartextPassword) |
            (Auth::Ok, State::GotGssResponse) |
            (Auth::Ok, State::GotMd5Password) |
            (Auth::Ok, State::Startup) =>
                Ok(State::Authenticated),
            (Auth::KerberosV5, State::Startup) =>
                Err(Unsupported(
                    "AuthenticationKerberosV5 is unsupported after PostgreSQL 9.3 \
                    which in turn is unsupported by PostgreSQL maintainers"
                )),
            (Auth::ScmCredential, State::Startup) =>
                Err(Unsupported(
                    "This message type is only issued by pre-9.1 servers. \
                    It may eventually be removed from the protocol specification."
                )),
            (_, State::Startup) =>
                Err(Todo("Authentication::* is not fully implemented yet".into())),
            _ =>
                Err(UnexpectedType(state, Side::Backend, type_byte)),
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

    async fn read_type_byte_from_both(&mut self) -> ConveyResult<(Side, TypeByte)> {
        let either = future::select(
            unwrap_stream!(&mut self.backend, Self::read_type_byte).boxed(),
            unwrap_stream!(&mut self.frontend, Self::read_type_byte).boxed(),
        ).await;
        // TODO: if both futures are ready, do we loose a result of the second one?
        let (side, byte) = match either {
            Either::Left((backend, _frontend)) => backend.map(|byte| (Side::Backend, byte)),
            Either::Right((frontend, _backend)) => frontend.map(|byte| (Side::Frontend, byte)),
        }?;
        let type_byte = TypeByte::try_from(byte).map_err(|_| UnknownType(side, byte))?;
        Ok((side, type_byte))
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

const TLS_SUPPORTED: u8 = b'S';
const TLS_NOT_SUPPORTED: u8 = b'N';
