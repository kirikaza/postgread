use crate::msg::util::io::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::{BufRead, Result as IoResult};

#[derive(PartialEq)]
pub struct Query (
    Vec<u8>,
);

impl Query {
    pub const TYPE_BYTE: u8 = b'Q';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream, Self::read_body).await
    }

    pub fn read_body<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: BufRead {
        let mut query = read_null_terminated(stream)?;
        query.pop().ok_or_else(|| error_other("Query: query doesn't contain even 0-byte"))?;
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
    use crate::msg::FrontendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let mut bytes = vec![
            b'Q',
            0, 0, 0, 14,  // len
        ];
        bytes.extend_from_slice(b"select 1;\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(Query (
                Vec::from("select 1;"),
            )),
            force_read_frontend(&mut bytes, false),
        );
    }

    fn ok_some(body: Query) -> Result<Option<FrontendMessage>, String> {
        ok_some_msg(body, FrontendMessage::Query)
    }
}
