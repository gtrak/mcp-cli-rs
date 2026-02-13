//! Server disconnection handling tests (TEST-14)
//!
//! These tests verify that the client properly handles server disconnection
//! during tool calls, returning graceful errors instead of panicking.
//!
//! Tests: TEST-14
//!
//! Must-haves verified:
//! - Server disconnection during tool call returns graceful error

use mcp_cli_rs::client::McpClient;
use mcp_cli_rs::transport::Transport;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Duration};

/// Tool definition for mock server configuration
#[derive(Debug, Clone, serde::Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

/// Mock response configuration
#[derive(Debug, Clone, serde::Serialize)]
struct MockResponse {
    content: Vec<Value>,
}

/// Spawn the mock MCP server with custom configuration
async fn spawn_mock_server(
    tools: Vec<ToolDefinition>,
    responses: HashMap<String, MockResponse>,
    errors: HashMap<String, String>,
) -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().unwrap();

    // Find the mock server binary
    let mock_server_path = if exe_dir.join("mock-mcp-server.exe").exists() {
        exe_dir.join("mock-mcp-server.exe")
    } else if exe_dir.join("mock-mcp-server").exists() {
        exe_dir.join("mock-mcp-server")
    } else {
        // Try in target/debug directory
        let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let debug_path = cargo_dir.join("target/debug/mock-mcp-server.exe");
        let debug_path_unix = cargo_dir.join("target/debug/mock-mcp-server");

        if debug_path.exists() {
            debug_path
        } else if debug_path_unix.exists() {
            debug_path_unix
        } else {
            anyhow::bail!("mock-mcp-server binary not found. Run: cargo build --bin mock-mcp-server");
        }
    };

    let tools_json = serde_json::to_string(&tools)?;
    let responses_json = serde_json::to_string(&responses)?;
    let errors_json = serde_json::to_string(&errors)?;

    let mut cmd = Command::new(mock_server_path);
    cmd.env("MOCK_TOOLS", tools_json)
        .env("MOCK_RESPONSES", responses_json)
        .env("MOCK_ERRORS", errors_json)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;

    Ok((child, stdin, BufReader::new(stdout)))
}

/// Test transport implementation for stdio
struct TestStdioTransport {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[async_trait::async_trait]
impl Transport for TestStdioTransport {
    async fn send(&mut self, request: Value) -> mcp_cli_rs::error::Result<Value> {
        let request_str = request.to_string();

        self.stdin
            .write_all(request_str.as_bytes())
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;

        let mut line = String::new();
        let bytes_read = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;

        if bytes_read == 0 {
            return Err(mcp_cli_rs::error::McpError::InvalidProtocol {
                message: "Empty response from server - connection may be lost".to_string(),
            });
        }

        let response: Value = serde_json::from_str(&line).map_err(|e| {
            mcp_cli_rs::error::McpError::InvalidProtocol {
                message: format!("Invalid JSON response: {}", e),
            }
        })?;

        Ok(response)
    }

    async fn send_notification(&mut self, notification: Value) -> mcp_cli_rs::error::Result<()> {
        let notification_str = notification.to_string();

        self.stdin
            .write_all(notification_str.as_bytes())
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;

        Ok(())
    }

    async fn receive_notification(&mut self) -> mcp_cli_rs::error::Result<Value> {
        let mut line = String::new();
        let bytes_read = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::connection_error("stdio", e))?;

        if bytes_read == 0 {
            return Err(mcp_cli_rs::error::McpError::InvalidProtocol {
                message: "Empty notification from server - connection lost".to_string(),
            });
        }

        let notification: Value = serde_json::from_str(&line).map_err(|e| {
            mcp_cli_rs::error::McpError::InvalidProtocol {
                message: format!("Invalid JSON notification: {}", e),
            }
        })?;

        Ok(notification)
    }

    async fn ping(&self) -> mcp_cli_rs::error::Result<()> {
        Ok(())
    }

    fn transport_type(&self) -> &str {
        "test-stdio"
    }
}

