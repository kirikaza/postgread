use crate::tls::interface::*;

use ::async_native_tls::{TlsAcceptor, TlsConnector, TlsStream};
use ::async_trait::async_trait;
use ::futures::io::{AsyncRead, AsyncWrite};

pub struct NativeTlsClient<'a> {
    pub connector: &'a TlsConnector,
    pub hostname: &'a str,
}

pub struct NativeTlsServer<'a>(pub &'a TlsAcceptor);

impl<'a, Plain> TlsProvider<Plain> for NativeTlsClient<'a>
where Plain: Send + Unpin {
    type Tls = TlsStream<Plain>;
    type Error = ::native_tls::Error;
}

impl<'a, Plain> TlsProvider<Plain> for NativeTlsServer<'a>
where Plain: Send + Unpin {
    type Tls = TlsStream<Plain>;
    type Error = ::native_tls::Error;
}

#[async_trait]
impl<'a, Plain> TlsClient<Plain> for NativeTlsClient<'a>
where Plain: AsyncRead + AsyncWrite + Send + Unpin {
    async fn connect(&self, plain: Plain) -> Result<TlsStream<Plain>, ::native_tls::Error>
    where Plain: 'async_trait {
        self.connector.connect(self.hostname, plain).await
    }
}

#[async_trait]
impl<'a, Plain> TlsServer<Plain> for NativeTlsServer<'a>
where Plain: AsyncRead + AsyncWrite + Send + Unpin {
    async fn accept(&self, plain: Plain) -> Result<TlsStream<Plain>, ::native_tls::Error>
    where Plain: 'async_trait {
        self.0.accept(plain).await
    }
}
