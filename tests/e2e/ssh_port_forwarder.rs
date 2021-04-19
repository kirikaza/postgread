use ::std::process::{Child, Command, Stdio};

pub struct Config<'a> {
    pub server_target_port: u16,
    pub client_ssh_port: u16,
    pub client_ssh_priv_key_path: &'a str,
    pub client_forwarding_port: u16,
}

pub struct SshPortForwarder {
    ssh: Child,
}

impl SshPortForwarder {
    pub fn start(config: &Config) -> Result<Self, String> {
        let forward_rule = format!("{}:localhost:{}", config.client_forwarding_port, config.server_target_port);
        let client_ssh_port = config.client_ssh_port.to_string();
        let mut ssh = Command::new("ssh")
            .stdin(Stdio::null())
            .args(&[
                "-i", config.client_ssh_priv_key_path,
                "-N",
                "-o", "IdentitiesOnly=yes",
                "-o", "NoHostAuthenticationForLocalhost=yes",
                "-o", "PasswordAuthentication=no",
                "-p", &client_ssh_port,
                "-R", &forward_rule,
                "-v",
                "tc@localhost",
            ])
            .spawn()
            .map_err(|e| format!("{}", e))?;
        match ssh.try_wait() {
            Ok(None) => Ok(Self { ssh }),
            Ok(Some(exit_code)) => Err(format!("ssh exited with code {}", exit_code)),
            Err(err) => Err(format!("couldn't check if ssh exited: {}", err)),
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.ssh.kill().map_err(|e| e.to_string())
    }
}
