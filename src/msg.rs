#![allow(clippy::large_enum_variant)]

pub mod body;
mod util;

use body::*;
use util::io::*;

use ::futures::io::{AsyncBufReadExt, Result as IoResult};
use ::std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum BackendMessage {
    Authentication(Authentication),
    BackendKeyData(BackendKeyData),
    CommandComplete(CommandComplete),
    DataRow(DataRow),
    ErrorResponse(ErrorResponse),
    ParameterStatus(ParameterStatus),
    ReadyForQuery(ReadyForQuery),
    RowDescription(RowDescription),
    Unknown(Unknown),
}

#[derive(Debug, PartialEq)]
pub enum FrontendMessage {
    Query(Query),
    Initial(Initial),
    Terminate(Terminate),
    Unknown(Unknown),
}

impl BackendMessage {
    pub async fn read<R>(stream: &mut R) -> IoResult<Option<Self>>
    where R: AsyncBufReadExt + Unpin {
        match accept_eof(read_u8(stream).await)? {
            Some(type_byte) => {
                let msg = match type_byte {
                    Authentication::TYPE_BYTE => Self::Authentication(Authentication::read(stream).await?),
                    BackendKeyData::TYPE_BYTE => Self::BackendKeyData(BackendKeyData::read(stream).await?),
                    CommandComplete::TYPE_BYTE => Self::CommandComplete(CommandComplete::read(stream).await?),
                    DataRow::TYPE_BYTE => Self::DataRow(DataRow::read(stream).await?),
                    ErrorResponse::TYPE_BYTE => Self::ErrorResponse(ErrorResponse::read(stream).await?),
                    ParameterStatus::TYPE_BYTE => Self::ParameterStatus(ParameterStatus::read(stream).await?),
                    ReadyForQuery::TYPE_BYTE => Self::ReadyForQuery(ReadyForQuery::read(stream).await?),
                    RowDescription::TYPE_BYTE => Self::RowDescription(RowDescription::read(stream).await?),
                    _ => Self::Unknown(Unknown::read(stream, type_byte).await?),
                };
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}

impl FrontendMessage {
    pub async fn read<R>(stream: &mut R, is_first_msg: bool) -> IoResult<Option<Self>>
    where R: AsyncBufReadExt + Unpin {
        if is_first_msg {
            let msg = Initial::read(stream).await?;
            Ok(Option::map(msg, Self::Initial))
        } else {
            match accept_eof(read_u8(stream).await)? {
                Some(type_byte) => {
                    let msg = match type_byte {
                        Query::TYPE_BYTE => Self::Query(Query::read(stream).await?),
                        Terminate::TYPE_BYTE => Self::Terminate(Terminate::read(stream).await?),
                        _ => Self::Unknown(Unknown::read(stream, type_byte).await?),
                    };
                    Ok(Some(msg))
                }
                None => Ok(None),
            }
        }
    }
}
