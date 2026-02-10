//! Orphan daemon cleanup module.
//!
//! This module provides functionality to detect and clean up orphaned daemon
//! processes that crashed without proper cleanup. It tracks daemon PIDs in
//! PID files and removes stale resources when a daemon is no longer running.
//!
//! The cleanup logic runs at CLI startup to ensure stale daemons are removed
//! before attempting to start a new one.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

use crate::daemon::cleanup_socket;

/// Get the PID file path for a socket
///
/// The PID file is placed alongside the socket file with .pid extension.
pub fn get_pid_file_path(socket_path: &Path) -> PathBuf {
    let mut path = socket_path.to_path_buf();
    path.set_extension("pid");
    path
}

/// Get the fingerprint file path for a socket
///
/// The fingerprint file is placed alongside the socket file with .fingerprint extension.
pub fn get_fingerprint_file_path(socket_path: &Path) -> PathBuf {
    let mut path = socket_path.to_path_buf();
    path.set_extension("fingerprint");
    path
}

/// Check if a process with the given PID is running
///
/// Returns true if the process exists and is running.
/// Platform-specific implementation using native APIs.
#[cfg(unix)]
pub fn is_daemon_running(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    // Send signal 0 to check if process exists
    // If it succeeds, the process is running
    match kill(Pid::from_raw(pid as i32), Signal::SIGZERO) {
        Ok(()) => true,
        Err(_) => false,
    }
}

#[cfg(windows)]
pub fn is_daemon_running(pid: u32) -> bool {
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess};
    use windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION;

    const STILL_ACTIVE: u32 = 259;

    unsafe {
        // Open the process with query information rights
        let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);

        if process_handle.is_null() {
            // Process doesn't exist or we don't have access
            return false;
        }

        let mut exit_code: u32 = 0;
        let success = GetExitCodeProcess(process_handle, &mut exit_code);

        // Close the handle
        let _ = windows_sys::Win32::Foundation::CloseHandle(process_handle);

        if success == 0 {
            return false;
        }

        // If exit code is STILL_ACTIVE (259), process is running
        exit_code == STILL_ACTIVE
    }
}

/// Read PID from PID file
///
/// Returns the PID if file exists and contains a valid number.
/// Returns Err if file doesn't exist or parsing fails.
pub fn read_daemon_pid(socket_path: &Path) -> Result<u32> {
    let pid_file = get_pid_file_path(socket_path);

    if !pid_file.exists() {
        return Err(anyhow::anyhow!("PID file not found: {:?}", pid_file));
    }

    let content = fs::read_to_string(&pid_file)?;
    let pid: u32 = content.trim().parse()?;

    Ok(pid)
}

/// Write PID to PID file
///
/// Called by daemon on startup to register itself.
pub fn write_daemon_pid(socket_path: &Path, pid: u32) -> Result<()> {
    let pid_file = get_pid_file_path(socket_path);
    let pid_str = pid.to_string();

    fs::write(&pid_file, &pid_str)?;

    tracing::info!("PID file written: {:?} -> {}", pid_file, pid_str);

    Ok(())
}

/// Clean up orphaned daemon resources
///
/// This function:
/// 1. Tries to connect to daemon via IPC to check if it's running
/// 2. If connection fails: daemon is dead/crashed
/// 3. Removes stale socket file
/// 4. Reads PID file and checks if process exists
/// 5. If process exists but not responding: kills it
/// 6. Removes PID file and fingerprint file
///
/// Returns Ok(()) if cleanup successful.
/// Returns error if IPC check needed but daemon still running (no cleanup needed).
pub async fn cleanup_orphaned_daemon(daemon_config: &Config, socket_path: &Path) -> Result<()> {
    // Try to connect via IPC to check if daemon is running
    let ipc_result = try_connect_via_ipc(daemon_config, socket_path);

    if let Ok(client) = ipc_result {
        // Daemon is running, nothing to clean up
        tracing::info!("Daemon is running, no cleanup needed");
        return Ok(());
    }

    // Daemon is not responding, proceed with cleanup
    tracing::warn!("Daemon not responding, cleaning up orphaned resources");

    // Remove socket file (already handled by cleanup_socket, but double-check)
    let socket_path_clone = socket_path.to_path_buf();
    let _ = cleanup_socket(socket_path_clone).await;

    // Check if daemon PID file exists
    if !get_pid_file_path(socket_path).exists() {
        tracing::info!("No PID file found, cleanup complete");
        return Ok(());
    }

    // Read PID
    let pid = read_daemon_pid(socket_path)?;

    // Check if process is still running
    if is_daemon_running(pid) {
        tracing::warn!(
            "Daemon process still exists (PID: {}), attempting to kill",
            pid
        );
        kill_daemon_process(pid)?;
    }

    // Remove PID file
    let pid_file = get_pid_file_path(socket_path);
    if pid_file.exists() {
        if let Err(e) = fs::remove_file(&pid_file) {
            tracing::warn!("Failed to remove PID file: {}", e);
        }
    }

    // Remove fingerprint file
    let fingerprint_file = get_fingerprint_file_path(socket_path);
    if fingerprint_file.exists() {
        if let Err(e) = fs::remove_file(&fingerprint_file) {
            tracing::warn!("Failed to remove fingerprint file: {}", e);
        }
    }

    tracing::info!("Orphaned daemon cleanup complete");
    Ok(())
}

