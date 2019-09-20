use futures::io::{AsyncRead, AsyncWrite};
use futures::task::{Context, Poll, Poll::*};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read, Write};
use std::pin::Pin;

pub struct DupReader<R, W> {
    state: State,
    from: R,
    to: W,
}

impl<R, W> DupReader<R, W> {
    pub fn new(from: R, to: W) -> DupReader<R, W> {
        DupReader { from, to, state: State::ToRead }
    }
}

impl<R: Read, W: Write> Read for DupReader<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let has_read = self.from.read(buf).map_err(|e| io::Error::new(e.kind(), DupErr::Read(e)))?;
        let has_written = self.to.write(&buf[..has_read]).map_err(|e| io::Error::new(e.kind(), DupErr::Write(e)))?;
        if has_read == has_written {
            Ok(has_written)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                DupErr::Mismatch { has_read, has_written },
            ))
        }
    }
}

impl<R: AsyncRead, W: AsyncWrite> AsyncRead for DupReader<R, W> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        // found such trick in futures_test::future::InterleavePending::subject,
        // so will assume this is safe actually
        let (state, from, to) = unsafe {
            let self_ref = self.get_unchecked_mut();
            (
                &mut self_ref.state,
                Pin::new_unchecked(&mut self_ref.from),
                Pin::new_unchecked(&mut self_ref.to),
            )
        };
        match *state {
            State::ToRead => poll_read_write(state, from, to, cx, buf),
            State::ToWrite(has_read) => poll_write(state, to, cx, buf, has_read),
            State::Failed => Ready(Err(io::Error::new(io::ErrorKind::Other, DupErr::Earlier))),
        }
    }
}

fn poll_read_write<R: AsyncRead, W: AsyncWrite>(
    state: &mut State,
    reader: Pin<&mut R>,
    writer: Pin<&mut W>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
) -> Poll<io::Result<usize>> {
    match ready!(reader.poll_read(cx, buf)) {
        Ok(has_read) => {
            *state = State::ToWrite(has_read);
            poll_write(state, writer, cx, buf, has_read)
        },
        Err(e) => {
            *state = State::Failed;
            Ready(Err(io::Error::new(e.kind(), DupErr::Read(e))))
        },
    }
}

fn poll_write<W: AsyncWrite>(state: &mut State, writer: Pin<&mut W>, cx: &mut Context<'_>, buf: &[u8], has_read: usize) -> Poll<io::Result<usize>> {
    let result = ready!(writer.poll_write(cx, &buf[..has_read]));
    match result {
        Ok(has_written) =>
            if has_read == has_written {
                *state = State::ToRead;
                Ready(Ok(has_written))
            } else {
                *state = State::Failed;
                Ready(Err(io::Error::new(io::ErrorKind::Other, DupErr::Mismatch { has_read, has_written })))
            },
        Err(e) => {
            *state = State::Failed;
            Ready(Err(io::Error::new(e.kind(), DupErr::Write(e))))
        },
    }
}

enum State {
    ToRead,
    ToWrite(usize),
    Failed,
}

