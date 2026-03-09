use std::sync::Arc;

use anyhow::{Context, Result};
use russh::client;
use tokio::process::Command;
use tokio::sync::oneshot;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use super::client::{ClientHandler, ConnectParams, FingerprintRequest};

/// Connect to an SSH server via a ProxyCommand.
///
/// The proxy command is executed as a shell subprocess, and its stdin/stdout
/// are used as the SSH transport (via `russh::client::connect_stream`).
pub async fn connect_via_proxy(
    proxy_cmd: &str,
    params: &ConnectParams,
    fp_tx: Option<oneshot::Sender<FingerprintRequest>>,
) -> Result<client::Handle<ClientHandler>> {
    // Expand %h and %p placeholders
    let expanded = proxy_cmd
        .replace("%h", &params.host)
        .replace("%p", &params.port.to_string());

    log::debug!("ProxyCommand: {:?}", expanded);

    // Spawn the proxy command via the platform shell.
    //
    // On Unix we use `sh -lc` (login shell) so that PATH and other
    // environment variables are properly initialised.  When the app is
    // launched from a desktop environment (e.g. macOS Finder / Linux
    // desktop entry) the inherited environment is minimal and may not
    // include directories like /usr/bin, causing `ssh` and similar tools
    // to be missing from PATH.
    //
    // On Windows `cmd /C` inherits the full system environment, so no
    // special handling is needed.
    #[cfg(unix)]
    let child = Command::new("sh")
        .arg("-lc")
        .arg(&expanded)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("Failed to spawn ProxyCommand")?;
    #[cfg(windows)]
    let child = Command::new("cmd")
        .arg("/C")
        .arg(&expanded)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        // Prevent the subprocess from creating a new console window on Windows.
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .context("Failed to spawn ProxyCommand")?;

    let stdin = child.stdin.unwrap();
    let stdout = child.stdout.unwrap();

    // Combine stdin/stdout into a single async stream
    let stream = tokio::io::join(stdout, stdin);

    let config = Arc::new(client::Config::default());
    let handler = ClientHandler::new(params.host.clone(), params.port, fp_tx);

    let handle = client::connect_stream(config, stream, handler)
        .await
        .context("Failed to connect via ProxyCommand")?;

    Ok(handle)
}
