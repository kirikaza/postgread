use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode};

#[derive(Clone, Debug, PartialEq)]
pub struct Terminate {}

impl Terminate {
    pub const TYPE_BYTE: u8 = b'X';
}

impl MsgDecode for Terminate {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Terminate);

    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::Terminate;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[];
        assert_decode_ok(Terminate {}, bytes);
    }
}
