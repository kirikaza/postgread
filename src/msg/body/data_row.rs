use crate::msg::util::io::*;
use crate::msg::util::read::*;
use ::futures::io::AsyncReadExt;
use ::hex;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::{Read, Result as IoResult};

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

    pub fn read_body<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: Read {
        let count = read_u16(stream)?;
        let mut columns = Vec::with_capacity(count as usize);
        for _ in 0..count {
            columns.push(Column::read(stream)?)
        }
        Ok(Self { columns })
    }
}

impl Column {
    pub fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: Read
    {
        match read_u32(stream)? as i32 {
            -1 => Ok(Self::Null),
            value_len if value_len >= 0 => {
                let value = read_vec(stream, value_len as usize)?;
                Ok(Self::Value(value))
            },
            x => Err(error_other(&format!("DataRow: incorrect length of column value {}", x))),
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
