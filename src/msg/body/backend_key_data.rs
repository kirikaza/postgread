use crate::msg::util::decode::*;

#[derive(Debug, PartialEq)]
pub struct BackendKeyData {
    process_id: u32,
    secret_key: u32,
}

impl BackendKeyData {
    pub const TYPE_BYTE: u8 = b'K';
}

impl MsgDecode for BackendKeyData {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let process_id = bytes.take_u32()?;
        let secret_key = bytes.take_u32()?;
        Ok(Self { process_id, secret_key })
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendKeyData};
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[
            0x1, 0x2, 0x3, 0x4,  // process ID
            0x5, 0x6, 0x7, 0x8,  // secret key
        ];
        assert_decode_ok(BackendKeyData { process_id: 0x01020304, secret_key: 0x05060708 }, bytes);
    }
}
