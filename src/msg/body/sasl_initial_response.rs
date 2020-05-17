use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{*, Problem::*};
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct SaslInitialResponse {
    pub selected_mechanism: Vec<u8>,
    pub mechanism_data: Option<Vec<u8>>,
}

impl MsgDecode for SaslInitialResponse {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::GssResponse_Or_Password_Or_SaslResponses);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let selected_mechanism = bytes.take_until_null()?;
        let mechanism_data = match bytes.take_u32()? as i32 {
            -1 =>
                None,
            len if len >= 0 =>
                Some(bytes.take_vec(len as usize)?),
            incorrect =>
                return Err(Incorrect(format!("mechanism data len should be >= -1 but is {}", incorrect))),
        };
        Ok(Self { selected_mechanism, mechanism_data })
    }
}

impl Debug for SaslInitialResponse {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("SaslInitialResponse")
            .field("selected_mechanism", &self.selected_mechanism)
            .field("mechanism_data", &self.mechanism_data.as_ref().map(hex::encode))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::SaslInitialResponse;
    use crate::msg::util::test::*;

    #[test]
    fn no_mechanism_data() {
        let bytes = &[
            b'O', b'T', b'P', 0,  // selected mechanism
            0xff, 0xff, 0xff, 0xff,  // -1 which means "no mechanism data"
        ];
        assert_decode_ok(SaslInitialResponse {
            selected_mechanism: Vec::from("OTP"),
            mechanism_data: None,
        }, bytes);
    }

    #[test]
    fn some_mechanism_data() {
        let bytes = &[
            b'S', b'K', b'E', b'Y', 0,  // selected mechanism
            0, 0, 0, 3,  // len of mechanism data
            0x12, 0x34, 0x56,
        ];
        assert_decode_ok(SaslInitialResponse {
            selected_mechanism: Vec::from("SKEY"),
            mechanism_data: Some(vec![0x12, 0x34, 0x56]),
        }, bytes);
    }
}
