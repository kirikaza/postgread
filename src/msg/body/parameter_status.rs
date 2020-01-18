use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct ParameterStatus {
    name: Vec<u8>,
    value: Vec<u8>,
}

impl ParameterStatus {
    pub const TYPE_BYTE: u8 = b'S';
}

impl MsgDecode for ParameterStatus {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let name = bytes.take_until_null()?;
        let value = bytes.take_until_null()?;
        Ok(Self { name, value })
    }
}

impl Debug for ParameterStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("ParameterStatus")
            .field(
                &String::from_utf8_lossy(&self.name),
                &String::from_utf8_lossy(&self.value))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::ParameterStatus;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"TimeZone\0UTC\0";
        assert_decode_ok(
            ParameterStatus {
                name: Vec::from(&b"TimeZone"[..]),
                value: Vec::from(&b"UTC"[..]),
            },
            bytes,
        );
    }
}
