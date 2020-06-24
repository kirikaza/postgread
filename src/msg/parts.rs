use crate::msg::util::decode::{*, Problem::*};
use ::std::fmt::{self, Debug, Formatter};
use ::hex;

#[derive(PartialEq)]
pub struct Bytes(pub Vec<u8>);

#[derive(Debug, PartialEq)]
pub enum Format {
    Text,
    Binary,
}

#[derive(PartialEq)]
pub struct Text(pub Vec<u8>);

#[derive(Debug, PartialEq)]
pub enum Value {
    Null,
    Bytes(Bytes),
}

impl Debug for Bytes {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Bytes")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl Debug for Text {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Text")
            .field(&String::from_utf8_lossy(&self.0))
            .finish()
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text(Vec::from(s))
    }
}

impl PartDecode for Format {
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        match bytes.take_u16()? {
            0 => Ok(Self::Text),
            1 => Ok(Self::Binary),
            x => Err(Unknown(format!("Unknown format {}", x)))
        }
    }
}

impl PartDecode for Text {
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        bytes.take_until_null().map(Self)
    }
}

impl PartDecode for Value {
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        match bytes.take_u32()? as i32 {
            -1 => Ok(Self::Null),
            value_len if value_len >= 0 => {
                let value = bytes.take_vec(value_len as usize)?;
                Ok(Self::Bytes(Bytes(value)))
            },
            x => Err(Incorrect(format!("length of value should be >= -1 but is {}", x))),
        }
    }
}

impl PartDecode for u32 {
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        bytes.take_u32()
    }
}

pub fn decode_vec<Part: PartDecode> (count: usize, bytes: &mut BytesSource) -> DecodeResult<Vec<Part>> {
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        items.push(Part::decode(bytes)?);
    }
    Ok(items)
}
