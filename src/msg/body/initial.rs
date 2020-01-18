use crate::msg::util::decode::*;
use ::std::fmt::{self, Debug, Formatter};

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

impl MsgDecode for Initial {
    const TYPE_BYTE_OPT: Option<u8> = None;

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        match Version::decode(bytes)? {
            Version { major: 1234, minor: 5678 } => {
                let process_id = bytes.take_u32()?;
                let secret_key = bytes.take_u32()?;
                Ok(Self::Cancel(Cancel { process_id, secret_key }))
            },
            Version { major: 1234, minor: 5679 } =>
                Ok(Self::SSL),
            version => {
                let params = StartupParam::decode_many(bytes)?;
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
    fn decode(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let major = bytes.take_u16()?;
        let minor = bytes.take_u16()?;
        Ok(Version { major, minor })
    }
}

#[derive(PartialEq)]
pub struct StartupParam {
    name: Vec<u8>,
    value: Vec<u8>,
}
impl StartupParam {
    fn decode_many(bytes: &mut BytesSource) -> DecodeResult<Vec<Self>> {
        let mut params = vec![];
        loop {
            let name = bytes.take_until_null()?;
            if name.is_empty() {
                break;
            }
            let value = bytes.take_until_null()?;
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
    use crate::msg::util::test::*;

    #[test]
    fn cancel() {
        let bytes: &[u8] = &[
            4, 210, 22, 46, // 4*256+210=1234, 22*256+46=5678, these numbers instead of version mean "cancel"
            0x1, 0x2, 0x3, 0x4,  // process ID
            0x5, 0x6, 0x7, 0x8,  // secret key
        ];
        assert_decode_ok(Initial::Cancel(Cancel { process_id: 0x01020304, secret_key: 0x05060708 }), bytes);
    }

    #[test]
    fn ssl() {
        let bytes: &[u8] = &[
            4, 210, 22, 47, // 4*256+210=1234, 22*256+47=5679, these numbers instead of version mean "SSL"
        ];
        assert_decode_ok(Initial::SSL, bytes);
    }

    #[test]
    fn startup_without_params() {
        let bytes = &[
            0, 3, 0, 1, // version
            0, // params
        ];
        assert_decode_ok(
            Initial::Startup(Startup {
                version: Version { major: 3, minor: 1 },
                params: vec![],
            }),
            bytes,
        );
    }

    #[test]
    fn startup_with_params() {
        let mut bytes = vec![
            0, 3, 1, 0, // version
        ];
        bytes.extend_from_slice(b"user\0root\0database\0postgres\0\0");
        let bytes = bytes.as_slice();
        assert_decode_ok(
            Initial::Startup(Startup {
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
            bytes,
        );
    }
}
