//! IPC communication integration tests
//!
//! Tests platform-specific IPC implementations (Unix sockets vs Windows named pipes)
//! using the unified IpcServer and IpcClient traits.

#[cfg(test)]
mod ipc_tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
    use tokio::time::{timeout, sleep};

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
        let mut server = mcp_cli_rs::ipc::create_ipc_server(&socket_path, None)
            .expect("Failed to create IPC server");

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (mut stream, _addr) = timeout(Duration::from_secs(5), server.accept())
                .await
                .expect("Server accept timed out")
                .expect("Failed to accept connection");

            // Read request
            let request = mcp_cli_rs::daemon::protocol::receive_request(&mut stream)
                .await
                .expect("Failed to receive request");

            // Verify we got a Ping request
            assert!(matches!(request, DaemonRequest::Ping));

            // Send response
            let response = DaemonResponse::Pong;
            mcp_cli_rs::daemon::protocol::send_response(&mut stream, &response)
                .await
                .expect("Failed to send response");
        });

        // Create IPC client
        let client = mcp_cli_rs::ipc::create_ipc_client(&socket_path)
            .expect("Failed to create IPC client");

        // Send request and get response
        let request = DaemonRequest::Ping;
        let response = client.send_request(&request)
            .await
            .expect("Failed to send request");

        // Verify response
        assert!(matches!(response, DaemonResponse::Pong));

        // Wait for server to complete
        timeout(Duration::from_secs(5), server_handle)
            .await
            .expect("Server task timed out")
            .expect("Server task failed")
            .expect("Server failed");

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
        let mut server = mcp_cli_rs::ipc::create_ipc_server(&socket_path, None)
            .expect("Failed to create IPC server");

        // Spawn server task to handle multiple connections
        let server_handle = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _addr) = timeout(Duration::from_secs(5), server.accept())
                    .await
                    .expect("Server accept timed out")
                    .expect("Failed to accept connection");

                // Read request
                let request = mcp_cli_rs::daemon::protocol::receive_request(&mut stream)
                    .await
                    .expect("Failed to receive request");

                // Verify Ping request
                assert!(matches!(request, DaemonRequest::Ping));

                // Send response
                let response = DaemonResponse::Pong;
                mcp_cli_rs::daemon::protocol::send_response(&mut stream, &response)
                    .await
                    .expect("Failed to send response");
            }
        });

        // Wait for server to start
        sleep(Duration::from_millis(100)).await;

        // Create 3 concurrent client connections
        let client_handles = (0..3).map(|i| {
            let socket_path = socket_path.clone();
            tokio::spawn(async move {
                let client = mcp_cli_rs::ipc::create_ipc_client(&socket_path)
                    .expect("Failed to create IPC client");

                let request = DaemonRequest::Ping;
                client.send_request(&request).await
            })
        }).collect::<Vec<_>>();

        // Wait for all clients to complete
        for (i, handle) in client_handles.into_iter().enumerate() {
            let result = timeout(Duration::from_secs(5), handle)
                .await
                .expect(&format!("Client {} timed out", i))
                .expect(&format!("Client {} task failed", i))
                .expect(&format!("Client {} send_request failed", i));

            assert!(matches!(result, DaemonResponse::Pong));
        }

        // Wait for server to complete
        timeout(Duration::from_secs(5), server_handle)
            .await
            .expect("Server task timed out")
            .expect("Server task failed")
            .expect("Server failed");

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
        let mut server = mcp_cli_rs::ipc::create_ipc_server(&socket_path, None)
            .expect("Failed to create IPC server");

        // Create large JSON object (simulating tool response with big content)
        let large_content = serde_json::json!({
            "text": "a".repeat(100_000)  // 100KB text (reduced from 1MB for faster tests)
        });

        let server_content = large_content.clone();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (mut stream, _addr) = timeout(Duration::from_secs(10), server.accept())
                .await
                .expect("Server accept timed out")
                .expect("Failed to accept connection");

            // Read request
            let _request = mcp_cli_rs::daemon::protocol::receive_request(&mut stream)
                .await
                .expect("Failed to receive request");

            // Send large response
            let response = DaemonResponse::ToolResult(server_content);
            mcp_cli_rs::daemon::protocol::send_response(&mut stream, &response)
                .await
                .expect("Failed to send response");
        });

        // Create IPC client
        let client = mcp_cli_rs::ipc::create_ipc_client(&socket_path)
            .expect("Failed to create IPC client");

        // Send ping request
        let request = DaemonRequest::Ping;
        let response = client.send_request(&request)
            .await
            .expect("Failed to send request");

        // Verify large content was transferred correctly
        assert!(matches!(response, DaemonResponse::ToolResult(_)));
        if let DaemonResponse::ToolResult(content) = response {
            assert_eq!(content, large_content);
        }

        // Wait for server to complete
        timeout(Duration::from_secs(15), server_handle)
            .await
            .expect("Server task timed out")
            .expect("Server task failed")
            .expect("Server failed");

        // Clean up
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&socket_path);
        }
    }
}
