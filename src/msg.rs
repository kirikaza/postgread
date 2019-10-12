use futures::io::{AsyncBufReadExt, AsyncReadExt};
use std::fmt::{self, Debug, Formatter};
use std::mem::{size_of_val};
use std::io::{self, ErrorKind::UnexpectedEof};

#[derive(Debug, PartialEq)]
pub enum BackendMessage {
    Authentication(Authentication),
    ParameterStatus(ParameterStatus),
    Unknown(Unknown),
}

#[derive(Debug, PartialEq)]
pub enum FrontendMessage {
    Startup(Startup),
    Unknown(Unknown),
}

impl BackendMessage {
    pub async fn read<R>(stream: &mut R) -> io::Result<Option<Self>>
    where R: AsyncBufReadExt + Unpin      
    {
        match accept_eof(read_u8(stream).await)? {
            Some(type_byte) => {
                let full_len = read_u32(stream).await?;
                Ok(Some(Self::read_body(stream, type_byte, full_len).await?))
            },
            None => Ok(None),
        }
    }

    async fn read_body<R>(stream: &mut R, type_byte: u8, full_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let body_len = full_len - size_of_val(&full_len) as u32;
        // TODO: protect from reading extra bytes like `stream.take(u64::from(body_len))`
        match type_byte {
            Authentication::TYPE_BYTE => {
                let body = Authentication::read(stream, body_len).await?;
                Ok(Self::Authentication(body))
            },
            ParameterStatus::TYPE_BYTE => {
                let body = ParameterStatus::read(stream).await?;
                Ok(Self::ParameterStatus(body))
            },
            _ => {
                let body = Unknown::read(stream, body_len, format!("message type {}", type_byte as char)).await?;
                Ok(Self::Unknown(body))
            },
        }
    }
}

impl FrontendMessage {
    pub async fn read<R>(stream: &mut R, is_first_msg: bool) -> io::Result<Option<Self>>
    where R: AsyncBufReadExt + Unpin
    {
        if is_first_msg {
            match accept_eof(read_u32(stream).await)? {
                Some(full_len) => {
                    Ok(Some(Self::read_body(stream, None, full_len).await?))
                },
                None => Ok(None),
            }
        } else {
            match accept_eof(read_u8(stream).await)? {
                Some(type_byte) => {
                    let full_len = read_u32(stream).await?;
                    Ok(Some(Self::read_body(stream, Some(type_byte), full_len).await?))
                },
                None => Ok(None),
            }
        }
    }

    async fn read_body<R>(stream: &mut R, type_byte: Option<u8>, full_len: u32) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let body_len = full_len - size_of_val(&full_len) as u32;
        // TODO: protect from reading extra bytes like `stream.take(u64::from(body_len))`
        match type_byte {
            None => {
                let body = Startup::read(stream).await?;
                Ok(Self::Startup(body))
            },
            Some(type_byte) => {
                let body = Unknown::read(stream, body_len, format!("message type {}", type_byte as char)).await?;
                Ok(Self::Unknown(body))
            },
        }
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
    Unknown(Unknown),
}

impl Authentication {
    const TYPE_BYTE: u8 = b'R';

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
            _ => Ok(Self::Unknown(Unknown::read(stream, left_len, format!("auth type {}", auth_type)).await?)),
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
}

#[derive(PartialEq)]
pub struct ParameterStatus {
    name: Vec<u8>,
    value: Vec<u8>,
}

impl ParameterStatus {
    const TYPE_BYTE: u8 = b'S';

    async fn read<R>(stream: &mut R) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let mut name = read_null_terminated(stream).await?;
        let mut value = read_null_terminated(stream).await?;
        name.pop();
        value.pop();
        Ok(Self { name, value })
    }
}

impl Debug for ParameterStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "ParameterStatus {{ \"{}\": \"{}\" }}",
               String::from_utf8_lossy(&self.name),
               String::from_utf8_lossy(&self.value))
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

#[derive(Debug, PartialEq)]
pub struct Unknown {
    note: String,
}

impl Unknown {
    async fn read<R>(stream: &mut R, body_len: u32, note: String) -> io::Result<Self>
    where R: AsyncBufReadExt + Unpin
    {
        read_and_drop(stream, body_len).await?;
        Ok(Self { note })
    }
}

fn accept_eof<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(data) =>
            Ok(Some(data)),
        Err(e) => match e.kind() {
            UnexpectedEof => Ok(None),
            _ => Err(e),
        },
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
        use super::super::{Authentication::{self, *}, BackendMessage};
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
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
                simplify(&mut BackendMessage::read(&mut bytes)),
            );
        }

        const fn msg(auth: Authentication) -> Result<Option<BackendMessage>, String> {
            Result::Ok(Some(BackendMessage::Authentication(auth)))
        }
    }

    mod parameter_status {
        use super::super::{BackendMessage, ParameterStatus};
        use super::simplify;

        #[test]
        fn simple() {
            let mut bytes = vec![
                b'S',
                0, 0, 0, 13, // len
            ];
            bytes.extend_from_slice(b"TimeZone\0UTC\0");
            let mut bytes = &bytes[..];
            assert_eq!(
                msg(ParameterStatus {
                    name: Vec::from(&b"TimeZone"[..]),
                    value: Vec::from(&b"UTC"[..]),
                }),
                simplify(&mut BackendMessage::read(&mut bytes)),
            );
        }

        const fn msg(startup: ParameterStatus) -> Result<Option<BackendMessage>, String> {
            Result::Ok(Some(BackendMessage::ParameterStatus(startup)))
        }
    }

    mod startup {
        use super::super::{FrontendMessage, Startup, StartupParam, Version};
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
                simplify(&mut FrontendMessage::read(&mut bytes, true)),
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
                simplify(&mut FrontendMessage::read(&mut bytes, true)),
            );
        }

        const fn msg(startup: Startup) -> Result<Option<FrontendMessage>, String> {
            Result::Ok(Some(FrontendMessage::Startup(startup)))
        }
    }

    use futures::task::Poll::*;
    use futures_test::task::noop_context;
    use std::io;
    use std::pin::Pin;

    fn simplify<M>(
        future: &mut dyn futures::Future<Output = io::Result<Option<M>>>
    ) -> Result<Option<M>, String> {
        let pinned = unsafe { Pin::new_unchecked(future) };
        match pinned.poll(&mut noop_context()) {
            Ready(ready) => ready.map_err(|e| e.to_string()),
            Pending => panic!("unexpected Pending in synchronous tests"),
        }
    }
}
