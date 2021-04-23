use ::bollard::Docker as DockerImpl;
use ::bollard::container::{*, Config as ContainerConfig};
use ::bollard::exec::{*, StartExecResults::*};
use ::bollard::errors::Error as DockerError;
use ::bollard::models::*;
use ::chrono::{DateTime, Utc};
use ::futures::{Stream, StreamExt, stream::TryStreamExt};
use ::std::collections::{HashMap, HashSet};
use ::std::future::Future;
use ::std::time::Duration;

pub struct Docker {
    docker_impl: DockerImpl,
}

#[derive(Debug)]
pub enum SimpleContainerState {
    Missing,
    Running,
    Stopped,
}

#[derive(Debug)]
pub enum ContainerReadiness {
    NotRunning(String),
    RunningNotHealthy(String),
    HealthyWithoutPort(String),
    ReadyWithPort(u16),
}

#[derive(Debug)]
pub struct ExecOutcome {
    pub exit_code: i64,
    pub std_out_err: Vec<String>,
}

impl Docker {
    pub fn connect() -> Result<Self, String> {
        let docker = DockerImpl::connect_with_local_defaults().map_err(|e| format!("{:?}", e))?;
        Ok(Self { docker_impl: docker })
    }

    pub async fn create(
        &self,
        container_name: &str,
        image: &str,
        run_cmd: Vec<&str>,
        published_ports: &[&str],
        env: HashMap<&str, &str>,
        health_cmd: &[&str],
        health_cmd_timeout: Duration,
    ) -> Result<ContainerCreateResponse, String> {
        let create_container_options = CreateContainerOptions {
            name: container_name,
        };
        let published_ports: HashMap<_, _> = published_ports.iter().map(|port| (*port, hashmap!())).collect();
        let env: Vec<String> = env.iter().map(|(name,value)| format!("{}={}", name, value)).collect();
        let env: Vec<&str> = env.iter().map(|s| s.as_str()).collect();
        let container_config = ContainerConfig {
            exposed_ports: Some(published_ports),
            image: Some(image),
            cmd: Some(run_cmd),
            env: Some(env),
            host_config: Some(HostConfig {
                publish_all_ports: Some(true),
                ..Default::default()
            }),
            healthcheck: Some(build_health_config(health_cmd, health_cmd_timeout)),
            ..Default::default()
        };
        let fut = self.docker_impl.create_container(Some(create_container_options), container_config);
        map_docker_err(fut).await
    }

    pub async fn start(&self, container_name: &str) -> Result<(), String> {
        let fut = self.docker_impl.start_container(container_name, None::<StartContainerOptions<String>>);
        map_docker_err(fut).await
    }

    pub async fn find(&self, container_name: &str) -> Result<SimpleContainerState, String> {
        let query = ListContainersOptions {
            all: true,  // including stopped
            filters: hashmap!{"name" => vec![container_name]},
            ..Default::default()
        };
        let fut = self.docker_impl.list_containers(Some(query));
        match map_docker_err(fut).await?.as_slice() {
            [] => Ok(SimpleContainerState::Missing),
            [container] => {
                let state = container.state.as_ref().ok_or("no state in container".to_owned())?;
                match state.as_str() {
                    "exited" => Ok(SimpleContainerState::Stopped),
                    "running" => Ok(SimpleContainerState::Running),
                    _ => Err(format!("unknown state {} with status {:?}", state, container.status)),
                }
            },
            containers => {
                let containers_ids: Vec<_> = containers.iter().map(|c| &c.id).collect();
                Err(format!("found more than one container: {:?}", containers_ids))
            },
        }
    }

    pub async fn stream_logs(&self, container_name: &str, since: DateTime<Utc>) -> impl Stream<Item=Result<String, String>> {
        let options = LogsOptions::<&str> {
            follow: true,
            stdout: true,
            stderr: true,
            since: since.timestamp(),
            ..Default::default()
        };
        let stream = self.docker_impl.logs(container_name, Some(options));
        stream.filter_map(|item| async move { match item {
            Ok(log) => pick_stdout_stderr(log).map(Ok),
            Err(err) => Some(Err(err.to_string())),
        }})
    }

