use crate::msg::util::decode::{*, Problem::*};
use crate::msg::util::read::*;
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
    pub const TYPE_BYTE: u8 = b'Z';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream, Self::decode_body).await
    }

    pub fn decode_body(bytes: &mut BytesSource, _body_len: u32) -> DecodeResult<Self> {
        let status = match bytes.take_u8()? {
            b'I' => Status::Idle,
            b'T' => Status::Transaction,
            b'E' => Status::Error,
            byte => return Err(Unknown(format!("status is unknown: {}", byte))),
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
            255,
        ];
        assert_eq!(
            Err("Unknown(\"status is unknown: 255\")".to_owned()),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: ReadyForQuery) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::ReadyForQuery)
    }
}
