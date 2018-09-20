use util;

use std::fmt::{self, Debug, Formatter};
use std::mem::size_of_val;
use std::io::{self, BufRead, Read};
use std::io::ErrorKind::UnexpectedEof;

type IO<T> = io::Result<T>;

#[derive(Debug, PartialEq)]
pub enum Message {
    Startup {
        version: Version,
        params: Vec<StartupParam>,
    },
    Unknown {
        type_sym: Option<char>,
        body_len: u32,
    },
}
impl Message {
    pub fn read<S: BufRead>
        (stream: &mut S, startup: bool) -> IO<Option<Self>>
    {
        let type_byte: Option<u8>;
        let full_len: u32;
        if startup {
            type_byte = None;
            match read_u32(stream) {
                Ok(o) => full_len = o,
                Err(ref e) if e.kind() == UnexpectedEof => return Ok(None),
                Err(e) => return Err(e),
            }
        } else {
            match read_u8(stream) {
                Ok(o) => type_byte = Some(o),
                Err(ref e) if e.kind() == UnexpectedEof => return Ok(None),
                Err(e) => return Err(e),
            }
            full_len = read_u32(stream)?;
        };
        let body_len = full_len - size_of_val(&full_len) as u32;
        // protect from reading extra bytes:
        let mut stream = stream.take(body_len as u64);
        match type_byte {
            None => Ok(Some(Message::Startup {
                version: Version::read(&mut stream)?,
                params: StartupParam::read_many(&mut stream)?,
            })),
            Some(type_byte) => {
                read_and_drop(&mut stream, body_len)?;
                Ok(Some(Message::Unknown { type_sym: Some(type_byte as char), body_len }))
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u16,
    minor: u16,
}
impl Version {
    fn read(stream: &mut Read) -> IO<Self> {
        Ok(Version {
            major: read_u16(stream)?,
            minor: read_u16(stream)?,
        })
    }
}

#[derive(PartialEq)]
pub struct StartupParam {
    name: Vec<u8>,
    value: Vec<u8>,
}
impl StartupParam {
    fn read_many(stream: &mut BufRead) -> IO<Vec<Self>> {
        let mut params = vec![];
        loop {
            let mut name = read_null_terminated(stream)?;
            if name.pop() != Some(0) {
                return Err(io::Error::new(io::ErrorKind::Other, "can't read startup param name"));
            }
            if name.is_empty() {
                break;
            }
            let mut value = read_null_terminated(stream)?;
            if value.pop() != Some(0) {
                return Err(io::Error::new(io::ErrorKind::Other, "cant' read startup param value"));
            }
            params.push(StartupParam { name, value });
        }
        Ok(params)
    }
}
impl Debug for StartupParam {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "StartupParam {{ \"{}\": \"{}\" }}",
               String::from_utf8_lossy(&self.name),
               String::from_utf8_lossy(&self.value))
    }
}

fn read_u8(stream: &mut Read) -> IO<u8> {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16(stream: &mut Read) -> IO<u16> {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf)?;
    Ok(util::u16_from_big_endian(&buf))
}

fn read_u32(stream: &mut Read) -> IO<u32> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    Ok(util::u32_from_big_endian(&buf))
}

fn read_null_terminated(stream: &mut BufRead) -> IO<Vec<u8>> {
    let mut buf = vec![];
    stream.read_until(0, &mut buf)?;
    Ok(buf)
}

fn read_and_drop(stream: &mut Read, num: u32) -> IO<()> {
    const BATCH: usize = 64*1024;
    let mut buf = [0u8; BATCH];
    let mut left = num as usize;
    while left >= BATCH {
        stream.read_exact(&mut buf)?;
        left -= BATCH;
    }
    let mut buf = &mut vec![0; left][..];
    stream.read_exact(&mut buf)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn startup_without_params() {
        let mut bytes: &[u8] = &[
            0,0,0,9, // len
            0,3,0,1, // version
            0, // params
        ];
        assert_eq!(
            Message::Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            },
            Message::read(&mut bytes, true).unwrap().unwrap(),
        );
    }

    #[test]
    fn startup_with_params() {
        let mut bytes = vec![
            0,0,0,37, // len
            0,3,1,0, // version
        ];
        bytes.extend_from_slice(b"user\0root\0database\0postgres\0\0");
        println!("{:?}", bytes);
        assert_eq!(
            Message::Startup {
                version: Version { major: 3, minor: 0x100 },
                params: vec![
                    StartupParam {
                        name: Vec::from(&b"user"[..]),
                        value: Vec::from(&b"root"[..]),
                    },
                    StartupParam {
                        name: Vec::from(&b"database"[..]),
                        value: Vec::from(&b"postgres"[..]),
                    },
                ],
            },
            Message::read(&mut &bytes[..], true).unwrap().unwrap(),
        );        
    }
}
