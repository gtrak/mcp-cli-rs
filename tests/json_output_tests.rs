//! Integration tests for JSON output mode
//!
//! Tests verify that --json flag produces valid, parseable JSON output
//! across all commands.

use std::fs;
use std::io::Write;
use std::process::Command;

/// Helper to run the CLI and capture JSON output
fn run_json_command(args: &[&str]) -> Result<serde_json::Value, String> {
    let output = Command::new("cargo")
        .args(&["run", "--"])
        .args(args)
        .args(&["--json"])
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip empty output (expected when no config)
    if stdout.trim().is_empty() {
        return Err("Empty output".to_string());
    }

    serde_json::from_str(&stdout)
        .map_err(|e| format!("Invalid JSON output: {}\nOutput: {}", e, stdout))
}

#[test]
fn test_list_json_schema() {
    // Test that list --json produces valid JSON with expected structure
    let result = run_json_command(&["list"]);

    // Skip if no config (expected in test environment)
    let json = match result {
        Ok(j) => j,
        Err(e) if e.contains("No servers configured") => return,
        Err(e) if e.contains("Empty output") => return,
        Err(e) => panic!("Unexpected error: {}", e),
    };

    // Verify structure
    assert!(json.get("servers").is_some(), "Missing 'servers' field");
    assert!(
        json.get("total_servers").is_some(),
        "Missing 'total_servers' field"
    );
    assert!(
        json.get("total_tools").is_some(),
        "Missing 'total_tools' field"
    );
}

#[test]
fn test_list_json_with_mock_config() {
    // Create a temporary config file for testing
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join("mcp_test_config_XXXXX.toml");

    // Create a unique temp config
    let config_path =
        std::env::temp_dir().join(format!("mcp_test_config_{}.toml", std::process::id()));

    let config_content = r#"
[[servers]]
name = "test-server"
command = "echo"
args = ["test"]
"#;

    let mut temp_file = fs::File::create(&config_path).expect("Failed to create temp file");
    temp_file
        .write_all(config_content.as_bytes())
        .expect("Failed to write config");
    temp_file.sync_all().expect("Failed to sync file");

    // Use the config file
    let output = Command::new("cargo")
        .args(&["run", "--", "-c"])
        .arg(&config_path)
        .args(&["list", "--json"])
        .output()
        .expect("Failed to run command");

    // Clean up
    let _ = fs::remove_file(&config_path);

    // Just verify it produces valid JSON (may be empty servers if echo doesn't respond)
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect(&format!("Invalid JSON: {}", stdout));
        assert!(json.get("servers").is_some(), "Missing 'servers' field");
    }
}

#[test]
fn test_json_no_color_interference() {
    // Verify that JSON output doesn't include ANSI color codes
    let output = Command::new("cargo")
        .args(&["run", "--", "list", "--json"])
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check for ANSI escape sequences
    assert!(
        !stdout.contains('\u{001b}'),
        "JSON output contains ANSI codes: {}",
        stdout
    );
}

#[test]
fn test_plain_text_when_piped() {
    // Verify plain text mode works when stdout is not a TTY
    // This is harder to test in Rust, but we can verify the flag logic
    use mcp_cli_rs::format::OutputMode;

    assert!(OutputMode::from_flags(false).is_human());
    assert!(OutputMode::from_flags(true).is_json());
}

#[test]
fn test_info_command_json_with_help() {
    // Test that `info --help` still works (not affected by --json flag in normal usage)
    let output = Command::new("cargo")
        .args(&["run", "--", "info", "--help"])
        .output()
        .expect("Failed to run command");

    assert!(
        output.status.success(),
        "info --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("USAGE"), "Help output should contain USAGE");
    assert!(stdout.contains("ARGS"), "Help output should contain ARGS");
}

#[test]
fn test_call_command_json_without_args() {
    // Test that call without required args produces an error
    let output = Command::new("cargo")
        .args(&["run", "--", "call", "server/tool", "--json"])
        .output()
        .expect("Failed to run command");

    // Command should fail (missing required arguments)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Either produces valid JSON error or a standard CLI error
    if !stdout.trim().is_empty() {
        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            // If JSON output, should have error status
            assert!(
                json.get("status").is_some(),
                "JSON error response should have 'status' field"
            );
        }
    }

    // Or stderr has error message
    assert!(
        stderr.contains("required")
            || stderr.contains("error")
            || stderr.contains("Error")
            || !output.status.success(),
        "Expected error message for missing required arguments"
    );
}
