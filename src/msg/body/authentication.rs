use crate::msg::type_byte::TypeByte;
use crate::msg::util::decode::{*, Problem::*};

#[derive(Debug, PartialEq)]
pub enum Authentication {
    CleartextPassword,
    Gss,
    GssContinue { auth_data: Vec<u8> },
    KerberosV5,
    Md5Password { salt: [u8; 4] },
    Ok,
    Sasl { auth_mechanisms: Vec<Vec<u8>> },
    SaslContinue { challenge_data: Vec<u8> },
    SaslFinal { additional_data: Vec<u8> },
    ScmCredential,
    Sspi,
}

impl Authentication {
    pub const TYPE_BYTE: u8 = b'R';
}

impl MsgDecode for Authentication {
    const TYPE_BYTE_OPT: Option<TypeByte> = Some(TypeByte::Authentication);

    fn decode_body(bytes: &mut BytesSource) -> DecodeResult<Self> {
        let auth_type = bytes.take_u32()?;
        match auth_type {
            0 => Ok(Self::Ok),
            2 => Ok(Self::KerberosV5),
            3 => Ok(Self::CleartextPassword),
            5 => decode_md5_password(bytes),
            6 => Ok(Self::ScmCredential),
            7 => Ok(Self::Gss),
            8 => decode_gss_continue(bytes),
            9 => Ok(Self::Sspi),
            10 => decode_sasl(bytes),
            11 => decode_sasl_continue(bytes),
            12 => decode_sasl_final(bytes),
            x => Err(Unknown(format!("has unknown sub-type {}", x))),
        }
    }
}

fn decode_gss_continue(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let auth_data = bytes.take_vec(bytes.left())?;
    Ok(Authentication::GssContinue { auth_data })
}

fn decode_md5_password(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let mut salt = [0u8; 4];
    bytes.take_slice(&mut salt)?;
    Ok(Authentication::Md5Password { salt })
}

fn decode_sasl(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let mut auth_mechanisms = vec![];
    loop {
        let mech = bytes.take_until_null()?;
        if mech.is_empty() {
            break;
        }
        auth_mechanisms.push(mech);
    }
    Ok(Authentication::Sasl { auth_mechanisms })
}

fn decode_sasl_continue(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let data = bytes.take_vec(bytes.left())?;
    Ok(Authentication::SaslContinue { challenge_data: data })
}

fn decode_sasl_final(bytes: &mut BytesSource) -> DecodeResult<Authentication> {
    let additional_data = bytes.take_vec(bytes.left())?;
    Ok(Authentication::SaslFinal { additional_data })
}

#[cfg(test)]
mod tests {
    use super::Authentication::*;
    use crate::msg::util::test::*;

    #[test]
    fn cleartext_password() {
        let bytes: &[u8] = &[
            0,0,0,3, // cleartext password is required
        ];
        assert_decode_ok(CleartextPassword, bytes);
    }

    #[test]
    fn gss() {
        let bytes: &[u8] = &[
            0,0,0,7, // GSSAPI authentication is required
        ];
        assert_decode_ok(Gss, bytes);
    }

    #[test]
    fn gss_continue() {
        let bytes: &[u8] = &[
            0,0,0,8, // contains GSS or SSPI data
            b'G', b'S', b'S', // data
        ];
        assert_decode_ok(GssContinue { auth_data: Vec::from("GSS") }, bytes);
    }

    #[test]
    fn kerberos_v5() {
        let bytes: &[u8] = &[
            0,0,0,2, // Kerberos V5 is required
        ];
        assert_decode_ok(KerberosV5, bytes);
    }

    #[test]
    fn md5_password() {
        let bytes: &[u8] = &[
            0,0,0,5, // MD5 password is required
            1,2,3,4, // salt
        ];
        assert_decode_ok(Md5Password { salt: [1,2,3,4] }, bytes);
    }

    #[test]
    fn ok() {
        let bytes: &[u8] = &[
            0,0,0,0, // ok
        ];
        assert_decode_ok(Ok, bytes);
    }

    #[test]
    fn sasl() {
        let bytes: &[u8] = &[
            0,0,0,10,  // contains list of SASL authentication mechanisms
            b'O', b'T', b'P', 0,  // first
            b'S', b'K', b'E', b'Y', 0,  // second
            0,  // list terminator
        ];
        assert_decode_ok(Sasl {
            auth_mechanisms: vec![
                Vec::from("OTP"),
                Vec::from("SKEY"),
            ]
        }, bytes);
    }

    #[test]
    fn sasl_continue() {
        let bytes: &[u8] = &[
            0,0,0,11,  // contains a SASL challenge data
            b'S', b'A', b'S', b'L',  // data
            b' ', b'C', b'O', b'N',  // data
            b'T'
        ];
        assert_decode_ok(SaslContinue { challenge_data: Vec::from("SASL CONT") }, bytes);
    }

    #[test]
    fn sasl_final() {
        let bytes: &[u8] = &[
            0,0,0,12,  // SASL authentication has completed
            b'S', b'A', b'S', b'L',  // data
            b' ', b'F', b'I', b'N',  // data
        ];
        assert_decode_ok(SaslFinal { additional_data: Vec::from("SASL FIN") }, bytes);
    }

    #[test]
    fn scm_credential() {
        let bytes: &[u8] = &[
            0,0,0,6, // SCM credentials message is required
        ];
        assert_decode_ok(ScmCredential, bytes);
    }

    #[test]
    fn sspi() {
        let bytes: &[u8] = &[
            0,0,0,9, // SSPI authentication is required
        ];
        assert_decode_ok(Sspi, bytes);
    }
}
