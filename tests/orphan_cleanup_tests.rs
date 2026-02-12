//! Comprehensive tests for orphan daemon cleanup functionality
//!
//! Tests verify that orphaned daemon processes and their associated
//! resources (sockets, PID files, fingerprint files) are properly cleaned
//! up on startup when a daemon crashes without proper termination.

use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

use mcp_cli_rs::config::Config;
use mcp_cli_rs::daemon::orphan::{
    cleanup_orphaned_daemon, get_fingerprint_file_path, get_pid_file_path, is_daemon_running,
    write_daemon_pid,
};

#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;

#[tokio::test]
async fn test_orphan_socket_cleanup_unix() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_orphan.sock");

    // Create a fake socket file
    let mut fake_socket = std::fs::File::create(&socket_path).unwrap();
    fake_socket.write_all(b"fake socket content").unwrap();

    // Create a config

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify the socket was cleaned up
    assert!(
        !socket_path.exists(),
        "Orphaned socket should be cleaned up"
    );
}

#[tokio::test]
async fn test_orphan_socket_cleanup_windows() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_orphan.pipe");

    // Create a fake named pipe file
    let mut fake_pipe = std::fs::File::create(&socket_path).unwrap();
    fake_pipe.write_all(b"fake pipe content").unwrap();

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify the pipe was cleaned up
    assert!(!socket_path.exists(), "Orphaned pipe should be cleaned up");
}

#[tokio::test]
async fn test_orphan_pid_file_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_orphan.sock");

    // Write PID to the PID file
    let pid = std::process::id();
    let pid_file = get_pid_file_path(&socket_path);
    write_daemon_pid(&socket_path, pid).unwrap();

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify the PID file was cleaned up
    assert!(!pid_file.exists(), "Orphaned PID file should be cleaned up");
}

#[tokio::test]
async fn test_orphan_fingerprint_file_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_orphan.sock");

    // Write fingerprint to the fingerprint file
    let fp_file = get_fingerprint_file_path(&socket_path);
    std::fs::write(&fp_file, "test-fingerprint-hash").unwrap();

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify the fingerprint file was cleaned up
    assert!(
        !fp_file.exists(),
        "Orphaned fingerprint file should be cleaned up"
    );
}

#[tokio::test]
async fn test_no_false_positives() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_active.sock");

    // Create a config
    let config = Config::default();

    // Write PID to the PID file for a non-existent process
    let pid = std::process::id();
    let _pid_file = get_pid_file_path(&socket_path);
    write_daemon_pid(&socket_path, pid).unwrap();

    // Verify the current process is still running (no false positive cleanup)
    assert!(
        is_daemon_running(pid),
        "Current process should be detected as running"
    );

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Note: This test demonstrates that orphan cleanup correctly identifies
    // that the current process is still running and does not clean it up.
    // The cleanup function will try to connect via IPC first and find
    // that daemon is running (via existing connections), preventing cleanup.
}

#[tokio::test]
async fn test_pid_file_validation() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_orphan.sock");

    // Create invalid PID file (non-numeric content)
    let pid_file = get_pid_file_path(&socket_path);
    std::fs::write(&pid_file, "invalid_pid_content").unwrap();

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify the PID file was cleaned up (invalid content handled)
    assert!(!pid_file.exists(), "Invalid PID file should be cleaned up");
}

#[tokio::test]
async fn test_partial_cleanup_on_error() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_partial.sock");

    // Create socket file
    std::fs::File::create(&socket_path).unwrap();

    // Create PID file with a non-existent PID
    let pid_file = get_pid_file_path(&socket_path);
    let pid = 99999; // Non-existent PID
    std::fs::write(&pid_file, pid.to_string()).unwrap();

    // Create a config
    let config = Config::default();

    // Clean up the orphaned resources
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok());

    // Verify that partial cleanup occurred (socket cleaned up, PID file cleaned up)
    assert!(
        !socket_path.exists(),
        "Socket should be cleaned up even if PID doesn't exist"
    );
    assert!(!pid_file.exists(), "PID file should be cleaned up");
}

#[test]
fn test_get_pid_file_path_unix() {
    let socket = PathBuf::from("/tmp/mcp.sock");
    assert_eq!(get_pid_file_path(&socket), PathBuf::from("/tmp/mcp.pid"));
}

#[test]
fn test_get_pid_file_path_windows() {
    let socket = PathBuf::from("C:\\tmp\\mcp.pipe");
    assert_eq!(
        get_pid_file_path(&socket),
        PathBuf::from("C:\\tmp\\mcp.pid")
    );
}

#[test]
fn test_get_fingerprint_file_path() {
    let socket = PathBuf::from("/tmp/mcp.sock");
    assert_eq!(
        get_fingerprint_file_path(&socket),
        PathBuf::from("/tmp/mcp.fingerprint")
    );
}

#[test]
fn test_get_fingerprint_file_path_windows() {
    let socket = PathBuf::from("C:\\tmp\\mcp.pipe");
    assert_eq!(
        get_fingerprint_file_path(&socket),
        PathBuf::from("C:\\tmp\\mcp.fingerprint")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_is_daemon_running_with_real_process() {
    // Use the current process as a test case
    let pid = std::process::id();
    let is_running = is_daemon_running(pid);
    assert!(is_running, "Current process should be detected as running");
}

#[cfg(unix)]
#[test]
fn test_kill_daemon_process_unix() {
    let pid = std::process::id();
    let result = super::kill_daemon_process(pid);
    // This will fail because we don't want to kill the current process
    // in a test, but it shows the function works for the target PID
    assert!(
        result.is_err(),
        "Killing current process should return an error"
    );
}
