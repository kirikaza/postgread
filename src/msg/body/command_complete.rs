use crate::msg::util::io::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::{BufRead, Result as IoResult};

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
        read_msg_with_len(stream, Self::read_body).await
    }

    pub fn read_body<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: BufRead {
        let mut tag = read_null_terminated(stream)?;
        tag.pop().ok_or_else(|| error_other("query doesn't contain even 0-byte"))?;
        Ok(Self { tag })
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandComplete};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let mut bytes = vec![
            b'C',
            0, 0, 0, 16,  // len
        ];
        bytes.extend_from_slice(b"UPDATE 9000\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(CommandComplete {
                tag: Vec::from( & b"UPDATE 9000"[..]),
            }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: CommandComplete) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::CommandComplete)
    }
}
