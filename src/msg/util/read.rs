use crate::msg::util::async_io::*;
use crate::msg::util::decode::{BytesSource, MsgDecode, Problem as DecodeError};
use crate::msg::util::encode::{BytesTarget, EncodeResult, Problem as EncodeError};
use ::futures::io::AsyncReadExt;
use ::std::io::Error as IoError;

pub type ReadResult<Msg> = Result<ReadData<Msg>, ReadError>;

#[derive(Debug)]
pub enum ReadError {
    IoError(IoError),
    EncodeError(EncodeError),
}

#[derive(Debug, PartialEq)]
pub struct ReadData<Msg> {
    pub bytes: Vec<u8>,
    pub msg_result: Result<Msg, MsgError>,
}

#[derive(Debug, PartialEq)]
pub enum MsgError {
    DecodeError(DecodeError),
    LeftUndecoded(usize),
}

pub async fn read_msg<R, Msg>(stream: &mut R) -> ReadResult<Msg>
where
    R: AsyncReadExt + Unpin,
    Msg: MsgDecode,
{
    use ReadError::*;
    use MsgError::*;
    let specified_len = read_u32(stream).await.map_err(IoError)?;
    let mut bytes = alloc_vec::<Msg>(specified_len);
    let mut bytes_target = BytesTarget::new(&mut bytes);
    put_header::<Msg>(specified_len, &mut bytes_target).map_err(EncodeError)?;
    let body_bytes = bytes_target.rest();
    stream.read_exact(body_bytes).await.map_err(IoError)?;
    let mut bytes_source = BytesSource::new(body_bytes);
    let msg_result = Msg::decode_body(&mut bytes_source)
        .map_err(DecodeError)
        .and_then(|msg| match bytes_source.left() {
            0 => Ok(msg),
            n => Err(LeftUndecoded(n)),
        });
    Ok(ReadData { bytes, msg_result })
}

fn alloc_vec<Msg: MsgDecode>(specified_len: u32) -> Vec<u8> {
    let type_len = if Msg::TYPE_BYTE_OPT.is_some() { 1 } else { 0 };
    let full_len = type_len + specified_len;
    vec![0u8; full_len as usize]
}

