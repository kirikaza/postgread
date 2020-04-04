use ::async_trait::async_trait;
use ::futures::io::{AsyncRead, AsyncWrite};

pub trait TlsProvider<Plain>
where
    Plain: AsyncRead + AsyncWrite + Send + Unpin,
    Self::Tls: AsyncRead + AsyncWrite + Send + Unpin,
    Self::Error: std::error::Error,
{
    type Tls;
    type Error;
}

#[async_trait]
pub trait TlsClient<Plain> : TlsProvider<Plain>
where Plain: AsyncRead + AsyncWrite + Send + Unpin {
    async fn connect(self: &Self, plain: Plain) -> Result<Self::Tls, Self::Error>
    where Plain: 'async_trait;
}

#[async_trait]
pub trait TlsServer<Plain> : TlsProvider<Plain>
where Plain: AsyncRead + AsyncWrite + Send + Unpin {
    async fn accept(self: &Self, plain: Plain) -> Result<Self::Tls, Self::Error>
    where Plain: 'async_trait;
}
