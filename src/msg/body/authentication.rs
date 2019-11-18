use crate::msg::util::io::*;
use super::unknown::Unknown;
use ::futures::io::{AsyncBufReadExt, AsyncReadExt};
use ::std::io::Result as IoResult;
use ::std::mem::size_of_val;

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
    pub const TYPE_BYTE: Option<u8> = Some(b'R');

    pub async fn read<R>(stream: &mut R, body_len: u32) -> IoResult<Self>
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
            _ => {
                let msg = format!("auth type {}", auth_type);
                Ok(Self::Unknown(Unknown::read(stream, left_len, msg).await?))
            },
        }
    }

    async fn read_md5_password<R>(stream: &mut R) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let mut salt = [0u8; 4];
        stream.read_exact(&mut salt).await?;
        Ok(Self::MD5Password { salt })
    }

    async fn read_gss_continue<R>(stream: &mut R, left_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        let mut auth_data = Vec::with_capacity(left_len as usize);
        stream.read_to_end(&mut auth_data).await?;
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
