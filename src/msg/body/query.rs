use crate::msg::util::decode::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(PartialEq)]
pub struct Query (
    Vec<u8>,
);

impl Query {
    pub const TYPE_BYTE: u8 = b'Q';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream).await
    }
}

impl MsgDecode for Query {
    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let query = bytes.take_until_null()?;
        Ok(Self(query))
    }
}

impl Debug for Query {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Query")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Query;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"select 1;\0";
        assert_decode_ok(Query(Vec::from("select 1;")), bytes);
    }
}
