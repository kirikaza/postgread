use futures::{Future};
use futures::future::{Either::*, Loop::*, err, loop_fn, ok};
use std::fmt::{self, Debug, Formatter};
use std::mem::{size_of_val};
use std::io::{BufRead, ErrorKind::UnexpectedEof};

use tokio::io::{self, AsyncRead};

#[derive(Debug, PartialEq)]
pub enum Message {
    AuthenticationOk,
    AuthenticationKerberosV5,
    AuthenticationCleartextPassword,
    AuthenticationMD5Password { salt: u32 },
    AuthenticationSCMCredential,
    AuthenticationGSS,
    AuthenticationSSPI,
    AuthenticationGSSContinue { auth_data: Vec<u8> },
    Startup {
        version: Version,
        params: Vec<StartupParam>,
    },
    Unknown { note: String },
}

impl Message {
    pub fn read<'a, R>(stream: R, is_first_msg: bool) -> impl Future<Item=Option<(R, Self)>, Error=io::Error> + 'a + Send
    where R : 'a + AsyncRead + BufRead + Send
    {
        read_u8(stream).then(move |res| match res {
            Ok((stream, first_byte)) => A(Self::read_ahead(first_byte, stream, is_first_msg).map(|stream_and_msg| {
                Some(stream_and_msg)
            })),
            Err(e) => match e.kind() {
                UnexpectedEof => B(ok(None)),
                _ => B(err(e)),
            },
        })
    }

    fn read_ahead<'a, R>(first_byte: u8, stream: R, is_first_msg: bool) -> impl Future<Item=(R, Self), Error=io::Error> + 'a + Send
    where R : 'a + AsyncRead + BufRead + Send
    {
        if is_first_msg {
            A(read_u32_tail(first_byte, stream).and_then(|(stream, full_len)| {
                Self::read_body(stream, None, full_len)
            }))
        } else {
            B(read_u32(stream).and_then(move |(stream, full_len)| {
                Self::read_body(stream, Some(first_byte), full_len)
            }))
        }
    }

    fn read_body<'a, R>(stream: R, type_byte: Option<u8>, full_len: u32) -> impl Future<Item=(R, Self), Error=io::Error> + 'a + Send
    where R : 'a + AsyncRead + BufRead + Send
    {
        let body_len = full_len - size_of_val(&full_len) as u32;
        // protect from reading extra bytes like `take()`
        let stream = stream.take(body_len as u64);
        match type_byte {
            None => Self::read_startup(stream),
            Some(b'R') => Self::read_auth(stream, body_len),
            Some(type_byte) => Self::read_unknown(stream, body_len, format!("unknown message type {}", type_byte as char)),
        }.map(|(stream, msg)| {
            (stream.into_inner(), msg)
        })
    }

    fn read_startup<'a, R>(stream: R) -> Box<'a + Future<Item=(R, Self), Error=io::Error> + Send>
    where R : 'a + AsyncRead + BufRead + Send
    {
        Box::new(Version::read(stream).and_then(|(stream, version)| {
            StartupParam::read_many(stream).map(move |(stream, params)| {
                (stream, Message::Startup { version, params })
            })
        }))
    }

    fn read_auth<'a, R>(stream: R, body_len: u32) -> Box<'a + Future<Item=(R, Self), Error=io::Error> + Send>
    where R : 'a + AsyncRead + BufRead + Send
    {
        Box::new(read_u32(stream).and_then(move |(stream, auth_type)| {
            let left_len = body_len - size_of_val(&auth_type) as u32;
            match auth_type {
                0 => Box::new(ok((stream, Message::AuthenticationOk))),
                2 => Box::new(ok((stream, Message::AuthenticationKerberosV5))),
                3 => Box::new(ok((stream, Message::AuthenticationCleartextPassword))),
                5 => Self::read_auth_cleartext_password(stream),
                6 => Box::new(ok((stream, Message::AuthenticationSCMCredential))),
                7 => Box::new(ok((stream, Message::AuthenticationGSS))),
                8 => Self::read_auth_gss_continue(stream, left_len),
                9 => Box::new(ok((stream, Message::AuthenticationSSPI))),
                _ => Self::read_unknown(stream, left_len, format!("unknown authentication sub-type {}", auth_type)),
            }
        }))
    }

    fn read_auth_cleartext_password<'a, R>(stream: R) -> Box<'a + Future<Item=(R, Self), Error=io::Error> + Send>
    where R : 'a + AsyncRead + BufRead + Send
    {
        Box::new(read_u32(stream).map(|(stream, salt)| {
            (stream, Message::AuthenticationMD5Password { salt })
        }))
    }

    fn read_auth_gss_continue<'a, R>(stream: R, left_len: u32) -> Box<'a + Future<Item=(R, Self), Error=io::Error> + Send>
    where R : 'a + AsyncRead + BufRead + Send
    {
        Box::new(io::read_to_end(stream, Vec::with_capacity(left_len as usize)).map(|(stream, auth_data)| {
            (stream, Message::AuthenticationGSSContinue { auth_data })
        }))
    }

