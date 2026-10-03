//! Selects the fork's own backend unless the caller explicitly supplies `--remote`.

use crate::AppServerTarget;
use crate::RemoteAppServerEndpoint;
use std::path::Path;

pub(super) fn app_server_target_for_launch(
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
    codex_home: &Path,
    workload_identity_selected: bool,
) -> std::io::Result<AppServerTarget> {
    if workload_identity_selected {
        if explicit_remote_endpoint.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "workload identity must be configured on the remote app-server host",
            ));
        }
        return Ok(AppServerTarget::Embedded);
    }

    Ok(match explicit_remote_endpoint {
        Some(endpoint) => {
            // An explicitly selected shared local server keeps local workspace semantics.
            // Comparing paths does not probe or implicitly connect to the socket.
            if let RemoteAppServerEndpoint::UnixSocket { socket_path } = &endpoint
                && codex_app_server_client::app_server_control_socket_path(codex_home)
                    .is_ok_and(|default_socket| default_socket == *socket_path)
            {
                AppServerTarget::LocalDaemon { endpoint }
            } else {
                AppServerTarget::Remote { endpoint }
            }
        }
        None => AppServerTarget::Embedded,
    })
}

#[cfg(test)]
#[path = "app_server_launch_tests.rs"]
mod tests;
