use crate::msg::util::decode::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(PartialEq)]
pub struct ParameterStatus {
    name: Vec<u8>,
    value: Vec<u8>,
}

impl ParameterStatus {
    pub const TYPE_BYTE: u8 = b'S';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin {
        read_msg_with_len(stream, Self::decode_body).await
    }

    pub fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let name = bytes.take_until_null()?;
        let value = bytes.take_until_null()?;
        Ok(Self { name, value })
    }
}

impl Debug for ParameterStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("ParameterStatus")
            .field(
                &String::from_utf8_lossy(&self.name),
                &String::from_utf8_lossy(&self.value))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::ParameterStatus;
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let mut bytes = vec![
            b'S',
            0, 0, 0, 17,  // len
        ];
        bytes.extend_from_slice(b"TimeZone\0UTC\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(ParameterStatus {
                name: Vec::from(&b"TimeZone"[..]),
                value: Vec::from(&b"UTC"[..]),
            }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: ParameterStatus) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::ParameterStatus)
    }
}
