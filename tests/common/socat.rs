use ::std::net::Ipv4Addr;

pub struct ArgsBuilder {
    pub log_level: LogLevel,
    pub log_program_name: Option<&'static str>,
    pub addr1: Addr,
    pub addr2: Addr,
}

pub struct Addr {
    type_: AddrType,
    options: Vec<AddrOption>,
}

pub enum AddrType {
    Tcp4(Tcp4),
    Tcp4Listen(Tcp4Listen),
}

#[derive(Clone)]
pub enum AddrOption {
    Fork,
    ReuseAddr,
    ReusePort,
}

pub struct Tcp4 {
    host: String,
    port: u16,
}

pub struct Tcp4Listen {
    interface: Option<Ipv4Addr>,
    port: u16,
}

#[allow(dead_code)]
pub enum LogLevel {
    Error,
    Warning,
    Notice,
    Info,
    Debug,
}

impl ArgsBuilder {
    pub fn build(&self) -> Vec<String> {
        let mut args: Vec<String> = vec![];
        self.add_log_level(&mut args);
        self.add_log_program_name(&mut args);
        args.push(self.addr1.build());
        args.push(self.addr2.build());
        args
    }

    fn add_log_level(&self, args: &mut Vec<String>) {
        let option = match self.log_level {
            LogLevel::Error => return,
            LogLevel::Warning => "-d",
            LogLevel::Notice => "-dd",
            LogLevel::Info => "-ddd",
            LogLevel::Debug => "-dddd",
        };
        args.push(option.to_owned());
    }

    fn add_log_program_name(&self, args: &mut Vec<String>) {
        if let Some(log_program_name) = &self.log_program_name {
            args.push("-lp".to_owned());
            args.push((*log_program_name).to_owned());
        }
    }
}

impl Addr {
    pub fn tcp4(host: &'static str, port: u16, options: &'static [AddrOption]) -> Self {
        Self {
            type_: AddrType::Tcp4(Tcp4 { host: host.to_owned(), port }),
            options: options.to_vec(),
        }
    }

    pub fn tcp4_listen(interface: Option<Ipv4Addr>, port: u16, options: &'static [AddrOption]) -> Self {
        Self {
            type_: AddrType::Tcp4Listen(Tcp4Listen { interface, port }),
            options: options.to_vec(),
        }
    }

    fn build(&self) -> String {
        let mut parts = vec![];
        match self.type_ {
            AddrType::Tcp4(Tcp4 {ref host, port}) => {
                parts.push(format!("tcp4:{}:{}", host, port));
            }
            AddrType::Tcp4Listen(Tcp4Listen { ref interface, port }) => {
                parts.push(format!("tcp4-listen:{}", port));
                if let Some(interface) = interface {
                    parts.push(format!("bind={}", interface))
                }
            }
        };
        for opt in &self.options {
            let x = match opt {
                AddrOption::Fork => "fork",
                AddrOption::ReuseAddr => "reuseaddr",
                AddrOption::ReusePort => "reuseport",
            };
            parts.push(x.to_owned())
        }
        parts.join(",")
    }
}