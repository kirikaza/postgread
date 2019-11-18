use crate::msg::util::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub enum Initial {
    Cancel(Cancel),
    SSL,
    Startup(Startup),
}

#[derive(Debug, PartialEq)]
pub struct Cancel {
    process_id: u32,
    secret_key: u32,
}

#[derive(Debug, PartialEq)]
pub struct Startup {
    version: Version,
    params: Vec<StartupParam>,
}

impl Initial {
    pub const TYPE_BYTE: Option<u8> = None;

    pub async fn read<R>(stream: &mut R, _body_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        match Version::read(stream).await? {
            Version { major: 1234, minor: 5678 } => {
                let process_id = read_u32(stream).await?;
                let secret_key = read_u32(stream).await?;
                Ok(Self::Cancel(Cancel { process_id, secret_key }))
            },
            Version { major: 1234, minor: 5679 } =>
                Ok(Self::SSL),
            version => {
                let params = StartupParam::read_many(stream).await?;
                Ok(Self::Startup(Startup { version, params }))
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
    async fn read<R>(stream: &mut R) -> IoResult<Self>
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
    async fn read_many<R>(stream: &mut R) -> IoResult<Vec<Self>>
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
        f.debug_struct("StartupParam")
            .field(
                &String::from_utf8_lossy(&self.name),
                &String::from_utf8_lossy(&self.value))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Cancel, Initial, Startup, StartupParam, Version};
    use crate::msg::FrontendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn cancel() {
        let mut bytes: &[u8] = &[
            0, 0, 0, 16, // len
            4, 210, 22, 46, // 4*256+210=1234, 22*256+46=5678, these numbers instead of version mean "cancel"
            0x1, 0x2, 0x3, 0x4,  // process ID
            0x5, 0x6, 0x7, 0x8,  // secret key
        ];
        assert_eq!(
            ok_some(Initial::Cancel(Cancel { process_id: 0x01020304, secret_key: 0x05060708 })),
            force_read_frontend(&mut bytes, true),
        );
    }

    #[test]
    fn ssl() {
        let mut bytes: &[u8] = &[
            0, 0, 0, 8, // len
            4, 210, 22, 47, // 4*256+210=1234, 22*256+47=5679, these numbers instead of version mean "SSL"
        ];
        assert_eq!(
            ok_some(Initial::SSL),
            force_read_frontend(&mut bytes, true),
        );
    }

    #[test]
    fn startup_without_params() {
        let mut bytes: &[u8] = &[
            0, 0, 0, 9, // len
            0, 3, 0, 1, // version
            0, // params
        ];
        assert_eq!(
            ok_some(Initial::Startup(Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            })),
            force_read_frontend(&mut bytes, true),
        );
    }

    #[test]
    fn startup_with_params() {
        let mut bytes = vec![
            0, 0, 0, 37, // len
            0, 3, 1, 0, // version
        ];
        bytes.extend_from_slice(b"user\0root\0database\0postgres\0\0");
        let mut bytes = &bytes[..];
        assert_eq!(
            ok_some(Initial::Startup(Startup {
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
            force_read_frontend(&mut bytes, true),
        );
    }

    fn ok_some(body: Initial) -> Result<Option<FrontendMessage>, String> {
        ok_some_msg(body, FrontendMessage::Initial)
    }
}
