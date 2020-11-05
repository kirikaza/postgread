extern crate async_std;
extern crate futures;
extern crate postgread;
extern crate structopt;

use postgread::server::{self, Config};

use async_std::task;
use std::io;
use structopt::StructOpt;

fn main() -> io::Result<()> {
    let config = Config::from_args();
    task::block_on(async {
        let server = server::listen(config).await?;
        server::loop_accepting(server).await
    })
}
