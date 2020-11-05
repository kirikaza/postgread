use crate::common::docker::{self, Docker, SimpleContainerState};

use ::async_std::task;
use ::chrono::offset::Utc;
use ::futures::StreamExt;
use ::std::collections::HashMap;
use ::std::time::Duration;

pub use crate::common::docker::ExecOutcome;

pub struct Started {
    pub readiness: Readiness,
    pub logs_watcher: task::JoinHandle<()>,
}

#[derive(Debug)]
pub enum Readiness {
    ReadyWithPort(u16),
    NotReady(String),
}

pub async fn create_or_start<HandleLog>(
    name: &str,
    reuse: bool,
    image: &str,
    command: Vec<&str>,
    env: HashMap<&str, &str>,
    published_port: &str,
    health_cmd: &[&str],
    health_cmd_timeout: Duration,
    readiness_checks: u8,
    handle_log: HandleLog,
) -> Result<Started, String>
where
    HandleLog: 'static + Fn(Result<String, String>) + Send
{
    let docker = Docker::connect()?;
    match docker.find(name).await? {
        SimpleContainerState::Missing => {
            docker.create(name, image, command, &[&published_port], env, health_cmd, health_cmd_timeout).await?;
            start_existing(&docker, name, published_port, health_cmd_timeout, readiness_checks, handle_log).await
        }
        SimpleContainerState::Running => {
            Err("container is running, probably after previous test run failed, exiting...".to_owned())
        }
        SimpleContainerState::Stopped => {
            if reuse {
                start_existing(&docker, name, published_port, health_cmd_timeout, readiness_checks, handle_log).await
            } else {
                Err("container is stopped but should not be reused, exiting...".to_owned())
            }
        }
    }
}

pub async fn remove_or_stop(name: &str, reuse: bool) -> Result<(), String> {
    let docker = Docker::connect()?;
    docker.stop(name).await?;
    if !reuse {
        docker.remove(name).await?;
    }
    Ok(())
}

pub async fn exec(name: &str, cmd: Vec<&str>) -> Result<ExecOutcome, String> {
    let docker = Docker::connect()?;
    docker.exec(name, cmd).await
}

async fn start_existing<HandleLog>(
    docker: &Docker,
    name: &str,
    published_port: &str,
    readiness_check_interval: Duration,
    readiness_checks_count: u8,
    handle_log: HandleLog,
) -> Result<Started, String>
where
    HandleLog: 'static + Fn(Result<String, String>) + Send
{
    let log_start = Utc::now();
    docker.start(name).await?;
    let mut logs_stream = docker.stream_logs(name, log_start).await.boxed();
    let logs_watcher = task::spawn(async move {
        while let Some(log) = logs_stream.next().await {
            handle_log(log)
        }
    });
    use Readiness::*;
    let mut unhealthy_err: String = "readiness has never been checked".to_owned();
    for _ in 0..readiness_checks_count {
        task::sleep(readiness_check_interval).await;
        let readiness_impl = docker.inspect_readiness(name, published_port).await?;
        use docker::ContainerReadiness as ReadinessImpl;
        match readiness_impl {
            ReadinessImpl::ReadyWithPort(port) => {
                return Ok(Started {
                    readiness: ReadyWithPort(port),
                    logs_watcher,
                })
            }
            ReadinessImpl::NotRunning(not_running) => {
                return Ok(Started {
                    readiness: NotReady(format!("container is not running: {}", not_running)),
                    logs_watcher,
                })
            }
            ReadinessImpl::HealthyWithoutPort(no_port) => {
                return Ok(Started {
                    readiness: NotReady(format!("container is healthy without port: {}", no_port)),
                    logs_watcher,
                })
            }
            ReadinessImpl::RunningNotHealthy(unhealthy) => {
                unhealthy_err = unhealthy
            }
        }
    }
    Ok(Started {
        readiness: NotReady(format!("container is still unhealthy: {}", unhealthy_err)),
        logs_watcher,
    })
}
