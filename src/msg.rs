use futures::io::{AsyncBufReadExt, AsyncReadExt};
use std::fmt::{self, Debug, Formatter};
use std::mem::{size_of_val};
use std::io::{self, ErrorKind::UnexpectedEof};

#[derive(Debug, PartialEq)]
pub enum Message {
    Authentication(Authentication),
    Startup(Startup),
    Unknown { note: String },
}

impl Message {
    pub async fn read<R>(stream: &mut R, is_first_msg: bool) -> io::Result<Option<Self>>
    where R: AsyncBufReadExt + Unpin      
    {
        match read_u8(stream).await {
            Ok(first_byte) =>
                Ok(Some(Self::read_ahead(first_byte, stream, is_first_msg).await?)),
            Err(e) => match e.kind() {
                UnexpectedEof => Ok(None),
                _ => Err(e),
            },
        }
    }

    async fn read_ahead<R>(first_byte: u8, stream: &mut R, is_first_msg: bool) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        if is_first_msg {
            let full_len = read_u32_tail(first_byte, stream).await?;
            Self::read_body(stream, None, full_len).await
        } else {
            let full_len = read_u32(stream).await?;
            Self::read_body(stream, Some(first_byte), full_len).await
        }
    }

    async fn read_body<R>(stream: &mut R, type_byte: Option<u8>, full_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let body_len = full_len - size_of_val(&full_len) as u32;
        // TODO: protect from reading extra bytes like `stream.take(u64::from(body_len))`
        match type_byte {
            None => Ok(Self::Startup(Startup::read(stream).await?)),
            Some(b'R') => Ok(Self::Authentication(Authentication::read(stream, body_len).await?)),
            Some(type_byte) => Self::read_unknown(stream, body_len, format!("unknown message type {}", type_byte as char)).await,
        }
    }


    async fn read_unknown<R>(stream: &mut R, left_len: u32, note: String) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        read_and_drop(stream, left_len).await?;
        Ok(Message::Unknown { note })
    }
}

#[derive(Debug, PartialEq)]
pub enum Authentication {
    Ok,
    KerberosV5,
    CleartextPassword,
    MD5Password { salt: [u8; 4] },
    SCMCredential,
    GSS,
    SSPI,
    GSSContinue { auth_data: Vec<u8> },
    Unknown { note: String },
}

impl Authentication {
    async fn read<R>(stream: &mut R, body_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let auth_type = read_u32(stream).await?;
        let left_len = body_len - size_of_val(&auth_type) as u32;
        match auth_type {
            0 => Ok(Self::Ok),
            2 => Ok(Self::KerberosV5),
            3 => Ok(Self::CleartextPassword),
            5 => Self::read_md5_password(stream).await,
            6 => Ok(Self::SCMCredential),
            7 => Ok(Self::GSS),
            8 => Self::read_gss_continue(stream, left_len).await,
            9 => Ok(Self::SSPI),
            _ => Self::read_unknown(stream, left_len, format!("unknown auth type {}", auth_type)).await,
        }
    }

    async fn read_md5_password<R>(stream: &mut R) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let mut salt = [0u8; 4];
        stream.read_exact(&mut salt).await?;
        Ok(Self::MD5Password { salt })
    }

    async fn read_gss_continue<R>(stream: &mut R, left_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let mut auth_data = Vec::with_capacity(left_len as usize);
        stream.read_to_end(&mut auth_data).await?;
        Ok(Self::GSSContinue { auth_data })
    }

    async fn read_unknown<R>(stream: &mut R, left_len: u32, note: String) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        read_and_drop(stream, left_len).await?;
        Ok(Self::Unknown { note })
    }
}

#[derive(Debug, PartialEq)]
pub struct Startup {
    version: Version,
    params: Vec<StartupParam>,
}

impl Startup {
    async fn read<R>(stream: &mut R) -> io::Result<Self>
        where R: AsyncBufReadExt + Unpin
    {
        let version = Version::read(stream).await?;
        let params = StartupParam::read_many(stream).await?;
        Ok(Startup { version, params })
    }
}

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u16,
    minor: u16,
}
impl Version {
    async fn read<R>(stream: &mut R) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let major = read_u16(stream).await?;
        let minor = read_u16(stream).await?;
        Ok(Version { major, minor })
    }
}

