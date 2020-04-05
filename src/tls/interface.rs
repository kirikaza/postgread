use ::async_trait::async_trait;

pub trait TlsProvider<Plain>
where
    Self::Tls: Send + Unpin,
    Self::Error: std::error::Error,
{
    type Tls;
    type Error;
}

#[async_trait]
pub trait TlsClient<Plain> : TlsProvider<Plain>
where Plain: Send + Unpin {
    async fn connect(&self, plain: Plain) -> Result<Self::Tls, Self::Error>
    where Plain: 'async_trait;
}

#[async_trait]
pub trait TlsServer<Plain> : TlsProvider<Plain>
where Plain: Send + Unpin {
    async fn accept(&self, plain: Plain) -> Result<Self::Tls, Self::Error>
    where Plain: 'async_trait;
}
