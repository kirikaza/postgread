use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode};
use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Terminate {}

impl Terminate {
    pub const TYPE_BYTE: u8 = b'X';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream).await
    }
}

impl MsgDecode for Terminate {
    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::Terminate;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[];
        assert_decode_ok(Terminate {}, bytes);
    }
}
