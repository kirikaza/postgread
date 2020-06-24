use crate::msg::parts::{Value, decode_vec};
use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::*;

#[derive(Debug, PartialEq)]
pub struct DataRow {
    pub columns: Vec<Value>,
}

impl DataRow {
    pub const TYPE_BYTE: u8 = b'D';
}

impl MsgDecode for DataRow {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::DataRow);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let columns = decode_vec(bytes.take_u16()? as usize, bytes)?;
        Ok(Self { columns })
    }
}

#[cfg(test)]
mod tests {
    use super::DataRow;
    use crate::msg::parts::{Bytes, Value};
    use crate::msg::util::test::*;

    #[test]
    fn no_columns() {
        let bytes: &[u8] = &[
            0, 0,  // columns count
        ];
        assert_decode_ok(DataRow { columns: vec![] }, bytes);
    }

    #[test]
    fn many_columns() {
        let bytes: &[u8] = &[
            0, 3,  // columns count
            // first column:
            0xFF, 0xFF, 0xFF, 0xFF,  //  value len=-1
            // second column:
            0, 0, 0, 0,  //  value len
            // third column:
            0, 0, 0, 3,  //  value len
            12, 34, 56  // value
        ];
        assert_decode_ok(
            DataRow { columns: vec![
                Value::Null,
                Value::Bytes(Bytes(vec![])),
                Value::Bytes(Bytes(vec![12, 34, 56])),
            ] },
            bytes,
        );
    }
}
