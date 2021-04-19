mod common;
mod e2e;

extern crate bollard;  // common::docker
extern crate chrono;  // common::container
#[macro_use] extern crate const_format;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate maplit;  // common::docker
extern crate rstest;

use crate::e2e::containers::{self, Containers, ExecOutcome};
use crate::e2e::ssh_port_forwarder::{self, SshPortForwarder};
use crate::common::global_fixture::*;

use postgread::server::{self, Server};
use postgread::convey::Message;

use ::async_std::task;
use ::rstest::*;
use ::std::env;
use ::std::fs;
use ::std::io;
use ::std::net::Ipv4Addr;
use ::std::path;
use ::std::sync::Arc;

#[rstest]
async fn t1(test_env: TestEnv) {
    println!("{:?}", test_env.exec_on_client(vec!["psql", "-h", "localhost", "-U", "missing"]).await.unwrap());
}

#[fixture]
fn test_env(containers_bound_fixture: ContainersBoundFixture) -> TestEnv {
    TestEnv::new(containers_bound_fixture)
}

struct TestEnv {
    postgread_server_handle: Option<task::JoinHandle<io::Result<()>>>,
    port_forwarder: SshPortForwarder,
    _containers_bound_fixture: ContainersBoundFixture,
}

impl TestEnv {
    fn new(containers_bound_fixture: ContainersBoundFixture) -> Self {
        let test_context = &containers_bound_fixture.test_context;
        let containers_ports = &test_context.containers_ports;
        let server = task::block_on(start_server(containers_ports.pg_server))
            .expect("could not start postgread server");
        let server_port = server.get_listen_port()
            .expect("could not get port listened by postgread server");
        let server_handle = task::spawn(server::loop_accepting(server, Arc::new(|_: Message| {})));
        let port_fwd_config = ssh_port_forwarder::Config {
            server_target_port: server_port,
            client_ssh_port: containers_ports.test_client,
            client_ssh_priv_key_path: &test_context.test_client_ssh_priv_key_path,
            client_forwarding_port: PG_DEFAULT_PORT,
        };
        let port_forwarder = SshPortForwarder::start(&port_fwd_config)
            .expect("could not start port forwarder (test client container -> postgread)");
        Self {
            postgread_server_handle: Some(server_handle),
            port_forwarder,
            _containers_bound_fixture: containers_bound_fixture,
        }
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
        env::set_var("TZ", "UTC");  // as inside containers
        let Containers { ports, logs_watchers } = containers::create_or_start_all(&ssh_keys.pub_key_content)?;
        let context = ContainersTestContext {
            containers_ports: ports,
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
    test_client_ssh_priv_key_path: String,
}

struct SshKeys {
    priv_key_path: String,
    pub_key_content: String,
}

impl ContainersGlobalFixture {
    const SSH_PRIV_KEY_PATH_ENVAR: &'static str = "POSTGREAD_TEST_CLIENT_SSH_PRIV_KEY_PATH";

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
