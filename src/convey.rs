use crate::async_std_compat::compat;
use crate::dup::DupReader;
use crate::msg::{BackendMessage, FrontendMessage};

use ::async_std::net::TcpStream;
use ::async_std::task;
use ::futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use ::std::io::Result as IoResult;

pub fn convey<BC, FC>(
    client: TcpStream,
    server: TcpStream,
    backend_callback: BC,
    frontend_callback: FC,
) where
    BC: 'static + Send + Fn(&IoResult<Option<BackendMessage>>) -> (),
    FC: 'static + Send + Fn(&IoResult<Option<FrontendMessage>>) -> (),
{
    match (compat(client).split(), compat(server).split()) {
        ((from_client, to_client), (from_server, to_server)) => {
            convey_frontend_messages(from_client, to_server, frontend_callback).unwrap();
            convey_backend_messages(from_server, to_client, backend_callback).unwrap();
        }
    }
}

fn convey_backend_messages<R, W, F>(
    from: R,
    to: W,
    callback: F,
) -> IoResult<()>
where
    R: 'static + AsyncRead + Send + Unpin,
    W: 'static + AsyncWrite + Send + Unpin,
    F: 'static + Send + Fn(&IoResult<Option<BackendMessage>>) -> (),
{
    task::spawn(async move {
        let mut dup = BufReader::new(DupReader::new(from, to));
        loop {
            let msg = BackendMessage::read(&mut dup).await;
            callback(&msg);
            match msg {
                Err(_) => break,
                Ok(None) => break,
                Ok(Some(_)) => {},
            }
        }
    });
    Ok(())
}

fn convey_frontend_messages<R, W, F>(
    from: R,
    to: W,
    callback: F,
) -> IoResult<()>
where
    R: 'static + AsyncRead + Send + Unpin,
    W: 'static + AsyncWrite + Send + Unpin,
    F: 'static + Send + Fn(&IoResult<Option<FrontendMessage>>) -> (),
{
    task::spawn(async move {
        let mut first = true;
        let mut dup = BufReader::new(DupReader::new(from, to));
        loop {
            let msg = FrontendMessage::read(&mut dup, first).await;
            callback(&msg);
            match msg {
                Err(_) => break,
                Ok(None) => break,
                Ok(Some(_)) => {},
            }
            first = false;
        }
    });
    Ok(())
}
