use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

#[derive(Clone, PartialEq)]
pub struct Query (
    pub Vec<u8>
);

impl Query {
    pub const TYPE_BYTE: u8 = b'Q';
}

impl MsgDecode for Query {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Query);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let query = bytes.take_until_null()?;
        Ok(Self(query))
    }
}

impl Debug for Query {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Query")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Query;
    use crate::msg::util::test::*;

    #[test]
    fn simple() {
        let bytes = b"select 1;\0";
        assert_decode_ok(Query(Vec::from("select 1;")), bytes);
    }
}
