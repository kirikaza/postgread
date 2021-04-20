mod common;
mod e2e;

extern crate bollard;  // common::docker
extern crate chrono;  // common::container
#[macro_use] extern crate claim;
#[macro_use] extern crate const_format;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;  // common::docker
extern crate rstest;

use crate::e2e::containers::{self, Containers, ExecOutcome};
use crate::e2e::ssh_port_forwarder::{self, SshPortForwarder};
use crate::common::global_fixture::*;

use postgread::convey::Message;
use postgread::convey::util::{BackendMsgClone, FrontendMsgClone};
use postgread::convey::util::MessageClone::{self, *};
use postgread::msg::body::{*, initial::*};
use postgread::server::{self, Server};

use ::async_std::task;
use ::rstest::*;
use ::std::env;
use ::std::fs;
use ::std::io;
use ::std::net::Ipv4Addr;
use ::std::path;
use ::std::sync::Arc;

macro_rules! backend {
    ( $Msg:ident $args:tt ) => { Backend(BackendMsgClone::$Msg($Msg $args)) };
    ( $Msg:ident::$ctor:ident ) => { Backend(BackendMsgClone::$Msg($Msg::$ctor)) };
    ( $Msg:ident::$ctor:ident $args:tt ) => { Backend(BackendMsgClone::$Msg($Msg::$ctor $args)) };
}

macro_rules! frontend {
    ( $Msg:ident $args:tt ) => { Frontend(FrontendMsgClone::$Msg($Msg $args)) };
    ( $Msg:ident::$ctor:ident ) => { Frontend(FrontendMsgClone::$Msg($Msg::$ctor)) };
    ( $Msg:ident::$ctor:ident $args:tt ) => { Frontend(FrontendMsgClone::$Msg($Msg::$ctor $args)) };
}

#[rstest]
async fn t1(test_env: TestEnv) {
    let conn_info = format!("host=localhost user=postgres password={}", test_env.postgres_passwd());
    let ExecOutcome { exit_code, std_out_err } = test_env.exec_on_client(vec!["psql", &conn_info]).await.unwrap();
    assert_matches!(std_out_err.as_slice(), []);
    assert_eq!(exit_code, 0);
    let messages = test_env.messages.lock().unwrap();
    assert_eq!(messages[0], frontend!(Initial::TLS));
    assert_eq!(messages[1], frontend!(Initial::Startup(Startup {
        version: Version { major: 3, minor: 0 },
        params: vec![
            StartupParam::new("user".into(), "postgres".into()),
            StartupParam::new("database".into(), "postgres".into()),
            StartupParam::new("application_name".into(), "psql".into()),
        ]
    })));
    assert_matches!(&messages[2], backend!(Authentication::Md5Password { salt: _ }));
    assert_matches!(&messages[3], frontend!(Password(_)));
    assert_eq!(messages[4],  backend!(Authentication::Ok));
    assert_eq!(messages[5],  backend!(ParameterStatus::new("application_name".into(), "psql".into())));
    assert_eq!(messages[6],  backend!(ParameterStatus::new("client_encoding".into(), "UTF8".into())));
    assert_eq!(messages[7],  backend!(ParameterStatus::new("DateStyle".into(), "ISO, MDY".into())));
    assert_eq!(messages[8],  backend!(ParameterStatus::new("integer_datetimes".into(), "on".into())));
    assert_eq!(messages[9],  backend!(ParameterStatus::new("IntervalStyle".into(), "postgres".into())));
    assert_eq!(messages[10], backend!(ParameterStatus::new("is_superuser".into(), "on".into())));
    assert_eq!(messages[11], backend!(ParameterStatus::new("server_encoding".into(), "UTF8".into())));
    assert_eq!(messages[12], backend!(ParameterStatus::new("server_version".into(), "12.6".into())));
    assert_eq!(messages[13], backend!(ParameterStatus::new("session_authorization".into(), "postgres".into())));
    assert_eq!(messages[14], backend!(ParameterStatus::new("standard_conforming_strings".into(), "on".into())));
    assert_eq!(messages[15], backend!(ParameterStatus::new("TimeZone".into(), "UTC".into())));
    assert_matches!(&messages[16], backend!(BackendKeyData { process_id: _, secret_key: _ }));
    assert_eq!(messages[17], backend!(ReadyForQuery { status: ready_for_query::Status::Idle }));
    assert_eq!(messages[18], frontend!(Terminate{}));
    assert_eq!(messages.len(), 19);
}

#[fixture]
fn test_env(containers_bound_fixture: ContainersBoundFixture) -> TestEnv {
    TestEnv::new(containers_bound_fixture)
}

struct TestEnv {
    messages: Arc<::std::sync::Mutex<Vec<MessageClone>>>,
    postgread_server_handle: Option<task::JoinHandle<io::Result<()>>>,
    port_forwarder: SshPortForwarder,
    containers_bound_fixture: ContainersBoundFixture,
}

