//! Integration test for daemon list functionality
//!
//! This test verifies that the daemon can successfully list servers and tools.
//! Used for regression testing when applying changes from master.

use std::process::Command;
use std::time::Duration;

/// Test list with no-daemon flag (direct mode)
#[test]
fn test_list_no_daemon() {
    println!("Testing: list command with --no-daemon");

    let output = Command::new("cargo")
        .args(&["run", "--", "list", "--no-daemon"])
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let has_servers = stdout.contains("Servers:")
                || stdout.contains("MCP Servers")
                || stdout.contains("Configured servers");
            let has_tools = stdout.contains("Tools:");

            println!("Exit code: {:?}", output.status.code());
            println!("has_servers: {}, has_tools: {}", has_servers, has_tools);

            // Should show servers and tools (or no servers message)
            assert!(
                has_servers || stdout.contains("No servers") || stderr.contains("No servers"),
                "Should show servers in no-daemon mode"
            );
        }
        Err(e) => {
            panic!("list --no-daemon failed: {:?}", e);
        }
    }
}

/// Test daemon can be spawned manually and queried
#[test]
fn test_manual_daemon_spawn() {
    println!("Testing: Manual daemon spawn and query");

    // Kill existing daemons
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "mcp-cli-rs.exe"])
        .output();

    std::thread::sleep(Duration::from_millis(500));

    // Spawn daemon with TTL
    let mut daemon = Command::new("cargo")
        .args(&["run", "--", "daemon", "--ttl", "10"])
        .spawn()
        .expect("Failed to spawn daemon");

    println!("Daemon spawned with PID: {:?}", daemon.id());

    // Wait for daemon to start
    std::thread::sleep(Duration::from_secs(2));

    // Try to list servers
    let output = Command::new("cargo")
        .args(&["run", "--", "list", "--require-daemon"])
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("List completed. stdout length: {}", stdout.len());
    assert!(
        stdout.contains("Servers:")
            || stdout.contains("MCP Servers")
            || stdout.contains("Configured servers")
            || stdout.contains("No servers"),
        "Should show servers when daemon is running"
    );

    // Cleanup
    let _ = daemon.kill();
    std::thread::sleep(Duration::from_millis(500));
}
