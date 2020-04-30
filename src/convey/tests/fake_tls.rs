use crate::tls::interface::*;

use ::async_trait::async_trait;
use ::std::fmt::{self, Display, Formatter};
use ::std::error;

pub struct FakeTlsClient();

pub struct FakeTlsServer();

#[derive(Debug)]
pub struct FakeTlsError();

impl<Plain> TlsProvider<Plain> for FakeTlsClient
where Plain: Send + Unpin {
    type Tls = Plain;
    type Error = FakeTlsError;
}

impl<Plain> TlsProvider<Plain> for FakeTlsServer
where Plain: Send + Unpin {
    type Tls = Plain;
    type Error = FakeTlsError;
}

#[async_trait]
impl<Plain> TlsClient<Plain> for FakeTlsClient
where Plain: Send + Unpin {
    async fn connect(&self, _plain: Plain) -> Result<Plain, FakeTlsError>
    where Plain: 'async_trait {
        unimplemented!()
    }
}

#[async_trait]
impl<Plain> TlsServer<Plain> for FakeTlsServer
where Plain: Send + Unpin {
    async fn accept(&self, _plain: Plain) -> Result<Plain, FakeTlsError>
    where Plain: 'async_trait {
        unimplemented!()
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