use crate::msg::parts::Text;
use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;

#[derive(Debug, PartialEq)]
pub struct Execute {
    pub portal_name: Text,
    pub rows_limit: u32,
}

impl MsgDecode for Execute {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Execute_or_ErrorResponse);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let portal_name = Text::decode(bytes)?;
        let rows_limit = bytes.take_u32()?;
        Ok(Self {
            portal_name,
            rows_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Execute;
    use crate::msg::util::test::*;

    #[test]
    fn unnamed_limited() {
        let bytes = &[
            0,  // unnamed portal
            0x12, 0x34, 0x56, 0x78,  // limited rows
        ];
        assert_decode_ok(Execute {
            portal_name: "".into(),
            rows_limit: 0x12345678,
        }, bytes);
    }

    #[test]
    fn named_unlimited() {
        let bytes = &[
            b'P', b'o', b'r', b't', b'a', b'L', 0,  // named portal
            0, 0, 0, 0,  // unlimited rows
        ];
        assert_decode_ok(Execute {
            portal_name: "PortaL".into(),
            rows_limit: 0,
        }, bytes);
    }

    #[test]
    fn named_limited() {
        let bytes = &[
            b'p', b'o', b'R', b't', b'a', b'L', 0,  // named portal
            0x23, 0x45, 0x67, 0x89,  // limited rows
        ];
        assert_decode_ok(Execute {
            portal_name: "poRtaL".into(),
            rows_limit: 0x23456789,
        }, bytes);
    }
}
