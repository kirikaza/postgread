extern crate async_std;
extern crate futures;
extern crate postgread;
extern crate structopt;

use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use postgread::convey::{convey, Message};
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

fn dump_msg(id: usize, msg: Message) {
    match msg {
        Message::Backend(backend_msg) =>
            println!("[{}] server sent {:?}", id, backend_msg),
        Message::Frontend(frontend_msg) =>
            println!("[{}] client sent {:?}", id, frontend_msg),
    }
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
                let result = convey(client, server, |msg| dump_msg(id, msg), ).await;
                println!("[{}] convey result is {:?}", id, result);
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
