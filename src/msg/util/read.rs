use crate::msg::util::async_io::*;
use ::futures::io::AsyncReadExt;
use ::std::mem::{size_of_val};
use ::std::io::Cursor;
use ::std::io::Result as IoResult;

pub async fn read_msg_with_len<R, Msg>(
    stream: &mut R,
    decode: impl Fn(&mut Cursor<Vec<u8>>, u32) -> IoResult<Msg>,
) -> IoResult<Msg>
where R: AsyncReadExt + Unpin {
    let body_len = read_body_len(stream).await?;
    read_msg_body(stream, body_len, decode).await
}

pub async fn read_msg_with_len_unless_eof<R, Msg>(
    stream: &mut R,
    read_body: impl Fn(&mut Cursor<Vec<u8>>, u32) -> IoResult<Msg>,
) -> IoResult<Option<Msg>>
where R: AsyncReadExt + Unpin {
    match accept_eof(read_body_len(stream).await)? {
        Some(body_len) => {
            let msg = read_msg_body(stream, body_len, read_body).await?;
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

async fn read_msg_body<R, Msg>(
    stream: &mut R,
    body_len: u32,
    decode: impl Fn(&mut Cursor<Vec<u8>>, u32) -> IoResult<Msg>,
) -> IoResult<Msg>
    where R: AsyncReadExt + Unpin {
    let vec = read_vec(stream, body_len as usize).await?;
    let mut cursor = Cursor::new(vec);
    let msg = decode(&mut cursor, body_len)?;
    let left_bytes = cursor.position() - body_len as u64;
    if left_bytes == 0 {
        Ok(msg)
    } else {
        let err = format!("{} bytes haven't been decoded", left_bytes);
        Err(error_other(&err))
    }
}