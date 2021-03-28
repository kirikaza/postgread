use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;

#[derive(Clone, Debug, PartialEq)]
pub struct EmptyQueryResponse {}

impl EmptyQueryResponse {
    pub const TYPE_BYTE: u8 = b'I';
}

impl MsgDecode for EmptyQueryResponse {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::EmptyQueryResponse);

    fn decode_body(_: &mut BytesSource) -> DecodeResult<Self> {
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::{EmptyQueryResponse};
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"";
        assert_decode_ok(EmptyQueryResponse {}, bytes);
    }
}
