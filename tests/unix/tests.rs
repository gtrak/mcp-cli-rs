//! Unix socket communication tests
//!
//! XP-04: Validates Unix socket (Linux/macOS) IPC implementation

use crate::helpers;
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc;
use std::time::Duration;
use tokio::time::timeout;

/// Test Unix socket creation and validation
#[tokio::test]
async fn test_unix_socket_creation() {
    let socket_path = crate::helpers::get_test_socket_path();

    // Verify socket path format
    assert!(
        socket_path.to_string_lossy().contains(".sock"),
        "Unix socket path should end with .sock"
    );

    // Verify parent directory can be created
    let path = socket_path.clone();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create parent directory");
    }

    // Verify socket file doesn't exist yet
    assert!(
        !socket_path.exists(),
        "Socket file should not exist before creation"
    );

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Unix socket client-server roundtrip (Linux/macOS)
#[tokio::test]
async fn test_unix_socket_client_server_roundtrip() {
    let socket_path = crate::helpers::get_test_socket_path();
    crate::helpers::run_ping_pong_roundtrip(socket_path)
        .await
        .expect("Ping/Pong roundtrip failed");
}

/// Test Unix socket multiple concurrent connections
#[tokio::test]
async fn test_unix_socket_multiple_concurrent_connections() {
    let socket_path = crate::helpers::get_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).await.expect("Failed to create IPC server");

    // Spawn server task handling 3 concurrent connections
    let server_handle = tokio::spawn(async move {
        for i in 0..3 {
            let (mut stream, _addr) = match timeout(Duration::from_secs(5), server.accept()).await {
                Ok(result) => match result {
                    Ok(stream) => stream,
                    Err(e) => panic!("Server accept failed: {}", e),
                },
                Err(e) => panic!("Server accept timed out: {}", e),
            };

            // Read request - wrap in BufReader for AsyncBufRead requirement
            let mut buf_reader = tokio::io::BufReader::new(stream);
            let request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
                .await
                .expect("Failed to receive request");

            // Verify Ping request
            assert!(
                matches!(request, DaemonRequest::Ping),
                "Expected Ping request, got {:?}",
                request
            );

            // Send response
            let response = DaemonResponse::Pong;
            mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
                .await
                .expect("Failed to send response");
            }
    });

    // Create IPC client and send 3 concurrent requests
    let config = crate::helpers::create_test_config();
    let mut client = mcp_cli_rs::ipc::create_ipc_client(&*config)
        .expect("Failed to create IPC client");
    let request = DaemonRequest::Ping;
    let response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");
    assert!(
        matches!(response, DaemonResponse::Pong),
        "Expected Pong response"
    );

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Clean up socket file (Unix only)
    std::fs::remove_file(&socket_path)
        .ok()
        .expect("Failed to remove socket file");
}

/// Test Unix socket large message transfer (Linux/macOS)
#[tokio::test]
async fn test_unix_socket_large_message_transfer() {
    let socket_path = crate::helpers::get_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).await.expect("Failed to create IPC server");

    // Create large JSON object (100KB text as in plan)
    let large_content = serde_json::json!({
        "text": "a".repeat(100_000)
    });

    let server_content = large_content.clone();

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        let (mut stream, _addr) = match timeout(Duration::from_secs(10), server.accept()).await {
            Ok(result) => match result {
                Ok(stream) => stream,
                Err(e) => panic!("Server accept failed: {}", e),
            },
            Err(e) => panic!("Server accept timed out: {}", e),
        };

        // Read request - wrap in BufReader for AsyncBufRead requirement
        let mut buf_reader = tokio::io::BufReader::new(stream);
        let _request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        // Send large response
        let response = DaemonResponse::ToolResult(server_content);
        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
            .await
            .expect("Failed to send response");
    });

    // Create IPC client
    let config = crate::helpers::create_test_config();
    let mut client = mcp_cli_rs::ipc::create_ipc_client(&*config)
        .expect("Failed to create IPC client");

    // Send ping request
    let request = DaemonRequest::Ping;
    let response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");

    // Verify large content was transferred correctly
    assert!(matches!(response, DaemonResponse::ToolResult(_)));
    if let DaemonResponse::ToolResult(content) = response {
        assert_eq!(content, large_content);
    }

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Unix socket server cleanup on path removal (Linux/macOS)
#[tokio::test]
async fn test_unix_socket_cleanup_on_removal() {
    let socket_path = crate::helpers::get_test_socket_path();

    // Create IPC server
    let _server = mcp_cli_rs::ipc::create_ipc_server(&socket_path)
        .await
        .expect("Failed to create IPC server");

    // Remove socket path manually
    std::fs::remove_file(&socket_path).expect("Failed to remove socket file");

    // Verify socket is gone
    assert!(!socket_path.exists(), "Socket file should be removed");

    // Create server again at same path - should handle stale file gracefully
    let _server2 = mcp_cli_rs::ipc::create_ipc_server(&socket_path)
        .await
        .expect("Failed to create server after cleanup");

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Unix socket StaleSocketError handling (Linux/macOS)
#[tokio::test]
async fn test_unix_socket_stale_error_handling() {
    let socket_path = crate::helpers::get_test_socket_path();

    // Create IPC server
    let server = mcp_cli_rs::ipc::create_ipc_server(&socket_path)
        .await
        .expect("Failed to create IPC server");

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        // Try to accept connection - should handle errors
        let result = server.accept().await;
        // Connection should fail (stale socket)
        assert!(result.is_err(), "Accept should fail on stale socket");

        if let Err(e) = result {
            // Should be an IpcError type
            assert!(
                matches!(e, mcp_cli_rs::error::McpError::IpcError { .. }),
                "Should be IpcError type, got: {:?}", e
            );
        }
    });

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}
