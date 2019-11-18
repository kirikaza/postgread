use crate::msg::{BackendMessage, FrontendMessage};

use ::futures::task::Poll::*;
use ::futures_test::task::noop_context;
use ::std::io::Result as IoResult;
use ::std::pin::Pin;

pub fn ok_some_msg<Msg, Body>(body: Body, wrap: fn(Body) -> Msg) -> Result<Option<Msg>, String> {
    Result::Ok(Some(wrap(body)))
}


pub fn force_read_backend(bytes: &mut &[u8]) -> Result<Option<BackendMessage>, String> {
    force_read(&mut BackendMessage::read(bytes))
}

pub fn force_read_frontend(bytes: &mut &[u8], is_first_msg: bool) -> Result<Option<FrontendMessage>, String> {
    force_read(&mut FrontendMessage::read(bytes, is_first_msg))
}

fn force_read<Msg>(
    future: &mut dyn futures::Future<Output = IoResult<Option<Msg>>>
) -> Result<Option<Msg>, String>
{
    let pinned = unsafe { Pin::new_unchecked(future) };
    match pinned.poll(&mut noop_context()) {
        Ready(ready) => ready.map_err(|e| e.to_string()),
        Pending => panic!("unexpected Pending in synchronous tests"),
    }
}