impl TestEnv {
    fn new(containers_bound_fixture: ContainersBoundFixture) -> Self {
        let test_context = &containers_bound_fixture.test_context;
        let containers_ports = &test_context.containers_ports;
        let server = task::block_on(start_server(containers_ports.pg_server))
            .expect("could not start postgread server");
        let server_port = server.get_listen_port()
            .expect("could not get port listened by postgread server");
        let messages = Arc::new(::std::sync::Mutex::new(vec![]));
        let messages2 = messages.clone();
        let server_handle = task::spawn(server::loop_accepting(
            server,
            Arc::new(move |msg_ref: Message| {
                messages2.lock().unwrap().push(MessageClone::make(msg_ref));
            })
        ));
        let port_fwd_config = ssh_port_forwarder::Config {
            server_target_port: server_port,
            client_ssh_port: containers_ports.test_client,
            client_ssh_priv_key_path: &test_context.test_client_ssh_priv_key_path,
            client_forwarding_port: PG_DEFAULT_PORT,
        };
        let port_forwarder = SshPortForwarder::start(&port_fwd_config)
            .expect("could not start port forwarder (test client container -> postgread)");
        Self {
            messages,
            postgread_server_handle: Some(server_handle),
            port_forwarder,
            containers_bound_fixture,
        }
    }

    fn postgres_passwd(&self) -> &str {
        &self.containers_bound_fixture.test_context.pg_server_postgres_passwd
    }

    async fn exec_on_client(&self, cmd: Vec<&str>) -> Result<ExecOutcome, String> {
        containers::exec_on_test_client(cmd).await
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let postgread_server_loop = std::mem::take(&mut self.postgread_server_handle)
            .expect("could not take server handle; it is strange");
        if let Some(early_exit) = task::block_on(postgread_server_loop.cancel()) {
            panic!("server exited earlier: {:?}", early_exit);
        }
        self.port_forwarder.stop()
            .expect("could not stop port forwarder (test client container -> postgread server");
    }
}

#[fixture]
fn containers_bound_fixture() -> ContainersBoundFixture {
    ContainersBoundFixture::new()
}

async fn start_server(pg_server_port: u16) -> Result<Server, String> {
    let config = server::Config {
        listen_addr: Ipv4Addr::LOCALHOST.into(),
        listen_port: 0,
        target_host: Ipv4Addr::LOCALHOST.to_string(),
        target_port: pg_server_port,
        cert_p12_file: concat!(env!("CARGO_MANIFEST_DIR"), "/try/cert.p12").to_owned(),
        cert_p12_password: "".to_owned(),
    };
    server::listen(config).await.map_err(|e| e.to_string())
}

type ContainersBoundFixture = BoundFixture<ContainersGlobalFixture>;

struct ContainersGlobalFixture();

impl GlobalFixture for ContainersGlobalFixture {
    fn setup() -> Result<(Self::TestContext, Self::TearDownHandle), String> {
        let ssh_keys = Self::read_ssh_keys()
            .map_err(|e| format!("specify a separate SSH priv/pub keypair for the tests: {}", e))?;
        let pg_server_postgres_passwd = Self::read_postgres_passwd()
            .map_err(|e| format!("specify a password for user postgres on PG server: {}", e))?;
        env::set_var("TZ", "UTC");  // as inside containers
        let Containers { ports, logs_watchers } = containers::create_or_start_all(
            &pg_server_postgres_passwd,
            &ssh_keys.pub_key_content,
        )?;
        let context = ContainersTestContext {
            containers_ports: ports,
            pg_server_postgres_passwd,
            test_client_ssh_priv_key_path: ssh_keys.priv_key_path
        };
        let tear_down = logs_watchers;
        Ok((context, tear_down))
    }

    fn tear_down(logs_watchers: Self::TearDownHandle) -> Result<(), String> {
        containers::remove_or_stop_all(logs_watchers)
    }

    type TestContext = ContainersTestContext;

    type TearDownHandle = containers::LogsWatchers;

    fn get_mutex_context() -> &'static GlobalMutexContext<Self::TestContext, Self::TearDownHandle> {
        &SHARED_CONTAINERS_CONTEXT
    }

    const TESTS_FILE_CONTENT: &'static str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", file!()));
}

#[derive(Clone)]
struct ContainersTestContext {
    containers_ports: containers::Ports,
    pg_server_postgres_passwd: String,
    test_client_ssh_priv_key_path: String,
}

struct SshKeys {
    priv_key_path: String,
    pub_key_content: String,
}

impl ContainersGlobalFixture {
    const SSH_PRIV_KEY_PATH_ENVAR: &'static str = "POSTGREAD_TEST_CLIENT_SSH_PRIV_KEY_PATH";
    const POSTGRES_PASSWD_ENVAR: &'static str = "POSTGREAD_TEST_PG_SERVER_PASSWD";

    fn read_postgres_passwd() -> Result<String, String> {
        env::var(Self::POSTGRES_PASSWD_ENVAR)
            .map_err(|e| format!("envar \"{}\": {}", Self::POSTGRES_PASSWD_ENVAR, e.to_string()))
    }

    fn read_ssh_keys() -> Result<SshKeys, String> {
        let priv_key_path = env::var(Self::SSH_PRIV_KEY_PATH_ENVAR)
            .map_err(|e| format!("envar \"{}\": {}", Self::SSH_PRIV_KEY_PATH_ENVAR, e.to_string()))?;
        if ! path::Path::new(&priv_key_path).exists() {
            return Err(format!("file \"{}\" is missing", priv_key_path))
        }
        let pub_key_path = priv_key_path.clone() + ".pub";
        let pub_key_content = fs::read_to_string(&pub_key_path)
            .map_err(|e| format!("file \"{}\": {}", &pub_key_path, e.to_string()))?;
        Ok(SshKeys { priv_key_path, pub_key_content })
    }
}

lazy_static! {
    static ref SHARED_CONTAINERS_CONTEXT: GlobalMutexContext<ContainersTestContext, containers::LogsWatchers> = new_global_mutex_context();
}

const PG_DEFAULT_PORT: u16 = 5432;
