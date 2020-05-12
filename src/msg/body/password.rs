use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(PartialEq)]
pub struct Password (
    pub Vec<u8>
);

impl Password {
    pub const TYPE_BYTE: u8 = b'p';
}

impl MsgDecode for Password {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::GssResponse_Or_Password);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let password = bytes.take_until_null()?;
        Ok(Self(password))
    }
}

impl Debug for Password {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Password")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Password;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"qwerty123\0";
        assert_decode_ok(Password("qwerty123".into()), bytes);
    }
}
