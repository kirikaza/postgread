use async_std::io::{Read as AsyncStdRead, Write as AsyncStdWrite};
use futures::io::{AsyncRead as FuturesAsyncRead, AsyncWrite as FuturesAsyncWrite};
use futures::task::{Context, Poll};
use std::io;
use std::pin::Pin;

pub fn compat<T>(x: T) -> Compat<T> {
    Compat(x)
}

pub struct Compat<T>(T);

impl<T> Compat<T> {
    fn pin_inner(self: Pin<&mut Self>) -> Pin<&mut T> {
        unsafe {
            let self_ref = self.get_unchecked_mut();
            Pin::new_unchecked(&mut self_ref.0)
        }
    }
}

impl<T: AsyncStdRead> FuturesAsyncRead for Compat<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        T::poll_read(self.pin_inner(), cx, buf)
    }
}

impl<T: AsyncStdWrite> FuturesAsyncWrite for Compat<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        T::poll_write(self.pin_inner(), cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        T::poll_flush(self.pin_inner(), cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        T::poll_close(self.pin_inner(), cx)
    }
}
