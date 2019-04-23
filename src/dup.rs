use bytes::BufMut;
use futures::{Async::*, Poll};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{Read};
use tokio::io::{self, AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

pub struct DupReader<R, W> {
    from: R,
    to: W,
}

impl<R, W> DupReader<R, W> {
    pub fn new(from: R, to: W) -> DupReader<R, W> {
        DupReader { from, to }
    }
}

impl<R: AsyncRead, W: AsyncWrite> Read for DupReader<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.from.read(buf).map_err(|e| io::Error::new(e.kind(), DupErr::Read(e)))?;
        let written = self.to.write(&buf[..read]).map_err(|e| io::Error::new(e.kind(), DupErr::Write(e)))?;
        if read == written {
            Ok(written)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                DupErr::Mismatch { read, written },
            ))
        }
    }
}

impl<R: AsyncRead, W: AsyncWrite> AsyncRead for DupReader<R, W> {
    fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, io::Error> {
        let read = try_ready!(self.from.poll_read(buf).map_err(|e| io::Error::new(e.kind(), DupErr::Read(e))));
        let written = try_ready!(self.to.poll_write(&buf[..read]).map_err(|e| io::Error::new(e.kind(), DupErr::Write(e))));
        if read == written {
            Ok(Ready(written))
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                DupErr::Mismatch { read, written },
            ))
        }
    }

    fn read_buf<B: BufMut>(&mut self, _buf: &mut B) -> Poll<usize, io::Error>
    where Self: Sized
    { unimplemented!() }
    
    fn split(self) -> (ReadHalf<Self>, WriteHalf<Self>)
    where Self: AsyncWrite
    { unimplemented!() }
}

#[derive(Debug)]
pub enum DupErr {
    Read(io::Error),
    Write(io::Error),
    Mismatch { read: usize, written: usize },
}
impl Error for DupErr {
    fn description(&self) -> &str {
        match *self {
            DupErr::Read(ref e) => e.description(),
            DupErr::Write(ref e) => e.description(),
            DupErr::Mismatch {..} => "read/written mismatch",
        }
    }
    fn cause(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            DupErr::Read(ref e) => e.source(),
            DupErr::Write(ref e) => e.source(),
            DupErr::Mismatch {..} => None,
        }
    }
}
impl Display for DupErr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            DupErr::Read(ref e) => write!(f, "read error: {}", e),
            DupErr::Write(ref e) => write!(f, "write error: {}", e),
            DupErr::Mismatch { read, written } => write!(f, "read {}, written {}", read, written)
        }
    }
}

#[cfg(test)]
mod test {
    use super::{DupErr, DupReader};
    use futures::{Async::*, Future, Poll};
    use std::fmt::{self, Debug, Formatter};
    use std::io::{Cursor, ErrorKind, Read, Write};
    use tokio::io::{self, AsyncRead, AsyncWrite};

    #[test]
    fn dup_reader_works_blocking() {
        let mut source = Cursor::new(vec![5, 6, 7]);
        let mut dest = Cursor::new(vec![0; 3]);
        {
            let mut dup_reader = DupReader { from: &mut source, to: &mut dest };
            let mut buf = [0; 2];
            assert_eq!(2, dup_reader.read(&mut buf).unwrap());
            assert_eq!([5, 6], buf);
        }
        assert_eq!([5, 6, 0], dest.get_ref()[..]);
        {
            let mut dup_reader = DupReader { from: &mut source, to: &mut dest };
            let mut buf = [0; 2];
            assert_eq!(1, dup_reader.read(&mut buf).unwrap());
            assert_eq!([7, 0], buf);
        }
        assert_eq!([5, 6, 7], dest.get_ref()[..]);
    }

