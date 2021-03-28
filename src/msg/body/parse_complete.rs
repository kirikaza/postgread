use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode};

#[derive(Clone, Debug, PartialEq)]
pub struct ParseComplete();

impl MsgDecode for ParseComplete {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::ParseComplete);

    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self())
    }
}

#[cfg(test)]
mod tests {
    use super::ParseComplete;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[];
        assert_decode_ok(ParseComplete(), bytes);
    }
}
