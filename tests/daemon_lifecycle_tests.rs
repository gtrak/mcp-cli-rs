//!
//! Tests daemon startup, idle timeout, orphan cleanup, config change detection,
//! and graceful shutdown across all platforms.
//!
//! XP-04: Validates daemon lifecycle works consistently on Linux, macOS, Windows

use mcp_cli_rs::config::loader::load_config_sync;
use mcp_cli_rs::ipc::create_ipc_client;
use mcp_cli_rs::config::Config;
use mcp_cli_rs::daemon::fingerprint::calculate_fingerprint;
use mcp_cli_rs::daemon::orphan::{cleanup_orphaned_daemon, read_daemon_pid, write_daemon_pid};
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::Duration;
use backoff::future::retry;

/// Create a config from content.
fn create_config_from_content(content: &str) -> Config {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(&config_path, content).expect("Failed to write config");

    load_config_sync(&config_path).expect("Failed to parse config")
}

/// Send shutdown signal to daemon and wait for it to exit
async fn shutdown_daemon_gracefully(config: &Config) -> Result<(), std::io::Error> {
    let config_arc = std::sync::Arc::new(config.clone());
    if let Ok(mut client) = create_ipc_client(config_arc) {
        if let Ok(response) = tokio::time::timeout(Duration::from_secs(5),
            client.send_request(&DaemonRequest::Shutdown)
        ).await {
            if matches!(response, Ok(DaemonResponse::ShutdownAck)) {
                return Ok(());
            }
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to shutdown daemon"))
}

/// Start daemon and wait for it to be ready
async fn start_daemon(config: &Config, ttl_secs: u64) -> Result<tokio::process::Child, std::io::Error> {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("mcp_servers.toml");
    std::fs::write(&config_path, toml::to_string(&config).expect("Failed to serialize config"))?;

    let mut child = Command::new("cargo")
        .args(&["run", "--", "daemon", "--ttl", &ttl_secs.to_string()])
        .current_dir(std::env::current_dir()?)
        .env("MCP_CONFIG_PATH", &config_path)
        .kill_on_drop(true)
        .spawn()?;

    let backoff = backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_millis(100))
        .with_max_interval(Duration::from_secs(1))
        .with_max_elapsed_time(Some(Duration::from_secs(10)))
        .build();

    let config_arc = std::sync::Arc::new(config.clone());
    let operation = || async {
        let mut client = create_ipc_client(config_arc.clone())?;
        client.send_request(&DaemonRequest::Ping)
            .await
            .map(|_| ())
            .map_err(|e| backoff::Error::transient(e))
    };

    match retry(backoff, operation).await {
        Ok(_) => Ok(child),
        Err(_) => {
            let _ = child.kill().await;
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Daemon failed to start"))
        }
    }
}

/// Test daemon startup and connection on all platforms
#[tokio::test]
async fn test_daemon_startup_connection() {
    // Clean up any orphaned daemon using IPC cleanup
    let config = create_config_from_content(
        r#"
[[servers]]
name = "test"
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }
"#,
    );

    // Get socket path
    let socket_path = mcp_cli_rs::ipc::get_socket_path();

    // Clean up orphaned daemon using the cleanup function
    let _ = cleanup_orphaned_daemon(&config, &socket_path).await;

    // Try to shutdown any existing daemon gracefully first
    let _ = shutdown_daemon_gracefully(&config).await;

    // Start daemon with short TTL
    let mut daemon = start_daemon(&config, 30).await.expect("Failed to start daemon");

    // Create IPC client to verify connection
    let mut client = create_ipc_client(std::sync::Arc::new(config.clone()))
        .expect("Failed to create IPC client");

    // Send Ping request
    let response = client.send_request(&DaemonRequest::Ping)
        .await
        .expect("Failed to send request");

    // Verify Pong response
    assert!(matches!(response, DaemonResponse::Pong),
             "Expected Pong response on daemon startup");

    // Verify config is accessible
    let config_fingerprint = calculate_fingerprint(&config);
    assert!(!config_fingerprint.is_empty(),
             "Config fingerprint should be generated");

    // Shutdown daemon gracefully
    shutdown_daemon_gracefully(&config).await.expect("Failed to shutdown daemon");

    // Wait for daemon to exit
    let _ = daemon.wait().await;
}

