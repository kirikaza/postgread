use crate::async_std_compat::compat;
use crate::msg::body::*;
use crate::msg::util::async_io;
use crate::msg::util::decode::{MsgDecode, Problem as DecodeProblem};
use crate::msg::util::encode::{Problem as EncodeProblem};
use crate::msg::util::read::*;

use ::async_std::net::TcpStream;
use ::futures::future::{self, Either, FutureExt};
use ::futures::io::{AsyncReadExt, AsyncWriteExt};
use ::std::io::{Error as IoError};

#[derive(Debug)]
pub enum ConveyError {
    DecodeError(DecodeProblem),
    EncodeError(EncodeProblem),
    IoError(IoError),
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

pub async fn convey<Callback>(
    frontend: TcpStream,
    backend: TcpStream,
    callback: Callback,
) -> ConveyResult<()>
where Callback: Fn(Message) -> () {
    let mut conveyor = Conveyor {
        frontend: &mut compat(frontend),
        backend: &mut compat(backend),
        callback,
    };
    conveyor.go().await
}

struct Conveyor<'a, F, B, C> {
    frontend: &'a mut F,
    backend: &'a mut B,
    callback: C,
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

impl<'a, F, B, C> Conveyor<'a, F, B, C>
where
    F: AsyncReadExt + AsyncWriteExt + Send + Unpin,
    B: AsyncReadExt + AsyncWriteExt + Send + Unpin,
    C: Fn(Message) -> (),
{
    #[allow(clippy::cognitive_complexity)]
    async fn go(self: &mut Self) -> ConveyResult<()> {
        let mut state = match read_frontend_through!(<Initial>, self) {
            Initial::Startup(_) =>
                State::Startup,
            Initial::Cancel(_) =>
                return Err(Todo("Cancel".into())),
            Initial::SSL =>
                return Err(Todo("SSL".into())),
        };
        use TypeByte::*;
        loop {
            let type_byte = self.read_u8().await?;
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
                }
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
        Self::read_msg_mapping_err(self.backend).await
    }

    async fn read_frontend<Msg: MsgDecode>(self: &mut Self) -> ConveyResult<(Vec<u8>, Msg)> {
        Self::read_msg_mapping_err(self.frontend).await
    }

    async fn read_msg_mapping_err<R, Msg>(stream: &mut R) -> ConveyResult<(Vec<u8>, Msg)>
    where
        R: AsyncReadExt + Unpin,
        Msg: MsgDecode,
    {
        let ReadData { bytes, msg_result } = read_msg(stream).await.map_err(|read_err|
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

    async fn read_u8(self: &mut Self) -> ConveyResult<TypeByte> {
        let either = future::select(
            async_io::read_u8(self.backend).boxed(),
            async_io::read_u8(self.frontend).boxed(),
        ).await;
        // TODO: if both futures are ready, do we loose a result of the second one?
        match either {
            Either::Left((backend, _frontend)) => backend.map(TypeByte::Backend),
            Either::Right((frontend, _backend)) => frontend.map(TypeByte::Frontend),
        }.map_err(IoError)
    }

    async fn write_backend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        self.backend.write_all(bytes).await.map_err(IoError)
    }

    async fn write_frontend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        self.frontend.write_all(bytes).await.map_err(IoError)
    }

}