#[derive(PartialEq)]
pub struct StartupParam {
    name: Vec<u8>,
    value: Vec<u8>,
}
impl StartupParam {
    async fn read_many<R>(stream: &mut R) -> io::Result<Vec<Self>>
    where R: AsyncBufReadExt + Unpin
    {
        let mut params = vec![];
        loop {
            let mut name = read_null_terminated(stream).await?;
            if name.pop() != Some(0) {
                return Err(error_other("can't read startup param name"));
            }
            if name.is_empty() {
                break;
            }
            let mut value = read_null_terminated(stream).await?;
            if value.pop() != Some(0) {
                return Err(error_other("can't read startup param value"));
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

async fn read_u8<R>(stream: &mut R) -> io::Result<u8>
where R: AsyncReadExt + Unpin
{
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await?;
    Ok(buf[0])
}

async fn read_u16<R>(stream: &mut R) -> io::Result<u16>
where R: AsyncReadExt + Unpin
{
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    Ok(u16::from_be_bytes(buf))
}

async fn read_u32<R>(stream: &mut R) -> io::Result<u32>
where R: AsyncReadExt + Unpin
{
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    Ok(u32::from_be_bytes(buf))
}

async fn read_u32_tail<R>(head: u8, stream: &mut R) -> io::Result<u32>
where R: AsyncReadExt + Unpin
{
    let mut tail = [0u8; 3];
    stream.read_exact(&mut tail).await?;
    let bytes = [head, tail[0], tail[1], tail[2]];
    Ok(u32::from_be_bytes(bytes))
}

async fn read_null_terminated<R>(stream: &mut R) -> io::Result<Vec<u8>>
where R: AsyncBufReadExt + Unpin
{
    let mut buf = vec![];
    stream.read_until(0, &mut buf).await?;
    Ok(buf)
}

async fn read_and_drop<R>(stream: R, num: u32) -> io::Result<()>
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

fn error_other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}

#[cfg(test)]
mod test {
    mod authentication {
        use super::super::{Authentication::{self, *}, Message};
        use super::simplify;

        #[test]
        fn authentication_ok() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,0, // ok
            ];
            assert_eq!(
                msg(Ok),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn authentication_kerberos_v5() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,2, // Kerberos V5 is required
            ];
            assert_eq!(
                msg(KerberosV5),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn authentication_cleartext_password() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,3, // cleartext password is required
            ];
            assert_eq!(
                msg(CleartextPassword),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn md5_password() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,12, // len
                0,0,0,5, // MD5 password is required
                1,2,3,4, // salt
            ];
            assert_eq!(
                msg(MD5Password { salt: [1,2,3,4] }),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn scm_credential() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,6, // SCM credentials message is required
            ];
            assert_eq!(
                msg(SCMCredential),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn gss() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,7, // GSSAPI authentication is required
            ];
            assert_eq!(
                msg(GSS),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn sspi() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,8, // len
                0,0,0,9, // SSPI authentication is required
            ];
            assert_eq!(
                msg(SSPI),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        #[test]
        fn gss_continue() {
            let mut bytes: &[u8] = &[
                b'R',
                0,0,0,11, // len
                0,0,0,8, // contains GSS or SSPI data
                b'G', b'S', b'S', // data
            ];
            assert_eq!(
                msg(GSSContinue { auth_data: "GSS".as_bytes().to_vec() }),
                simplify(&mut Message::read(&mut bytes, false)),
            );
        }

        const fn msg(auth: Authentication) -> Result<Option<Message>, String> {
            Result::Ok(Some(Message::Authentication(auth)))
        }
    }

    mod startup {
        use super::super::{Message, Startup, StartupParam, Version};
        use super::simplify;

        #[test]
        fn without_params() {
            let mut bytes: &[u8] = &[
                0, 0, 0, 9, // len
                0, 3, 0, 1, // version
                0, // params
            ];
            assert_eq!(
                msg(Startup {
                    version: Version { major: 3, minor: 1 },
                    params: vec![],
                }),
                simplify(&mut Message::read(&mut bytes, true)),
            );
        }

        #[test]
        fn with_params() {
            let mut bytes = vec![
                0, 0, 0, 37, // len
                0, 3, 1, 0, // version
            ];
            bytes.extend_from_slice(b"user\0root\0database\0postgres\0\0");
            let mut bytes = &bytes[..];
            assert_eq!(
                msg(Startup {
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
                }),
                simplify(&mut Message::read(&mut bytes, true)),
            );
        }

        const fn msg(startup: Startup) -> Result<Option<Message>, String> {
            Result::Ok(Some(Message::Startup(startup)))
        }
    }

    use super::Message;
    use futures::task::Poll::*;
    use futures_test::task::noop_context;
    use std::io;
    use std::pin::Pin;

    fn simplify(
        future: &mut dyn futures::Future<Output = io::Result<Option<Message>>>
    ) -> Result<Option<Message>, String> {
        let pinned = unsafe { Pin::new_unchecked(future) };
        match pinned.poll(&mut noop_context()) {
            Ready(ready) => ready.map_err(|e| e.to_string()),
            Pending => panic!("unexpected Pending in synchronous tests"),
        }
    }
}
