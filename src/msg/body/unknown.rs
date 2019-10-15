use super::super::io::*;

use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Unknown {
    note: String,
}

impl Unknown {
    pub async fn read<R>(stream: &mut R, body_len: u32, note: String) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        read_and_drop(stream, body_len).await?;
        Ok(Self { note })
    }
}
