use futures::io::{AsyncBufReadExt, AsyncReadExt};
use std::fmt::{self, Debug, Formatter};
use std::mem::{size_of_val};
use std::io::{self, ErrorKind::UnexpectedEof};

#[derive(Debug, PartialEq)]
pub enum Message {
    AuthenticationOk,
    AuthenticationKerberosV5,
    AuthenticationCleartextPassword,
    AuthenticationMD5Password { salt: [u8; 4] },
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
            None => Self::read_startup(stream).await,
            Some(b'R') => Self::read_auth(stream, body_len).await,
            Some(type_byte) => Self::read_unknown(stream, body_len, format!("unknown message type {}", type_byte as char)).await,
        }
    }

    async fn read_startup<R>(stream: &mut R) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let version = Version::read(stream).await?;
        let params = StartupParam::read_many(stream).await?;
        Ok(Message::Startup { version, params })
    }

    async fn read_auth<R>(stream: &mut R, body_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let auth_type = read_u32(stream).await?;
        let left_len = body_len - size_of_val(&auth_type) as u32;
        match auth_type {
            0 => Ok(Message::AuthenticationOk),
            2 => Ok(Message::AuthenticationKerberosV5),
            3 => Ok(Message::AuthenticationCleartextPassword),
            5 => Self::read_auth_md5_password(stream).await,
            6 => Ok(Message::AuthenticationSCMCredential),
            7 => Ok(Message::AuthenticationGSS),
            8 => Self::read_auth_gss_continue(stream, left_len).await,
            9 => Ok(Message::AuthenticationSSPI),
            _ => Self::read_unknown(stream, left_len, format!("unknown authentication sub-type {}", auth_type)).await,
        }
    }

    async fn read_auth_md5_password<R>(stream: &mut R) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let mut salt = [0u8; 4];
        stream.read_exact(&mut salt).await?;
        Ok(Message::AuthenticationMD5Password { salt })
    }

    async fn read_auth_gss_continue<R>(stream: &mut R, left_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        let mut auth_data = Vec::with_capacity(left_len as usize);
        stream.read_to_end(&mut auth_data).await?;
        Ok(Message::AuthenticationGSSContinue { auth_data })
    }

    async fn read_unknown<R>(stream: &mut R, left_len: u32, note: String) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin      
    {
        read_and_drop(stream, left_len).await?;
        Ok(Message::Unknown { note })
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
    use super::*;
    use futures::task::Poll::*;
    use futures_test::task::noop_context;
    use std::pin::Pin;

    #[test]
    fn authentication_ok() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,0, // ok
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationOk)),
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
            Ok(Some(Message::AuthenticationKerberosV5)),
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
            Ok(Some(Message::AuthenticationCleartextPassword)),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn authentication_md5_password() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,12, // len
            0,0,0,5, // MD5 password is required
            1,2,3,4, // salt
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationMD5Password { salt: [1,2,3,4] })),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn authentication_scm_credential() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,6, // SCM credentials message is required
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationSCMCredential)),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn authentication_gss() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,7, // GSSAPI authentication is required
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationGSS)),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn authentication_sspi() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,9, // SSPI authentication is required
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationSSPI)),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn authentication_gss_continue() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,11, // len
            0,0,0,8, // contains GSS or SSPI data
            b'G', b'S', b'S', // data
        ];
        assert_eq!(
            Ok(Some(Message::AuthenticationGSSContinue { auth_data: "GSS".as_bytes().to_vec() })),
            simplify(&mut Message::read(&mut bytes, false)),
        );
    }
    
    #[test]
    fn startup_without_params() {
        let mut bytes: &[u8] = &[
            0,0,0,9, // len
            0,3,0,1, // version
            0, // params
        ];
        assert_eq!(
            Ok(Some(Message::Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            })),
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
            Ok(Some(Message::Startup {
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
            })),
           simplify(&mut Message::read(&mut bytes, true)),
        );        
    }

    fn simplify(
        future: &mut futures::Future<Output = io::Result<Option<Message>>>
    ) -> Result<Option<Message>, String> {
        let pinned = unsafe { Pin::new_unchecked(future) };
        match pinned.poll(&mut noop_context()) {
            Ready(ready) => ready.map_err(|e| e.to_string()),
            Pending => panic!("unexpected Pending in synchronous tests"),
        }
    }
}
