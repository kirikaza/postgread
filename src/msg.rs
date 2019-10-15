pub mod body {
    pub mod authentication;
    pub mod parameter_status;
    pub mod startup;
    pub mod unknown;
}
mod io;

#[cfg(test)]
mod test_util;

use body::authentication::Authentication;
use body::parameter_status::ParameterStatus;
use body::startup::Startup;
use body::unknown::Unknown;
use io::*;

use ::futures::io::{AsyncBufReadExt, Result as IoResult};
use ::std::fmt::Debug;
use ::std::mem::{size_of_val};

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
    pub async fn read<R>(stream: &mut R) -> IoResult<Option<Self>>
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

    async fn read_body<R>(stream: &mut R, type_byte: u8, full_len: u32) -> IoResult<Self>
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
    pub async fn read<R>(stream: &mut R, is_first_msg: bool) -> IoResult<Option<Self>>
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

    async fn read_body<R>(stream: &mut R, type_byte: Option<u8>, full_len: u32) -> IoResult<Self>
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