    fn read_unknown<'a, R>(stream: R, left_len: u32, note: String) -> Box<'a + Future<Item=(R, Self), Error=io::Error> + Send>
    where R : 'a + AsyncRead + BufRead + Send
    {
        Box::new(read_and_drop(stream, left_len).map(move |stream| {
            (stream, Message::Unknown { note })
        }))
    }
}

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u16,
    minor: u16,
}
impl Version {
    fn read<R>(stream: R) -> impl Future<Item=(R, Self), Error=io::Error> + Send
    where R : AsyncRead + Send
    {
        read_u16(stream).and_then(|(stream, major)| {
            read_u16(stream).map(move |(stream, minor)| {
                (stream, Version { major, minor })
            })
        })
    }
}

#[derive(PartialEq)]
pub struct StartupParam {
    name: Vec<u8>,
    value: Vec<u8>,
}
impl StartupParam {
    fn read_many<R>(stream: R) -> impl Future<Item=(R, Vec<Self>), Error=io::Error> + Send
    where R : AsyncRead + BufRead + Send
    {
        loop_fn((stream, vec![]), |(stream, mut params)| {
            read_null_terminated(stream).and_then(move |(stream, mut name)| {
                if name.pop() != Some(0) {
                    A(err(error_other("can't read startup param name")))
                } else if name.is_empty() {
                    A(ok(Break((stream, params))))
                } else {
                    B(read_null_terminated(stream).and_then(|(stream, mut value)| {
                        if value.pop() != Some(0) {
                            Err(error_other("cant' read startup param value"))
                        } else {
                            params.push(StartupParam { name, value });
                            Ok(Continue((stream, params)))
                        }
                    }))
                }
            })
        })
    }
}
impl Debug for StartupParam {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "StartupParam {{ \"{}\": \"{}\" }}",
               String::from_utf8_lossy(&self.name),
               String::from_utf8_lossy(&self.value))
    }
}

fn read_u8<R>(stream: R) -> impl Future<Item=(R, u8), Error=io::Error> + Send
where R : AsyncRead + Send
{
    io::read_exact(stream, [0u8; 1])
        .map(|(stream, buf)| (stream, buf[0]))
}

fn read_u16<R>(stream: R) -> impl Future<Item=(R, u16), Error=io::Error> + Send
where R : AsyncRead + Send
{
    io::read_exact(stream, [0u8; 2])
        .map(|(stream, buf)| (stream, u16::from_be_bytes(buf)))
}

fn read_u32<R>(stream: R) -> impl Future<Item=(R, u32), Error=io::Error> + Send
where R : AsyncRead + Send
{
    io::read_exact(stream, [0u8; 4])
        .map(|(stream, buf)| (stream, u32::from_be_bytes(buf)))
}

fn read_u32_tail<R>(head: u8, stream: R) -> impl Future<Item=(R, u32), Error=io::Error> + Send
where R : AsyncRead + Send
{
    io::read_exact(stream, [0u8; 3])
        .map(move |(stream, tail)| {
            let buf = [head, tail[0], tail[1], tail[2]];
            (stream, u32::from_be_bytes(buf))
        })
}

fn read_null_terminated<R>(stream: R) -> impl Future<Item=(R, Vec<u8>), Error=io::Error> + Send
where R: AsyncRead + BufRead + Send
{
    io::read_until(stream, 0, vec![])
}

fn read_and_drop<R>(stream: R, num: u32) -> impl Future<Item=R, Error=io::Error> + Send
where R: AsyncRead + Send
{
    const BATCH: usize = 64*1024;
    loop_fn((stream, num as usize), |(stream, left)| {
        if left < BATCH {
            A(io::read_exact(stream, vec![0u8; left])
                .map(|(stream, _)| Break(stream)))
        } else {
            B(io::read_exact(stream, vec![0u8; BATCH])
                .map(move |(stream, _)| Continue((stream, left - BATCH))))
        }
    })
}

fn error_other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::{Async::*, Poll};

    #[test]
    fn startup_without_params() {
        let mut bytes: &[u8] = &[
            0,0,0,9, // len
            0,3,0,1, // version
            0, // params
        ];
        assert_eq!(
            Ok(Ready(Some(Message::Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            }))),
            simplify(&mut Message::read(&mut bytes, true)),
        );
    }

    #[test]
    fn startup_with_params() {
        let mut bytes = vec![
            0,0,0,37, // len
            0,3,1,0, // version
        ];
        bytes.extend_from_slice(b"user\0root\0database\0postgres\0\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            Ok(Ready(Some(Message::Startup {
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
            }))),
           simplify(&mut Message::read(&mut bytes, true)),
        );        
    }

    fn simplify<R>(future: &mut Future<Item=Option<(R, Message)>, Error=io::Error>) -> Poll<Option<Message>, String>
    where R : AsyncRead + Debug
    {
        match future.poll() {
            Ok(Ready(ready)) => Ok(Ready(ready.map(|(_stream, msg)| msg))),
            Ok(NotReady) => Ok(NotReady),
            Err(e) => Err(e.to_string()),
        }
    }
}
