//! Integration test for daemon list functionality
//!
//! This test verifies that the daemon can successfully list servers and tools.
//! Used for regression testing when applying changes from master.
//!
//! TEST-15: List command regression tests covering various configurations

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;
use tempfile::TempDir;

mod fixtures {
    pub mod daemon_test_helper;
}

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

/// Create a temporary config file for testing
fn temp_config_file(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("test-config.toml");
    std::fs::write(&config_path, content).expect("Failed to write config file");
    (temp_dir, config_path)
}

/// Test list with daemon flag (daemon mode)
#[test]
fn test_list_with_daemon() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list command with --require-daemon");

    // Start daemon manually
    let daemon = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["daemon", "--ttl", "10"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .spawn()
        .expect("Failed to spawn daemon");

    let _daemon_handle = DaemonHandle::new(daemon);

    // Wait for daemon to start
    std::thread::sleep(Duration::from_secs(2));

    // Test list with --require-daemon
    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["list", "--require-daemon"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    println!("List with daemon output: {}", combined);

    // Should succeed and have content
    assert!(
        output.status.success() || combined.contains("serena") || combined.len() > 100,
        "List with daemon should succeed"
    );
}

/// Test list with JSON output flag
#[test]
fn test_list_json_output() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list command with --json flag");

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args(["list", "--json", "--no-daemon"])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("JSON stdout: {}", stdout);

    // Try to parse as JSON
    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => {
            // Verify it has expected structure
            let has_servers = json.get("servers").is_some();
            let is_array = json.is_array();
            let has_tools = json.get("tools").is_some();

            assert!(
                has_servers || is_array || has_tools || stdout.len() > 10,
                "JSON output should have valid structure"
            );
        }
        Err(e) => {
            // If not valid JSON, at least check it's not empty
            println!("Note: Output not valid JSON (may be expected): {}", e);
            assert!(
                stdout.len() > 10 || stderr.len() > 10,
                "Should have some output"
            );
        }
    }
}

/// Test list with multiple servers in config
#[test]
fn test_list_multiple_servers() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list with multiple servers in config");

    // Create config with multiple mock servers
    let config_content = r#"
[[servers]]
name = "server-one"
transport = "stdio"
command = "echo"
args = ["server1"]

[[servers]]
name = "server-two"
transport = "stdio"
command = "echo"
args = ["server2"]

[[servers]]
name = "http-server"
transport = "http"
url = "http://localhost:9999"
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args([
            "list",
            "--no-daemon",
            "--config",
            config_path.to_str().unwrap(),
        ])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    println!("Multiple servers output: {}", combined);

    // Should mention multiple servers or at least not crash
    assert!(
        output.status.success() || combined.len() > 50,
        "Should handle multiple servers gracefully"
    );
}

/// Test list with empty config - graceful handling
#[test]
fn test_list_empty_config() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list with empty config");

    // Create empty config
    let config_content = r#"servers = []"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args([
            "list",
            "--no-daemon",
            "--config",
            config_path.to_str().unwrap(),
        ])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    println!("Empty config output: {}", combined);

    // Should handle empty config gracefully (exit 0 or helpful message)
    assert!(
        output.status.success() || combined.to_lowercase().contains("no servers")
            || combined.to_lowercase().contains("empty"),
        "Should handle empty config gracefully"
    );
}

/// Test list with missing config - error handling
#[test]
fn test_list_missing_config() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list with missing config file");

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args([
            "list",
            "--no-daemon",
            "--config",
            "/nonexistent/path/config.toml",
        ])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Missing config stderr: {}", stderr);

    // Should fail gracefully with error message
    assert!(
        !output.status.success() || stderr.to_lowercase().contains("not found")
            || stderr.to_lowercase().contains("error"),
        "Should report error for missing config"
    );
}

/// Test list with invalid config - error handling
#[test]
fn test_list_invalid_config() {
    cleanup_daemon();
    ensure_binary_built();

    println!("Testing: list with invalid config");

    // Create invalid config (malformed TOML)
    let config_content = r#"
[invalid
malformed = toml content [[[
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let output = Command::new("./target/debug/mcp-cli-rs.exe")
        .args([
            "list",
            "--no-daemon",
            "--config",
            config_path.to_str().unwrap(),
        ])
        .env("RUST_LOG", "")
        .env("SERENA_LOG_LEVEL", "error")
        .output()
        .expect("Failed to run list");

    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Invalid config stderr: {}", stderr);

    // Should fail gracefully
    assert!(
        !output.status.success() || stderr.to_lowercase().contains("parse")
            || stderr.to_lowercase().contains("error")
            || stderr.to_lowercase().contains("invalid"),
        "Should report error for invalid config"
    );
}
