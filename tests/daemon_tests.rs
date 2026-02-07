//! Daemon lifecycle and functionality tests.

use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::config::Config;
use mcp_cli_rs::daemon::fingerprint::calculate_fingerprint;
use mcp_cli_rs::daemon::orphan::{read_daemon_pid, write_daemon_pid};
use tempfile::TempDir;

/// Create a config from content.
fn create_config_from_content(content: &str) -> Config {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.toml");
    std::fs::write(&config_path, content).expect("Failed to write config");

    runtime
        .block_on(load_config(&config_path))
        .expect("Failed to parse config")
}

/// Test config fingerprinting.
#[test]
fn test_config_fingerprinting() {
    // Create test configs with different content
    let config1 = create_config_from_content(
        r#"
[servers.test]
transport = { type = "stdio", command = "echo" }
"#,
    );

    let config2 = create_config_from_content(
        r#"
[servers.test]
transport = { type = "stdio", command = "echo" }

[servers.test2]
transport = { type = "stdio", command = "cat" }
"#,
    );

    // Calculate fingerprints
    let fp1 = calculate_fingerprint(&config1);
    let fp2 = calculate_fingerprint(&config2);

    println!("Config1 fingerprint: {}", fp1);
    println!("Config2 fingerprint: {}", fp2);

    // Fingerprints should be different (different content)
    assert_ne!(
        fp1, fp2,
        "Different configs should have different fingerprints"
    );

    // Same config should have same fingerprint
    let fp1_again = calculate_fingerprint(&config1);
    assert_eq!(fp1, fp1_again, "Same config should have same fingerprint");

    println!("✓ Config fingerprinting test passed");
}

/// Test PID file reading and writing.
#[test]
fn test_pid_file_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test-sock.pid");

    // Write PID
    let test_pid = 12345;
    write_daemon_pid(&socket_path, test_pid).expect("Failed to write PID");

    // Read back
    let read_pid = read_daemon_pid(&socket_path).expect("Failed to read PID");

    assert_eq!(test_pid, read_pid, "Read PID should match written PID");

    println!("✓ PID file operations test passed");
}

/// Test graceful error handling for missing PID file.
#[test]
fn test_missing_pid_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("non-existent.pid");

    // Reading non-existent PID file should handle gracefully (return error)
    let result = read_daemon_pid(&socket_path);

    assert!(result.is_err(), "Missing PID file should return error");

    println!("✓ Missing PID file test passed");
}

/// Test fingerprint uniqueness.
#[test]
fn test_fingerprint_uniqueness() {
    let config = create_config_from_content(
        r#"
[servers.test-server]
description = "Test server"
transport = { type = "stdio", command = "echo", args = ["test"] }

[servers.test-server-2]
description = "Another test server"
transport = { type = "stdio", command = "cat" }
"#,
    );

    // Generate fingerprint
    let fp1 = calculate_fingerprint(&config);

    // Wait a bit
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Generate again
    let fp2 = calculate_fingerprint(&config);

    // Should be identical (deterministic)
    assert_eq!(fp1, fp2, "Fingerprint should be deterministic");

    // Should be a proper SHA256 hex string (64 characters)
    assert_eq!(
        fp1.len(),
        64,
        "SHA256 fingerprint should be 64 hex characters"
    );

    println!("✓ Fingerprint uniqueness test passed");
}

/// Test fingerprint for empty config.
#[test]
fn test_fingerprint_empty_config() {
    let content = r#""#; // Empty config

    let config = create_config_from_content(content);

    let fp = calculate_fingerprint(&config);

    // Should still produce a valid fingerprint
    assert_eq!(fp.len(), 64, "Fingerprint should be 64 hex characters");

    println!("✓ Empty config fingerprint test passed");
}
