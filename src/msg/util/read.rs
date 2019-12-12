use crate::msg::util::async_io::*;
use crate::msg::util::decode::{BytesSource, DecodeResult};
use ::futures::io::AsyncReadExt;
use ::std::mem::{size_of_val};
use ::std::io::Result as IoResult;

pub async fn read_msg_with_len<R, Msg, F>(
    stream: &mut R,
    decode: F,
) -> IoResult<Msg>
where
    R: AsyncReadExt + Unpin,
    F: Fn(&mut BytesSource) -> DecodeResult<Msg>,
{
    let body_len = read_body_len(stream).await?;
    read_msg_body(stream, body_len, decode).await
}

pub async fn read_msg_with_len_unless_eof<R, Msg, F>(
    stream: &mut R,
    decode: F,
) -> IoResult<Option<Msg>>
where
    R: AsyncReadExt + Unpin,
    F: Fn(&mut BytesSource) -> DecodeResult<Msg>,
{
    match accept_eof(read_body_len(stream).await)? {
        Some(body_len) => {
            let msg = read_msg_body(stream, body_len, decode).await?;
            Ok(Some(msg))
        },
        None => Ok(None),
    }
}

pub async fn read_body_len<R>(stream: &mut R) -> IoResult<u32>
where R: AsyncReadExt + Unpin {
    let full_len = read_u32(stream).await?;
    Ok(full_len - size_of_val(&full_len) as u32)
}

async fn read_msg_body<R, Msg, F>(
    stream: &mut R,
    body_len: u32,
    decode: F,
) -> IoResult<Msg>
where
    R: AsyncReadExt + Unpin,
    F: Fn(&mut BytesSource) -> DecodeResult<Msg>,
{
    let vec = read_vec(stream, body_len as usize).await?;
    let mut bytes_source = BytesSource::new(vec.as_slice());
    match decode(&mut bytes_source) {
        Ok(msg) => match bytes_source.left() {
            0 => Ok(msg),
            left_bytes => Err(error_other(&format!("{} bytes haven't been decoded", left_bytes))),
        },
        Err(problem) => Err(error_other(&format!("{:?}", problem))),
    }
}
