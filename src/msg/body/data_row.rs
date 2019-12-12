use crate::msg::util::decode::{*, Problem::*};
use crate::msg::util::read::*;
use ::futures::io::AsyncReadExt;
use ::hex;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct DataRow {
    columns: Vec<Column>,
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

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncReadExt + Unpin {
        read_msg_with_len(stream, Self::read_body).await
    }

    pub fn read_body(stream: &mut BytesSource, _body_len: u32) -> DecodeResult<Self> {
        let count = stream.take_u16()?;
        let mut columns = Vec::with_capacity(count as usize);
        for i in 0..count {
            columns.push(Column::read(stream, i)?)
        }
        Ok(Self { columns })
    }
}

impl Column {
    pub fn read(stream: &mut BytesSource, index: u16) -> DecodeResult<Self> {
        match stream.take_u32()? as i32 {
            -1 => Ok(Self::Null),
            value_len if value_len >= 0 => {
                let value = stream.take_vec(value_len as usize)?;
                Ok(Self::Value(value))
            },
            x => Err(Incorrect(format!("column[{}]: length of value should be >= -1 but is {}", index, x))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataRow, Column::*};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn no_columns() {
        let mut bytes: &[u8] = &[
            b'D',
            0, 0, 0, 6,  // len
            0, 0,  // columns count
        ];
        assert_eq!(
            ok_some(DataRow { columns: vec![] }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn many_columns() {
        let mut bytes: &[u8] = &[
            b'D',
            0, 0, 0, 21,  // len
            0, 3,  // columns count
            // first column:
            0xFF, 0xFF, 0xFF, 0xFF,  //  value len=-1
            // second column:
            0, 0, 0, 0,  //  value len
            // third column:
            0, 0, 0, 3,  //  value len
            12, 34, 56  // value
        ];
        assert_eq!(
            ok_some(DataRow { columns: vec![
                Null,
                Value(vec![]),
                Value(vec![12, 34, 56]),
            ] }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: DataRow) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::DataRow)
    }
}
