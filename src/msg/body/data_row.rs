use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{*, Problem::*};
use ::hex;
use ::std::fmt::{self, Debug, Formatter};

#[derive(Debug, PartialEq)]
pub struct DataRow {
    pub columns: Vec<Column>,
}

#[derive(PartialEq)]
pub enum Column {
    Null,
    Value(Vec<u8>),
}
impl Debug for Column {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Null => f.write_str("NULL"),
            Self::Value(vec) => f.write_str(&hex::encode(&vec)),
        }
    }
}


impl DataRow {
    pub const TYPE_BYTE: u8 = b'D';
}

impl MsgDecode for DataRow {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::DataRow);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let count = bytes.take_u16()?;
        let mut columns = Vec::with_capacity(count as usize);
        for i in 0..count {
            columns.push(Column::decode(bytes, i)?)
        }
        Ok(Self { columns })
    }
}

impl Column {
    pub fn decode(bytes: &mut BytesSource, index: u16) -> DecodeResult<Self> {
        match bytes.take_u32()? as i32 {
            -1 => Ok(Self::Null),
            value_len if value_len >= 0 => {
                let value = bytes.take_vec(value_len as usize)?;
                Ok(Self::Value(value))
            },
            x => Err(Incorrect(format!("column[{}]: length of value should be >= -1 but is {}", index, x))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataRow, Column::*};
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
                Null,
                Value(vec![]),
                Value(vec![12, 34, 56]),
            ] },
            bytes,
        );
    }
}
