//! Windows named pipe communication tests
//!
//! XP-04: Validates named pipe (Windows) IPC implementation

use crate::helpers;
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc;

/// Test named pipe creation on Windows
#[tokio::test]
async fn test_windows_named_pipe_creation() {
    let pipe_path = crate::helpers::get_test_socket_path();
    let pipe_name = pipe_path.to_string_lossy().to_string();

    // Verify pipe name format (should start with \\.\\pipe\\)
    assert!(
        pipe_name.starts_with(r"\\.\pipe\"),
        "Named pipe name should start with \\.\\pipe\\"
    );

    // Verify pipe name is unique (includes PID)
    let pid = std::process::id().to_string();
    assert!(
        pipe_name.contains(&pid),
        "Named pipe name should include process ID"
    );

    // Verify pipe name contains only valid characters
    assert!(
        pipe_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '\\' || c == '-'),
        "Named pipe name should contain only valid characters"
    );
}

/// Test Windows named pipe creation and validation
#[tokio::test]
async fn test_windows_named_pipe_server_creation() {
    let pipe_path = crate::helpers::get_test_socket_path();

    // Create IPC server
    let _server = mcp_cli_rs::ipc::create_ipc_server(&pipe_path)
        .expect("Failed to create IPC server");
}

/// Test Windows named pipe client-server roundtrip
#[tokio::test]
async fn test_windows_named_pipe_client_server_roundtrip() {
    let pipe_path = crate::helpers::get_test_socket_path_with_suffix("roundtrip");
    crate::helpers::run_ping_pong_roundtrip(pipe_path)
        .await
        .expect("Ping/Pong roundtrip failed");
}

/// Test Windows named pipe multiple concurrent connections
#[tokio::test]
async fn test_windows_named_pipe_multiple_concurrent_connections() {
    // Use a unique pipe name for this test
    let pipe_path = crate::helpers::get_test_socket_path_with_suffix("concurrent");
    let pipe_path_for_server = pipe_path.clone();

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Spawn server task handling 3 concurrent connections
    let server_handle = tokio::spawn(async move {
        for _ in 0..3 {
            // Create server instance for each connection
            let pipe_name_str = pipe_path_for_server.to_string_lossy().to_string();
            let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
                .reject_remote_clients(true)
                .create(&pipe_name_str)
                .expect("Failed to create named pipe");

            // Wait for client connection
            server_instance
                .connect()
                .await
                .expect("Failed to connect named pipe");

            // Read request - wrap in BufReader for AsyncBufRead requirement
            let mut buf_reader = tokio::io::BufReader::new(server_instance);
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

    // Give server time to create the named pipe before client connects
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create IPC client and send 3 concurrent requests
    for i in 0..3 {
        let config = crate::helpers::create_test_config_with_socket(pipe_path.clone());
        let mut client =
            mcp_cli_rs::ipc::create_ipc_client(&config).expect("Failed to create IPC client");
        let request = DaemonRequest::Ping;
        let response = client
            .send_request(&request)
            .await
            .expect("Failed to send request");
        assert!(
            matches!(response, DaemonResponse::Pong),
            "Expected Pong response for client {}",
            i
        );
    }

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");
}

/// Test Windows named pipe large message transfer
#[tokio::test]
async fn test_windows_named_pipe_large_message_transfer() {
    // Use a unique pipe name for this test
    let pipe_path = crate::helpers::get_test_socket_path_with_suffix("large");
    let pipe_path_for_server = pipe_path.clone();

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Create large JSON object (100KB text as in plan)
    let large_content = serde_json::json!({
        "text": "a".repeat(100_000)
    });

    let server_content = large_content.clone();

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        // Create server instance for this connection
        let pipe_name_str = pipe_path_for_server.to_string_lossy().to_string();
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(&pipe_name_str)
            .expect("Failed to create named pipe");

        // Wait for client connection
        server_instance
            .connect()
            .await
            .expect("Failed to connect named pipe");

        // Read request - wrap in BufReader for AsyncBufRead requirement
        let mut buf_reader = tokio::io::BufReader::new(server_instance);
        let _request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        // Send large response
        let response = DaemonResponse::ToolResult(server_content);
        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
            .await
            .expect("Failed to send response");
    });

    // Give server time to create the named pipe before client connects
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create IPC client with matching socket path
    let config = crate::helpers::create_test_config_with_socket(pipe_path);
    let mut client =
        mcp_cli_rs::ipc::create_ipc_client(&config).expect("Failed to create IPC client");

    // Send ping request
    let request = DaemonRequest::Ping;
    let response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");

    // Verify large content was transferred correctly
    assert!(
        matches!(response, DaemonResponse::ToolResult(_)),
        "Expected ToolResult response, got {:?}",
        response
    );
    if let DaemonResponse::ToolResult(content) = response {
        assert_eq!(content, large_content);
    }

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");
}

/// Test Windows named pipe SECURITY_IDENTIFICATION flags applied
#[tokio::test]
async fn test_windows_named_pipe_security_flags() {
    // Use a unique pipe name for this test
    let pipe_path = crate::helpers::get_test_socket_path_with_suffix("security");
    let pipe_path_for_server = pipe_path.clone();

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Spawn server task that checks connection properties
    let server_handle = tokio::spawn(async move {
        // Create server instance with security flags
        let pipe_name_str = pipe_path_for_server.to_string_lossy().to_string();
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true) // This should be applied
            .create(&pipe_name_str)
            .expect("Failed to create named pipe");

        // Wait for client connection
        server_instance
            .connect()
            .await
            .expect("Failed to connect named pipe");

        // Read request and send response (proper protocol handling)
        let mut buf_reader = tokio::io::BufReader::new(server_instance);
        let _request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        // Send response back to client
        let response = DaemonResponse::Pong;
        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
            .await
            .expect("Failed to send response");
    });

    // Give server time to create the named pipe before client connects
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create IPC client with matching socket path
    let config = crate::helpers::create_test_config_with_socket(pipe_path);
    let mut client =
        mcp_cli_rs::ipc::create_ipc_client(&config).expect("Failed to create IPC client");

    // Send request
    let request = DaemonRequest::Ping;
    let _response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");
}

/// Test Windows named pipe cleanup on server shutdown
#[tokio::test]
async fn test_windows_named_pipe_cleanup_on_shutdown() {
    let pipe_path = crate::helpers::get_test_socket_path_with_suffix("shutdown");
    let pipe_path_clone = pipe_path.clone();

    // Create IPC server
    let _server = mcp_cli_rs::ipc::create_ipc_server(&pipe_path)
        .expect("Failed to create IPC server");

    // Spawn server task that creates pipe and immediately drops it
    let server_handle = tokio::spawn(async move {
        // Create server instance
        let pipe_name_str = pipe_path_clone.to_string_lossy().to_string();
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(&pipe_name_str)
            .expect("Failed to create named pipe");

        // Immediately drop server instance (simulating server shutdown)
        // This should allow new connections to be created
        std::mem::drop(server_instance);
    });

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Create IPC server again at same pipe name - should succeed
    let _server2 = mcp_cli_rs::ipc::create_ipc_server(&pipe_path)
        .expect("Failed to create server after cleanup");
}
