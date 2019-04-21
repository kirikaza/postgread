extern crate futures;
extern crate postgread;
#[macro_use]
extern crate structopt;
extern crate tokio;

use futures::{Future, Stream};
use futures::future::{Loop::*, loop_fn};
use postgread::dup::DupReader;
use postgread::msg::Message;
use std::net::{IpAddr, SocketAddr};
use std::io::BufReader;
use structopt::StructOpt;
use tokio::io;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};

#[derive(StructOpt)]
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

fn convey_messages<R, W>(from: R, to: W, first: bool, name: &'static str) -> io::Result<()>
where
    R: 'static + AsyncRead + Send,
    W: 'static + AsyncWrite + Send,
{
    let dup = BufReader::new(DupReader::new(from, to));
    tokio::spawn(loop_fn((dup, first), move |(stream, first)| {
        Message::read(stream, first).map(move |x| match x {
            None => {
                println!("{} finished", name);
                Break(())
            },
            Some((stream, msg)) => {
                println!("{} sent {:?}", name, msg);
                Continue((stream, false))
            },
        })
    }).map_err(move |err| {
        println!("{} behaved unexpectedly: {:?}", name, err)
    }));
    Ok(())
}

fn handle_client(config: &Config, client: TcpStream) -> io::Result<()> {
    println!("accepted client {:?}", client.peer_addr().unwrap());
    let target_ip = config.target_host.parse()
        .map_err(|err| io::Error::new(io::ErrorKind::NotConnected, err))?;
    let server_endpoint = SocketAddr::new(target_ip, config.target_port);
    let task = TcpStream::connect(&server_endpoint).and_then(move |server| {
        println!("connected to target server {}", server.local_addr().unwrap());
        match (client.split(), server.split()) {
            ((from_client, to_client), (from_server, to_server)) => {
                convey_messages(from_client, to_server, true, "client")?;
                convey_messages(from_server, to_client, false, "server")?;
                Ok(())
            }
        }
    }).map_err(|err| {
        println!("could not connect to target host: {:?}", err);
    });
    tokio::spawn(task);
    Ok(())
}


fn main() {
    let config = Config::from_args();
    let listen = SocketAddr::new(config.listen_addr, config.listen_port);
    let listener = TcpListener::bind(&listen).unwrap();
    let server = listener.incoming().for_each(move |stream| {
        handle_client(&config, stream)
    }).map_err(|err| {
        println!("cannot accept connection: {:?}", err);
    });
    tokio::run(server);
}
