use crate::msg::util::async_io::*;
use crate::msg::util::read::*;

use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Unknown {
    pub note: String,
}

impl Unknown {
    pub async fn read<R>(stream: &mut R, type_byte: u8) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        let body_len = read_body_len(stream).await?;
        read_and_drop(stream, body_len).await?;
        let note = format!("message type {:?}", type_byte as char);
        Ok(Self { note })
    }
}
