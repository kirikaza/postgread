extern crate async_std;
extern crate futures;
extern crate postgread;
extern crate structopt;

use postgread::server::{self, Config};
use postgread::convey::Message;

use async_std::task;
use std::io;
use std::sync::Arc;
use structopt::StructOpt;

fn dump_msg(msg: Message) {
    match msg {
        Message::Backend(backend_msg) =>
            println!("postgread got from server {:?}", backend_msg),
        Message::Frontend(frontend_msg) =>
            println!("postgread got from client {:?}", frontend_msg),
    }
}

fn main() -> io::Result<()> {
    let config = Config::from_args();
    task::block_on(async {
        let server = server::listen(config).await?;
        server::loop_accepting(server, Arc::new(dump_msg)).await
    })
}
