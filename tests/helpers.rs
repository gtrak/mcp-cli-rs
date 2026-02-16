//! Test helpers for MCP CLI integration tests
//!
//! Provides common patterns for:
//! - Temporary directory management (TestEnvironment)
//! - Platform-specific socket/pipe path generation
//! - IPC server/client roundtrip patterns
//! - Test configuration factories

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::BufReader;
use tokio::time::timeout;

use mcp_cli_rs::config::Config;
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
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

/// Run a simple IPC roundtrip test (Ping -> Pong)
///
/// Creates server, spawns task, sends Ping request, awaits Pong response.
/// This pattern is repeated across 10+ tests in the codebase.
pub async fn run_ping_pong_roundtrip(socket_path: PathBuf) -> anyhow::Result<()> {
    // Create IPC server
    let mut server = ipc::create_ipc_server(&socket_path).await?;

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        let (mut stream, _addr) =
            match timeout(Duration::from_secs(5), server.accept()).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => panic!("Server accept failed: {}", e),
                Err(e) => panic!("Server accept timed out: {}", e),
            };

        // Read request
        let mut buf_reader = BufReader::new(stream);
        let request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        // Verify Ping request
        assert!(matches!(request, DaemonRequest::Ping));

        // Send Pong response
        let response = DaemonResponse::Pong;
        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
            .await
            .expect("Failed to send response");
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create IPC client
    let config = Config::with_socket_path(socket_path.clone());
    let mut client = ipc::create_ipc_client(&config)?;

    // Send Ping request
    let request = DaemonRequest::Ping;
    let response = client.send_request(&request).await?;

    // Verify Pong response
    assert!(matches!(response, DaemonResponse::Pong));

    // Wait for server to complete
    server_handle.await?;

    // Clean up socket (Unix only)
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}

/// Create an IPC server that handles a single request-response cycle
///
/// Returns a join handle for the server task
pub async fn spawn_single_response_server(
    socket_path: PathBuf,
    expected_request: DaemonRequest,
    response: DaemonResponse,
) -> tokio::task::JoinHandle<()> {
    let mut server = ipc::create_ipc_server(&socket_path)
        .await
        .expect("Failed to create IPC server");

    tokio::spawn(async move {
        let (mut stream, _addr) =
            match timeout(Duration::from_secs(5), server.accept()).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => panic!("Server accept failed: {}", e),
                Err(e) => panic!("Server accept timed out: {}", e),
            };

        let mut buf_reader = BufReader::new(stream);
        let request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        assert!(
            std::mem::discriminant(&request) == std::mem::discriminant(&expected_request),
            "Request type mismatch"
        );

        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
            .await
            .expect("Failed to send response");
    })
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

/// Create a default test configuration
///
/// Provides a standard config for tests that don't need custom settings
pub fn create_test_config() -> Arc<Config> {
    Arc::new(Config::default())
}

/// Create a test configuration with custom socket path
///
/// Useful for tests that need to specify their own IPC endpoint
pub fn create_test_config_with_socket(socket_path: PathBuf) -> Arc<Config> {
    Arc::new(Config::with_socket_path(socket_path))
}

/// Create a test configuration with daemon socket path for tests
///
/// This is the most common test configuration pattern.
/// Uses a unique socket path based on process ID to avoid conflicts.
pub fn create_test_daemon_config() -> Arc<Config> {
    let socket_path = get_test_socket_path();
    Arc::new(Config::with_socket_path(socket_path))
}

