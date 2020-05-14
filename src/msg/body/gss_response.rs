use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct GssResponse(
    pub Vec<u8>
);

impl GssResponse {
    pub const TYPE_BYTE: u8 = b'p';
}

impl MsgDecode for GssResponse {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::GssResponse_Or_Password);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let data = bytes.take_vec(bytes.left())?;
        Ok(Self(data))
    }
}

impl Debug for GssResponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("GssResponse")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::GssResponse;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc];
        assert_decode_ok(GssResponse(vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]), bytes);
    }
}
