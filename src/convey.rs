use crate::async_std_compat::compat;
use crate::msg::body::*;
use crate::msg::util::async_io::*;
use crate::msg::util::decode::{MsgDecode, Problem as DecodeProblem};
use crate::msg::util::encode::{Problem as EncodeProblem};
use crate::msg::util::read::*;

use ::async_std::net::TcpStream;
use ::futures::io::{AsyncReadExt, AsyncWriteExt};
use ::std::io::{Error as IoError};

#[derive(Debug)]
pub enum ConveyError {
    DecodeError(DecodeProblem),
    EncodeError(EncodeProblem),
    IoError(IoError),
    LeftUndecoded(usize),
    Todo(String),
    UnexpectedType(u8, String),
}

pub type ConveyResult<T> = Result<T, ConveyError>;

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

enum State {
    Initial,
    Startup,
    Authenticated,
    GotAllBackendParams,
    ReadyForQuery,
    GotQuery,
    NextDataRow,
    NoMoreResponses,
    Final,
}

impl<'a, F, B, C> Conveyor<'a, F, B, C>
where
    F: AsyncReadExt + AsyncWriteExt + Unpin,
    B: AsyncReadExt + AsyncWriteExt + Unpin,
    C: Fn(Message) -> (),
{
    async fn go(self: &mut Self) -> ConveyResult<()> {
        let mut state = State::Initial;
        loop {
            state = match state {
                State::Initial => match read_frontend_through!(<Initial>, self) {
                    Initial::Startup(_) =>
                        Ok(State::Startup),
                    Initial::Cancel(_) =>
                        Err(Todo("Cancel".into())),
                    Initial::SSL =>
                        Err(Todo("SSL".into())),
                }
                State::Startup => match self.read_backend_u8().await? {
                    Authentication::TYPE_BYTE => {
                        let authentication = read_backend_through!(<Authentication>, self);
                        self.process_authentication(authentication).await
                    },
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::Final)
                    }
                    type_byte => {
                        Err(UnexpectedType(type_byte, "continue_startup".to_owned()))
                    },
                },
                State::Authenticated => match self.read_backend_u8().await? {
                    ParameterStatus::TYPE_BYTE => {
                        read_backend_through!(<ParameterStatus>, self);
                        Ok(State::Authenticated)
                    },
                    BackendKeyData::TYPE_BYTE => {
                        read_backend_through!(<BackendKeyData>, self);
                        Ok(State::GotAllBackendParams)
                    }
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::Final)
                    }
                    type_byte => {
                        Err(UnexpectedType(type_byte, "get_backend_param".to_owned()))
                    },
                },
                State::GotAllBackendParams => match self.read_backend_u8().await? {
                    ReadyForQuery::TYPE_BYTE => {
                        read_backend_through!(<ReadyForQuery>, self);
                        Ok(State::ReadyForQuery)
                    },
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::Final)
                    }
                    type_byte => {
                        Err(UnexpectedType(type_byte, "finish_startup".to_owned()))
                    },
                },
                State::ReadyForQuery => match self.read_frontend_u8().await? {
                    Query::TYPE_BYTE => {
                        read_frontend_through!(<Query>, self);
                        Ok(State::GotQuery)
                    },
                    Terminate::TYPE_BYTE => {
                        read_frontend_through!(<Terminate>, self);
                        Ok(State::Final)
                    },
                    type_byte => {
                        Err(UnexpectedType(type_byte, "get_query".to_owned()))
                    },
                },
                State::GotQuery => match self.read_backend_u8().await? {
                    CommandComplete::TYPE_BYTE => {
                        read_backend_through!(<CommandComplete>, self);
                        Ok(State::GotQuery)
                    },
                    ReadyForQuery::TYPE_BYTE => {
                        read_backend_through!(<ReadyForQuery>, self);
                        Ok(State::ReadyForQuery)
                    },
                    RowDescription::TYPE_BYTE => {
                        read_backend_through!(<RowDescription>, self);
                        Ok(State::NextDataRow)
                    },
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::NoMoreResponses)
                    },
                    type_byte => {
                        Err(UnexpectedType(type_byte, "get_query_response".to_owned()))
                    },
                },
                State::NextDataRow => match self.read_backend_u8().await? {
                    CommandComplete::TYPE_BYTE => {
                        read_backend_through!(<CommandComplete>, self);
                        Ok(State::GotQuery)
                    },
                    DataRow::TYPE_BYTE => {
                        read_backend_through!(<DataRow>, self);
                        Ok(State::NextDataRow)
                    },
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::NoMoreResponses)
                    },
                    type_byte => {
                        Err(UnexpectedType(type_byte, "get_data_row".to_owned()))
                    },
                },
                State::NoMoreResponses => match self.read_backend_u8().await? {
                    ReadyForQuery::TYPE_BYTE => {
                        read_backend_through!(<ReadyForQuery>, self);
                        Ok(State::ReadyForQuery)
                    },
                    ErrorResponse::TYPE_BYTE => {
                        read_backend_through!(<ErrorResponse>, self);
                        Ok(State::NoMoreResponses)
                    },
                    type_byte => {
                        Err(UnexpectedType(type_byte, "finish_query_responses".to_owned()))
                    },
                },
                State::Final =>
                    return Ok(())
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

    async fn read_backend_u8(self: &mut Self) -> ConveyResult<u8> {
        read_u8(self.backend).await.map_err(IoError)
    }

    async fn read_frontend_u8(self: &mut Self) -> ConveyResult<u8> {
        read_u8(self.frontend).await.map_err(IoError)
    }

    async fn write_backend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        self.backend.write_all(bytes).await.map_err(IoError)
    }

    async fn write_frontend(self: &mut Self, bytes: &[u8]) -> ConveyResult<()> {
        self.frontend.write_all(bytes).await.map_err(IoError)
    }

}
