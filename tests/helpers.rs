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
