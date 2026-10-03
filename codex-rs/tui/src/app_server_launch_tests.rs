use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[cfg(unix)]
#[tokio::test]
async fn default_launch_uses_embedded_backend_with_a_live_daemon_socket() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let socket_path = codex_app_server_client::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(socket_path.as_path().parent().expect("socket parent"))?;
    let listener = tokio::net::UnixListener::bind(socket_path.as_path())?;

    assert_eq!(
        app_server_target_for_launch(
            /*explicit_remote_endpoint*/ None,
            codex_home.path(),
            /*workload_identity_selected*/ false,
        )?,
        AppServerTarget::Embedded
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(/*millis*/ 20),
            listener.accept(),
        )
        .await
        .is_err()
    );
    Ok(())
}

#[test]
fn explicit_default_socket_selects_shared_local_backend() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: codex_app_server_client::app_server_control_socket_path(codex_home.path())?,
    };
    assert_eq!(
        app_server_target_for_launch(
            Some(endpoint.clone()),
            codex_home.path(),
            /*workload_identity_selected*/ false,
        )?,
        AppServerTarget::LocalDaemon { endpoint }
    );
    Ok(())
}

#[test]
fn explicit_remote_socket_keeps_remote_workspace_semantics() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::from_absolute_path(codex_home.path().join("explicit.sock"))?,
    };
    assert_eq!(
        app_server_target_for_launch(
            Some(endpoint.clone()),
            codex_home.path(),
            /*workload_identity_selected*/ false,
        )?,
        AppServerTarget::Remote { endpoint }
    );
    Ok(())
}

#[test]
fn workload_identity_requires_embedded_backend() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    assert_eq!(
        app_server_target_for_launch(
            /*explicit_remote_endpoint*/ None,
            codex_home.path(),
            /*workload_identity_selected*/ true,
        )?,
        AppServerTarget::Embedded
    );
    let endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: codex_app_server_client::app_server_control_socket_path(codex_home.path())?,
    };
    let error = app_server_target_for_launch(
        Some(endpoint),
        codex_home.path(),
        /*workload_identity_selected*/ true,
    )
    .expect_err("remote hosts must own workload identity");
    assert_eq!(
        error.to_string(),
        "workload identity must be configured on the remote app-server host"
    );
    Ok(())
}
