use crate::common::socat;

use ::std::process::{Child, Command, Stdio};

pub struct PortForwarder {
    socat: Child,
}

impl PortForwarder {
    pub fn start(server_port: u16, client_socat_port: u16) -> Result<Self, String> {
        use socat::*;
        let args_builder = ArgsBuilder {
            log_level: LogLevel::Notice,
            log_program_name: Some("port_forwarder"),
            addr1: Addr::tcp4("localhost", server_port, &[]),
            addr2: Addr::tcp4("localhost", client_socat_port, &[]),
        };
        let mut socat = Command::new("socat")
            .stdin(Stdio::null())
            .args(args_builder.build())
            .spawn()
            .map_err(|e| format!("{}", e))?;
        match socat.try_wait() {
            Ok(None) => Ok(Self { socat }),
            Ok(Some(exit_code)) => Err(format!("socat exited with code {}", exit_code)),
            Err(err) => Err(format!("couldn't check if socat exited: {}", err)),
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.socat.kill().map_err(|e| e.to_string())
    }
}
