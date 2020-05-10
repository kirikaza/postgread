use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct GSSResponse (
    pub Vec<u8>
);

impl GSSResponse {
    pub const TYPE_BYTE: u8 = b'p';
}

impl MsgDecode for GSSResponse {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let data = bytes.take_vec(bytes.left())?;
        Ok(Self(data))
    }
}

impl Debug for GSSResponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("GSSResponse")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::GSSResponse;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc];
        assert_decode_ok(GSSResponse(vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]), bytes);
    }
}
