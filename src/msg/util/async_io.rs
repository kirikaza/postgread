use futures::io::{AsyncBufReadExt, AsyncReadExt};
use std::io::{Error, ErrorKind, Result as IoResult};

pub fn error_other(text: &str) -> Error {
    Error::new(ErrorKind::Other, text)
}

pub fn accept_eof<T>(result: IoResult<T>) -> IoResult<Option<T>> {
    match result {
        Ok(data) =>
            Ok(Some(data)),
        Err(e) => match e.kind() {
            ErrorKind::UnexpectedEof => Ok(None),
            _ => Err(e),
        },
    }
}

pub async fn read_u8<R>(stream: &mut R) -> IoResult<u8>
where R: AsyncReadExt + Unpin
{
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await?;
    Ok(buf[0])
}

pub async fn read_vec<R>(stream: &mut R, len: usize) -> IoResult<Vec<u8>>
where R: AsyncReadExt + Unpin
{
    let mut buf = vec![0u8; len];
    stream.read_exact(buf.as_mut_slice()).await?;
    Ok(buf)
}

pub async fn read_u32<R>(stream: &mut R) -> IoResult<u32>
where R: AsyncReadExt + Unpin
{
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    Ok(u32::from_be_bytes(buf))
}

pub async fn read_and_drop<R>(stream: R, num: u32) -> IoResult<()>
where R: AsyncBufReadExt + Unpin
{
    const BATCH: usize = 64*1024;
    let mut stream = stream;
    let mut left = num as usize;
    let mut buf = [0u8; BATCH];
    while left >= BATCH {
        stream.read_exact(&mut buf).await?;
        left -= BATCH;
    }
    let mut buf = &mut vec![0; left][..];
    stream.read_exact(&mut buf).await?;
    Ok(())
}
