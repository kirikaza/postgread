use super::super::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(Default, PartialEq)]
pub struct ErrorResponse {
    // https://www.postgresql.org/docs/current/protocol-error-fields.html
    localized_severity: Option<Vec<u8>>,
    severity: Option<Vec<u8>>,
    code: Option<Vec<u8>>,
    message: Option<Vec<u8>>,
    detail: Option<Vec<u8>>,
    hint: Option<Vec<u8>>,
    position: Option<Vec<u8>>,
    internal_position: Option<Vec<u8>>,
    internal_query: Option<Vec<u8>>,
    where_: Option<Vec<u8>>,
    schema: Option<Vec<u8>>,
    table: Option<Vec<u8>>,
    column: Option<Vec<u8>>,
    data_type: Option<Vec<u8>>,
    constraint: Option<Vec<u8>>,
    file: Option<Vec<u8>>,
    line: Option<Vec<u8>>,
    routine: Option<Vec<u8>>,
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

impl Debug for ErrorResponse {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        fmt_struct_of_opt_fields!(
            self,
            formatter,
            ErrorResponse,
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
        $stream:ident,
        $result:ident,
        $($field_type_byte:expr => $field:ident),*
    ) => {
        loop {
            match read_u8($stream).await? {
                0 => break,
                $(
                    $field_type_byte => {
                        let mut value = read_null_terminated($stream).await?;
                        if value.pop().is_none() {
                            return Err(error_other(concat!("ErrorResponse: field ", stringify!($field), " doesn't contain even 0-byte")))
                        }
                        $result.$field = Some(value);
                    },
                )*
                x => return Err(error_other(&format!("ErrorResponse: incorrect field type {}", x))),
            }
        }
    };
}

impl ErrorResponse {
    pub const TYPE_BYTE: Option<u8> = Some(b'E');

    pub async fn read<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let mut body = Self { ..Default::default() };
        read_struct_of_opt_fields!(stream, body,
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
    use super::{ErrorResponse};
    use crate::msg::BackendMessage;
    use crate::msg::test_util::*;

    #[test]
    fn no_fields() {
        let mut bytes: &[u8] = &[
            b'E',
            0, 0, 0, 5,  // len
            0,  // zero instead of field type means "no fields more"
        ];
        assert_eq!(
            ok_some(ErrorResponse { .. Default::default() }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn all_fields() {
        let mut bytes = vec![
            b'E',
            0, 0, 0, 78,  // len
        ];
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
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(ErrorResponse {
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
            }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: ErrorResponse) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::ErrorResponse)
    }
}
