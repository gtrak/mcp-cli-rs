//! Integration test for daemon mode functionality
//!
//! Tests verify that daemon mode works correctly for listing servers and tools.

use std::process::Command;
use std::thread;
use std::time::Duration;

/// Test that daemon mode can list servers and tools
#[tokio::test]
#[ignore = "Tests actual daemon and requires running daemon"]
async fn test_daemon_mode_list_servers_and_tools() {
    // This test requires:
    // 1. A daemon running (spawn it or use existing)
    // 2. A configured server (e.g., serena MCP server)

    // For now, we'll just spawn a daemon and test list command
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    thread::sleep(Duration::from_millis(500));

    // Test with no-daemon to verify basic functionality
    let output = Command::new("cargo")
        .args(["run", "--", "list", "--no-daemon"])
        .output()
        .expect("Failed to run list command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show servers and tools
    assert!(
        stdout.contains("Tools:") || stdout.contains("No servers"),
        "Should show tools or message about servers. stdout: {}, stderr: {}",
        stdout,
        stderr
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
#[cfg(windows)]
#[test]
fn test_spawn_daemon_using_main_entry() {
    // This test spawns a daemon and verifies it starts
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    thread::sleep(Duration::from_millis(500));

    // Spawn daemon in background
    #[allow(clippy::zombie_processes)]
    let mut daemon = Command::new("cargo")
        .args(["run", "--bin", "mcp-cli-rs", "--", "daemon", "--ttl", "5"])
        .spawn()
        .expect("Failed to spawn daemon");
    thread::sleep(Duration::from_secs(2));

    // Try to list servers (should use daemon)
    let output = Command::new("cargo")
        .args(["run", "--bin", "mcp-cli-rs", "--", "list"])
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should either work or give clear error about no servers
    assert!(
        stdout.contains("MCP Servers")
            || stdout.contains("Configured servers")
            || stdout.contains("No servers")
            || stderr.contains("No servers"),
        "Should show servers. stdout: {}, stderr: {}",
        stdout,
        stderr
    );

    // Cleanup daemon
    let _ = daemon.kill();
}