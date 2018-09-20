use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Read, Write};

pub struct DupReader<'a> {
    from: &'a mut Read,
    to: &'a mut Write,
}
impl<'a> DupReader<'a> {
    pub fn new(from: &'a mut Read, to: &'a mut Write) -> DupReader<'a> {
        DupReader { from, to }
    }
}
impl<'a> Read for DupReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.from.read(buf).map_err(|e| io::Error::new(e.kind(), DupErr::Read(e)))?;
        let written = self.to.write(&buf[..read]).map_err(|e| io::Error::new(e.kind(), DupErr::Write(e)))?;
        if read == written {
            Ok(written)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                DupErr::Mismatch { read: read, written: written },
            ))
        }
    }
}

#[derive(Debug)]
pub enum DupErr {
    Read(io::Error),
    Write(io::Error),
    Mismatch { read: usize, written: usize },
}
impl Error for DupErr {
    fn description(&self) -> &str {
        match self {
            &DupErr::Read(ref e) => e.description(),
            &DupErr::Write(ref e) => e.description(),
            &DupErr::Mismatch {..} => "read/written mismatch",
        }
    }
    fn cause(&self) -> Option<&Error> {
        match self {
            &DupErr::Read(ref e) => e.cause(),
            &DupErr::Write(ref e) => e.cause(),
            &DupErr::Mismatch {..} => None,
        }
    }
}
impl Display for DupErr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &DupErr::Read(ref e) => write!(f, "read error: {}", e),
            &DupErr::Write(ref e) => write!(f, "write error: {}", e),
            &DupErr::Mismatch { read, written } => write!(f, "read {}, written {}", read, written)
        }
    }
}

#[cfg(test)]
mod test {
    use super::{DupErr, DupReader};
    use std::io::{self, ErrorKind, Read, Write};
    
    #[test]
    fn dup_reader_works() {
        let mut source: &[u8] = &[5, 6, 7][..];
        let mut dest = vec![];
        {
            let mut dup_reader = DupReader { from: &mut source, to: &mut dest };
            let mut buf = [0u8; 2];
            assert_eq!(2, dup_reader.read(&mut buf).unwrap());
            assert_eq!([5, 6], buf);
        }
        assert_eq!(vec![5, 6], dest);
        {
            let mut dup_reader = DupReader { from: &mut source, to: &mut dest };
            let mut buf = [0u8; 2];
            assert_eq!(1, dup_reader.read(&mut buf).unwrap());
            assert_eq!([7, 0], buf);
        }
        assert_eq!(vec![5, 6, 7], dest);
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

    #[test]
    fn dup_reader_keeps_read_error() {
        let mut buf = [0u8; 5];
        let mut dup_reader = DupReader {
            from: &mut Mock::Failure { kind: ErrorKind::UnexpectedEof },
            to: &mut Mock::Success { count: 5 },
        };
        let io_err = dup_reader.read(&mut buf).unwrap_err();
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
        let mut buf = [0u8; 5];
        let mut dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Failure { kind: ErrorKind::BrokenPipe },
        };
        let io_err = dup_reader.read(&mut buf).unwrap_err();
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
        let mut buf = [0u8; 5];
        // 5 == 5
        let mut dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 5 },
        };
        assert_eq!(5, dup_reader.read(&mut buf).unwrap());
        // 5 != 2
        let mut dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 2 },
        };
        let io_err = dup_reader.read(&mut buf).unwrap_err();
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
        let mut dup_reader = DupReader {
            from: &mut Mock::Success { count: 5 },
            to: &mut Mock::Success { count: 7 },
        };
        let io_err = dup_reader.read(&mut buf).unwrap_err();
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
