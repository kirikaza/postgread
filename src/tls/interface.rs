use ::async_trait::async_trait;
use ::futures::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait TlsProvider<Plain>
where
    Plain: AsyncRead + AsyncWrite + Send + Unpin,
    Self::Tls: AsyncRead + AsyncWrite + Send + Unpin,
    Self::Error: std::error::Error,
{
    type Tls;
    type Error;

    async fn handshake(self: &Self, plain: Plain) -> Result<Self::Tls, Self::Error>
    where Plain: 'async_trait;
}
