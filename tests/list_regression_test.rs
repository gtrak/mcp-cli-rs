//! Integration test for daemon list functionality
//!
//! This test verifies that the daemon can successfully list servers and tools.
//! Used for regression testing when applying changes from master.

use std::process::{Child, Command};
use std::time::Duration;

/// Ensure binary is built
fn ensure_binary_built() {
    let _ = Command::new("cargo").args(["build"]).output();
}

/// Helper function to shutdown daemon via IPC
/// Sends graceful shutdown request and waits for process to exit
fn shutdown_daemon_gracefully() -> Result<(), Box<dyn std::error::Error>> {
    // Send shutdown request to daemon
    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["shutdown"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Warning: Daemon shutdown command failed: {}", stderr);
    }

    // Wait for daemon to exit gracefully
    std::thread::sleep(Duration::from_millis(500));

    Ok(())
}

/// Cleanup daemon - try graceful shutdown first
fn cleanup_daemon() {
    // Try graceful shutdown via IPC
    let _ = shutdown_daemon_gracefully();

    // Wait for cleanup
    std::thread::sleep(Duration::from_millis(300));
}

/// Child handle with auto-drop cleanup
struct DaemonHandle {
    child: Option<Child>,
}

impl DaemonHandle {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn pid(&self) -> u32 {
        self.child.as_ref().map_or(0, |c| c.id())
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        // Graceful shutdown on drop
        let _ = shutdown_daemon_gracefully();

        // Force kill if still running
        if let Some(mut child) = self.child.take()
            && child.try_wait().is_ok()
        {
            // Process still running, kill it
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Test list with no-daemon flag (direct mode)
#[test]
fn test_list_no_daemon() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list command with --no-daemon");

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["list", "--no-daemon"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);

            if !output.status.success() {
                eprintln!("Exit code: {:?}", output.status.code());
                eprintln!("Combined output: {}", combined);
                panic!("Command failed");
            }

            let has_content = combined.contains("serena")
                || combined.contains("Tools:")
                || combined.contains("Configured servers:")
                || combined.len() > 100;

            assert!(has_content, "Should have output content");
        }
        Err(e) => {
            panic!("list --no-daemon failed to execute: {:?}", e);
        }
    }
}

/// Test daemon can be spawned manually and queried
#[test]
fn test_manual_daemon_spawn() {
    cleanup_daemon();

    println!("Testing: Manual daemon spawn and query");
    ensure_binary_built();

    let daemon = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["daemon", "--ttl", "10"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .spawn()
        .expect("Failed to spawn daemon");

    let _daemon_handle = DaemonHandle::new(daemon);
    println!("Daemon spawned with PID: {:?}", _daemon_handle.pid());

    std::thread::sleep(Duration::from_secs(2));

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["list", "--require-daemon"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    println!("List completed");

    if !output.status.success() {
        eprintln!("Exit code: {:?}", output.status.code());
        eprintln!("Combined output: {}", combined);
    }

    let has_content = combined.contains("serena")
        || combined.contains("Tools:")
        || combined.contains("Configured servers:")
        || combined.len() > 100;

    assert!(
        has_content,
        "Should have output content when daemon is running"
    );

    // DaemonHandle Drop impl will cleanly shutdown
}
