use crate::msg::util::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct ReadyForQuery {
    status: Status,
}

#[derive(Debug, PartialEq)]
pub enum Status {
    Idle,
    Transaction,
    Error,
}

impl ReadyForQuery {
    pub const TYPE_BYTE: Option<u8> = Some(b'Z');

    pub async fn read<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let status = match read_u8(stream).await? {
            b'I' => Status::Idle,
            b'T' => Status::Transaction,
            b'E' => Status::Error,
            byte => return Err(error_other(&format!("incorrect status {}", byte as char))),
        };
        Ok(Self { status })
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadyForQuery, Status::*};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn idle() {
        let mut bytes: &[u8] = &[
            b'Z',
            0, 0, 0, 5, // len
            b'I',
        ];
        assert_eq!(
            ok_some(ReadyForQuery { status: Idle }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn transaction() {
        let mut bytes: &[u8] = &[
            b'Z',
            0, 0, 0, 5, // len
            b'T',
        ];
        assert_eq!(
            ok_some(ReadyForQuery { status: Transaction }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn error() {
        let mut bytes: &[u8] = &[
            b'Z',
            0, 0, 0, 5, // len
            b'E',
        ];
        assert_eq!(
            ok_some(ReadyForQuery { status: Error }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn incorrect_status() {
        let mut bytes: &[u8] = &[
            b'Z',
            0, 0, 0, 5, // len
            b'Z',
        ];
        assert_eq!(
            Err("incorrect status Z".to_owned()),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: ReadyForQuery) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::ReadyForQuery)
    }
}
