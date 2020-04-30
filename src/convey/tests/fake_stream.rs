use super::super::*;

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
}

pub struct FakeStream {
    side: FakeStreamSide,
    items: Arc<Mutex<VecDeque<TwoFakeStreamsItem>>>,
}

#[derive(Debug)]
pub struct TwoFakeStreamsItem {
    side: FakeStreamSide,
    msg_part: FakeMsgPart,
}

pub enum FakeMsgPart {
    TypeByte(u8),
    Body(Box<dyn Any + Send>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FakeStreamSide {
    Backend,
    Frontend,
}

use FakeMsgPart::*;
use FakeStreamSide::*;

impl TwoFakeStreams {
    pub fn new() -> Self {
        Self { items: Arc::new(Mutex::new(VecDeque::new())) }
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
            self.push_type(side, type_byte)
        }
        self.push_body(side, body)
    }

    fn push_type(&mut self, side: FakeStreamSide, type_byte: u8) {
        self.push_msg_part(side, TypeByte(type_byte));
    }

    fn push_body<Msg>(&mut self, side: FakeStreamSide, body: Msg)
    where Msg: 'static + MsgDecode + Send {
        self.push_msg_part(side, Body(Box::new(body)))
    }

    fn push_msg_part(&mut self, side: FakeStreamSide, msg_part: FakeMsgPart) {
        let mut items = self.items.lock().unwrap();
        items.push_back(TwoFakeStreamsItem {
            side,
            msg_part,
        });
    }

    pub fn backend_stream(&mut self) -> FakeStream {
        FakeStream { side: Backend, items: self.items.clone() }
    }

    pub fn frontend_stream(&mut self) -> FakeStream {
        FakeStream { side: Frontend, items: self.items.clone() }
    }
}

#[async_trait]
impl ConveyReader for FakeStream {
    async fn read_msg<Msg>(&mut self) -> ReadResult<Msg>
    where Msg: 'static + MsgDecode {
        match self.fake_read_msg_part().await.map_err(ReadError::IoError)? {
            Body(any) => {
                let msg: Box<Msg> = any.downcast().expect("fake stream gave a message of unexpected type");
                Ok(ReadData { bytes: vec![], msg_result: Ok(*msg) })
            }
            TypeByte(_) => panic!("fake stream gave a type byte instead of a message"),
        }
    }

    async fn read_type_byte(&mut self) -> IoResult<u8> {
        match self.fake_read_msg_part().await? {
            TypeByte(type_byte) => Ok(type_byte),
            Body(_) => panic!("fake stream gave a message instead of a type byte"),
        }
    }
}

#[async_trait]
impl ConveyWriter for FakeStream {
    async fn write_bytes(&mut self, _bytes: &[u8]) -> IoResult<()> {
        Ok(())  // Just ignore the bytes for now
    }
}

impl FakeStream {
    async fn fake_read_msg_part(&mut self) -> IoResult<FakeMsgPart> {
        self.next().await.ok_or(
            IoError::new(UnexpectedEof, format!("fake {:?} is ended", self.side))
        )
    }
}


impl Stream for FakeStream {
    type Item = FakeMsgPart;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<FakeMsgPart>> {
        use Poll::*;
        let mut items = self.items.lock().unwrap();
        match items.front() {
            None =>
                Ready(None),
            Some(item) if item.side == self.side =>
                Ready(Some(items.pop_front().unwrap().msg_part)),
            Some(_) =>
                Pending,
        }
    }
}

impl Unpin for FakeStream {}

impl Debug for FakeMsgPart {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            TypeByte(type_byte) => write!(f, "TypeByte('{}'={})", *type_byte as char, type_byte),
            Body(any) => write!(f, "Body({:?})", any),
        }
    }
}
