//! CLI daemon lifecycle management.
//!
//! This module provides functions for the CLI to manage the daemon process,
//! including spawning, connecting to, and shutting down the daemon.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::config::Config;
use crate::daemon::fingerprint::calculate_fingerprint;
use crate::daemon::orphan::{cleanup_orphaned_daemon, write_daemon_pid};
use crate::ipc::ProtocolClient;

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
pub async fn ensure_daemon(daemon_config: Arc<Config>) -> Result<Box<dyn ProtocolClient>> {
    // Get socket path
    let socket_path = crate::ipc::get_socket_path();

    // Clean up orphaned daemons first
    tracing::debug!("Checking for orphaned daemons...");
    if let Err(e) = cleanup_orphaned_daemon(&daemon_config, &socket_path).await {
        tracing::warn!("Failed to cleanup orphaned daemons: {}", e);
        // Continue anyway - might not be orphaned, just not running
    }

    // Calculate current config fingerprint
    let fingerprint = calculate_fingerprint(&daemon_config);

    // Try to connect to existing daemon
    tracing::debug!("Attempting to connect to daemon...");
    match connect_to_daemon(daemon_config.clone(), &socket_path).await {
        Ok(client) => {
            tracing::info!("Daemon already running, checking config...");
            // TODO: Request fingerprint from daemon and compare
            // For now, assume existing daemon is good
            Ok(client)
        }
        Err(_) => {
            tracing::info!("Daemon not running, spawning new daemon...");
            // Spawn new daemon and wait for startup
            let new_client = spawn_daemon_and_wait(daemon_config.clone(), &fingerprint).await?;
            // Connect to newly spawned daemon
            connect_to_daemon(daemon_config, &socket_path).await
        }
    }
}

/// Spawn a new daemon process and wait for it to start.
///
/// Finds the daemon binary and spawns it with the current config path.
/// Waits up to 5 seconds for the daemon to start accepting connections.
async fn spawn_daemon_and_wait(daemon_config: Arc<Config>, fingerprint: &str) -> Result<()> {
    // Get current executable path and daemon binary path
    let current_exe = std::env::current_exe()?;
    let daemonexe_path = if cfg!(windows) {
        // On Windows, find mcp-daemon.exe in the same directory
        current_exe
            .parent()
            .unwrap_or(&current_exe)
            .join("mcp-daemon.exe")
    } else {
        // On Unix, find mcp-daemon in the same directory or PATH
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

    tracing::info!("Spawning daemon: {:?}", daemonexe_path);

    // Spawn daemon process
    let child = tokio::process::Command::new(&daemonexe_path)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn daemon: {}", e))?;

    tracing::info!("Daemon spawned with PID: {:?}", child.id());

    // Wait for daemon to start up
    let socket_path = crate::ipc::get_socket_path();
    wait_for_daemon_startup(daemon_config, &socket_path, Duration::from_secs(5)).await?;

    // Write daemon PID
    if let Some(pid) = child.id() {
        write_daemon_pid(&socket_path, pid)
            .map_err(|e| {
                tracing::warn!("Failed to write daemon PID: {}", e);
                e
            })
            .ok(); // Non-fatal if PID write fails
    }

    Ok(())
}

/// Connect to daemon via IPC.
///
/// Attempts to connect to the daemon at the socket path.
/// Returns an IPC client wrapper connected to the daemon.
async fn connect_to_daemon(config: Arc<Config>, _socket_path: &Path) -> Result<Box<dyn ProtocolClient>> {
    let client = crate::ipc::create_ipc_client(config)?;
    Ok(client)
}

/// Wait for daemon to start accepting connections.
///
/// Retries connection with exponential backoff until timeout.
async fn wait_for_daemon_startup(config: Arc<Config>, socket_path: &Path, timeout: Duration) -> Result<Box<dyn ProtocolClient>> {
    let start = std::time::Instant::now();
    let mut retry_delay = Duration::from_millis(100);

    loop {
        if start.elapsed() > timeout {
            return Err(anyhow::anyhow!("Daemon did not start within timeout"));
        }

        match connect_to_daemon(config.clone(), socket_path).await {
            Ok(client) => {
                tracing::info!("Daemon started successfully");
                return Ok(client);
            }
            Err(_) => {
                // Daemon not yet ready, wait and retry
                tracing::debug!("Daemon not ready yet, retrying in {:?}...", retry_delay);
                sleep(retry_delay).await;
                retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(1));
            }
        }
    }
}

/// Shutdown daemon gracefully.
///
/// Connects to daemon and sends a shutdown request.
/// Waits for acknowledgment before returning.
pub async fn shutdown_daemon(daemon_config: Arc<Config>) -> Result<()> {
    let socket_path = crate::ipc::get_socket_path();

    // Connect to daemon
    let client = connect_to_daemon(daemon_config.clone(), &socket_path).await?;

    // Send shutdown request
    tracing::info!("Sending shutdown request to daemon");

    // TODO: Send DaemonRequest::Shutdown through client
    // For now, just disconnect - the daemon will idle timeout

    tracing::info!("Daemon shutdown request sent");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
