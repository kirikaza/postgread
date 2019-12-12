use crate::msg::util::decode::{*, Problem::*};
use crate::msg::util::read::*;
use ::futures::io::AsyncReadExt;
use ::std::io::Result as IoResult;

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
}

impl Authentication {
    pub const TYPE_BYTE: u8 = b'R';

    pub async fn read<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncReadExt + Unpin {
        read_msg_with_len(stream, Self::decode_body).await
    }

    pub fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let auth_type = bytes.take_u32()?;
        match auth_type {
            0 => Ok(Self::Ok),
            2 => Ok(Self::KerberosV5),
            3 => Ok(Self::CleartextPassword),
            5 => Self::decode_md5_password(bytes),
            6 => Ok(Self::SCMCredential),
            7 => Ok(Self::GSS),
            8 => Self::decode_gss_continue(bytes),
            9 => Ok(Self::SSPI),
            x => Err(Unknown(format!("has unknown sub-type {}", x))),
        }
    }

    fn decode_md5_password(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let mut salt = [0u8; 4];
        bytes.take_slice(&mut salt)?;
        Ok(Self::MD5Password { salt })
    }

    fn decode_gss_continue(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let auth_data = bytes.take_vec(bytes.left())?;
        Ok(Self::GSSContinue { auth_data })
    }
}

#[cfg(test)]
mod tests {
    use super::Authentication::{self, *};
    use crate::msg::BackendMessage;
    use crate::msg::util::test::*;

    #[test]
    fn ok() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,0, // ok
        ];
        assert_eq!(
            ok_some(Ok),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn kerberos_v5() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,2, // Kerberos V5 is required
        ];
        assert_eq!(
            ok_some(KerberosV5),
            force_read_backend(&mut bytes),
        );
    }

    #[test]
    fn cleartext_password() {
        let mut bytes: &[u8] = &[
            b'R',
            0,0,0,8, // len
            0,0,0,3, // cleartext password is required
        ];
        assert_eq!(
            ok_some(CleartextPassword),
            force_read_backend(&mut bytes),
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
            ok_some(MD5Password { salt: [1,2,3,4] }),
            force_read_backend(&mut bytes),
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
            ok_some(SCMCredential),
            force_read_backend(&mut bytes),
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
            ok_some(GSS),
            force_read_backend(&mut bytes),
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
            ok_some(SSPI),
            force_read_backend(&mut bytes),
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
            ok_some(GSSContinue { auth_data: "GSS".as_bytes().to_vec() }),
            force_read_backend(&mut bytes),
        );
    }

    fn ok_some(body: Authentication) -> Result<Option<BackendMessage>, String> {
        ok_some_msg(body, BackendMessage::Authentication)
    }
}
