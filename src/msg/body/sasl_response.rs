use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct SaslResponse {
    pub mechanism_data: Vec<u8>,
}

impl MsgDecode for SaslResponse {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::GssResponse_or_Password_or_SaslResponses);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let mechanism_data = bytes.take_vec(bytes.left())?;
        Ok(Self { mechanism_data })
    }
}

impl Debug for SaslResponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("SaslResponse")
            .field("mechanism_data", &hex::encode(&self.mechanism_data))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::SaslResponse;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = &[
            0x12, 0x34, 0x56,
        ];
        assert_decode_ok(SaslResponse {
            mechanism_data: vec![0x12, 0x34, 0x56],
        }, bytes);
    }
}
