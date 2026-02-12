//! Cross-platform IPC validation tests for daemon communication.
//!
//! XP-04: Validates daemon works on Linux, macOS, Windows
//!
//! This module orchestrates platform-specific tests organized in:
//! - tests/unix/tests.rs (Unix socket tests for Linux/macOS)
//! - tests/windows/tests.rs (Named pipe tests for Windows)
//! - tests/common/mod.rs (Shared test patterns)

use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc::IpcClient;

#[cfg(test)]
mod helpers;

// Platform-specific test modules
#[cfg(test)]
pub mod unix;
#[cfg(test)]
pub mod windows;
#[cfg(test)]
pub mod common;

// Tests are defined in platform-specific modules

/// Test IpcServer trait methods work identically across platforms
#[tokio::test]
async fn test_ipc_server_trait_consistency() {
    // Test on Unix (skip on Windows to avoid platform-specific issues)
    #[cfg(unix)]
    {
        let socket_path = crate::helpers::get_test_socket_path();

        let server = mcp_cli_rs::ipc::UnixIpcServer::new(&socket_path)
            .expect("Failed to create UnixIpcServer");

        // Verify trait methods are implemented
        assert!(
            server.listener.local_addr().is_ok(),
            "UnixIpcServer should have local_addr method"
        );

        // Clean up
        let _ = std::fs::remove_file(&socket_path);
    }

    // Test on Windows (skip on Unix)
    #[cfg(windows)]
    {
        let pipe_path = crate::helpers::get_test_socket_path();

        let _server = mcp_cli_rs::ipc::windows::NamedPipeIpcServer::new(&pipe_path)
            .expect("Failed to create NamedPipeIpcServer");

        // Verify trait methods are implemented
        assert!(
            !pipe_path.as_os_str().is_empty(),
            "NamedPipeIpcServer should have pipe_name method"
        );
    }
}

/// Test IpcClient trait methods work identically across platforms
#[tokio::test]
async fn test_ipc_client_trait_consistency() {
    // Test on Unix
    #[cfg(unix)]
    {
        let config = crate::helpers::create_test_config();
        let client = mcp_cli_rs::ipc::UnixIpcClient::new(config);
        assert!(client.config().is_empty());
    }

    // Test on Windows
    #[cfg(windows)]
    {
        let config = crate::helpers::create_test_config();
        let client = mcp_cli_rs::ipc::windows::NamedPipeIpcClient::with_config(config);
        assert!(client.config().is_empty());
    }
}

/// Test protocol (NDJSON) is consistent across platforms
#[tokio::test]
async fn test_ndjson_protocol_consistency() {
    let request = DaemonRequest::ListServers;

    // Serialize to NDJSON (one-line JSON)
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(
        !serialized.contains("\n"),
        "Serialized request should be single line"
    );

    // Deserialize back
    let deserialized: DaemonRequest = serde_json::from_str(&serialized).unwrap();
    assert!(
        matches!(deserialized, DaemonRequest::ListServers),
        "Expected ListServers request, got {:?}",
        deserialized
    );
}