    pub async fn exec(&self, container_name: &str, command: Vec<&str>) -> Result<ExecOutcome, String> {
        let create_options = CreateExecOptions {
            cmd: Some(command),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        let docker = &self.docker_impl;
        let create_fut = docker.create_exec(container_name, create_options);
        let CreateExecResults { id: exec_id } = map_docker_err(create_fut).await?;
        let start_stream = docker.start_exec(&exec_id, None::<StartExecOptions>);
        let std_out_err = extract_logs(start_stream).await?;
        let inspect_fut = docker.inspect_exec(&exec_id);
        let ExecInspectResponse { exit_code, .. } = map_docker_err(inspect_fut).await?;
        if let Some(exit_code) = exit_code {
            return Ok(ExecOutcome { std_out_err, exit_code })
        }
        return Err("attached command finished without exit code".to_owned())
    }

    pub async fn inspect_readiness(&self, container_name: &str, exposed_port: &str) -> Result<ContainerReadiness, String> {
        let fut = self.docker_impl.inspect_container(container_name, None::<InspectContainerOptions>);
        let container_info = map_docker_err(fut).await?;
        let state = container_info.state.as_ref().ok_or("inspect response misses .state")?;
        let state_status = state.status.ok_or("inspect response misses .state.status")?;
        use ContainerReadiness::*;
        if state_status != ContainerStateStatusEnum::RUNNING {
            return Ok(NotRunning(format!("container is {}", state_status)))
        }
        if let Err(unhealthy) = check_container_healthy(state) {
            return Ok(RunningNotHealthy(unhealthy));
        }
        match pick_host_bound_port(&container_info, exposed_port) {
            Ok(port) => Ok(ReadyWithPort(port)),
            Err(no_port) => Ok(HealthyWithoutPort(no_port)),
        }
    }

    pub async fn stop(&self, container_name: &str) -> Result<(), String> {
        let fut = self.docker_impl.stop_container(container_name, None::<StopContainerOptions>);
        map_docker_err(fut).await
    }

    pub async fn remove(&self, container_name: &str) -> Result<(), String> {
        let fut = self.docker_impl.remove_container(container_name, None::<RemoveContainerOptions>);
        map_docker_err(fut).await
    }
}

fn build_health_config(cmd: &[&str], timeout: Duration) -> HealthConfig {
    const NON_SHELL_PREFIX: &[&str] = &["CMD"];
    let cmd = NON_SHELL_PREFIX.iter().chain(cmd.iter()).map(|s| (*s).to_owned()).collect();
    HealthConfig {
        test: Some(cmd),
        start_period: Some(timeout.as_nanos() as i64),
        interval: Some(timeout.as_nanos() as i64),
        timeout: Some(timeout.as_nanos() as i64),
        ..Default::default()
    }
}

fn check_container_healthy(state: &ContainerState) -> Result<(), String> {
    let health = state.health.as_ref().ok_or("inspect response misses .state.health")?;
    let health_status = health.status.ok_or("inspect response misses .state.health.status")?;
    if health_status == HealthStatusEnum::HEALTHY {
        return Ok(())
    }
    let health_last_log = health.log.as_ref()
        .and_then(|logs| logs.last())
        .and_then(|last| last.output.as_ref());
    Err(format!("container health is {}, last its log is {:?}", health_status, health_last_log))
}

fn pick_host_bound_port(container_info: &ContainerInspectResponse, exposed_port: &str) -> Result<u16, String> {
    let bindings = container_info.network_settings.as_ref()
        .ok_or("inspect response misses .state.network_settings")?
        .ports.as_ref()
        .ok_or("inspect response misses .state.network_settings.ports")?
        .get(exposed_port)
        .and_then(|opt| opt.as_ref())  // flatten Option<&Option<T>> to Option<&T>
        .ok_or(format!("inspect response misses .state.network_settings.ports[{}]", exposed_port))?
        .as_slice();
    let host_ports = bindings.iter()
        .filter_map(|b| b.host_port.as_ref())
        .collect::<HashSet<_>>().into_iter()
        .collect::<Vec<_>>();
    match host_ports.as_slice() {
        [] => {
            Err(format!(".state.network_settings.ports[{}] is either missing or has no bindings or has no host_port inside", exposed_port))
        },
        [host_port] => {
            let host_port = host_port.parse::<u16>()
                .map_err(|e| format!(".state.network_settings.ports[{}].host_port is not a number: {}", exposed_port, e))?;
            Ok(host_port)
        },
        _ => {
            Err(format!(".state.network_settings.ports[{}] has several bindings: {:?}", exposed_port, bindings))
        },
    }
}

async fn extract_logs(
    start_results: impl Stream<Item=Result<StartExecResults, DockerError>>
) -> Result<Vec<String>, String> {
    let log_results = start_results.try_filter_map(|start_data| async move {
        match start_data {
            Attached { log } => Ok(pick_stdout_stderr(log)),
            Detached => Ok(None),
        }
    });
    log_results.try_collect().await.map_err(|e| e.to_string())
}

fn pick_stdout_stderr(log: LogOutput) -> Option<String> {
    match log {
        LogOutput::StdErr { message } |
        LogOutput::StdOut { message } =>
            Some(String::from_utf8_lossy(message.as_ref()).into_owned()),
        LogOutput::StdIn { .. } |
        LogOutput::Console { .. } =>
            None,
    }
}

async fn map_docker_err<T, Fut>(fut: Fut) -> Result<T, String>
    where Fut: Future<Output=Result<T, DockerError>> {
    fut.await.map_err(|e| format!("{}", e.to_string()))
}
