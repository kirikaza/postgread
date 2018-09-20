extern crate postgread;
use postgread::convey::MsgConveyer;

#[macro_use] extern crate structopt;
use structopt::StructOpt;

use std::io;
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(StructOpt)]
#[structopt(name="postgread")]
struct Config {
    #[structopt(long = "target-host")]
    target_host: String,

    #[structopt(long = "target-port", default_value = "5432")]
    target_port: u16,
}

fn handle_client(config: &Config, client: &mut TcpStream) -> io::Result<()> {
    let server_endpoint = (config.target_host.as_str(), config.target_port);
    let server = TcpStream::connect(server_endpoint).unwrap();
    let mut client_clone = client.try_clone()?;
    let mut server_clone = server.try_clone()?;
    let from_client = thread::spawn(move || {
        for msg in MsgConveyer::from_client(&mut client_clone, &mut server_clone) {
            println!("client >>> {:?} >>> server", msg.unwrap());
        }
        println!("client finished");
    });
    let mut client_clone = client.try_clone()?;
    let mut server_clone = server.try_clone()?;
    let from_server = thread::spawn(move || {
        for msg in MsgConveyer::from_server(&mut server_clone, &mut client_clone) {
            println!("client <<< {:?} <<< server", msg.unwrap());
        }
        println!("server finished");
    });
    
    let from_client_ok = from_client.join().is_ok();
    let from_server_ok = from_server.join().is_ok();
    if from_client_ok && from_server_ok {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "can't join some thread(s)"))
    }
}

fn main() {
    let config = Config::from_args();
    let listener = TcpListener::bind("127.0.0.1:15432").unwrap();
    for stream in listener.incoming() {
        handle_client(&config, &mut stream.unwrap()).unwrap();
    }
}
