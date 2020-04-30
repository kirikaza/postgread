use crate::msg::util::decode::{*, Problem::*};
use ::std::fmt::{self, Debug, Formatter};

#[derive(Debug, PartialEq)]
pub struct ErrorResponse(pub ErrorOrNoticeFields);

#[derive(Debug, PartialEq)]
pub struct NoticeResponse(pub ErrorOrNoticeFields);

impl ErrorResponse {
    pub const TYPE_BYTE: u8 = b'E';
}

impl MsgDecode for ErrorResponse {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        ErrorOrNoticeFields::decode(bytes).map(Self)
    }
}

impl NoticeResponse {
    pub const TYPE_BYTE: u8 = b'N';
}

impl MsgDecode for NoticeResponse {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        ErrorOrNoticeFields::decode(bytes).map(Self)
    }
}

#[derive(Default, PartialEq)]
pub struct ErrorOrNoticeFields {
    // https://www.postgresql.org/docs/current/protocol-error-fields.html
    pub localized_severity: Option<Vec<u8>>,
    pub severity: Option<Vec<u8>>,
    pub code: Option<Vec<u8>>,
    pub message: Option<Vec<u8>>,
    pub detail: Option<Vec<u8>>,
    pub hint: Option<Vec<u8>>,
    pub position: Option<Vec<u8>>,
    pub internal_position: Option<Vec<u8>>,
    pub internal_query: Option<Vec<u8>>,
    pub where_: Option<Vec<u8>>,
    pub schema: Option<Vec<u8>>,
    pub table: Option<Vec<u8>>,
    pub column: Option<Vec<u8>>,
    pub data_type: Option<Vec<u8>>,
    pub constraint: Option<Vec<u8>>,
    pub file: Option<Vec<u8>>,
    pub line: Option<Vec<u8>>,
    pub routine: Option<Vec<u8>>,
}

macro_rules! fmt_opt_field {
    (
        $self:ident,
        $debug_struct:ident,
        $field:ident
    ) => {
        match &($self.$field) {
            Some(vec) => $debug_struct.field(stringify!($field), &String::from_utf8_lossy(&vec)),
            None => &mut $debug_struct,
        }
    };
}

macro_rules! fmt_struct_of_opt_fields {
    (
        $self:ident,
        $formatter:ident,
        $name:ident,
        $($field:ident),*
    ) => {
        {
            let mut debug_struct = $formatter.debug_struct(stringify!($name));
            $(
                fmt_opt_field!($self, debug_struct, $field);
            )*
            debug_struct.finish()
        }
    };
}

impl Debug for ErrorOrNoticeFields {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        fmt_struct_of_opt_fields!(
            self,
            formatter,
            ErrorOrNoticeFields,
            localized_severity,
            severity,
            code,
            message,
            detail,
            hint,
            position,
            internal_position,
            internal_query,
            where_,
            schema,
            table,
            column,
            data_type,
            constraint,
            file,
            line,
            routine
        )
    }
}

macro_rules! read_struct_of_opt_fields {
    (
        $bytes:ident,
        $result:ident,
        $($field_type_byte:expr => $field:ident),*
    ) => {
        {
            let mut index = 0;
            loop {
                match $bytes.take_u8()? {
                    0 => break,
                    $(
                        $field_type_byte => {
                            let value = $bytes.take_until_null()?;
                            $result.$field = Some(value);
                        },
                    )*
                    x => return Err(Unknown(format!("field[{}] has unknown type {}", index, x))),
                };
                index += 1;
            }
        }
    };
}

impl ErrorOrNoticeFields {
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let mut body = Self { ..Default::default() };
        read_struct_of_opt_fields!(bytes, body,
            b'S' => localized_severity,
            b'V' => severity,
            b'C' => code,
            b'M' => message,
            b'D' => detail,
            b'H' => hint,
            b'P' => position,
            b'p' => internal_position,
            b'q' => internal_query,
            b'W' => where_,
            b's' => schema,
            b't' => table,
            b'c' => column,
            b'd' => data_type,
            b'n' => constraint,
            b'F' => file,
            b'L' => line,
            b'R' => routine
        );
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorResponse, ErrorOrNoticeFields, NoticeResponse};
    use crate::msg::util::test::*;

    #[test]
    fn no_fields() {
        let bytes: &[u8] = &[0];  // zero instead of field type means "no fields more"
        assert_decode_ok(ErrorResponse(ErrorOrNoticeFields { .. Default::default() }), bytes);
        assert_decode_ok(NoticeResponse(ErrorOrNoticeFields { .. Default::default() }), bytes);
    }

    #[test]
    fn all_fields() {
        let mut bytes = vec![];
        bytes.extend_from_slice(b"S\xd1\x8d\xd0\xb9\0");
        bytes.extend_from_slice(b"Vab\0");
        bytes.extend_from_slice(b"C12\0");
        bytes.extend_from_slice(b"Mcd\0");
        bytes.extend_from_slice(b"Def\0");
        bytes.extend_from_slice(b"Hgh\0");
        bytes.extend_from_slice(b"P34\0");
        bytes.extend_from_slice(b"p56\0");
        bytes.extend_from_slice(b"qij\0");
        bytes.extend_from_slice(b"Wkl\0");
        bytes.extend_from_slice(b"smn\0");
        bytes.extend_from_slice(b"top\0");
        bytes.extend_from_slice(b"cqr\0");
        bytes.extend_from_slice(b"dst\0");
        bytes.extend_from_slice(b"nuv\0");
        bytes.extend_from_slice(b"Fwx\0");
        bytes.extend_from_slice(b"L78\0");
        bytes.extend_from_slice(b"Ryz\0");
        bytes.extend_from_slice(&[0]);
        let bytes = bytes.as_slice();
        let expected_fields = || ErrorOrNoticeFields {
            localized_severity: Some(Vec::from("эй")),
            severity: Some(Vec::from("ab")),
            code: Some(Vec::from("12")),
            message: Some(Vec::from("cd")),
            detail: Some(Vec::from("ef")),
            hint: Some(Vec::from("gh")),
            position: Some(Vec::from("34")),
            internal_position: Some(Vec::from("56")),
            internal_query: Some(Vec::from("ij")),
            where_: Some(Vec::from("kl")),
            schema: Some(Vec::from("mn")),
            table: Some(Vec::from("op")),
            column: Some(Vec::from("qr")),
            data_type: Some(Vec::from("st")),
            constraint: Some(Vec::from("uv")),
            file: Some(Vec::from("wx")),
            line: Some(Vec::from("78")),
            routine: Some(Vec::from("yz")),
        };
        assert_decode_ok(ErrorResponse(expected_fields()), bytes);
        assert_decode_ok(NoticeResponse(expected_fields()), bytes);
    }
}
