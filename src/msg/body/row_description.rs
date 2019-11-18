use crate::msg::util::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct RowDescription {
    fields: Vec<Field>,
}

#[derive(PartialEq)]
pub struct Field {
    name: Vec<u8>,
    column_oid: u32,
    column_attr_num: u16,
    type_oid: u32,
    type_size: i16,  // pg_type.typlen
    type_modifier: i32, // pg_attribute.atttypmod
    format: Format,
}

impl Debug for Field {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Field")
            .field("name", &String::from_utf8_lossy(&self.name))
            .field("column_oid", &self.column_oid)
            .field("column_attr_num", &self.column_attr_num)
            .field("type_oid", &self.type_oid)
            .field("type_size", &self.type_size)
            .field("type_modifier", &self.type_modifier)
            .field("format", &self.format)
            .finish()
    }
}

#[derive(Debug, PartialEq)]
pub enum Format {
    Text,
    Binary,
}

impl RowDescription {
    pub const TYPE_BYTE: Option<u8> = Some(b'T');

    pub async fn read<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let count = read_u16(stream).await?;
        let mut fields = Vec::with_capacity(count as usize);
        for _ in 0..count {
            fields.push(Field::read(stream).await?)
        }
        Ok(Self { fields })
    }
}

impl Field {
    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let mut name = read_null_terminated(stream).await?;
        name.pop().ok_or_else(|| error_other("RowDescription: field name doesn't contain even 0-byte"))?;
        let column_oid = read_u32(stream).await?;
        let column_attr_num = read_u16(stream).await?;
        let type_oid = read_u32(stream).await?;
        let type_size = read_u16(stream).await? as i16;
        let type_modifier = read_u32(stream).await? as i32;
        let format = match read_u16(stream).await? {
            0 => Format::Text,
            1 => Format::Binary,
            x => return Err(error_other(&format!("RowDescription: incorrect format {}", x)))
        };
        Ok(Self { name, column_oid, column_attr_num, type_oid, type_size, type_modifier, format })
    }
}

#[cfg(test)]
mod tests {
    use super::{RowDescription, Field, Format::*};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn no_fields() {
        let mut bytes: &[u8] = &[
            b'T',
            0, 0, 0, 6,  // len
            0, 0,  // fields count
        ];
        assert_eq!(
            ok_some(RowDescription { fields: vec![] }),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn two_fields() {
        let mut bytes: &[u8] = &[
            b'T',
            0, 0, 0, 53,  // len
            0, 2,  // fields count
            // first field:
            b'F', b'i', b'r', b's', b't', 0,  // name
            0x10, 0x11, 0x12, 0x13,  // column oid
            0x14, 0x15,  // column attr num
            0x16, 0x17, 0x18, 0x19,  // type oid
            0x1A, 0x1B,  // type_size
            0x1C, 0x1D, 0x1E, 0x1F,  // type modifier
            0, 0,  //  format=text
            // second field:
            b'S', b'e', b'c', b'o', b'n', b'd', 0,  // name
            0x2F, 0x2E, 0x2D, 0x2C,  // column oid
            0x2B, 0x2A,  // column attr num
            0x29, 0x28, 0x27, 0x26,  // type oid
            0x25, 0x24,  // type_size
            0x23, 0x22, 0x21, 0x20,  // type modifier
            0, 1,  //  format=binary
        ];
        assert_eq!(
            ok_some(RowDescription { fields: vec![
                Field {
                    name: Vec::from("First"),
                    column_oid: 0x10111213,
                    column_attr_num: 0x1415,
                    type_oid: 0x16171819,
                    type_size: 0x1A1B,
                    type_modifier: 0x1C1D1E1F,
                    format: Text,
                },
                Field {
                    name: Vec::from("Second"),
                    column_oid: 0x2F2E2D2C,
                    column_attr_num: 0x2B2A,
                    type_oid: 0x29282726,
                    type_size: 0x2524,
                    type_modifier: 0x23222120,
                    format: Binary,
                },
            ] }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: RowDescription) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::RowDescription)
    }
}