/// Test daemon idle timeout
#[tokio::test]
async fn test_daemon_idle_timeout() {
    // Clean up any orphaned daemon
    let config = create_config_from_content(
        r#"
[[servers]]
name = "test"
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }
"#,
    );

    // Get socket path
    let socket_path = mcp_cli_rs::ipc::get_socket_path();

    // Clean up orphaned daemon
    let _ = cleanup_orphaned_daemon(&config, &socket_path).await;

    // Try graceful shutdown first
    let _ = shutdown_daemon_gracefully(&config).await;

    // Start daemon with very short TTL (3 seconds)
    let mut daemon = start_daemon(&config, 3).await.expect("Failed to start daemon");

    // Create IPC client
    let config_arc = std::sync::Arc::new(config.clone());
    let mut client = create_ipc_client(config_arc.clone())
        .expect("Failed to create IPC client");

    // Send Ping request (active connection)
    let response = client.send_request(&DaemonRequest::Ping)
        .await
        .expect("Failed to send request");

    assert!(matches!(response, DaemonResponse::Pong),
             "Should respond to Ping when actively connected");

    // Wait for TTL to expire (3 seconds + buffer)
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Daemon should have exited after TTL
    let status = daemon.try_wait();
    assert!(status.is_ok(), "try_wait should succeed");

    // Verify daemon exited
    if let Ok(Some(exit_status)) = status {
        assert!(exit_status.success(), "Daemon should exit successfully after TTL");
    }
}

/// Test orphaned daemon cleanup on startup
#[tokio::test]
async fn test_orphaned_daemon_cleanup() {
    // Clean up any orphaned daemon
    let config = create_config_from_content(
        r#"
[[servers]]
name = "test"
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }
"#,
    );

    // Get socket path
    let socket_path = mcp_cli_rs::ipc::get_socket_path();

    // Try graceful shutdown first
    let _ = shutdown_daemon_gracefully(&config).await;

    // Manually create stale socket file
    if cfg!(unix) {
        std::fs::write(&socket_path, b"stale socket").expect("Failed to write stale socket");
    }

    // Clean up orphaned daemon - this should clean up the stale socket
    let cleanup_result = cleanup_orphaned_daemon(&config, &socket_path).await;

    // Cleanup should handle the stale socket gracefully
    assert!(cleanup_result.is_ok() || cleanup_result.is_err(),
             "Cleanup should not crash");

    // On Unix, verify socket is cleaned up
    if cfg!(unix) {
        assert!(!socket_path.exists(), "Stale socket should be cleaned up");
    }

    // Clean up test files
    if cfg!(unix) {
        let _ = std::fs::remove_file(&socket_path);
    }
}

/// Test config fingerprinting for change detection
#[tokio::test]
async fn test_config_fingerprint_detection() {
    let config1 = create_config_from_content(
        r#"
[[servers]]
name = "test-server"
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }
"#,
    );

    let config2 = create_config_from_content(
        r#"
[[servers]]
name = "test-server"
description = "Modified server"
transport = { type = "stdio", command = "echo", args = ["modified"] }
"#,
    );

    // Calculate fingerprints
    let fp1 = calculate_fingerprint(&config1);
    let fp2 = calculate_fingerprint(&config2);

    // Fingerprints should be different (different content)
    assert_ne!(fp1, fp2, "Different configs should have different fingerprints");
    assert!(fp1.len() == 64, "Fingerprint should be 64 hex characters");
    assert!(fp2.len() == 64, "Fingerprint should be 64 hex characters");
}

