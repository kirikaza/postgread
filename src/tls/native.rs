use crate::tls::interface::*;

use ::async_native_tls::{TlsAcceptor, TlsStream};
use ::async_trait::async_trait;
use ::futures::io::{AsyncRead, AsyncWrite};

pub struct NativeTlsProvider<'a>(pub &'a TlsAcceptor);

#[async_trait]
impl<'a, Plain> TlsProvider<Plain> for NativeTlsProvider<'a>
where Plain: AsyncRead + AsyncWrite + Send + Unpin {
    type Tls = TlsStream<Plain>;
    type Error = ::native_tls::Error;

    async fn handshake(self: &Self, plain: Plain) -> Result<TlsStream<Plain>, ::native_tls::Error>
    where Plain: 'async_trait {
        self.0.accept(plain).await
    }
}
