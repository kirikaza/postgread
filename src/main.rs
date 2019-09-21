#![feature(async_closure)]

extern crate futures;
extern crate postgread;
extern crate structopt;
extern crate tokio;

use futures::io::{AsyncRead, AsyncWrite, BufReader};
use postgread::dup::DupReader;
use postgread::msg::Message;
use postgread::tokio_compat::compat;
use std::net::{IpAddr, SocketAddr};
use std::io;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};

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

fn convey_messages<R, W>(from: R, to: W, mut first: bool, name: &'static str) -> io::Result<()>
where
    R: 'static + AsyncRead + Send + Unpin,
    W: 'static + AsyncWrite + Send + Unpin,
{
    tokio::spawn(async move {
        let mut dup = BufReader::new(DupReader::new(from, to));
        loop {
            match Message::read(&mut dup, first).await {
                Ok(None) => {
                    println!("{} finished", name);
                    break;
                },
                Ok(Some(msg)) => {
                    println!("{} sent {:?}", name, msg);
                    first = false;
                },
                Err(err) => {
                    println!("{} behaved unexpectedly: {:?}", name, err);
                    break;
                },
            }
        }
    });
    Ok(())
}

async fn handle_client(config: Config, client: TcpStream) -> io::Result<()> {
    println!("accepted client {:?}", client.peer_addr().unwrap());
    let target_ip = config.target_host.parse()
        .map_err(|err| io::Error::new(io::ErrorKind::NotConnected, err))?;
    let server_endpoint = SocketAddr::new(target_ip, config.target_port);
    tokio::spawn(async move {
        match TcpStream::connect(&server_endpoint).await {
            Ok(server) => {
                println!("connected to target server {}", server.local_addr().unwrap());
                match (client.split(), server.split()) {
                    ((from_client, to_client), (from_server, to_server)) => {
                        convey_messages(compat(from_client), compat(to_server), true, "client").unwrap();
                        convey_messages(compat(from_server), compat(to_client), false, "server").unwrap();
                    }
                }
            },
            Err(err) => {
                println!("could not connect to target host: {:?}", err);
            },
        }
    });
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = Config::from_args();
    let listen = SocketAddr::new(config.listen_addr, config.listen_port);
    let mut listener = TcpListener::bind(&listen).unwrap();
    loop {
        let (stream, _) = listener.accept().await?;
        let config = config.clone();
        tokio::spawn(async move {
            handle_client(config, stream).await.unwrap_or_else(|err| {
                println!("could not handle client: {:?}", err)
            });
        });
    }
}
