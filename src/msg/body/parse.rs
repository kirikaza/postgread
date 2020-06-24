use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;
use crate::msg::parts::{Text, decode_vec};

#[derive(Debug, PartialEq)]
pub struct Parse {
    pub prepared_statement_name: Text,
    pub query: Text,
    pub parameters_types: Vec<u32>,
}

impl MsgDecode for Parse {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Parse);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let prepared_statement_name = Text::decode(bytes)?;
        let query = Text::decode(bytes)?;
        let parameters_types = decode_vec(bytes.take_u16()? as usize, bytes)?;
        Ok(Self { prepared_statement_name, query, parameters_types })
    }
}

#[cfg(test)]
mod tests {
    use super::Parse;
    use crate::msg::util::test::*;

    #[test]
    fn unnamed_without_types() {
        let bytes = &[
            0,  // no prepared statement name
            b'S', b'Q', b'L', 0,  // SQL query
            0, 0,  // no parameters types
        ];
        assert_decode_ok(Parse {
            prepared_statement_name: "".into(),
            query: "SQL".into(),
            parameters_types: vec![],
        }, bytes);
    }

    #[test]
    fn named_without_types() {
        let bytes = &[
            b'N', b'a', b'm', b'e', 0,  // no prepared statement name
            b's', b'q', b'l', 0,  // SQL query
            0, 0,  // no parameters types
        ];
        assert_decode_ok(Parse {
            prepared_statement_name: "Name".into(),
            query: "sql".into(),
            parameters_types: vec![],
        }, bytes);
    }

    #[test]
    fn named_with_types() {
        let bytes = &[
            b'N', b'a', b'm', b'e', 0,  // no prepared statement name
            b's', b'q', b'l', 0,  // SQL query
            0, 3,  // 3 parameters types
            0x12, 0x34, 0x56, 0x78,
            0x23, 0x45, 0x67, 0x89,
            0x35, 0x79, 0xbd, 0xf1,
        ];
        assert_decode_ok(Parse {
            prepared_statement_name: "Name".into(),
            query: "sql".into(),
            parameters_types: vec![0x12345678, 0x23456789, 0x3579bdf1],
        }, bytes);
    }
}
