//! Comprehensive tests for orphan daemon cleanup functionality
//!
//! Tests verify that orphaned daemon processes and their associated
//! resources (sockets, PID files, fingerprint files) are properly cleaned
//! up on startup when a daemon crashes without proper termination.

use std::io::Write;
use std::path::PathBuf;

use mcp_cli_rs::config::Config;
use mcp_cli_rs::daemon::orphan::{
    cleanup_orphaned_daemon, get_fingerprint_file_path, get_pid_file_path, is_daemon_running,
    kill_daemon_process, write_daemon_pid,
};

#[cfg(test)]
mod helpers;

#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;

#[tokio::test]
async fn test_orphan_socket_cleanup_unix() {
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_orphan.sock");

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
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_orphan.pipe");

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
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_orphan.sock");

    // Write PID to the PID file - use a non-existent PID
    // (don't use current process ID as it would try to kill the test itself)
    let fake_pid = 99998;
    let pid_file = get_pid_file_path(&socket_path);
    std::fs::write(&pid_file, fake_pid.to_string()).unwrap();

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
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_orphan.sock");

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
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_active.sock");

    // Create a config
    let config = Config::default();

    // Write PID to the PID file for a non-existent process (not the current test process!)
    // Use a very high PID that definitely doesn't exist
    let fake_pid = 999999;
    let pid_file = get_pid_file_path(&socket_path);
    std::fs::write(&pid_file, fake_pid.to_string()).unwrap();

    // Verify the fake process is not running
    assert!(
        !is_daemon_running(fake_pid),
        "Fake process should not be detected as running"
    );

    // Clean up the orphaned resources
    // Since the socket doesn't exist and the PID doesn't exist, it should clean up everything
    let result: anyhow::Result<()> = cleanup_orphaned_daemon(&config, &socket_path).await;
    assert!(result.is_ok(), "Cleanup should succeed");
    
    // Verify the PID file was cleaned up (since the process doesn't exist)
    assert!(!pid_file.exists(), "PID file should be cleaned up when process doesn't exist");
}

#[tokio::test]
async fn test_pid_file_validation() {
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_orphan.sock");

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
    let env = helpers::TestEnvironment::new();
    let socket_path = env.path().join("test_partial.sock");

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
    // Spawn a child process that will sleep, then try to kill it
    use std::process::{Command, Stdio};
    
    let mut child = Command::new("sleep")
        .arg("10")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn test process");
    
    let pid = child.id();
    
    // Give the process a moment to start
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Verify the process is running
    assert!(is_daemon_running(pid), "Child process should be running");
    
    // Kill the process
    let result = kill_daemon_process(pid);
    assert!(result.is_ok(), "Killing child process should succeed");
    
    // Wait for the process to exit - need to reap the zombie process
    // is_daemon_running returns true for zombie processes until wait() is called
    let wait_result = child.wait();
    assert!(wait_result.is_ok(), "Waiting for child should succeed");
    
    // Verify the process is no longer running
    assert!(!is_daemon_running(pid), "Child process should be killed after SIGTERM");
}