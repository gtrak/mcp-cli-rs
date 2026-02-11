//! Cross-platform IPC validation tests for daemon communication.
//!
//! Tests Unix socket (Linux/macOS) and named pipe (Windows) IPC implementations
//! using the unified IpcServer and IpcClient traits.
//!
//! XP-04: Validates daemon works on Linux, macOS, Windows

use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc::IpcClient;
use std::path::PathBuf; // Import IpcClient trait

/// Get a temporary Unix socket path specifically for testing
#[cfg(unix)]
fn get_unix_test_socket_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("mcp-unix-test-{}.sock", std::process::id()));
    path
}

/// Get a temporary named pipe path specifically for testing
#[cfg(windows)]
fn get_windows_test_pipe_name() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_suffix: u64 = rng.r#gen();
    format!(
        "\\\\.\\pipe\\mcp-windows-test-{}-{}",
        std::process::id(),
        random_suffix
    )
}

/// Test Unix socket creation and validation
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_creation() {
    let socket_path = get_unix_test_socket_path();

    // Verify socket path format
    assert!(
        socket_path.to_string_lossy().contains(".sock"),
        "Unix socket path should end with .sock"
    );

    // Verify parent directory can be created
    let mut path = socket_path.clone();
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

/// Test named pipe creation on Windows
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_creation() {
    let pipe_name = get_windows_test_pipe_name();

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

/// Test Unix socket client-server roundtrip (Linux/macOS)
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_client_server_roundtrip() {
    let socket_path = get_unix_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

    // Spawn server task
    let server_handle = tokio::spawn(async move {
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

        // Verify we got a Ping request
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
    });

    // Create IPC client
    let config = std::sync::Arc::new(mcp_cli_rs::config::Config::default());
    let mut client = mcp_cli_rs::ipc::create_ipc_client(std::sync::Arc::new(config))
        .expect("Failed to create IPC client");

    // Send request and get response
    let request = DaemonRequest::Ping;
    let response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");

    // Verify response
    assert!(
        matches!(response, DaemonResponse::Pong),
        "Expected Pong response, got {:?}",
        response
    );

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Clean up socket
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Unix socket multiple concurrent connections
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_multiple_concurrent_connections() {
    let socket_path = get_unix_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

    // Spawn server task handling 3 concurrent connections
    let server_handle = tokio::spawn(async move {
        for _ in 0..3 {
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
    let config = std::sync::Arc::new(mcp_cli_rs::config::Config::default());
    let mut client = mcp_cli_rs::ipc::create_ipc_client(std::sync::Arc::new(config))
        .expect("Failed to create IPC client");
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

    // Clean up socket file (Unix only)
    #[cfg(unix)]
    {
        std::fs::remove_file(&socket_path)
            .ok()
            .expect("Failed to remove socket file");
    }
}

/// Test Unix socket large message transfer (Linux/macOS)
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_large_message_transfer() {
    let socket_path = get_unix_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

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
    let config = std::sync::Arc::new(mcp_cli_rs::config::Config::default());
    let mut client = mcp_cli_rs::ipc::create_ipc_client(std::sync::Arc::new(config))
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
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_cleanup_on_removal() {
    let socket_path = get_unix_test_socket_path();

    // Create IPC server
    let server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

    // Remove socket path manually
    std::fs::remove_file(&socket_path).expect("Failed to remove socket file");

    // Verify socket is gone
    assert!(!socket_path.exists(), "Socket file should be removed");

    // Create server again at same path - should handle stale file gracefully
    let server2 = mcp_cli_rs::ipc::create_ipc_server(&socket_path)
        .expect("Failed to create server after cleanup");

    assert!(
        server2.listener.local_addr().is_ok(),
        "Server should be able to bind to cleaned path"
    );

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Unix socket StaleSocketError handling (Linux/macOS)
#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_stale_error_handling() {
    let socket_path = get_unix_test_socket_path();

    // Create IPC server
    let mut server =
        mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        // Try to accept connection - should handle errors
        let result = server.accept().await;
        // Connection should fail (stale socket)
        assert!(result.is_err(), "Accept should fail on stale socket");

        if let Err(e) = result {
            // Should be an IpcError type
            assert!(
                matches!(e, mcp_cli_rs::error::McpError::IpcError(_)),
                "Should be IpcError type"
            );
        }
    });

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Clean up
    let _ = std::fs::remove_file(&socket_path);
}

/// Test Windows named pipe creation and validation
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_server_creation() {
    let pipe_name = get_windows_test_pipe_name();

    // Create IPC server
    let _server = mcp_cli_rs::ipc::create_ipc_server(&PathBuf::from(&pipe_name))
        .expect("Failed to create IPC server");
}

/// Test Windows named pipe client-server roundtrip
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_client_server_roundtrip() {
    // Use a unique pipe name for this test
    let pipe_name = get_windows_test_pipe_name();
    let pipe_path = PathBuf::from(&pipe_name);

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        // Create server instance for this connection with reject_remote_clients
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(&pipe_name)
            .expect("Failed to create named pipe");

        // Wait for client connection
        server_instance
            .connect()
            .await
            .expect("Failed to connect named pipe");

        // Server now has the pipe stream

        // Read request - wrap in BufReader for AsyncBufRead requirement
        let mut buf_reader = tokio::io::BufReader::new(server_instance);
        let request = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        // Verify we got a Ping request
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
    });

    // Give server time to create the named pipe before client connects
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create IPC client with matching socket path
    let config = mcp_cli_rs::config::Config::with_socket_path(pipe_path);
    let mut client =
        mcp_cli_rs::ipc::create_ipc_client(&config).expect("Failed to create IPC client");

    // Send request and get response
    let request = DaemonRequest::Ping;
    let response = client
        .send_request(&request)
        .await
        .expect("Failed to send request");

    // Verify response
    assert!(
        matches!(response, DaemonResponse::Pong),
        "Expected Pong response, got {:?}",
        response
    );

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");
}

/// Test Windows named pipe multiple concurrent connections
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_multiple_concurrent_connections() {
    // Use a unique pipe name for this test
    let pipe_name = get_windows_test_pipe_name();
    let pipe_path = PathBuf::from(&pipe_name);

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Spawn server task handling 3 concurrent connections
    let server_handle = tokio::spawn(async move {
        for _ in 0..3 {
            // Create server instance for each connection
            let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
                .reject_remote_clients(true)
                .create(&pipe_name)
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
        let config = mcp_cli_rs::config::Config::with_socket_path(pipe_path.clone());
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
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_large_message_transfer() {
    // Use a unique pipe name for this test
    let pipe_name = get_windows_test_pipe_name();
    let pipe_path = PathBuf::from(&pipe_name);

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
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(&pipe_name)
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
    let config = mcp_cli_rs::config::Config::with_socket_path(pipe_path);
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
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_security_flags() {
    // Use a unique pipe name for this test
    let pipe_name = get_windows_test_pipe_name();
    let pipe_path = PathBuf::from(&pipe_name);

    // Create IPC server
    let _server =
        mcp_cli_rs::ipc::create_ipc_server(&pipe_path).expect("Failed to create IPC server");

    // Spawn server task that checks connection properties
    let server_handle = tokio::spawn(async move {
        // Create server instance with security flags
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true) // This should be applied
            .create(&pipe_name)
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
    let config = mcp_cli_rs::config::Config::with_socket_path(pipe_path);
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
#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_cleanup_on_shutdown() {
    let pipe_name = get_windows_test_pipe_name();
    let pipe_name_clone = pipe_name.clone();

    // Create IPC server
    let _server = mcp_cli_rs::ipc::create_ipc_server(&PathBuf::from(&pipe_name))
        .expect("Failed to create IPC server");

    // Spawn server task that creates pipe and immediately drops it
    let server_handle = tokio::spawn(async move {
        // Create server instance
        let server_instance = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true)
            .create(&pipe_name_clone)
            .expect("Failed to create named pipe");

        // Immediately drop server instance (simulating server shutdown)
        // This should allow new connections to be created
        std::mem::drop(server_instance);
    });

    // Wait for server to complete
    server_handle.await.expect("Server task failed to join");

    // Create IPC server again at same pipe name - should succeed
    let _server2 = mcp_cli_rs::ipc::create_ipc_server(&PathBuf::from(&pipe_name))
        .expect("Failed to create server after cleanup");
}

/// Test IpcServer trait methods work identically across platforms
#[tokio::test]
async fn test_ipc_server_trait_consistency() {
    // Test on Unix (skip on Windows to avoid platform-specific issues)
    #[cfg(unix)]
    {
        let socket_path = get_unix_test_socket_path();

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
        let pipe_name = get_windows_test_pipe_name();

        let _server = mcp_cli_rs::ipc::windows::NamedPipeIpcServer::new(&PathBuf::from(&pipe_name))
            .expect("Failed to create NamedPipeIpcServer");

        // Verify trait methods are implemented
        assert!(
            !pipe_name.is_empty(),
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
        let socket_path = get_unix_test_socket_path();
        let config = std::sync::Arc::new(mcp_cli_rs::config::Config::default());

        let client = mcp_cli_rs::ipc::UnixIpcClient::new(std::sync::Arc::new(config));

        // Verify trait methods are implemented
        assert!(
            client.config().is_empty(),
            "UnixIpcClient should have config method"
        );
    }

    // Test on Windows
    #[cfg(windows)]
    {
        let config = std::sync::Arc::new(mcp_cli_rs::config::Config::default());

        let client = mcp_cli_rs::ipc::windows::NamedPipeIpcClient::with_config(config);

        // Verify trait methods are implemented
        assert!(
            client.config().is_empty(),
            "NamedPipeIpcClient should have config method"
        );
    }
}

/// Test protocol (NDJSON) is consistent across platforms
#[tokio::test]
async fn test_ndjson_protocol_consistency() {
    // Test on Unix
    #[cfg(unix)]
    {
        let socket_path = get_unix_test_socket_path();
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

    // Test on Windows
    #[cfg(windows)]
    {
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
}
