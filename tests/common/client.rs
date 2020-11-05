//use crate::common::docker;
//
//use docker::{ContainerManager, ContainerState, run_with_tokio};
//
//use ::std::env;
//use ::std::time::Duration;
//use ::std::process::Command;
//
//pub fn start_server() -> Result<u16, String> {
//    let reuse = read_envar_reuse_container();
//    let fut = create_or_start_container(reuse);
//    run_with_tokio(fut)
//}
//
//pub fn stop_server() -> Result<(), String> {
//    let reuse = read_envar_reuse_container();
//    let fut = remove_or_stop_container(reuse);
//    run_with_tokio(fut)
//}
//
//const REUSE_ENVAR: &str = "POSTGREAD_TEST_REUSE_CONTAINERS";
//
//fn read_envar_reuse_container() -> bool {
//    env::var(REUSE_ENVAR).unwrap_or_default() == "1"
//}
//
//async fn create_or_start_container(reuse_container: bool) -> Result<u16, String> {
//    let container_manager = ContainerManager::connect("postgread_test_client")?;
//    match container_manager.find().await? {
//        ContainerState::Missing => {
//            container_manager.create("alpine/socat", &[REVERSE_FORWARD_PORT]).await?;
//            start_existing_container(&container_manager).await
//        }
//        ContainerState::Running => {
//            Err("container is running, probably after previous test run failed, exiting...".into())
//        }
//        ContainerState::Stopped => {
//            if reuse_container {
//                start_existing_container(&container_manager).await
//            } else {
//                Err("container is stopped but should not be reused, exiting...".into())
//            }
//        }
//    }
//}
//
//async fn start_existing_container(
//    container_manager: &ContainerManager<'_>,
//) -> Result<u16, String> {
//    container_manager.start().await?;
//    let host_bound_port = container_manager.inspect_port_binding(REVERSE_FORWARD_PORT).await?;
//    Ok(host_bound_port)
//}
//
//async fn remove_or_stop_container(reuse_container: bool) -> Result<(), String> {
//    let container_manager = ContainerManager::connect("postgread_test_client")?;
//    container_manager.stop().await?;
//    if !reuse_container {
//        container_manager.remove().await?;
//    }
//    Ok(())
//}
//
//const REVERSE_FORWARD_PORT: &str = "4000/tcp";