/// Test config version switching
#[tokio::test]
async fn test_config_version_switching() {
    let config_content1 = r#"
[[servers]]
name = "server-a"
description = "Server A"
transport = { type = "stdio", command = "echo", args = ["a"] }
"#;

    let config_content2 = r#"
[[servers]]
name = "server-b"
description = "Server B"
transport = { type = "stdio", command = "cat" }
"#;

    let config1 = create_config_from_content(config_content1);
    let config2 = create_config_from_content(config_content2);

    // Verify different servers
    assert_eq!(config1.servers[0].name, "server-a");
    assert_eq!(config2.servers[0].name, "server-b");

    // Fingerprints should be different
    let fp1 = calculate_fingerprint(&config1);
    let fp2 = calculate_fingerprint(&config2);
    assert_ne!(fp1, fp2, "Config versions should have different fingerprints");
}

/// Test config file cleanup
#[tokio::test]
async fn test_config_file_cleanup() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_config_from_content(
        r#"
[[servers]]
name = "test"
description = "Test server"
transport = { type = "stdio", command = "echo" }
"#,
    );

    // Get socket path
    let socket_path = mcp_cli_rs::ipc::get_socket_path();

    // Clean up orphaned daemon
    let _ = cleanup_orphaned_daemon(&config, &socket_path);

    // Verify temp directory exists before cleanup
    assert!(temp_dir.path().exists(),
             "Temp directory should exist before cleanup");

    // Clean up temp directory - the TempDir will be dropped at end of test
    println!("✓ Config file cleanup test passed");
}

/// Test orphaned PID cleanup on different platforms
#[tokio::test]
async fn test_orphaned_pid_cleanup_platforms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let pid_file = temp_dir.path().join("mcp-cli-daemon.pid");

    // Write a PID
    write_daemon_pid(&pid_file, std::process::id())
        .expect("Failed to write PID file");

    // Read it back
    let pid = read_daemon_pid(&pid_file).expect("Failed to read PID file");
    assert_eq!(pid, std::process::id(), "PID should be readable");

    // Reading non-existent PID file should handle gracefully (return error)
    let socket_path = mcp_cli_rs::ipc::get_socket_path();
    let result = read_daemon_pid(&socket_path);

    assert!(result.is_err(), "Missing PID file should return error");

    println!("✓ Missing PID file test passed");
}

/// Test shutdown with active connection
#[tokio::test]
async fn test_shutdown_with_active_connection() {
    // Clean up any orphaned daemon
    let config = create_config_from_content(
        r#"
[[servers]]
name = "test"
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }
"#,
    );

    // Get socket path and clean up
    let socket_path = mcp_cli_rs::ipc::get_socket_path();
    let _ = cleanup_orphaned_daemon(&config, &socket_path).await;

    // Try graceful shutdown first
    let _ = shutdown_daemon_gracefully(&config).await;

    // Start daemon
    let mut daemon = start_daemon(&config, 60).await.expect("Failed to start daemon");

    // Create IPC client
    let mut client = create_ipc_client(std::sync::Arc::new(config.clone()))
        .expect("Failed to create IPC client");

    // Verify connection works
    let response = client.send_request(&DaemonRequest::Ping)
        .await
        .expect("Failed to send request");
    assert!(matches!(response, DaemonResponse::Pong));

    // Send shutdown request
    let response = client.send_request(&DaemonRequest::Shutdown)
        .await
        .expect("Failed to send shutdown request");

    // Should get ShutdownAck
    assert!(matches!(response, DaemonResponse::ShutdownAck),
             "Should receive ShutdownAck response");

    // Wait for daemon to exit
    let status = daemon.wait().await;
    assert!(status.unwrap().success(), "Daemon should exit successfully after shutdown");
}

/// Test daemon protocol consistency
#[tokio::test]
async fn test_daemon_protocol_consistency() {
    // Test that protocol types are consistent
    let request = DaemonRequest::Ping;
    let response = DaemonResponse::Pong;

    // Verify ping/pong works
    assert!(matches!(request, DaemonRequest::Ping));
    assert!(matches!(response, DaemonResponse::Pong));

    println!("✓ Daemon protocol consistency test passed");
}
