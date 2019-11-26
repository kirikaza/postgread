use std::io::{BufRead, Error, ErrorKind, Read, Result as IoResult};

pub fn error_other(text: &str) -> Error {
    Error::new(ErrorKind::Other, text)
}

pub fn read_u8<R>(stream: &mut R) -> IoResult<u8>
where R: Read {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16<R>(stream: &mut R) -> IoResult<u16>
where R: Read {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

pub fn read_vec<R>(stream: &mut R, len: usize) -> IoResult<Vec<u8>>
where R: Read {
    let mut buf = vec![0u8; len];
    stream.read_exact(buf.as_mut_slice())?;
    Ok(buf)
}

pub fn read_u32<R>(stream: &mut R) -> IoResult<u32>
where R: Read {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

pub fn read_null_terminated<R>(stream: &mut R) -> IoResult<Vec<u8>>
where R: BufRead
{
    let mut buf = vec![];
    stream.read_until(0, &mut buf)?;
    Ok(buf)
}
