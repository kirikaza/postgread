use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode};

#[derive(Debug, PartialEq)]
pub struct BindComplete();

impl MsgDecode for BindComplete {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::BindComplete);

    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self())
    }
}

#[cfg(test)]
mod tests {
    use super::BindComplete;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[];
        assert_decode_ok(BindComplete(), bytes);
    }
}
