extern crate async_std;
extern crate futures;
extern crate postgread;
extern crate structopt;

use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use async_native_tls::{TlsAcceptor, TlsConnector};
use postgread::convey::{ConveyResult, Conveyor, Message};
use postgread::tls::native::{NativeTlsServer, NativeTlsClient};
use std::fs;
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

    #[structopt(long = "cert-p12-file")]
    cert_p12_file: String,

    #[structopt(long = "cert-p12-password", default_value = "")]
    cert_p12_password: String,
}

fn dump_msg(id: usize, msg: Message) {
    match msg {
        Message::Backend(backend_msg) =>
            println!("[{}] server sent {:?}", id, backend_msg),
        Message::Frontend(frontend_msg) =>
            println!("[{}] client sent {:?}", id, frontend_msg),
    }
}

pub async fn convey<Callback>(
    frontend: TcpStream,
    backend: TcpStream,
    callback: Callback,
    frontend_tls_acceptor: &TlsAcceptor,
    backend_tls_connector: &TlsConnector,
) -> ConveyResult<()>
    where Callback: Fn(Message) -> () + Send {
    Conveyor::start(
        frontend,
        backend,
        NativeTlsServer(frontend_tls_acceptor),
        NativeTlsClient { connector: backend_tls_connector, hostname: "localhost" },
        callback,
    ).await
}

async fn handle_client(config: Config, tls_acceptor: TlsAcceptor, id: usize, client: TcpStream) -> io::Result<()> {
    println!("[{}] accepted client {:?}", id, client.peer_addr().unwrap());
    let target_ip = config.target_host.parse()
        .map_err(|err| io::Error::new(io::ErrorKind::NotConnected, err))?;
    let server_endpoint = SocketAddr::new(target_ip, config.target_port);
    task::spawn(async move {
        match TcpStream::connect(&server_endpoint).await {
            Ok(server) => {
                println!("[{}] connected to target server {}", id, server.local_addr().unwrap());
                let result = convey(client, server, |msg| dump_msg(id, msg), &tls_acceptor, &new_tls_connector()).await;
                println!("[{}] convey result is {:?}", id, result);
            },
            Err(err) => {
                println!("[{}] could not connect to target host: {:?}", id, err);
            },
        }
    });
    Ok(())
}

fn tls_error_to_io_error(tls_error: native_tls::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, tls_error.to_string())
}

fn new_tls_acceptor(config: &Config) -> io::Result<TlsAcceptor> {
    use native_tls::{Identity, TlsAcceptor as TlsAcceptorImpl};
    let cert_p12 = fs::read(&config.cert_p12_file)?;
    let tls_identity = Identity::from_pkcs12(&cert_p12, &config.cert_p12_password);
    let tls_acceptor_impl = tls_identity.and_then(TlsAcceptorImpl::new);
    tls_acceptor_impl.map(TlsAcceptor::from).map_err(tls_error_to_io_error)
}

fn new_tls_connector() -> TlsConnector {
    TlsConnector::new().danger_accept_invalid_certs(true)  // TODO make it configurable
}

fn main() -> io::Result<()> {
    let config = Config::from_args();
    let tls_acceptor = new_tls_acceptor(&config)?;
    let listen = SocketAddr::new(config.listen_addr, config.listen_port);
    task::block_on(async move {
        let listener = TcpListener::bind(&listen).await?;
        let next_client_id = Arc::new(AtomicUsize::new(1));
        while let Some(stream) = listener.incoming().next().await {
            let stream = stream?;
            let config = config.clone();
            let tls_acceptor = tls_acceptor.clone();
            let next_client_id = next_client_id.clone();
            task::spawn(async move {
                let client_id = next_client_id.fetch_add(1, Ordering::SeqCst);
                handle_client(config, tls_acceptor, client_id, stream).await.unwrap_or_else(|err| {
                    println!("[{}] could not handle client: {:?}", client_id, err)
                });
            });
        }
        Ok(())
    })
}
