use crate::msg::util::decode::{*, Problem::*};

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
}

impl MsgDecode for Authentication {
    const TYPE_BYTE_OPT: Option<u8> = Some(Self::TYPE_BYTE);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let auth_type = bytes.take_u32()?;
        match auth_type {
            0 => Ok(Self::Ok),
            2 => Ok(Self::KerberosV5),
            3 => Ok(Self::CleartextPassword),
            5 => decode_md5_password(bytes),
            6 => Ok(Self::SCMCredential),
            7 => Ok(Self::GSS),
            8 => decode_gss_continue(bytes),
            9 => Ok(Self::SSPI),
            x => Err(Unknown(format!("has unknown sub-type {}", x))),
        }
    }
}

fn decode_md5_password(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let mut salt = [0u8; 4];
    bytes.take_slice(&mut salt)?;
    Ok(Authentication::MD5Password { salt })
}

fn decode_gss_continue(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let auth_data = bytes.take_vec(bytes.left())?;
    Ok(Authentication::GSSContinue { auth_data })
}

#[cfg(test)]
mod tests {
    use super::Authentication::*;
    use crate::msg::util::test::*;

    #[test]
    fn ok() {
        let bytes: &[u8] = &[
            0,0,0,0, // ok
        ];
        assert_decode_ok(Ok, bytes);
    }

    #[test]
    fn kerberos_v5() {
        let bytes: &[u8] = &[
            0,0,0,2, // Kerberos V5 is required
        ];
        assert_decode_ok(KerberosV5, bytes);
    }

    #[test]
    fn cleartext_password() {
        let bytes: &[u8] = &[
            0,0,0,3, // cleartext password is required
        ];
        assert_decode_ok(CleartextPassword, bytes);
    }

    #[test]
    fn md5_password() {
        let bytes: &[u8] = &[
            0,0,0,5, // MD5 password is required
            1,2,3,4, // salt
        ];
        assert_decode_ok(MD5Password { salt: [1,2,3,4] }, bytes);
    }

    #[test]
    fn scm_credential() {
        let bytes: &[u8] = &[
            0,0,0,6, // SCM credentials message is required
        ];
        assert_decode_ok(SCMCredential, bytes);
    }

    #[test]
    fn gss() {
        let bytes: &[u8] = &[
            0,0,0,7, // GSSAPI authentication is required
        ];
        assert_decode_ok(GSS, bytes);
    }

    #[test]
    fn sspi() {
        let bytes: &[u8] = &[
            0,0,0,9, // SSPI authentication is required
        ];
        assert_decode_ok(SSPI, bytes);
    }

    #[test]
    fn gss_continue() {
        let bytes: &[u8] = &[
            0,0,0,8, // contains GSS or SSPI data
            b'G', b'S', b'S', // data
        ];
        assert_decode_ok(GSSContinue { auth_data: Vec::from("GSS") }, bytes);
    }
}
