use crate::msg::parts::{Format, Text, Value, decode_vec};
use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Bind {
    pub prepared_statement_name: Text,
    pub portal_name: Text,
    pub parameters_formats: Vec<Format>,
    pub parameters_values: Vec<Value>,
    pub results_formats: Vec<Format>,
}

impl MsgDecode for Bind {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Bind);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let prepared_statement_name = Text::decode(bytes)?;
        let portal_name = Text::decode(bytes)?;
        let parameters_formats = decode_vec(bytes.take_u16()? as usize, bytes)?;
        let parameters_values = decode_vec(bytes.take_u16()? as usize, bytes)?;
        let results_formats = decode_vec(bytes.take_u16()? as usize, bytes)?;
        Ok(Self {
            prepared_statement_name,
            portal_name,
            parameters_formats,
            parameters_values,
            results_formats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Bind;
    use crate::msg::util::test::*;
    use crate::msg::parts::{Bytes, Format, Value};

    #[test]
    fn empty() {
        let bytes = &[
            0,  // unnamed prepared statement
            0,  // unnamed portal
            0, 0,  // no parameters formats
            0, 0,  // no parameters values
            0, 0,  // no results formats
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "".into(),
            portal_name: "".into(),
            parameters_formats: vec![],
            parameters_values: vec![],
            results_formats: vec![],
        }, bytes);
    }

    #[test]
    fn named_prep_stat() {
        let bytes = &[
            b'p', b'r', b' ', b's', b't', 0,  // named prepared statement
            0,  // unnamed portal
            0, 0,  // no parameters formats
            0, 0,  // no parameters values
            0, 0,  // no results formats
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "pr st".into(),
            portal_name: "".into(),
            parameters_formats: vec![],
            parameters_values: vec![],
            results_formats: vec![],
        }, bytes);
    }

    #[test]
    fn named_portal() {
        let bytes = &[
            0,  // unnamed prepared statement
            b'p', b'o', b'r', b't', b'a', b'l', 0,  // named portal
            0, 0,  // no parameters formats
            0, 0,  // no parameters values
            0, 0,  // no results formats
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "".into(),
            portal_name: "portal".into(),
            parameters_formats: vec![],
            parameters_values: vec![],
            results_formats: vec![],
        }, bytes);
    }

    #[test]
    fn parameters_formats() {
        let bytes = &[
            0,  // unnamed prepared statement
            0,  // unnamed portal
            0, 3,  // 3 parameters formats follow
            0, 0,
            0, 1,
            0, 0,
            0, 0,  // no parameters values
            0, 0,  // no results formats
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "".into(),
            portal_name: "".into(),
            parameters_formats: vec![Format::Text, Format::Binary, Format::Text],
            parameters_values: vec![],
            results_formats: vec![],
        }, bytes);
    }


    #[test]
    fn parameters_values() {
        let bytes = &[
            0,  // unnamed prepared statement
            0,  // unnamed portal
            0, 0,  // no parameters formats
            0, 3,  // 3 parameters values
            0, 0, 0, 5,  // first is 5 bytes long
            34, 55, 89, 144, 233,  // just different numbers
            0, 0, 0, 0,  // second is 0 bytes long
            0xff, 0xff, 0xff, 0xff,  // third is -1 which means NULL
            0, 0,  // no results formats
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "".into(),
            portal_name: "".into(),
            parameters_formats: vec![],
            parameters_values: vec![
                Value::Bytes(Bytes(vec![34, 55, 89, 144, 233])),
                Value::Bytes(Bytes(vec![])),
                Value::Null,
            ],
            results_formats: vec![],
        }, bytes);
    }

    #[test]
    fn results_formats() {
        let bytes = &[
            0,  // unnamed prepared statement
            0,  // unnamed portal
            0, 0,  // no parameters formats
            0, 0,  // no parameters values
            0, 3,  // 3 results formats
            0, 1,
            0, 0,
            0, 1,
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "".into(),
            portal_name: "".into(),
            parameters_formats: vec![],
            parameters_values: vec![],
            results_formats: vec![Format::Binary, Format::Text, Format::Binary],
        }, bytes);
    }

    #[test]
    fn all() {
        let bytes = &[
            b'P', b'r', b'e', b'P', 0,  // named prepared statement
            b'P', b'o', b'r', b't', b'a', b'L', 0,  // named portal
            0, 2,  // 2 parameters formats
            0, 1,
            0, 0,
            0, 3,  // 3 parameters values
            0xff, 0xff, 0xff, 0xff,  // first is -1 which means NULL
            0, 0, 0, 5,  // second is 5 bytes long
            34, 55, 89, 144, 233,  // just different numbers
            0, 0, 0, 0,  // third is 0 bytes long
            0, 4,  // 4 results formats
            0, 0,
            0, 1,
            0, 1,
            0, 0,
        ];
        assert_decode_ok(Bind {
            prepared_statement_name: "PreP".into(),
            portal_name: "PortaL".into(),
            parameters_formats: vec![
                Format::Binary,
                Format::Text,
            ],
            parameters_values: vec![
                Value::Null,
                Value::Bytes(Bytes(vec![34, 55, 89, 144, 233])),
                Value::Bytes(Bytes(vec![])),
            ],
            results_formats: vec![
                Format::Text,
                Format::Binary,
                Format::Binary,
                Format::Text,
            ],
        }, bytes);
    }
}
