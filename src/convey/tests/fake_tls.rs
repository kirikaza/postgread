use crate::tls::interface::*;

use ::async_trait::async_trait;
use ::std::fmt::{self, Display, Formatter};
use ::std::error;

pub struct FakeTlsClient();

pub struct FakeTlsServer();

pub struct FakeTlsStream<Plain> {
    pub plain: Plain,
}

#[derive(Debug)]
pub struct FakeTlsError();

impl<Plain> TlsProvider<Plain> for FakeTlsClient
where Plain: Send + Unpin {
    type Tls = FakeTlsStream<Plain>;
    type Error = FakeTlsError;
}

impl<Plain> TlsProvider<Plain> for FakeTlsServer
where Plain: Send + Unpin {
    type Tls = FakeTlsStream<Plain>;
    type Error = FakeTlsError;
}

#[async_trait]
impl<Plain> TlsClient<Plain> for FakeTlsClient
where Plain: Send + Unpin {
    async fn connect(&self, plain: Plain) -> Result<FakeTlsStream<Plain>, FakeTlsError>
    where Plain: 'async_trait {
        Ok(FakeTlsStream { plain })
    }
}

#[async_trait]
impl<Plain> TlsServer<Plain> for FakeTlsServer
where Plain: Send + Unpin {
    async fn accept(&self, plain: Plain) -> Result<FakeTlsStream<Plain>, FakeTlsError>
    where Plain: 'async_trait {
        Ok(FakeTlsStream { plain })
    }
}

impl Display for FakeTlsError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "FakeTlsError")
    }
}

impl error::Error for FakeTlsError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}