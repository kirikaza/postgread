use crate::msg::util::decode::*;

use ::std::cmp::PartialEq;
use ::std::fmt::Debug;

pub fn assert_decode_ok<Msg>(
    expected: Msg,
    bytes: &[u8],
) where Msg: Debug + MsgDecode + PartialEq {
    assert_decode(Ok(expected), bytes)
}

pub fn assert_decode_err<Msg>(
    expected: Problem,
    bytes: &[u8],
) where Msg: Debug + MsgDecode + PartialEq {
    assert_decode::<Msg>(Err(expected), bytes)
}

fn assert_decode<Msg>(
    expected: DecodeResult<Msg>,
    bytes: &[u8],
) where Msg: Debug + MsgDecode + PartialEq {
    let mut bytes_source = BytesSource::new(bytes);
    assert_eq!(expected, Msg::decode_body(&mut bytes_source));
    assert_eq!(0, bytes_source.left());
}
