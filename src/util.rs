pub fn u16_from_big_endian(bytes: &[u8; 2]) -> u16 {
    ((bytes[0] as u16) << 8) |
    ((bytes[1] as u16)     )
}

pub fn u32_from_big_endian(bytes: &[u8; 4]) -> u32 {
    ((bytes[0] as u32) << 24) |
    ((bytes[1] as u32) << 16) |
    ((bytes[2] as u32) <<  8) |
    ((bytes[3] as u32)      )
}

#[cfg(test)]
mod test {
    use super::{u16_from_big_endian, u32_from_big_endian};
    
    #[test]
    fn u16_from_be() {
        assert_eq!(0x0079, u16_from_big_endian(&[0, 0x79]));
        assert_eq!(0x7900, u16_from_big_endian(&[0x79, 0]));
    }

    #[test]
    fn u32_from_be() {
        assert_eq!(0x00000079, u32_from_big_endian(&[0, 0, 0, 0x79]));
        assert_eq!(0x00007900, u32_from_big_endian(&[0, 0, 0x79, 0]));
        assert_eq!(0x00790000, u32_from_big_endian(&[0, 0x79, 0, 0]));
        assert_eq!(0x79000000, u32_from_big_endian(&[0x79, 0, 0, 0]));
    }
}
