use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode};

#[derive(Debug, PartialEq)]
pub struct Sync();

impl MsgDecode for Sync {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::ParameterStatus_or_Sync);

    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self())
    }
}

#[cfg(test)]
mod tests {
    use super::Sync;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes: &[u8] = &[];
        assert_decode_ok(Sync(), bytes);
    }
}