    #[test]
    fn dup_reader_works_async() {
        let mut source = Cursor::new(vec![5, 6, 7]);
        let mut dest = Cursor::new(vec![0; 4]);
        {
            let dup_reader = DupReader { from: &mut source, to: &mut dest };
            assert_eq!(
                Ready(vec![5, 6]),
                io::read_exact(dup_reader, vec![0; 2])
                    .map(|(_, buf)| buf).poll().unwrap()
            );
        }
        assert_eq!([5, 6, 0, 0], dest.get_ref()[..]);
        {
            let dup_reader = DupReader { from: &mut source, to: &mut dest };
            assert_eq!(
                Ready(vec![7]),
                io::read_exact(dup_reader, vec![0])
                    .map(|(_, buf)| buf).poll().unwrap()
            );
        }
        assert_eq!([5, 6, 7, 0], dest.get_ref()[..]);
        {
            let dup_reader = DupReader { from: &mut source, to: &mut dest };
            let io_err = io::read_exact(dup_reader, vec![0])
                .map(|(_, buf)| buf).poll().expect_err("expected error because of EOF");
            assert_eq!(ErrorKind::UnexpectedEof, io_err.kind());
            let err = io_err.get_ref().unwrap();
            assert_eq!("early eof", format!("{}", err));
        }
        assert_eq!([5, 6, 7, 0], dest.get_ref()[..]);
    }

    enum Mock {
        Success { count: usize },
        Failure { kind: ErrorKind },
    }
    impl Read for Mock {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self {
                &mut Mock::Success { count } => Ok(count),
                &mut Mock::Failure { kind } => Err(io::Error::new(kind, "mock read failure")),
            }
        }
    }
    impl AsyncRead for Mock {}
    impl Write for Mock {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self {
                &mut Mock::Success { count } => Ok(count),
                &mut Mock::Failure { kind } => Err(io::Error::new(kind, "mock write failure")),
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            match self {
                &mut Mock::Success { count } => Ok(()),
                &mut Mock::Failure { kind } => Err(io::Error::new(kind, "mock flush failure")),
            }
        }
    }
    impl AsyncWrite for Mock {
        fn shutdown(&mut self) -> Poll<(), io::Error> {
            unimplemented!()
        }
    }

    #[test]
    fn dup_reader_keeps_read_error() {
        let dup_reader = DupReader {
            from: &mut Mock::Failure { kind: ErrorKind::UnexpectedEof },
            to: &mut Mock::Success { count: 5 },
        };
        let io_err = io::read_exact(dup_reader, vec![0; 5])
            .map(|(_, buf)| buf).poll().expect_err("expected mock error");
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
        let dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Failure { kind: ErrorKind::BrokenPipe },
        };
        let io_err = io::read_exact(dup_reader, vec![0; 5])
            .map(|(_, buf)| buf).poll().expect_err("expected mock error");
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
        let dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 5 },
        };
        assert_eq!(
            Ready(vec![0; 5]),
            io::read_exact(dup_reader, vec![0; 5])
                .map(|(_, buf)| buf).poll().unwrap()
        );
        // 5 != 2
        let dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 2 },
        };
        let io_err = io::read_exact(dup_reader, vec![0; 5])
            .map(|(_, buf)| buf).poll().expect_err("expected mismatch error (5 != 2)");
        assert_eq!(ErrorKind::Other, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Mismatch { read, written } => {
                assert_eq!(5, read);
                assert_eq!(2, written);
            },
            _ => panic!("expected DupErr::Mismatch"),
        }
        // 5 != 7
        let dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 7 },
        };
        let io_err = io::read_exact(dup_reader, vec![0; 5])
            .map(|(_, buf)| buf).poll().expect_err("expected mismatch error (5 != 7)");
        assert_eq!(ErrorKind::Other, io_err.kind());
        let err = io_err.get_ref().unwrap();
        let dup_err = err.downcast_ref::<DupErr>().unwrap();
        match dup_err {
            &DupErr::Mismatch { read, written } => {
                assert_eq!(5, read);
                assert_eq!(7, written);
            },
            _ => panic!("expected DupErr::Mismatch"),
        }
    }
}
