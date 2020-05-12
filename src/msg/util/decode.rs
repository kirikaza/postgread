use crate::msg::type_byte::TypeByte;

#[derive(Debug, PartialEq)]
pub enum Problem {
    NeedMoreBytes(usize),
    NoNullByte,
    Incorrect(String),  // clearly incorrect
    Unknown(String),  // maybe unknown protocol extension
}

pub type DecodeResult<Ok> = std::result::Result<Ok, Problem>;

pub trait MsgDecode : Sized {
    const TYPE_BYTE_OPT: Option<TypeByte>;

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self>;
}

pub struct BytesSource<'a> {
    slice: &'a [u8],
    pos: usize,
}

impl<'a> BytesSource<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice, pos: 0 }
    }

    pub fn left(&self) -> usize {
        self.slice.len() - self.pos
    }

    pub fn take_u8(&mut self) -> DecodeResult<u8> {
        self.take_bounded(1, |s| s[0])
    }

    pub fn take_u16(&mut self) -> DecodeResult<u16> {
        self.take_bounded(2, |s| u16::from_be_bytes([s[0], s[1]]))
    }

    pub fn take_u32(&mut self) -> DecodeResult<u32> {
        self.take_bounded(4, |s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn take_slice(&mut self, result: &mut [u8]) -> DecodeResult<()> {
        self.take_bounded(result.len(), |s| result.copy_from_slice(s))
    }

    pub fn take_vec(&mut self, len: usize) -> DecodeResult<Vec<u8>> {
        self.take_bounded(len, |s| s.to_vec())
    }

    pub fn take_until_null(&mut self) -> DecodeResult<Vec<u8>> {
        // hope the explicit loop is the best for optimizer
        for null_pos in self.pos .. self.slice.len() {
            if self.slice[null_pos] == 0 {
                let res = self.slice[self.pos .. null_pos].to_vec();
                self.pos = null_pos + 1;  // jump over the found 0-byte
                return Ok(res)
            }
        }
        self.pos = self.slice.len();
        Err(Problem::NoNullByte)
    }

    fn take_bounded<Result, Decode>(&mut self, needed: usize, decode: Decode) -> DecodeResult<Result>
    where Decode: FnOnce(&'a [u8]) -> Result {
        if self.pos + needed <= self.slice.len() {
            let sub_slice = &self.slice[self.pos .. self.pos + needed];
            self.pos += needed;
            Ok(decode(sub_slice))
        } else {
            let left = self.slice.len() - self.pos;
            self.pos = self.slice.len();
            Err(Problem::NeedMoreBytes(needed - left))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BytesSource as BS, Problem::*};

    #[test]
    fn left() {
        let bs = BS::new(&[]);
        assert_eq!(0, bs.left());
        let mut bs = BS::new(&[10, 11]);
        assert_eq!(2, bs.left());
        assert_eq!(2, bs.left());
        bs.pos += 1;
        assert_eq!(1, bs.left());
        assert_eq!(1, bs.left());
        bs.pos += 1;
        assert_eq!(0, bs.left());
    }

    #[test]
    fn take_u8() {
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_u8());
        let mut bs = BS::new(&[10, 11]);
        assert_eq!(Ok(10), bs.take_u8());
        assert_eq!(Ok(11), bs.take_u8());
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_u8());
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_u8());
    }

    #[test]
    fn take_u16() {
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NeedMoreBytes(2)), bs.take_u16());
        let mut bs = BS::new(&[0xa1, 0xb2, 0xc3, 0xd4, 0xe5]);
        assert_eq!(Ok(0xa1b2), bs.take_u16());
        assert_eq!(Ok(0xc3d4), bs.take_u16());
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_u16());
        assert_eq!(Err(NeedMoreBytes(2)), bs.take_u16());
    }

    #[test]
    fn take_u32() {
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NeedMoreBytes(4)), bs.take_u32());
        let mut bs = BS::new(&[0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6]);
        assert_eq!(Ok(0xa1b2c3d4), bs.take_u32());
        assert_eq!(Err(NeedMoreBytes(2)), bs.take_u32());
        assert_eq!(Err(NeedMoreBytes(4)), bs.take_u32());
    }

    #[test]
    fn take_slice() {
        let mut res = [0; 3];
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NeedMoreBytes(3)), bs.take_slice(&mut res));
        let mut bs = BS::new(&[0xa1, 0xb2, 0xc3, 0xd4, 0xe5]);
        assert_eq!(Ok(()), bs.take_slice(&mut res));
        assert_eq!([0xa1, 0xb2, 0xc3], res);
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_slice(&mut res));
        assert_eq!(Err(NeedMoreBytes(3)), bs.take_vec(3));
    }

    #[test]
    pub fn take_vec() {
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NeedMoreBytes(3)), bs.take_vec(3));
        let mut bs = BS::new(&[0xa1, 0xb2, 0xc3, 0xd4, 0xe5]);
        assert_eq!(Ok(vec![0xa1, 0xb2, 0xc3]), bs.take_vec(3));
        assert_eq!(Err(NeedMoreBytes(1)), bs.take_vec(3));
        assert_eq!(Err(NeedMoreBytes(3)), bs.take_vec(3));
    }

    #[test]
    pub fn take_null_terminated() {
        let mut bs = BS::new(&[]);
        assert_eq!(Err(NoNullByte), bs.take_until_null());
        let mut bs = BS::new(&[10, 11, 12, 0, 20, 21, 0, 30]);
        assert_eq!(Ok(vec![10, 11, 12]), bs.take_until_null());
        assert_eq!(Ok(vec![20, 21]), bs.take_until_null());
        assert_eq!(1, bs.left());
        assert_eq!(Err(NoNullByte), bs.take_until_null());
        assert_eq!(0, bs.left());
    }
}
