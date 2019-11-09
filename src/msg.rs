pub mod body {
    pub mod authentication;
    pub mod backend_key_data;
    pub mod command_complete;
    pub mod data_row;
    pub mod parameter_status;
    pub mod query;
    pub mod ready_for_query;
    pub mod row_description;
    pub mod startup;
    pub mod terminate;
    pub mod unknown;
}
mod io;

#[cfg(test)]
mod test_util;

use body::authentication::Authentication;
use body::backend_key_data::BackendKeyData;
use body::command_complete::CommandComplete;
use body::data_row::DataRow;
use body::parameter_status::ParameterStatus;
use body::query::Query;
use body::ready_for_query::ReadyForQuery;
use body::row_description::RowDescription;
use body::startup::Startup;
use body::terminate::Terminate;
use body::unknown::Unknown;
use io::*;

use ::futures::io::{AsyncBufReadExt, Result as IoResult};
use ::std::fmt::Debug;
use ::std::mem::{size_of_val};

#[derive(Debug, PartialEq)]
pub enum BackendMessage {
    Authentication(Authentication),
    BackendKeyData(BackendKeyData),
    CommandComplete(CommandComplete),
    DataRow(DataRow),
    ParameterStatus(ParameterStatus),
    ReadyForQuery(ReadyForQuery),
    RowDescription(RowDescription),
    Unknown(Unknown),
}

#[derive(Debug, PartialEq)]
pub enum FrontendMessage {
    Query(Query),
    Startup(Startup),
    Terminate(Terminate),
    Unknown(Unknown),
}

macro_rules! read_body_and_wrap {
    (
        $body_type_name:ident,
        $($arg:expr),*
    ) => {
        {
            let body = $body_type_name::read($($arg,)*).await?;
            Ok(Self::$body_type_name(body))
        }
    };
}

macro_rules! read_body_by_type {
    (
        ($stream:ident, $type_byte:ident, $full_len:ident),
        [$($body_type_name: ident),*]
    ) => {
        {
            let body_len = $full_len - size_of_val(&$full_len) as u32;
            // TODO: protect from reading extra bytes like `stream.take(u64::from(body_len))`
            match $type_byte {
                $(
                    $body_type_name::TYPE_BYTE =>
                        read_body_and_wrap!($body_type_name, $stream, body_len),
                )*
                _ => {
                    let type_sym = $type_byte.map(|byte| byte as char);
                    let msg = format!("message type {:?}", type_sym);
                    read_body_and_wrap!(Unknown, $stream, body_len, msg)
                },

            }
        }
    };
}

impl BackendMessage {
    pub async fn read<R>(stream: &mut R) -> IoResult<Option<Self>>
    where R: AsyncBufReadExt + Unpin
    {
        Ok(
            match accept_eof(read_u8(stream).await)? {
                Some(type_byte) => {
                    let full_len = read_u32(stream).await?;
                    Some(Self::read_body(stream, Some(type_byte), full_len).await?)
                }
                None => None,
            }
        )
    }

    async fn read_body<R>(stream: &mut R, type_byte: Option<u8>, full_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        read_body_by_type! {
            (stream, type_byte, full_len),
            [
                Authentication,
                BackendKeyData,
                CommandComplete,
                DataRow,
                ParameterStatus,
                ReadyForQuery,
                RowDescription
            ]
        }
    }
}

impl FrontendMessage {
    pub async fn read<R>(stream: &mut R, is_first_msg: bool) -> IoResult<Option<Self>>
    where R: AsyncBufReadExt + Unpin
    {
        Ok(
            if is_first_msg {
                match accept_eof(read_u32(stream).await)? {
                    Some(full_len) => Some(Self::read_body(stream, None, full_len).await?),
                    None => None,
                }
            } else {
                match accept_eof(read_u8(stream).await)? {
                    Some(type_byte) => {
                        let full_len = read_u32(stream).await?;
                        Some(Self::read_body(stream, Some(type_byte), full_len).await?)
                    }
                    None => None,
                }
            }
        )
    }

    async fn read_body<R>(stream: &mut R, type_byte: Option<u8>, full_len: u32) -> IoResult<Self>
    where R: AsyncBufReadExt + Unpin
    {
        read_body_by_type! {
            (stream, type_byte, full_len),
            [Query, Startup, Terminate]
        }
    }
}
