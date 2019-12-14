use crate::msg::util::decode::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(PartialEq)]
pub struct CommandComplete {
    tag: Vec<u8>,
}
impl Debug for CommandComplete {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("CommandComplete")
            .field("tag", &String::from_utf8_lossy(&self.tag))
            .finish()
    }
}

impl CommandComplete {
    pub const TYPE_BYTE: u8 = b'C';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncReadExt + Unpin {
        read_msg_with_len(stream, Self::decode_body).await
    }
}

impl MsgDecode for CommandComplete {
    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let tag = bytes.take_until_null()?;
        Ok(Self { tag })
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandComplete};
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"UPDATE 9000\0";
        assert_decode_ok(CommandComplete { tag: Vec::from( & b"UPDATE 9000"[..]), }, bytes);
    }
}
