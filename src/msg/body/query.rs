use super::super::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(PartialEq)]
pub struct Query (
    Vec<u8>,
);

impl Query {
    pub const TYPE_BYTE: Option<u8> = Some(b'Q');

    pub async fn read<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
        where R: AsyncBufReadExt + Unpin
    {
        let mut query = read_null_terminated(stream).await?;
        query.pop().ok_or_else(|| error_other("query doesn't contain even 0-byte"))?;
        Ok(Self(query))
    }
}

impl Debug for Query {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Query(\"{}\")", String::from_utf8_lossy(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::Query;
    use crate::msg::FrontendMessage;
    use crate::msg::test_util::*;

    #[test]
    fn simple() {
        let mut bytes = vec![
            b'Q',
            0, 0, 0, 15, // len
        ];
        bytes.extend_from_slice(b"select 1;\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(Query (
                Vec::from(&b"select 1;"[..]),
            )),
            force_read_frontend(&mut bytes, false),
        );
    }

    fn ok_some(body: Query) -> Result<Option<FrontendMessage>, String> {
        ok_some_msg(body, FrontendMessage::Query)
    }
}
