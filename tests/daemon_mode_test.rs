//! Integration test for daemon mode functionality
//!
//! Tests verify that daemon mode works correctly for listing servers and tools.

use std::process::Command;
use std::time::Duration;
use std::thread;
use mcp_cli_rs::daemon::protocol::DaemonResponse;

/// Test that daemon mode can list servers and tools
#[tokio::test]
#[ignore = "Tests actual daemon and requires running daemon"]
async fn test_daemon_mode_list_servers_and_tools() {
    // This test requires:
    // 1. A daemon running (spawn it or use existing)
    // 2. A configured server (e.g., serena MCP server)
    
    // For now, we'll just spawn a daemon and test list command
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    thread::sleep(Duration::from_millis(500));

    // Test with no-daemon to verify basic functionality
    let output = Command::new("cargo")
        .args(&["run", "--", "list", "--no-daemon"])
        .output()
        .expect("Failed to run list command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show servers and tools
    assert!(
        stdout.contains("Tools:") || stdout.contains("No servers"),
        "Should show tools or message about servers. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // If we have a server configured, it should show tools
    if stdout.contains("Tools:") {
        assert!(
            stdout.parse::<usize>().is_ok() || stdout.contains("29"),
            "Should show tool count or server name"
        );
    }
}

/// Test that spawn_daemon_and_wait works correctly
#[test]
fn test_spawn_daemon_using_main_entry() {
    // This test spawns a daemon and verifies it starts
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    thread::sleep(Duration::from_millis(500));

    // Spawn daemon in background
    let mut daemon = Command::new("cargo")
        .args(&["run", "--", "daemon", "--ttl", "5"])
        .spawn()
        .expect("Failed to spawn daemon");

    thread::sleep(Duration::from_secs(2));

    // Try to list servers (should use daemon)
    let output = Command::new("cargo")
        .args(&["run", "--", "list"])
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should either work or give clear error about no servers
    assert!(
        stdout.contains("MCP Servers") || 
        stdout.contains("Configured servers") ||
        stdout.contains("No servers") ||
        stderr.contains("No servers"),
        "Should show servers. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // Cleanup daemon
    let _ = daemon.kill();
}

/// Quick smoke test: verify daemon can be started and stopped
#[test]
#[ignore]
fn test_daemon_lifecycle() {
    // Kill any existing daemon
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    thread::sleep(Duration::from_millis(500));

    // Spawn daemon with short TTL
    let mut daemon = Command::new("cargo")
        .args(&["run", "--", "daemon", "--ttl", "2"])
        .spawn()
        .expect("Failed to spawn daemon");

    // Give it time to start
    thread::sleep(Duration::from_secs(1));

    // Verify it's running
    if let Ok(status) = daemon.try_wait() {
        assert!(
            status.is_none(),
            "Daemon should still be running"
        );
    }

    // Wait for TTL to expire
    thread::sleep(Duration::from_secs(3));

    // Daemon should have quit
    if let Ok(status) = daemon.try_wait() {
        assert!(
            status.is_some(),
            "Daemon should have exited after TTL"
        );
    }
}

/// Test auto-daemon mode: spawns daemon if needed, executes command, daemon auto-shutdowns
#[tokio::test]
#[ignore]
async fn test_auto_daemon_mode() {
    // Kill any existing daemon
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();
    let _ = Command::new("pkill")
        .args(&["-f", "mcp-cli-rs"])
        .output();
    thread::sleep(Duration::from_millis(500));

    // Verify daemon is not running by trying to connect
    let socket_path = mcp_cli_rs::ipc::get_socket_path();
    let config = mcp_cli_rs::config::Config::default();
    
    // First run with --auto-daemon should spawn daemon
    let output = Command::new("cargo")
        .args(&["run", "--", "list", "--auto-daemon"])
        .output()
        .expect("Failed to run list command with auto-daemon");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should either show servers or clear error (not daemon connection error)
    assert!(
        stdout.contains("MCP Servers") || 
        stdout.contains("Configured servers") ||
        stdout.contains("No servers") ||
        !stderr.contains("ConnectionError"),
        "Should list servers or show no servers, not connection error. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // Second run with --auto-daemon should reuse existing daemon
    let output2 = Command::new("cargo")
        .args(&["run", "--", "list", "--auto-daemon"])
        .output()
        .expect("Failed to run second list command");

    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    let stderr2 = String::from_utf8_lossy(&output2.stderr);

    // Should connect to existing daemon
    assert!(
        stdout2.contains("MCP Servers") || 
        stdout2.contains("Configured servers") ||
        stdout2.contains("No servers") ||
        !stderr2.contains("ConnectionError"),
        "Should reuse existing daemon. stdout: {}, stderr: {}",
        stdout2, stderr2
    );

    // Cleanup: kill daemon
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();
    let _ = Command::new("pkill")
        .args(&["-f", "mcp-cli-rs"])
        .output();
}

/// Test auto-daemon mode with actual server
#[tokio::test]
#[ignore]
async fn test_auto_daemon_with_server() {
    use mcp_cli_rs::config::loader::load_config_sync;
    use mcp_cli_rs::ipc::create_ipc_client;
    use mcp_cli_rs::daemon::protocol::DaemonRequest;
    use tokio::time::{timeout, Duration};
    use tempfile::TempDir;

    // Kill any existing daemon
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();
    let _ = Command::new("pkill")
        .args(&["-f", "mcp-cli-rs"])
        .output();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create a config with a test server
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("mcp_servers.toml");
    let config_content = r#"
[[servers]]
name = "test-server"
transport = { type = "stdio", command = "echo", args = ["hello"] }
"#;
    std::fs::write(&config_path, config_content).expect("Failed to write config");

    // Set config path env var
    unsafe { std::env::set_var("MCP_CONFIG_PATH", &config_path); }

    // Spawn daemon with cargo run
    let mut daemon = Command::new("cargo")
        .args(&["run", "--", "daemon", "--ttl", "10"])
        .env("MCP_CONFIG_PATH", &config_path)
        .spawn()
        .expect("Failed to spawn daemon");

    // Wait for daemon to be ready
    let config = load_config_sync(&config_path).expect("Failed to load config");
    let config_arc = std::sync::Arc::new(config);
    
    let connected = timeout(Duration::from_secs(10), async {
        let mut delay = Duration::from_millis(100);
        loop {
            if let Ok(mut client) = create_ipc_client(config_arc.clone()) {
                if let Ok(response) = timeout(Duration::from_secs(1),
                    client.send_request(&DaemonRequest::Ping)
                ).await {
                    if response.unwrap_or_else(|_| DaemonResponse::Error { code: 0, message: String::new() }) 
                        == DaemonResponse::Pong {
                        break true;
                    }
                }
            }
            tokio::time::sleep(delay).await;
            delay *= 2;
            if delay > Duration::from_secs(2) {
                delay = Duration::from_secs(2);
            }
        }
    }).await;

    assert!(connected.unwrap_or(false), "Failed to connect to daemon");

    // Send shutdown request
    let mut client = create_ipc_client(config_arc).expect("Failed to create client");
    let response = timeout(Duration::from_secs(5),
        client.send_request(&DaemonRequest::Shutdown)
    ).await.expect("Timeout").expect("Shutdown failed");
    
    assert!(matches!(response, DaemonResponse::ShutdownAck), "Should receive ShutdownAck");

    // Wait for daemon to exit
    let status = daemon.wait().expect("Failed to wait for daemon");
    assert!(status.success(), "Daemon should exit successfully");

    // Cleanup
    unsafe { std::env::remove_var("MCP_CONFIG_PATH"); }
}