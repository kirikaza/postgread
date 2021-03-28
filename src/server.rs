use crate::convey::{Message, convey};
use crate::tls::native::{NativeTlsServer, NativeTlsClient};

use ::async_std::net::{TcpListener, TcpStream};
use ::async_std::stream::StreamExt;
use ::async_std::task;
use ::async_native_tls::{TlsAcceptor, TlsConnector};
use ::std::fs;
use ::std::io;
use ::std::net::{IpAddr, SocketAddr};
use ::std::sync::Arc;
use ::std::sync::atomic::{AtomicUsize, Ordering};
use ::chrono::{Local, SecondsFormat};
use ::structopt::StructOpt;

#[derive(Clone, StructOpt)]
#[structopt(name="postgread")]
pub struct Config {
    #[structopt(long = "listen-addr", default_value = "127.0.0.1")]
    pub listen_addr: IpAddr,

    #[structopt(long = "listen-port", default_value = "5432")]
    pub listen_port: u16,

    #[structopt(long = "target-host")]
    pub target_host: String,

    #[structopt(long = "target-port", default_value = "5432")]
    pub target_port: u16,

    #[structopt(long = "cert-p12-file")]
    pub cert_p12_file: String,

    #[structopt(long = "cert-p12-password", default_value = "")]
    pub cert_p12_password: String,
}

async fn handle_client<Callback>(
    target_host: String,
    target_port: u16,
    tls_acceptor: TlsAcceptor,
    client_id: usize,
    client: TcpStream,
    callback: Arc<Callback>,
) -> io::Result<()>
where Callback: for<'a> Fn(Message<'a>) + Send + Sync + 'static {
    let listen_port = client.local_addr().map(|addr| addr.port()).unwrap_or(0);
    println!("postgread[:{}] #{} is new connection from {:?}", listen_port, client_id, client.peer_addr().unwrap());
    let target_ip = target_host.parse()
        .map_err(|err| io::Error::new(io::ErrorKind::NotConnected, err))?;
    let server_endpoint = SocketAddr::new(target_ip, target_port);
    task::spawn(async move {
        match TcpStream::connect(&server_endpoint).await {
            Ok(server) => {
                println!("{} postgread[:{}] #{} connected to target server {}", format_now(), listen_port, client_id, server.local_addr().unwrap());
                let frontend_tls_server = NativeTlsServer(&tls_acceptor);
                let backend_tls_client = NativeTlsClient { connector: &new_tls_connector(), hostname: "localhost" };
                let result = convey(client, server, frontend_tls_server, backend_tls_client, &*callback).await;
                println!("{} postgread[:{}] #{} stopped conveying with {:?}", format_now(), listen_port, client_id, result);
            },
            Err(err) => {
                println!("{} postgread[:{}] #{} could not connect to target host: {:?}", format_now(), listen_port, client_id, err);
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

pub async fn listen(config: Config) -> io::Result<Server> {
    let tls_acceptor = new_tls_acceptor(&config)?;
    let socket = SocketAddr::new(config.listen_addr, config.listen_port);
    let tcp_listener = TcpListener::bind(&socket).await?;
    Ok(Server { tls_acceptor, tcp_listener, config })
}

pub struct Server {
    tls_acceptor: TlsAcceptor,
    tcp_listener: TcpListener,
    config: Config,
}

impl Server {
    pub fn get_listen_port(&self) -> io::Result<u16> {
        self.tcp_listener.local_addr().map(|addr| addr.port())
    }
}

pub async fn loop_accepting<Callback>(server: Server, callback: Arc<Callback>) -> io::Result<()>
where Callback: for<'a> Fn(Message<'a>) + Send + Sync + 'static {
    let Server { tls_acceptor, tcp_listener, config } = server;
    let target_host = config.target_host;
    let target_port = config.target_port;
    let mut incoming = tcp_listener.incoming();
    let next_client_id = Arc::new(AtomicUsize::new(1));
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let tls_acceptor = tls_acceptor.clone();
        let next_client_id = next_client_id.clone();
        let target_host = target_host.clone();
        let callback = callback.clone();
        task::spawn(async move {
            let client_id = next_client_id.fetch_add(1, Ordering::SeqCst);
            let local_port = stream.local_addr().map(|addr| addr.port()).unwrap_or(0);
            handle_client(target_host, target_port, tls_acceptor, client_id, stream, callback).await.unwrap_or_else(|err| {
                println!("{} postgread[:{}] #{} could not be handled: {:?}", format_now(), local_port, client_id, err)
            });
        });
    }
    Ok(())
}

fn format_now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}