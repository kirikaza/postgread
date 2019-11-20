use crate::msg::util::io::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct BackendKeyData {
    process_id: u32,
    secret_key: u32,
}

impl BackendKeyData {
    pub const TYPE_BYTE: u8 = b'K';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream, Self::read_body).await
    }

    pub async fn read_body<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        let process_id = read_u32(stream).await?;
        let secret_key = read_u32(stream).await?;
        Ok(Self { process_id, secret_key })
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendKeyData};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let mut bytes: &[u8] = &[
            b'K',
            0, 0, 0, 12,  // len
            0x1, 0x2, 0x3, 0x4,  // process ID
            0x5, 0x6, 0x7, 0x8,  // secret key
        ];
        assert_eq!(
            ok_some(BackendKeyData { process_id: 0x01020304, secret_key: 0x05060708 }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: BackendKeyData) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::BackendKeyData)
    }
}
