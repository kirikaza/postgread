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
}

impl MsgDecode for ReadyForQuery {
    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
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
    use crate::msg::util::decode::Problem::*;
    use crate::msg::util::test::*;

    #[test]
    fn idle() {
        let bytes = b"I";
        assert_decode_ok(ReadyForQuery { status: Idle }, bytes);
    }

    #[test]
    fn transaction() {
        let bytes = b"T";
        assert_decode_ok(ReadyForQuery { status: Transaction }, bytes);
    }

    #[test]
    fn error() {
        let bytes = b"E";
        assert_decode_ok(ReadyForQuery { status: Error }, bytes);
    }

    #[test]
    fn incorrect_status() {
        let bytes = &[255u8];
        assert_decode_err::<ReadyForQuery>(Unknown("status is unknown: 255".to_owned()), bytes);
    }
}