fn put_header<Msg: MsgDecode>(specified_len: u32, bytes_target: &mut BytesTarget) -> EncodeResult<()> {
    if let Some(type_byte) = Msg::TYPE_BYTE_OPT {
        bytes_target.put_u8(type_byte.into())?;
    }
    bytes_target.put_u32(specified_len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::util::decode::{BytesSource, DecodeResult, MsgDecode, Problem as DecodeError};
    use crate::msg::type_byte::TypeByte;
    use ::futures::executor::block_on;
    use ::std::fmt::Debug;
    use ::std::marker::PhantomData;
    use ::std::io::ErrorKind::UnexpectedEof;

    macro_rules! test_all_types {
        (
            <$body: ty>,
            $test_fn: ident,
            $fn_name_without_type: ident,
            $fn_name_having_type: ident
        ) => {
            #[test]
            fn $fn_name_without_type() {
                $test_fn::<WithoutType, $body>()
            }

            #[test]
            fn $fn_name_having_type() {
                $test_fn::<HavingType, $body>()
            }
        }
    }

    macro_rules! test_any_body {
        (<$body: ty>) => {
            test_all_types!{<$body>, test_no_len, no_len_without_type, no_len_having_type}
            test_all_types!{<$body>, test_valid, valid_without_type, valid_having_type}
            test_all_types!{<$body>, test_extra, extra_without_type, extra_having_type}
        }
    }

    macro_rules! test_non_empty_body {
        (<$body: ty>) => {
            test_all_types!{<$body>, test_different, different_without_type, different_having_type}
            test_all_types!{<$body>, test_cut, cut_without_type, cut_having_type}
            test_all_types!{<$body>, test_len_is_less, len_is_less_without_type, len_is_less_having_type}
            test_all_types!{<$body>, test_len_is_more, len_is_more_without_type, len_is_more_having_type}
        }
    }

    mod empty_body {
        use super::*;
        test_any_body!{<EmptyBody>}
    }

    mod small_body {
        use super::*;
        test_any_body!{<SmallBody>}
        test_non_empty_body!{<SmallBody>}
    }

    mod medium_body {
        use super::*;
        test_any_body!{<MediumBody>}
        test_non_empty_body!{<MediumBody>}
    }

    mod large_body {
        use super::*;
        test_any_body!{<LargeBody>}
        test_non_empty_body!{<LargeBody>}
    }

    mod huge_body {
        use super::*;
        test_any_body!{<HugeBody>}
        test_non_empty_body!{<HugeBody>}
    }

    fn test_no_len<T: Type, B: Body>() {
        assert_io_err_eof(read_test_msg::<T, B>(&vec![]));
        assert_io_err_eof(read_test_msg::<T, B>(&vec![0]));
        assert_io_err_eof(read_test_msg::<T, B>(&vec![0, 0]));
        assert_io_err_eof(read_test_msg::<T, B>(&vec![0, 0, 0]));
    }

    fn test_cut<T: Type, B: Body>() {
        let cut_bytes = [
            &B::LEN_BYTES[..],
            &B::EXPECTED_BYTES[0 .. B::EXPECTED_BYTES.len() - 1],
        ].concat();
        assert_io_err_eof(read_test_msg::<T, B>(&cut_bytes));
    }

    fn test_valid<T: Type, B: Body>() {
        let origin_bytes = [
            &B::LEN_BYTES[..],
            B::EXPECTED_BYTES,
        ].concat();
        let ReadData { bytes, msg_result } = read_test_msg::<T, B>(&origin_bytes).unwrap();
        assert_eq_bytes::<T>(origin_bytes, bytes);
        assert_ok_test_msg(msg_result);
    }

    fn test_extra<T: Type, B: Body>() {
        let valid_bytes = [
            &B::LEN_BYTES[..],
            B::EXPECTED_BYTES,
        ].concat();
        let extra_bytes = [
            &valid_bytes[..],
            b"+",
        ].concat();
        let ReadData { bytes, msg_result } = read_test_msg::<T, B>(&extra_bytes).unwrap();
        assert_eq_bytes::<T>(valid_bytes, bytes);
        assert_ok_test_msg(msg_result);
    }

    fn test_len_is_less<T: Type, B: Body>() {
        let mut patched_bytes = [
            &B::LEN_BYTES[0..3], &[B::LEN_BYTES[3]-1][..],
            B::EXPECTED_BYTES,
        ].concat();
        let ReadData { bytes, msg_result } = read_test_msg::<T, B>(&patched_bytes).unwrap();
        patched_bytes.pop();  // len is decreased
        assert_eq_bytes::<T>(patched_bytes, bytes);
        match msg_result {
            Err(MsgError::DecodeError(DecodeError::NeedMoreBytes(1))) => {},
            _ => panic!(concat!("decoded should be ", stringify!(Err(MsgError::DecodeError(DecodeError::NeedMoreBytes(1)))))),
        }
    }

    fn test_len_is_more<T: Type, B: Body>() {
        let longer_bytes = [
            &B::LEN_BYTES[0..3], &[B::LEN_BYTES[3]+1][..],
            B::EXPECTED_BYTES, b"+",
        ].concat();
        let ReadData { bytes, msg_result } = read_test_msg::<T, B>(&longer_bytes).unwrap();
        assert_eq_bytes::<T>(longer_bytes, bytes);
        match msg_result {
            Err(MsgError::LeftUndecoded(1)) => {},
            _ => panic!(concat!("decoded should be ", stringify!(Err(MsgError::LeftUndecoded(1))))),
        }
    }

    fn test_different<T: Type, B: Body>() {
        let diff_bytes = [
            &B::LEN_BYTES[..],
            &B::EXPECTED_BYTES[0 .. B::EXPECTED_BYTES.len() - 1], b"*",
        ].concat();
        let ReadData { bytes, msg_result } = read_test_msg::<T, B>(&diff_bytes).unwrap();
        assert_eq_bytes::<T>(diff_bytes, bytes);
        match msg_result {
            Err(MsgError::DecodeError(DecodeError::Incorrect(_))) => {},
            _ => panic!(concat!("decoded should be ", stringify!(Err(DecodeProblem::Incorrect)))),
        }
    }

    fn assert_eq_bytes<T: Type>(expected: Vec<u8>, actual: Vec<u8>) {
        match T::TYPE_BYTE_OPT {
            Some(type_byte) => unsafe {
                assert_eq!(u8::from(type_byte) as char, actual[0] as char);
                assert_eq!(String::from_utf8_unchecked(expected), String::from_utf8_unchecked(actual)[1..]);
            },
            None => unsafe {
                assert_eq!(String::from_utf8_unchecked(expected), String::from_utf8_unchecked(actual));
            }
        }
    }

    fn assert_ok_test_msg<T: Type, B: Body>(decoded: Result<TestMsg<T, B>, MsgError>) {
        assert_eq!(Ok(TestMsg::new()), decoded);
    }

    fn assert_io_err_eof<T: Type, B: Body>(read_result: ReadResult<TestMsg<T, B>>) {
        assert!(read_result.is_err());
        match read_result {
            Err(ReadError::IoError(io_error)) => {
                assert_eq!(UnexpectedEof, io_error.kind());
            },
            _ => panic!(concat!("read_result should be ", stringify!(Err(ReadError::IoError)))),
        }
    }

    fn read_test_msg<T: Type, B: Body>(bytes: &Vec<u8>) -> ReadResult<TestMsg<T, B>> {
        block_on(read_msg(&mut bytes.as_slice()))
    }

    #[derive(Debug, PartialEq)]
    struct TestMsg<TypeByte, DecodeBody> {
        type_byte_impl: PhantomData<TypeByte>,
        decode_body_impl: PhantomData<DecodeBody>,
    }

    impl<T, B> TestMsg<T, B> {
        fn new() -> Self {
            Self {
                type_byte_impl: PhantomData,
                decode_body_impl: PhantomData,
            }
        }
    }

    impl<T: Type, B: Body> MsgDecode for TestMsg<T, B> {
        const TYPE_BYTE_OPT: Option<TypeByte> = T::TYPE_BYTE_OPT;

        fn decode_body(bytes_source: &mut BytesSource) -> DecodeResult<Self> {
            let taken = bytes_source.take_vec(B::EXPECTED_BYTES.len())?;
            if taken != B::EXPECTED_BYTES {
                return Err(DecodeError::Incorrect(unsafe {
                    format!("taken {} but expected {}",
                            String::from_utf8_unchecked(taken),
                            String::from_utf8_unchecked(B::EXPECTED_BYTES.to_vec()),
                    )
                }));
            }
            Ok(Self::new())
        }
    }

    trait Type: Debug + PartialEq {
        const TYPE_BYTE_OPT: Option<TypeByte>;
    }

    trait Body: Debug + PartialEq {
        const LEN_BYTES: [u8; 4];

        const EXPECTED_BYTES: &'static [u8];

        fn decode_body(bs: &mut BytesSource) -> DecodeResult<()> {
            for i in 0..Self::EXPECTED_BYTES.len() {
                let found = bs.take_u8()?;
                let expected = Self::EXPECTED_BYTES[i];
                if found != expected {
                    return Err(DecodeError::Incorrect(
                        format!("found {} at index {} but expected {}", found, i, expected)
                    ));
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, PartialEq)]
    struct WithoutType();
    impl Type for WithoutType {
        const TYPE_BYTE_OPT: Option<TypeByte> = None;
    }

    #[derive(Debug, PartialEq)]
    struct HavingType();
    impl Type for HavingType {
        const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::RowDescription);
    }

    #[derive(Debug, PartialEq)]
    struct EmptyBody();
    impl Body for EmptyBody {
        const LEN_BYTES: [u8; 4] = [0, 0, 0, 0 + 4];
        const EXPECTED_BYTES: &'static [u8] = &[];
    }

    #[derive(Debug, PartialEq)]
    struct SmallBody();
    impl Body for SmallBody {
        const LEN_BYTES: [u8; 4] = [0, 0, 0, 5 + 4];
        const EXPECTED_BYTES: &'static [u8] = b"small";  // len fits 1 byte
    }

    #[derive(Debug, PartialEq)]
    struct MediumBody();
    impl Body for MediumBody {
        const LEN_BYTES: [u8; 4] = [0, 0, 1, 3 + 4];
        const EXPECTED_BYTES: &'static [u8] = &[b'M'; 0x103];  // len fits 2 bytes
    }

    #[derive(Debug, PartialEq)]
    struct LargeBody();
    impl Body for LargeBody {
        const LEN_BYTES: [u8; 4] = [0, 1, 3, 5 + 4];
        const EXPECTED_BYTES: &'static [u8] = &[b'L'; 0x10305];  // len fits 3 bytes
    }

    #[derive(Debug, PartialEq)]
    struct HugeBody();
    impl Body for HugeBody {
        const LEN_BYTES: [u8; 4] = [1, 3, 5, 7 + 4];
        const EXPECTED_BYTES: &'static [u8] = &[b'H'; 0x1030507];  // len fits 4 bytes
    }
}