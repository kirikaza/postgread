use crate::msg::parts::{Text, decode_vec};
use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;

#[derive(Debug, PartialEq)]
pub struct NegotiateProtocolVersion {
    pub newest_backend_minor: u32,
    pub unrecognized_options: Vec<Text>,
}

impl NegotiateProtocolVersion {
    pub const TYPE_BYTE: u8 = b'v';
}

impl MsgDecode for NegotiateProtocolVersion {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::NegotiateProtocolVersion);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let newest_backend_minor = bytes.take_u32()?;
        let unrecognized_options = decode_vec(bytes.take_u32()? as usize, bytes)?;
        Ok(Self { newest_backend_minor, unrecognized_options })
    }
}

#[cfg(test)]
mod tests {
    use super::NegotiateProtocolVersion;
    use crate::msg::util::test::*;

    #[test]
    fn no_unrecognized_options() {
        let bytes: &[u8] = &[
            0x12, 0x34, 0x56, 0x78,  // newest backend minor version
               0,    0,    0,    0,  // count of unrecognized options
        ];
        assert_decode_ok(
            NegotiateProtocolVersion {
                newest_backend_minor: 0x12345678,
                unrecognized_options: vec![]
            },
            bytes,
        );
    }

    #[test]
    fn many_unrecognized_options() {
        let mut bytes = vec![
            0x12, 0x34, 0x56, 0x78,  // newest backend minor version
               0,    0,    0,    3,  // count of unrecognized options
        ];
        bytes.extend_from_slice(b"first\0");
        bytes.extend_from_slice(b"second\0");
        bytes.extend_from_slice(b"third\0");
        let bytes = bytes.as_slice();
        assert_decode_ok(
            NegotiateProtocolVersion {
                newest_backend_minor: 0x12345678,
                unrecognized_options: vec![
                    "first".into(),
                    "second".into(),
                    "third".into(),
                ],
            },
            bytes,
        );
    }
}
