use crate::msg::util::io::*;
use ::futures::Future;
use ::futures::io::{AsyncBufReadExt, AsyncReadExt};
use ::std::mem::{size_of_val};
use ::std::io::Result as IoResult;

pub async fn read_msg_with_len<'r, R, Msg, Fut>(
    stream: &'r mut R,
    read_body: impl Fn(&'r mut R, u32) -> Fut,
) -> IoResult<Msg>
where
    R: AsyncBufReadExt + Unpin,
    Fut: Future<Output=IoResult<Msg>>,
{
    let body_len = read_body_len(stream).await?;
    read_msg_body(stream, body_len, read_body).await
}

pub async fn read_msg_with_len_unless_eof<'r, R, Msg, Fut>(
    stream: &'r mut R,
    read_body: impl Fn(&'r mut R, u32) -> Fut,
) -> IoResult<Option<Msg>>
where
    R: AsyncBufReadExt + Unpin,
    Fut: Future<Output=IoResult<Msg>>,
{
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

async fn read_msg_body<'r, R, Msg, Fut>(
    stream: &'r mut R,
    body_len: u32,
    read_body: impl Fn(&'r mut R, u32) -> Fut,
) -> IoResult<Msg>
where
    R: AsyncBufReadExt + Unpin,
    Fut: Future<Output=IoResult<Msg>>,
{
    // TODO: protect from reading extra bytes like `stream.take(u64::from(body_len))`
    Ok(read_body(stream, body_len).await?)
}