/// Remove PID file
///
/// Public helper for daemon shutdown cleanup
pub fn remove_pid_file(socket_path: &PathBuf) -> Result<()> {
    let pid_file = get_pid_file_path(socket_path);
    if pid_file.exists() {
        if let Err(e) = fs::remove_file(&pid_file) {
            tracing::warn!("Failed to remove PID file: {}", e);
        }
    }
    Ok(())
}

/// Remove fingerprint file
///
/// Public helper for daemon shutdown cleanup
pub fn remove_fingerprint_file(socket_path: &PathBuf) -> Result<()> {
    let fp_file = get_fingerprint_file_path(socket_path);
    if fp_file.exists() {
        if let Err(e) = fs::remove_file(&fp_file) {
            tracing::warn!("Failed to remove fingerprint file: {}", e);
        }
    }
    Ok(())
}

/// Try to connect to daemon via IPC
///
/// Returns Ok(()) if connection succeeds, Err otherwise.
fn try_connect_via_ipc(daemon_config: &Config, _socket_path: &Path) -> Result<()> {
    // Try to create IPC client to check if daemon is running
    // Note: This may fail if daemon is not running, which is expected
    use std::sync::Arc;
    let _client = crate::ipc::create_ipc_client(Arc::new(daemon_config.clone()))?;
    Ok(())
}

/// Kill a daemon process by PID
///
/// Sends SIGTERM (Unix) or calls TerminateProcess (Windows)
pub fn kill_daemon_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid as i32), Signal::SIGTERM)?;
        tracing::info!("Sent SIGTERM to daemon PID: {}", pid);
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess};
        use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;

        unsafe {
            let process_handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if process_handle.is_null() {
                return Err(anyhow::anyhow!(
                    "Failed to open process handle for PID: {}",
                    pid
                ));
            }

            let success = TerminateProcess(process_handle, 1);
            let _ = windows_sys::Win32::Foundation::CloseHandle(process_handle);

            if success == 0 {
                return Err(anyhow::anyhow!("Failed to terminate process PID: {}", pid));
            }

            tracing::info!("Terminated daemon process PID: {}", pid);
        }
    }

    Ok(())
}

/// Check if daemon is currently running
///
/// Returns true if daemon is alive and responding.
pub fn is_daemon_alive(socket_path: &Path) -> bool {
    let pid_result = read_daemon_pid(socket_path);

    if let Ok(pid) = pid_result {
        is_daemon_running(pid)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_file_path() {
        let socket = PathBuf::from("/tmp/mcp.sock");
        assert_eq!(get_pid_file_path(&socket), PathBuf::from("/tmp/mcp.pid"));

        let socket = PathBuf::from("C:\\tmp\\mcp.pipe");
        // set_extension replaces the extension, so mcp.pipe becomes mcp.pid
        assert_eq!(
            get_pid_file_path(&socket),
            PathBuf::from("C:\\tmp\\mcp.pid")
        );
    }

    #[test]
    fn test_fingerprint_file_path() {
        let socket = PathBuf::from("/tmp/mcp.sock");
        assert_eq!(
            get_fingerprint_file_path(&socket),
            PathBuf::from("/tmp/mcp.fingerprint")
        );
    }
}
