use crate::common::container::{self, Started, Readiness};

use ::async_std::task::{self, JoinHandle};
use ::futures::join;
use ::std::env;
use ::std::time::Duration;

pub use crate::common::container::ExecOutcome;

#[derive(Debug)]
pub struct Containers {
    pub ports: Ports,
    pub logs_watchers: LogsWatchers,
}

#[derive(Clone, Debug)]
pub struct Ports {
    pub pg_server: u16,
    pub test_client: u16,
}

#[derive(Debug)]
pub struct LogsWatchers {
    pg_server: task::JoinHandle<()>,
    test_client: task::JoinHandle<()>,
}

const REUSE_ENVAR: &str = "POSTGREAD_TEST_REUSE_CONTAINERS";

fn read_reuse_envar() -> bool {
    env::var(REUSE_ENVAR).unwrap_or_default() == "1"
}

pub fn create_or_start_all(pg_server_postgres_passwd: &str, test_client_ssh_pub_key_content: &str) -> Result<Containers, String> {
    let reuse = read_reuse_envar();
    eprintln!("creating/starting all containers (reuse={})", reuse);
    let (pg_server, test_client) = task::block_on(async {
        join!(
            create_or_start_pg_server(reuse, pg_server_postgres_passwd),
            create_or_start_test_client(reuse, test_client_ssh_pub_key_content),
        )
    });
    let (pg_server, test_client) = (pg_server?, test_client?);
    use Readiness::ReadyWithPort;
    let pg_server_logs_watcher = pg_server.logs_watcher;
    let test_client_logs_watcher = test_client.logs_watcher;
    match (pg_server.readiness, test_client.readiness) {
        (ReadyWithPort(pg_server_port), ReadyWithPort(test_client_port)) => {
            Ok(Containers {
                ports: Ports {
                    pg_server: pg_server_port,
                    test_client: test_client_port,
                },
                logs_watchers: LogsWatchers {
                    pg_server: pg_server_logs_watcher,
                    test_client: test_client_logs_watcher,
                },
            })
        }
        some_not_ready => {
            let cleanup_err_or_empty = do_remove_or_stop_all(pg_server_logs_watcher, test_client_logs_watcher)
                .err().map_or("".to_owned(), |e| format!("and could not remove/stop them too: {}", e));
            Err(format!("PG server and/or test client are not ready: {:?}{}", some_not_ready, cleanup_err_or_empty))
        }
    }
}

pub fn remove_or_stop_all(logs_watchers: LogsWatchers) -> Result<(), String> {
    do_remove_or_stop_all(
        logs_watchers.pg_server,
        logs_watchers.test_client,
    )
}

pub fn do_remove_or_stop_all(
    pg_server_logs_watcher: JoinHandle<()>,
    test_client_logs_watcher: JoinHandle<()>,
) -> Result<(), String> {
    let reuse = read_reuse_envar();
    let (pg_server_stopped, test_client_stopped) = task::block_on(async {
        join!(
            container::remove_or_stop(PG_SERVER_CONTAINER_NAME, reuse),
            container::remove_or_stop(TEST_CLIENT_CONTAINER_NAME, reuse),
        )
    });
    pg_server_stopped.map_err(|e| format!("could not remove/stop PG server container: {}", e))?;
    test_client_stopped.map_err(|e| format!("could not remove/stop test client container: {}", e))?;
    eprintln!("removed/stopped all containers");
    task::block_on(async {
        join!(
            pg_server_logs_watcher,
            test_client_logs_watcher,
        )
    });
    Ok(())
}

pub async fn exec_on_test_client(cmd: Vec<&str>) -> Result<ExecOutcome, String> {
    container::exec(TEST_CLIENT_CONTAINER_NAME, cmd).await
}

async fn create_or_start_pg_server(reuse: bool, postgres_passwd: &str) -> Result<Started, String> {
    container::create_or_start(
        PG_SERVER_CONTAINER_NAME,
        reuse,
        "postgres:12-alpine",
        vec![],
        hashmap!["POSTGRES_PASSWORD" => postgres_passwd],
        concatcp!(PG_SERVER_EXPOSED_PORT, "/tcp"),
        &["pg_isready", "--quiet", "--timeout=0"],
        PG_SERVER_HEALTH_CMD_TIMEOUT,
        PG_SERVER_READINESS_CHECKS,
        print_pg_server_log,
    ).await
        .map_err(|e| format!("could not create/start PG server container: {}", e))
}

async fn create_or_start_test_client(reuse: bool, ssh_pub_key_content: &str) -> Result<Started, String> {
    container::create_or_start(
        TEST_CLIENT_CONTAINER_NAME,
        reuse,
        "postgread/test-client:2",
        vec![],
        hashmap![
            "POSTGREAD_TEST_CLIENT_SSH_PUB_KEY_CONTENT" => ssh_pub_key_content
        ],
        concatcp!(TEST_CLIENT_EXPOSED_PORT, "/tcp"),
        &["lsof", "-n", "-P", "-a", "-i", concatcp!("tcp:", TEST_CLIENT_EXPOSED_PORT), "-s", "tcp:listen"],
        TEST_CLIENT_HEALTH_CMD_TIMEOUT,
        TEST_CLIENT_READINESS_CHECKS,
        print_test_client_log,
    ).await
        .map_err(|e| format!("could not create/start test client container: {}", e))
}

fn print_pg_server_log(log: Result<String, String>) {
    match log {
        Ok(log) => print!("{}", log.replacen(" [", " pg_server[", 1)),
        Err(err) => eprint!("could not get a log of pg server container: {}", err),
    }
}

fn print_test_client_log(log: Result<String, String>) {
    match log {
        Ok(log) => print!("{}", log),
        Err(err) => eprint!("could not get a log of test client container: {}", err),
    }
}

const PG_SERVER_CONTAINER_NAME: &str = "postgread_test_pg_server";
const PG_SERVER_EXPOSED_PORT: u16 = 5432;
const PG_SERVER_HEALTH_CMD_TIMEOUT: Duration = Duration::from_secs(2);
const PG_SERVER_READINESS_CHECKS: u8 = 180;

const TEST_CLIENT_CONTAINER_NAME: &str = "postgread_test_client";
const TEST_CLIENT_EXPOSED_PORT: u16 = 22;
const TEST_CLIENT_HEALTH_CMD_TIMEOUT: Duration = Duration::from_secs(2);
const TEST_CLIENT_READINESS_CHECKS: u8 = 30;
