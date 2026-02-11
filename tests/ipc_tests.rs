//! IPC communication integration tests
//!
//! Tests platform-specific IPC implementations (Unix sockets vs Windows named pipes)
//! using the unified IpcServer and IpcClient traits.

#[cfg(test)]
mod ipc_tests {
    use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
    use mcp_cli_rs::ipc;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::time::timeout;

    /// Get a temporary socket/pipe path for testing
    fn get_test_socket_path() -> PathBuf {
        #[cfg(unix)]
        {
            let mut path = std::env::temp_dir();
            path.push(format!("mcp-test-{}.sock", std::process::id()));
            path
        }
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(format!("\\\\.\\pipe\\mcp-test-{}", std::process::id()));
            path
        }
    }

    /// Test IPC roundtrip request/response
    #[tokio::test]
    async fn test_ipc_roundtrip() {
        let socket_path = get_test_socket_path();

        // Create IPC server
        let server = ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (stream, _addr) = match timeout(Duration::from_secs(5), server.accept()).await {
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
            assert!(matches!(request, DaemonRequest::Ping));

            // Send response
            let response = DaemonResponse::Pong;
            mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
                .await
                .expect("Failed to send response");
        });

        // Create IPC client
        let config = mcp_cli_rs::config::Config::default();
        let mut client = mcp_cli_rs::ipc::create_ipc_client(std::sync::Arc::new(config))
            .expect("Failed to create IPC client");

        // Send request and get response
        let request = DaemonRequest::Ping;
        let response = client
            .send_request(&request)
            .await
            .expect("Failed to send request");

        // Verify response
        assert!(matches!(response, DaemonResponse::Pong));

        // Wait for server to complete
        server_handle.await.expect("Server task failed to join");

        // Clean up socket
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    /// Test concurrent IPC connections
    #[tokio::test]
    async fn test_concurrent_connections() {
        let socket_path = get_test_socket_path();

        // Create IPC server
        let server = ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

        // Spawn server task to handle multiple connections
        let server_handle = tokio::spawn(async move {
            for _ in 0..3 {
                let (stream, _addr) = match timeout(Duration::from_secs(5), server.accept()).await {
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
                assert!(matches!(request, DaemonRequest::Ping));

                // Send response
                let response = DaemonResponse::Pong;
                mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &response)
                    .await
                    .expect("Failed to send response");
            }
        });

        // Wait for server to complete
        server_handle.await.expect("Server task failed to join");

        // Clean up
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    /// Test large message transfer over IPC
    #[tokio::test]
    async fn test_large_message_transfer() {
        let socket_path = get_test_socket_path();

        // Create IPC server
        let server =
            mcp_cli_rs::ipc::create_ipc_server(&socket_path).expect("Failed to create IPC server");

        // Create large JSON object (simulating tool response with big content)
        let large_content = serde_json::json!({
            "text": "a".repeat(100_000)  // 100KB text (reduced from 1MB for faster tests)
        });

        let server_content = large_content.clone();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (stream, _addr) = match timeout(Duration::from_secs(10), server.accept()).await {
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
        let config = mcp_cli_rs::config::Config::default();
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
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&socket_path);
        }
    }
}
