extern crate async_std;
extern crate futures;
extern crate postgread;
extern crate structopt;

use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use postgread::dup::DupReader;
use postgread::msg::{BackendMessage, FrontendMessage};
use postgread::async_std_compat::compat;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use structopt::StructOpt;

#[derive(Clone, StructOpt)]
#[structopt(name="postgread")]
struct Config {
    #[structopt(long = "listen-addr", default_value = "127.0.0.1")]
    listen_addr: IpAddr,

    #[structopt(long = "listen-port", default_value = "5432")]
    listen_port: u16,

    #[structopt(long = "target-host")]
    target_host: String,

    #[structopt(long = "target-port", default_value = "5432")]
    target_port: u16,
}

fn convey_backend_messages<R, W>(id: usize, from: R, to: W) -> io::Result<()>
where
    R: 'static + AsyncRead + Send + Unpin,
    W: 'static + AsyncWrite + Send + Unpin,
{
    task::spawn(async move {
        let mut dup = BufReader::new(DupReader::new(from, to));
        loop {
            match BackendMessage::read(&mut dup).await {
                Ok(None) => {
                    println!("[{}] server finished", id);
                    break;
                },
                Ok(Some(msg)) => {
                    println!("[{}] server sent {:?}", id, msg);
                },
                Err(err) => {
                    println!("[{}] server behaved unexpectedly: {:?}", id, err);
                    break;
                },
            }
        }
    });
    Ok(())
}

fn convey_frontend_messages<R, W>(id: usize, from: R, to: W) -> io::Result<()>
    where
        R: 'static + AsyncRead + Send + Unpin,
        W: 'static + AsyncWrite + Send + Unpin,
{
    task::spawn(async move {
        let mut first = true;
        let mut dup = BufReader::new(DupReader::new(from, to));
        loop {
            match FrontendMessage::read(&mut dup, first).await {
                Ok(None) => {
                    println!("[{}] client finished", id);
                    break;
                },
                Ok(Some(msg)) => {
                    println!("[{}] client sent {:?}", id, msg);
                },
                Err(err) => {
                    println!("[{}] client behaved unexpectedly: {:?}", id, err);
                    break;
                },
            }
            first = false;
        }
    });
    Ok(())
}

async fn handle_client(config: Config, id: usize, client: TcpStream) -> io::Result<()> {
    println!("[{}] accepted client {:?}", id, client.peer_addr().unwrap());
    let target_ip = config.target_host.parse()
        .map_err(|err| io::Error::new(io::ErrorKind::NotConnected, err))?;
    let server_endpoint = SocketAddr::new(target_ip, config.target_port);
    task::spawn(async move {
        match TcpStream::connect(&server_endpoint).await {
            Ok(server) => {
                println!("[{}] connected to target server {}", id, server.local_addr().unwrap());
                match (compat(client).split(), compat(server).split()) {
                    ((from_client, to_client), (from_server, to_server)) => {
                        convey_frontend_messages(id, from_client, to_server).unwrap();
                        convey_backend_messages(id, from_server, to_client).unwrap();
                    }
                }
            },
            Err(err) => {
                println!("[{}] could not connect to target host: {:?}", id, err);
            },
        }
    });
    Ok(())
}

fn main() -> io::Result<()> {
    let config = Config::from_args();
    let listen = SocketAddr::new(config.listen_addr, config.listen_port);
    task::block_on(async move {
        let listener = TcpListener::bind(&listen).await?;
        let next_client_id = Arc::new(AtomicUsize::new(1));
        while let Some(stream) = listener.incoming().next().await {
            let stream = stream?;
            let config = config.clone();
            let next_client_id = next_client_id.clone();
            task::spawn(async move {
                let client_id = next_client_id.fetch_add(1, Ordering::SeqCst);
                handle_client(config, client_id, stream).await.unwrap_or_else(|err| {
                    println!("[{}] could not handle client: {:?}", client_id, err)
                });
            });
        }
        Ok(())
    })
}
