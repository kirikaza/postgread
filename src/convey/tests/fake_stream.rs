use super::super::*;
use crate::convey::tests::fake_tls::*;

use ::async_trait::async_trait;
use ::std::any::Any;
use ::std::collections::VecDeque;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::ErrorKind::UnexpectedEof;
use ::std::pin::Pin;
use ::std::sync::{Arc, Mutex};
use ::futures::stream::{Stream, StreamExt};
use ::futures::task::{Context, Poll};

pub struct TwoFakeStreams {
    items: Arc<Mutex<VecDeque<TwoFakeStreamsItem>>>,
    backend_tls_started: bool,
    frontend_tls_started: bool,
}

pub struct FakeStream {
    side: FakeStreamSide,
    items: Arc<Mutex<VecDeque<TwoFakeStreamsItem>>>,
}

#[derive(Debug)]
pub struct TwoFakeStreamsItem {
    side: FakeStreamSide,
    data_form: FakeDataForm,
}

#[derive(Debug)]
pub enum FakeDataForm {
    Encrypted(FakeData),
    NotEncrypted(FakeData),
}

pub enum FakeData {
    TypeByte(u8),
    Body(Box<dyn Any + Send>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FakeStreamSide {
    Backend,
    Frontend,
}

use FakeDataForm::*;
use FakeData::*;
use FakeStreamSide::*;

impl TwoFakeStreams {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(VecDeque::new())),
            backend_tls_started: false,
            frontend_tls_started: false,
        }
    }

    pub fn backend_accepts_tls(&mut self) {
        self.push_type_byte(Backend, TLS_SUPPORTED);
        self.backend_tls_started = true;
    }

    pub fn backend_rejects_tls(&mut self) {
        self.push_type_byte(Backend, TLS_NOT_SUPPORTED);
    }

    pub fn frontend_starts_tls(&mut self) {
        self.frontend_tls_started = true;
    }

    pub fn push_backend<Msg>(&mut self, body: Msg)
    where Msg: 'static + MsgDecode + Send {
        self.push(Backend, body)
    }

    pub fn push_frontend<Msg>(&mut self, body: Msg)
    where Msg: 'static + MsgDecode + Send {
        self.push(Frontend, body)
    }

    pub fn untaken(&mut self) -> VecDeque<TwoFakeStreamsItem> {
        let mut items = self.items.lock().unwrap();
        items.split_off(0)
    }

    fn push<Msg>(&mut self, side: FakeStreamSide, body: Msg)
    where Msg: 'static + MsgDecode + Send {
        if let Some(type_byte) = Msg::TYPE_BYTE_OPT {
            self.push_type_byte(side, type_byte)
        }
        self.push_body(side, body)
    }

    fn push_type_byte(&mut self, side: FakeStreamSide, type_byte: u8) {
        self.push_data(side, TypeByte(type_byte));
    }

    fn push_body<Msg>(&mut self, side: FakeStreamSide, body: Msg)
    where Msg: 'static + MsgDecode + Send {
        self.push_data(side, Body(Box::new(body)))
    }

    fn push_data(&mut self, side: FakeStreamSide, data: FakeData) {
        let encrypted = match side {
            Frontend => self.frontend_tls_started,
            Backend => self.backend_tls_started,
        };
        let data_form = (if encrypted { Encrypted } else { NotEncrypted })(data);
        let mut items = self.items.lock().unwrap();
        items.push_back(TwoFakeStreamsItem {
            side,
            data_form,
        });
    }

    pub fn backend_stream(&mut self) -> FakeStream {
        FakeStream { side: Backend, items: self.items.clone() }
    }

    pub fn frontend_stream(&mut self) -> FakeStream {
        FakeStream { side: Frontend, items: self.items.clone() }
    }
}

macro_rules! impl_fake_read {
    ($fake_stream:expr, $from_form:ident, $from_data:ident) => {{
        let data_form = $fake_stream.fake_read_data().await;
        let data = data_form.map(|form| $from_form($fake_stream.side, form));
        $from_data($fake_stream.side, data)
    }};
}

#[async_trait]
impl ConveyReader for FakeStream {
    async fn read_msg<Msg>(&mut self) -> ReadResult<Msg>
    where Msg: 'static + MsgDecode {
        impl_fake_read!(self, unwrap_not_encrypted, unwrap_msg_body)
    }

    async fn read_type_byte(&mut self) -> IoResult<u8> {
        impl_fake_read!(self, unwrap_not_encrypted, unwrap_type_byte)
    }
}

#[async_trait]
impl ConveyReader for FakeTlsStream<FakeStream> {
    async fn read_msg<Msg>(&mut self) -> ReadResult<Msg>
    where Msg: 'static + MsgDecode {
        impl_fake_read!(self.plain, unwrap_encrypted, unwrap_msg_body)
    }

    async fn read_type_byte(&mut self) -> IoResult<u8> {
        impl_fake_read!(self.plain, unwrap_encrypted, unwrap_type_byte)
    }
}

#[async_trait]
impl ConveyWriter for FakeStream {
    async fn write_bytes(&mut self, _bytes: &[u8]) -> IoResult<()> {
        Ok(())  // Just ignore the bytes for now
    }
}

#[async_trait]
impl ConveyWriter for FakeTlsStream<FakeStream> {
    async fn write_bytes(&mut self, _bytes: &[u8]) -> IoResult<()> {
        Ok(())  // Just ignore the bytes for now
    }
}

fn unwrap_msg_body<Msg>(side: FakeStreamSide, data: IoResult<FakeData>) -> ReadResult<Msg>
where Msg: 'static + MsgDecode {
    match data.map_err(ReadError::IoError)? {
        Body(any) => {
            let msg: Box<Msg> = any.downcast().expect(&format!("fake {:?} gave a message of unexpected type", side));
            Ok(ReadData { bytes: vec![], msg_result: Ok(*msg) })
        }
        TypeByte(_) => panic!("fake {:?} gave a type byte instead of a message", side),
    }
}

fn unwrap_type_byte(side: FakeStreamSide, data: IoResult<FakeData>) -> IoResult<u8> {
    match data? {
        TypeByte(type_byte) => Ok(type_byte),
        Body(_) => panic!("fake {:?} gave a message instead of a type byte", side),
    }
}

fn unwrap_not_encrypted(side: FakeStreamSide, data_form: FakeDataForm) -> FakeData {
    if let NotEncrypted(data) = data_form {
        data
    } else {
        panic!("expected not encrypted data in fake {:?}", side)
    }
}

fn unwrap_encrypted(side: FakeStreamSide, data_form: FakeDataForm) -> FakeData {
    if let Encrypted(data) = data_form {
        data
    } else {
        panic!("expected encrypted data in fake {:?}", side)
    }
}

impl FakeStream {
    async fn fake_read_data(&mut self) -> IoResult<FakeDataForm> {
        self.next().await.ok_or(
            IoError::new(UnexpectedEof, format!("fake {:?} is ended", self.side))
        )
    }
}

impl Stream for FakeStream {
    type Item = FakeDataForm;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<FakeDataForm>> {
        use Poll::*;
        let mut items = self.items.lock().unwrap();
        match items.front() {
            None =>
                Ready(None),
            Some(item) if item.side == self.side =>
                Ready(Some(items.pop_front().unwrap().data_form)),
            Some(_) =>
                Pending,
        }
    }
}

impl Unpin for FakeStream {}

impl Debug for FakeData {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            TypeByte(type_byte) => write!(f, "TypeByte('{}'={})", *type_byte as char, type_byte),
            Body(any) => write!(f, "Body({:?})", any),
        }
    }
}