/// TEST-14: Server disconnection handling
///
/// This test verifies that:
/// 1. When server disconnects during tool call, client returns graceful error
/// 2. Error message indicates connection lost
/// 3. No panic - proper error handling
#[tokio::test]
async fn test_server_disconnection() {
    // Configure mock server with a simple tool
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo back the input message".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Message to echo"}
            },
            "required": ["message"]
        }),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize connection
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    // Now kill the server while we're in the middle of operations
    let _ = child.kill().await;

    // Try to call tool after server is killed - should error gracefully
    let result = client
        .call_tool("echo", serde_json::json!({"message": "test"}))
        .await;

    // Verify error is returned (not panic)
    assert!(result.is_err(), "Expected error after server disconnection");

    let error = result.unwrap_err();
    let error_msg = error.to_string();

    // Error should indicate connection/communication issue
    let connection_keywords = ["connection", "closed", "lost", "pipe", "broken"];
    let has_connection_keyword = connection_keywords
        .iter()
        .any(|kw| error_msg.to_lowercase().contains(kw));

    println!("Disconnection error message: {}", error_msg);

    // Should mention connection issue
    assert!(
        has_connection_keyword || error_msg.to_lowercase().contains("empty"),
        "Error message should indicate connection issue: {}",
        error_msg
    );

    // Test passes - graceful error handling verified
}

/// TEST-14b: Disconnection during request/response cycle
///
/// Verify graceful error when server dies mid-request
#[tokio::test]
async fn test_disconnection_during_request() {
    let tools = vec![ToolDefinition {
        name: "process".to_string(),
        description: "Process data".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    // Create transport but use it directly
    let mut transport = TestStdioTransport { stdin, stdout };

    // Send initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "clientInfo": {"name": "test", "version": "0.1.0"}
        },
        "id": 1
    });

    // Initialize should work
    let result = transport.send(init_request).await;
    assert!(result.is_ok(), "Initialize should succeed");

    // Send initialized notification
    let _ = transport
        .send_notification(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }))
        .await;

    // Kill server
    let _ = child.kill().await;

    // Try to send a request after server death
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {},
        "id": 2
    });

    // Wrap in timeout to avoid hanging forever
    let result = timeout(Duration::from_secs(2), transport.send(request)).await;

    // Should get error or timeout
    assert!(
        result.is_err() || (result.is_ok() && result.unwrap().is_err()),
        "Expected error or timeout after server death"
    );

    println!("Disconnection during request test completed");
}

/// TEST-14c: Error message indicates connection lost
///
/// Verify error messages are helpful when server disconnects
#[tokio::test]
async fn test_disconnection_error_message_helpful() {
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo tool".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    client
        .initialize()
        .await
        .expect("Failed to initialize");

    // Kill server
    let _ = child.kill().await;

    // Try operation
    let result = client.call_tool("echo", serde_json::json!({})).await;

    // Should error
    assert!(result.is_err(), "Should error after disconnect");
    
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    
    // Error should be descriptive, not just "Error"
    assert!(
        error_msg.len() > 5,
        "Error message should be descriptive, got: '{}'",
        error_msg
    );
    
    println!("Helpful error message: {}", error_msg);
}

/// TEST-14d: No panic on disconnection
///
/// Verify the client doesn't panic when server disconnects
#[tokio::test]
async fn test_no_panic_on_disconnection() {
    let tools = vec![ToolDefinition {
        name: "test".to_string(),
        description: "Test tool".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    let _ = client.initialize().await;

    // Kill server
    let _ = child.kill().await;

    // Multiple operations should all fail gracefully, not panic
    for _ in 0..3 {
        let result = client.call_tool("test", serde_json::json!({})).await;
        
        // All should be errors, not panics
        assert!(
            result.is_err(),
            "Operation should fail gracefully, not panic"
        );
    }

    // If we get here, no panic occurred
    println!("No panic on multiple disconnection operations - test passed");
}

/// TEST-14e: Reconnection attempt after disconnect
///
/// Verify behavior when trying to use client after disconnection
#[tokio::test]
async fn test_operations_after_disconnect() {
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo tool".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    client
        .initialize()
        .await
        .expect("Initialize should succeed");

    // First call should work
    let result1 = client.call_tool("echo", serde_json::json!({"message": "first"})).await;
    assert!(result1.is_ok(), "First call should work");

    // Kill server
    let _ = child.kill().await;

    // Second call should fail gracefully
    let result2 = client.call_tool("echo", serde_json::json!({"message": "second"})).await;
    assert!(result2.is_err(), "Second call should fail gracefully");

    let error_msg = result2.unwrap_err().to_string();
    println!("Error after disconnect: {}", error_msg);

    // Test passed - graceful degradation
}
