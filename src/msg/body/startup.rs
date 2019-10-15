use super::super::io::*;
use ::futures::io::AsyncBufReadExt;
use ::std::fmt::{self, Debug, Formatter};
use ::std::io::Result as IoResult;

#[derive(Debug, PartialEq)]
pub struct Startup {
    version: Version,
    params: Vec<StartupParam>,
}

impl Startup {
    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
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
        write!(f, "StartupParam {{ \"{}\": \"{}\" }}",
               String::from_utf8_lossy(&self.name),
               String::from_utf8_lossy(&self.value))
    }
}

#[cfg(test)]
mod tests {
    use super::{Startup, StartupParam, Version};
    use crate::msg::FrontendMessage;
    use crate::msg::test_util::*;

    #[test]
    fn without_params() {
        let mut bytes: &[u8] = &[
            0, 0, 0, 9, // len
            0, 3, 0, 1, // version
            0, // params
        ];
        assert_eq!(
            ok_some(Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            }),
            force_read_frontend(&mut bytes, true),
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
            ok_some(Startup {
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
            force_read_frontend(&mut bytes, true),
        );
    }

    fn ok_some(body: Startup) -> Result<Option<FrontendMessage>, String> {
        ok_some_msg(body, FrontendMessage::Startup)
    }
}
