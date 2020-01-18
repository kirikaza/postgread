use ::std::mem::size_of;

#[derive(Debug, PartialEq)]
pub enum Problem {
    OutOfSpace(usize),
}

pub type EncodeResult<Ok> = std::result::Result<Ok, Problem>;

pub struct BytesTarget<'a> {
    slice: &'a mut [u8],
    pos: usize,
}

macro_rules! put_bounded {
    (
        $typ: ident,
        $self: ident,
        $value: ident
    ) => {{
        const LEN: usize = size_of::<$typ>();
        if $self.pos + LEN <= $self.slice.len() {
            let bytes: [u8; LEN] = $value.to_be_bytes();
            for i in 0..LEN {
                $self.slice[$self.pos + i] = bytes[i];
            }
            $self.pos += LEN;
            Ok(())
        } else {
            let left = $self.slice.len() - $self.pos;
            $self.pos = $self.slice.len();
            Err(Problem::OutOfSpace(LEN - left))
        }
    }};
}

impl<'a> BytesTarget<'a> {
    pub fn new(slice: &'a mut [u8]) -> Self {
        Self { slice, pos: 0 }
    }

    pub fn left(self: &Self) -> usize {
        self.slice.len() - self.pos
    }

    pub fn rest(self: &mut Self) -> &mut [u8] {
        &mut self.slice[self.pos..]
    }

    pub fn put_u8(self: &mut Self, value: u8) -> EncodeResult<()> {
        put_bounded!(u8, self, value)
    }

    pub fn put_u32(self: &mut Self, value: u32) -> EncodeResult<()> {
        put_bounded!(u32, self, value)
    }
}

#[cfg(test)]
mod tests {
    use super::{BytesTarget as BT, Problem::*};

    #[test]
    fn left() {
        let slice = &mut [];
        let bt = BT::new(slice);
        assert_eq!(0, bt.left());
        let slice = &mut [0, 0];
        let mut bt = BT::new(slice);
        assert_eq!(2, bt.left());
        assert_eq!(2, bt.left());
        bt.pos += 1;
        assert_eq!(1, bt.left());
        assert_eq!(1, bt.left());
        bt.pos += 1;
        assert_eq!(0, bt.left());
    }

    #[test]
    fn put_u8() {
        let slice = &mut [];
        let mut bt = BT::new(slice);
        assert_eq!(Err(OutOfSpace(1)), bt.put_u8(0xa1));
        let slice = &mut [0; 2];
        let mut bs = BT::new(slice);
        assert_eq!(Ok(()), bs.put_u8(0xa1));
        assert_eq!([0xa1, 0], *bs.slice);
        assert_eq!(Ok(()), bs.put_u8(0xb2));
        assert_eq!([0xa1, 0xb2], *bs.slice);
        assert_eq!(Err(OutOfSpace(1)), bs.put_u8(0xc3));
        assert_eq!(Err(OutOfSpace(1)), bs.put_u8(0xd4));
    }

    #[test]
    fn put_u32() {
        let slice = &mut [];
        let mut bt = BT::new(slice);
        assert_eq!(Err(OutOfSpace(4)), bt.put_u32(0xa1b2c3d4));
        let slice = &mut [0; 6];
        let mut bs = BT::new(slice);
        assert_eq!(Ok(()), bs.put_u32(0xa1b2c3d4));
        assert_eq!([0xa1, 0xb2, 0xc3, 0xd4, 0, 0], *bs.slice);
        assert_eq!(Err(OutOfSpace(2)), bs.put_u32(0xe5f6a7b8));
        assert_eq!(Err(OutOfSpace(4)), bs.put_u32(0xc9d0e1f2));
    }
}
