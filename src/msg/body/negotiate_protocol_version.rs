use crate::msg::util::decode::*;

#[derive(Debug, PartialEq)]
pub struct NegotiateProtocolVersion {
    pub newest_backend_minor: u32,
    pub unrecognized_options: Vec<Vec<u8>>,
}

impl NegotiateProtocolVersion {
    pub const TYPE_BYTE: u8 = b'v';
}

impl MsgDecode for NegotiateProtocolVersion {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let newest_backend_minor = bytes.take_u32()?;
        let count = bytes.take_u32()?;
        let mut unrecognized_options = Vec::with_capacity(count as usize);
        for _ in 0..count {
            unrecognized_options.push(bytes.take_until_null()?)
        }
        Ok(Self { newest_backend_minor, unrecognized_options })
    }
}

#[cfg(test)]
mod tests {
    use super::{NegotiateProtocolVersion};
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
                    Vec::from("first"),
                    Vec::from("second"),
                    Vec::from("third"),
                ],
            },
            bytes,
        );
    }
}