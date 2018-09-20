use dup::DupReader;
use msg::Message;

use std::io::{self, BufReader, Read, Write};

pub struct MsgConveyer<'a> {
    dup: BufReader<DupReader<'a>>,
    from_client: bool,
    first_msg: bool,
}

impl<'a> MsgConveyer<'a> {
    pub fn from_client(
        client: &'a mut Read,
        server: &'a mut Write,
    ) -> Self {
        MsgConveyer {
            dup: BufReader::new(DupReader::new(client, server)),
            from_client: true,
            first_msg: true,
        }
    }
    pub fn from_server(
        server: &'a mut Read,
        client: &'a mut Write,
    ) -> Self {
        MsgConveyer {
            dup: BufReader::new(DupReader::new(server, client)),
            from_client: false,
            first_msg: true,
        }
    }
}

impl<'a> Iterator for MsgConveyer<'a> {
    type Item = io::Result<Message>;

    fn next(&mut self) -> Option<io::Result<Message>> {
        let msg = Message::read(&mut self.dup, self.from_client && self.first_msg);
        self.first_msg = false;
        match msg {
            Ok(None) => None,
            Ok(Some(m)) => Some(Ok(m)),
            Err(e) => Some(Err(e)),
        }
    }
}