#[derive(Debug)]
pub enum DupErr {
    Read(io::Error),
    Write(io::Error),
    Mismatch { has_read: usize, has_written: usize },
    Earlier,
}
impl Error for DupErr {
    fn description(&self) -> &str {
        match *self {
            DupErr::Read(ref e) => e.description(),
            DupErr::Write(ref e) => e.description(),
            DupErr::Mismatch {..} => "read/written mismatch",
            DupErr::Earlier => "failed earlier",
        }
    }
    fn cause(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            DupErr::Read(ref e) => e.source(),
            DupErr::Write(ref e) => e.source(),
            DupErr::Mismatch {..} => None,
            DupErr::Earlier => None,
        }
    }
}
impl Display for DupErr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            DupErr::Read(ref e) => write!(f, "read error: {}", e),
            DupErr::Write(ref e) => write!(f, "write error: {}", e),
            DupErr::Mismatch { has_read, has_written } => write!(f, "has_read {}, has_written {}", has_read, has_written),
            DupErr::Earlier => write!(f, "failed earlier"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{DupErr, DupReader};
    use futures::Future;
    use futures::task::{Context, Poll, Poll::*};
    use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite};
    use futures_test::task::noop_context;
    use std::fmt::{self, Debug, Formatter};
    use std::io::{self, Cursor, ErrorKind, Read, Write};
    use std::pin::Pin;

    fn poll_ok<T>(fut: &mut Future<Output=io::Result<T>>) -> T {
        let pinned = unsafe { Pin::new_unchecked(fut) };
        match pinned.poll(&mut noop_context()) {
            Ready(ready) => ready.unwrap(),
            Pending => panic!("unexpected Pending in synchronous tests"),
        }
    }
    
    fn poll_err<T: Debug>(fut: &mut Future<Output=io::Result<T>>, expected_err: &str) -> io::Error {
        let pinned = unsafe { Pin::new_unchecked(fut) };
        match pinned.poll(&mut noop_context()) {
            Ready(ready) => ready.expect_err(expected_err),
            Pending => panic!("unexpected Pending in synchronous tests"),
        }
    }
    
    #[test]
    fn dup_reader_works_blocking() {
        let mut source = Cursor::new(vec![5, 6, 7]);
        let mut dest = Cursor::new(vec![0; 3]);
        {
            let mut dup_reader = DupReader::new(&mut source, &mut dest);
            let mut buf = [0; 2];
            assert_eq!(2, Read::read(&mut dup_reader, &mut buf).unwrap());
            assert_eq!([5, 6], buf);
        }
        assert_eq!([5, 6, 0], dest.get_ref()[..]);
        {
            let mut dup_reader = DupReader::new(&mut source, &mut dest);
            let mut buf = [0; 2];
            assert_eq!(1, Read::read(&mut dup_reader, &mut buf).unwrap());
            assert_eq!([7, 0], buf);
        }
        assert_eq!([5, 6, 7], dest.get_ref()[..]);
    }

    #[test]
    fn dup_reader_works_async() {
        let mut source = Cursor::new(vec![5, 6, 7]);
        let mut dest = Cursor::new(vec![0; 4]);
        {
            let mut dup_reader = DupReader::new(&mut source, &mut dest);
            let mut buf = vec![0; 2];
            poll_ok(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf));
            assert_eq!(vec![5, 6], buf);
        }
        assert_eq!([5, 6, 0, 0], dest.get_ref()[..]);
        {
            let mut dup_reader = DupReader::new(&mut source, &mut dest);
            let mut buf = vec![0];
            poll_ok(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf));
            assert_eq!(vec![7], buf);
        }
        assert_eq!([5, 6, 7, 0], dest.get_ref()[..]);
        {
            let mut dup_reader = DupReader::new(&mut source, &mut dest);
            let mut buf = vec![0];
            let io_err = poll_err(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf), "expected error because of EOF");
            assert_eq!(ErrorKind::UnexpectedEof, io_err.kind());
        }
        assert_eq!([5, 6, 7, 0], dest.get_ref()[..]);
    }

    enum Mock {
        Success { count: usize },
        Failure { kind: ErrorKind },
    }
    impl Mock {
        fn poll_mock<Res>(self: Pin<&mut Self>, mk_res: fn(usize) -> Res, err: &str) -> Poll<io::Result<Res>> {
            match unsafe { self.get_unchecked_mut() } {
                &mut Mock::Success { count } => Ready(Ok(mk_res(count))),
                &mut Mock::Failure { kind } => Ready(Err(io::Error::new(kind, err))),
            }
        }
    }
    impl AsyncRead for Mock {
        fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
            self.poll_mock(|count| count, "mock read failure")
        }
    }
    impl AsyncWrite for Mock {
        fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
            self.poll_mock(|count| count, "mock write failure")
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
            self.poll_mock(|count| (), "mock flush failure")
        }
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
            self.poll_mock(|count| (), "mock close failure")
        }
    }

    #[test]
    fn dup_reader_keeps_read_error() {
        let from = &mut Mock::Failure { kind: ErrorKind::UnexpectedEof };
        let to = &mut Mock::Success { count: 5 };
        let mut dup_reader = DupReader::new(from, to);
        let mut buf = vec![0; 5];
        let io_err = poll_err(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf), "expected mock error");
        assert_eq!(ErrorKind::UnexpectedEof, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Read(ref cause) => {
                let err = cause.get_ref().unwrap();
                assert_eq!("mock read failure", err.to_string());
            },
            _ => panic!("expected DupErr::Read"),
        }
    }
    
    #[test]
    fn dup_reader_keeps_write_error() {
        let from = &mut Mock::Success { count: 5 };
        let to = &mut Mock::Failure { kind: ErrorKind::BrokenPipe };
        let mut dup_reader = DupReader::new(from, to);
        let mut buf = vec![0; 5];
        let io_err = poll_err(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf), "expected mock error");
        assert_eq!(ErrorKind::BrokenPipe, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Write(ref cause) => {
                let err = cause.get_ref().unwrap();
                assert_eq!("mock write failure", err.to_string());
            },
            _ => panic!("expected DupErr::Write"),
        }
    }

    #[test]
    fn dup_reader_detects_mismatch() {
        // 5 == 5
        let from = &mut Mock::Success { count: 5 };
        let to = &mut Mock::Success { count: 5 };
        let mut dup_reader = DupReader::new(from, to);
        let mut buf = vec![0; 5];
        poll_ok(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf));
        assert_eq!(vec![0; 5], buf);
        // 5 != 2
        let from = &mut Mock::Success { count: 5 };
        let to = &mut Mock::Success { count: 2 };
        let mut dup_reader = DupReader::new(from, to);
        let mut buf = vec![0; 5];
        let io_err = poll_err(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf), "expected mismatch error (5 != 2)");
        assert_eq!(ErrorKind::Other, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Mismatch { has_read, has_written } => {
                assert_eq!(5, has_read);
                assert_eq!(2, has_written);
            },
            _ => panic!("expected DupErr::Mismatch"),
        }
        // 5 != 7
        let from = &mut Mock::Success { count: 5 };
        let to = &mut Mock::Success { count: 7 };
        let mut dup_reader = DupReader::new(from, to);
        let mut buf = vec![0; 5];
        let io_err = poll_err(&mut AsyncReadExt::read_exact(&mut dup_reader, &mut buf), "expected mismatch error (5 != 7)");
        assert_eq!(ErrorKind::Other, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Mismatch { has_read, has_written } => {
                assert_eq!(5, has_read);
                assert_eq!(7, has_written);
            },
            _ => panic!("expected DupErr::Mismatch"),
        }
    }
}
