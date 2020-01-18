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

macro_rules! read_matched_backend_through {
    (
        $self:ident,
        $context:expr,
        match {
            $(
                $msg_type:ident($msg_var:tt) => $handle:expr
            ),+,
        }
    ) => {
        match $self.read_backend_u8().await? {
            $(
                $msg_type::TYPE_BYTE => {
                    let $msg_var = read_backend_through!(<$msg_type>, $self);
                    $handle
                }
            ),+,
            type_byte => {
                Err(UnexpectedType(type_byte, $context.to_owned()))
            },
        }
    }
}

macro_rules! read_matched_frontend_through {
    (
        $self:ident,
        $context:expr,
        match {
            $(
                $msg_type:ident($msg_var:tt) => $handle:expr
            ),+,
        }
    ) => {
        match $self.read_frontend_u8().await? {
            $(
                $msg_type::TYPE_BYTE => {
                    let $msg_var = read_frontend_through!(<$msg_type>, $self);
                    $handle
                }
            ),+,
            type_byte => {
                Err(UnexpectedType(type_byte, $context.to_owned()))
            },
        }
    }
}

enum State {
    ReadyForQuery,
    Final,
}

impl<'a, F, B, C> Conveyor<'a, F, B, C>
where
    F: AsyncReadExt + AsyncWriteExt + Unpin,
    B: AsyncReadExt + AsyncWriteExt + Unpin,
    C: Fn(Message) -> (),
{
    async fn go(self: &mut Self) -> ConveyResult<()> {
        use State::*;
        let mut state = self.wait_initial().await?;
        loop {
            match state {
                ReadyForQuery =>
                    state = self.wait_query().await?,
                Final =>
                    return Ok(())
            }
        }
    }

    async fn wait_initial(self: &mut Self) -> ConveyResult<State> {
        match read_frontend_through!(<Initial>, self) {
            Initial::Startup(_) =>
                self.continue_startup().await,
            Initial::Cancel(_) =>
                Err(Todo("Cancel".into())),
            Initial::SSL =>
                Err(Todo("SSL".into())),
        }
    }

    async fn continue_startup(self: &mut Self) -> ConveyResult<State> {
        read_matched_backend_through!(
            self,
            "continue_startup",
            match {
                Authentication(authentication) =>
                    self.process_authentication(authentication).await,
                ErrorResponse(_) =>
                    Ok(State::Final),
            }
        )
    }

    async fn process_authentication(self: &mut Self, authentication: Authentication) -> ConveyResult<State> {
        match authentication {
            Authentication::Ok => self.finish_startup().await,
            _ => Err(Todo("Authentication::TYPE_BYTE != Ok".into())),
        }
    }

    async fn finish_startup(self: &mut Self) -> ConveyResult<State> {
        loop {
            read_matched_backend_through!(
                self,
                "finish_startup/1",
                match {
                    ParameterStatus(_) => continue,
                    BackendKeyData(_) => break,
                    ErrorResponse(_) => return Ok(State::Final),
                }
            )?
        }
        read_matched_backend_through!(
            self,
            "finish_startup/2",
            match {
                ReadyForQuery(_) => Ok(State::ReadyForQuery),
                ErrorResponse(_) => Ok(State::Final),
            }
        )
    }
    
    async fn wait_query(self: &mut Self) -> ConveyResult<State> {
        read_matched_frontend_through!(
            self,
            "wait_query_and_responses/1",
            match {
                Query(_) => Ok(()),
                Terminate(_) => return Ok(State::Final),
            }
        )?;
        self.wait_query_responses().await
    }

    async fn wait_query_responses(self: &mut Self) -> ConveyResult<State> {
        enum QueryState {
            NextResponse,
            NextDataRow,
            NoMoreResponses,
        }
        use QueryState::*;
        let mut query_state = NextResponse;
        loop {
            match query_state {
                NextResponse => read_matched_backend_through!(
                    self,
                    "wait_query_and_responses/2",
                    match {
                        CommandComplete(_) => Ok(()),
                        ReadyForQuery(_) => return Ok(State::ReadyForQuery),
                        RowDescription(_) => { query_state = NextDataRow; Ok(()) },
                        ErrorResponse(_) => { query_state = NoMoreResponses; Ok(()) },
                    }
                )?,
                NextDataRow => read_matched_backend_through!(
                    self,
                    "wait_query_and_responses/3",
                    match {
                        CommandComplete(_) => { query_state = NextResponse; Ok(()) },
                        DataRow(_) => Ok(()),
                        ErrorResponse(_) => { query_state = NoMoreResponses; Ok(()) },
                    }
                )?,
                NoMoreResponses => read_matched_backend_through!(
                    self,
                    "wait_query_and_responses/4",
                    match {
                        ReadyForQuery(_) => return Ok(State::ReadyForQuery),
                        ErrorResponse(_) => { query_state = NoMoreResponses; Ok(()) },
                    }
                )?,
            };
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
