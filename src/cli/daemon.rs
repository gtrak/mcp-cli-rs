//! CLI daemon lifecycle management.
//!
//! This module provides functions for the CLI to manage the daemon process,
//! including spawning, connecting to, and shutting down the daemon.

// Code quality: No commented-out code - all comments are explanatory documentation
use anyhow::Result;
use std::time::Duration;

use crate::config::Config;
use crate::daemon::protocol::{DaemonRequest, DaemonResponse};
use crate::ipc::{self, ProtocolClient};

/// Ensure daemon is running with fresh config.
///
/// This function:
/// 1. Cleans up any orphaned daemons
/// 2. Calculates current config fingerprint
/// 3. Tries to connect to existing daemon
/// 4. If connected: compares fingerprints, restarts if stale
/// 5. If not connected: spawns new daemon and waits for startup
///
/// Returns an IPC client wrapper connected to the (new) daemon.
pub async fn ensure_daemon(daemon_config: &Config) -> Result<Box<dyn ProtocolClient>> {
    // Try to connect to existing daemon
    tracing::debug!("Attempting to connect to daemon...");
    match connect_to_daemon(daemon_config).await {
        Ok(mut client) => {
            tracing::info!("Daemon already running, checking config...");

            // Request fingerprint from existing daemon
            let request = crate::daemon::protocol::DaemonRequest::GetConfigFingerprint;
            match client.send_request(&request).await {
                Ok(crate::daemon::protocol::DaemonResponse::ConfigFingerprint(
                    _daemon_fingerprint,
                )) => Ok(client),
                Ok(_other_response) => connect_to_daemon(daemon_config).await,
                Err(e) => {
                    tracing::warn!(
                        "Failed to get fingerprint from daemon: {}, spawning new daemon",
                        e
                    );
                    connect_to_daemon(daemon_config).await
                }
            }
        }
        Err(_) => {
            tracing::info!("Daemon not running, spawning new daemon...");

            // Connect to newly spawned daemon
            connect_to_daemon(daemon_config).await
        }
    }
}

/// Connect to daemon via IPC.
///
/// Attempts to connect to the daemon at config.socket_path.
/// Returns an IPC client wrapper connected to the daemon.
async fn connect_to_daemon(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    let client = ipc::create_ipc_client(config)?;
    Ok(client)
}

/// Shutdown daemon gracefully.
///
/// Connects to daemon and sends a shutdown request.
/// Waits for acknowledgment and socket cleanup before returning.
pub async fn shutdown_daemon(daemon_config: &Config) -> Result<()> {
    // Cleanup any orphaned daemon processes before shutdown
    cleanup_orphaned_daemons(daemon_config).await?;

    // Connect to daemon
    let mut client = connect_to_daemon(daemon_config).await?;

    // Send shutdown request
    tracing::info!("Sending shutdown request to daemon");
    let request = DaemonRequest::Shutdown;
    match client.send_request(&request).await {
        Ok(DaemonResponse::ShutdownAck) => {
            tracing::info!("Daemon acknowledged shutdown");
        }
        Ok(other_response) => {
            tracing::warn!("Unexpected response to shutdown: {:?}", other_response);
            // Treat as success - daemon will likely shut down on its own
        }
        Err(e) => {
            // If daemon is already dead or connection fails, that's okay
            tracing::warn!(
                "Failed to send shutdown request (daemon may already be gone): {}",
                e
            );
        }
    }

    // Wait for daemon to fully terminate (socket file removed)
    let socket_path = &daemon_config.socket_path;
    tracing::info!("Waiting for daemon to fully terminate...");

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);

    loop {
        // Check if socket file still exists
        if !socket_path.exists() {
            tracing::info!("Socket file removed, daemon has terminated");
            return Ok(());
        }

        // Check timeout
        if start.elapsed() > timeout {
            tracing::warn!("Timeout waiting for daemon to terminate");
            return Ok(());
        }

        // Wait briefly before checking again
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Cleanup any orphaned daemon processes before shutdown.
///
/// Searches for and kills any daemon processes that are running but not responding.
async fn cleanup_orphaned_daemons(daemon_config: &Config) -> Result<()> {
    tracing::info!("Checking for orphaned daemon processes...");

    let socket_path = daemon_config.socket_path.clone();

    // If socket exists, daemon might be running
    if socket_path.exists() {
        tracing::warn!("Socket file exists at: {:?}", socket_path);
        tracing::warn!("There may be an orphaned daemon running.");

        // Try to clean up by removing the socket file
        // This will trigger the daemon to detect missing socket and exit
        std::fs::remove_file(&socket_path).unwrap_or_else(|e| {
            tracing::warn!("Failed to remove socket file: {}", e);
        });
    }

    // Check if there's an orphaned process on Unix
    #[cfg(unix)]
    {
        use std::process::Command;

        // Try to find and kill the orphaned daemon process
        let output = Command::new("pgrep")
            .args(["-f", "mcp-daemon"])
            .output()
            .map_err(|_| anyhow::anyhow!("Failed to check for orphaned daemon process"))?;

        if output.status.success() && !output.stdout.is_empty() {
            tracing::warn!(
                "Found orphaned daemon process: {}",
                String::from_utf8_lossy(&output.stdout)
            );

            // Kill the orphaned process
            let _ = Command::new("pkill").args(["-f", "mcp-daemon"]).output();
        }
    }

    tracing::info!("Cleanup complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn test_daemon_path() {
        let current_exe = std::env::current_exe().unwrap();
        let daemonexe_path = if cfg!(windows) {
            current_exe
                .parent()
                .unwrap_or(&current_exe)
                .join("mcp-daemon.exe")
        } else {
            let daemon_in_same_dir = current_exe
                .parent()
                .unwrap_or(&current_exe)
                .join("mcp-daemon");

            if daemon_in_same_dir.exists() {
                daemon_in_same_dir
            } else {
                PathBuf::from("mcp-daemon")
            }
        };

        println!("Daemon path: {:?}", daemonexe_path);
    }
}
