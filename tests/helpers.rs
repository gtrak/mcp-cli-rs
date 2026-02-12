//! Test helpers for MCP CLI integration tests
//!
//! Provides common patterns for:
//! - Temporary directory management (TestEnvironment)
//! - Platform-specific socket/pipe path generation
//! - IPC server/client roundtrip patterns
//! - Test configuration factories

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

use mcp_cli_rs::ipc;

/// Get a platform-specific test socket/pipe path for testing
///
/// Returns Unix socket path on Linux/macOS (e.g., /tmp/mcp-test-12345.sock)
/// Returns Windows named pipe path on Windows (e.g., \\.\pipe\mcp-test-12345)
pub fn get_test_socket_path() -> PathBuf {
    #[cfg(unix)]
    {
        let mut path = std::env::temp_dir();
        path.push(format!("mcp-test-{}.sock", std::process::id()));
        path
    }
    #[cfg(windows)]
    {
        let mut path = std::env::temp_dir();
        path.push(format!(r"\\.\pipe\mcp-test-{}", std::process::id()));
        path
    }
}

/// Get a unique test socket/pipe path with custom suffix
///
/// Useful for creating multiple distinct test endpoints
pub fn get_test_socket_path_with_suffix(suffix: &str) -> PathBuf {
    #[cfg(unix)]
    {
        let mut path = std::env::temp_dir();
        path.push(format!("mcp-test-{}-{}.sock", std::process::id(), suffix));
        path
    }
    #[cfg(windows)]
    {
        let mut path = std::env::temp_dir();
        path.push(format!(
            r"\\.\pipe\mcp-test-{}-{}",
            std::process::id(),
            suffix
        ));
        path
    }
}

/// Test environment with temporary directory cleanup
pub struct TestEnvironment {
    pub temp_dir: TempDir,
}

impl TestEnvironment {
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp directory"),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

// Add remaining helper functions in subsequent tasks
