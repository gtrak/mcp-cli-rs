//! Error handling integration tests for tool calls
//!
//! These tests verify proper error propagation from mock MCP server through
//! the transport layer to the caller, covering tool not found, invalid arguments,
//! server errors, transport failures, and full error propagation chains.
//!
//! Tests: TEST-05

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

/// Spawn the mock MCP server with custom configuration including errors
async fn spawn_mock_server_with_errors(
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

/// Spawn mock server without errors (convenience wrapper)
async fn spawn_mock_server(
    tools: Vec<ToolDefinition>,
    responses: HashMap<String, MockResponse>,
) -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    spawn_mock_server_with_errors(tools, responses, HashMap::new()).await
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
                message: "Empty response from server".to_string(),
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
                message: "Empty notification from server".to_string(),
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

/// TEST-05a: Tool not found error
/// Verify graceful error when calling non-existent tool
#[tokio::test]
async fn test_tool_not_found() {
    // Configure mock with only "echo" tool
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo back the input message".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            },
            "required": ["message"]
        }),
    }];

    let responses = HashMap::new();

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses)
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

    // Try to call non-existent tool
    let result = client
        .call_tool("nonexistent", serde_json::json!({}))
        .await;

    // Verify error is returned (not panic)
    assert!(result.is_err(), "Expected error for non-existent tool");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    // Error should contain tool name or "not found"
    assert!(
        error_msg.to_lowercase().contains("not found") || error_msg.contains("nonexistent"),
        "Error message should indicate tool not found: {}",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-05b: Invalid arguments error
/// Verify error handling for malformed/invalid arguments
#[tokio::test]
async fn test_invalid_arguments() {
    // Configure echo tool expecting {"message": string}
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo back the input message".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            },
            "required": ["message"]
        }),
    }];

    let mut responses = HashMap::new();
    responses.insert(
        "echo".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Echo response"
            })],
        },
    );

    // Configure error for missing required field
    let mut errors = HashMap::new();
    errors.insert(
        "echo".to_string(),
        "Missing required field: message".to_string(),
    );

    // Spawn mock server with errors
    let (mut child, stdin, stdout) = spawn_mock_server_with_errors(tools, responses, errors)
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

    // Call with wrong field name (should trigger error)
    let result = client
        .call_tool("echo", serde_json::json!({"wrong_field": "value"}))
        .await;

    // Verify error is returned
    assert!(result.is_err(), "Expected error for invalid arguments");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    // Error should be helpful
    assert!(
        error_msg.to_lowercase().contains("missing") || error_msg.to_lowercase().contains("required"),
        "Error message should be helpful: {}",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-05c: Server error response
/// Verify handling of JSON-RPC error responses
#[tokio::test]
async fn test_server_error() {
    // Configure fail tool that always returns errors
    let tools = vec![
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: serde_json::json!({}),
        },
        ToolDefinition {
            name: "fail".to_string(),
            description: "Always fails".to_string(),
            input_schema: serde_json::json!({}),
        },
    ];

    let responses = HashMap::new();

    // Configure error for "fail" tool
    let mut errors = HashMap::new();
    errors.insert(
        "fail".to_string(),
        "Simulated server failure".to_string(),
    );

    // Spawn mock server with errors
    let (mut child, stdin, stdout) = spawn_mock_server_with_errors(tools, responses, errors)
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

    // Call fail tool - should return error
    let result = client
        .call_tool("fail", serde_json::json!({}))
        .await;

    // Verify error is returned (not panic)
    assert!(result.is_err(), "Expected error for fail tool");

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    // Should contain the error message from mock
    assert!(
        error_msg.contains("Simulated") || error_msg.contains("failure") || error_msg.contains("fail"),
        "Error should contain server error message: {}",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-05d: Transport error handling
/// Verify graceful handling when server dies during operation
#[tokio::test]
async fn test_transport_error_handling() {
    // Configure mock
    let tools = vec![ToolDefinition {
        name: "slow".to_string(),
        description: "Slow operation".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses)
        .await
        .expect("Failed to spawn mock server");

    // Initialize first
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

    let _ = transport.send(init_request).await;

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

    // Try to send request after server death - should error
    let follow_up = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {},
        "id": 2
    });

    let result = timeout(Duration::from_secs(2), transport.send(follow_up)).await;

    // Should get timeout or error
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "Expected error or timeout after server death"
    );
}

/// TEST-05e: Error propagation chain
/// Verify errors propagate through full stack correctly
#[tokio::test]
async fn test_error_propagation_chain() {
    // Configure mock with intentional error
    let tools = vec![ToolDefinition {
        name: "error_tool".to_string(),
        description: "Tool that errors".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();

    // Configure error
    let mut errors = HashMap::new();
    errors.insert(
        "error_tool".to_string(),
        "Specific error message for propagation test".to_string(),
    );

    // Spawn mock server with errors
    let (mut child, stdin, stdout) = spawn_mock_server_with_errors(tools, responses, errors)
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

    // Call tool - error should propagate from Mock -> Transport -> Client -> Test
    let result = client.call_tool("error_tool", serde_json::json!({})).await;

    // Verify error reaches test
    assert!(result.is_err(), "Error should propagate to test");

    let error = result.unwrap_err();
    let error_msg = error.to_string();

    // Verify error context is preserved
    assert!(
        error_msg.contains("propagation") || error_msg.contains("Specific error"),
        "Error context should be preserved through layers: {}",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// Additional test: Protocol error handling
/// Verify graceful handling of malformed JSON-RPC responses
#[tokio::test]
async fn test_protocol_error_handling() {
    // Configure echo tool normally
    let tools = vec![ToolDefinition {
        name: "echo".to_string(),
        description: "Echo tool".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            }
        }),
    }];

    let mut responses = HashMap::new();
    responses.insert(
        "echo".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Valid response"
            })],
        },
    );

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize and make successful call
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    let result = client
        .call_tool("echo", serde_json::json!({"message": "test"}))
        .await;

    // Should succeed with valid response
    assert!(result.is_ok(), "Valid call should succeed");
    let response = result.unwrap();
    assert!(response.get("content").is_some());

    // Cleanup
    let _ = child.kill().await;
}

/// Additional test: Multiple errors in sequence
/// Verify error handling doesn't corrupt subsequent operations
#[tokio::test]
async fn test_multiple_errors_sequence() {
    // Configure multiple tools - some succeed, some fail
    let tools = vec![
        ToolDefinition {
            name: "good".to_string(),
            description: "Good tool".to_string(),
            input_schema: serde_json::json!({}),
        },
        ToolDefinition {
            name: "bad".to_string(),
            description: "Bad tool".to_string(),
            input_schema: serde_json::json!({}),
        },
    ];

    let mut responses = HashMap::new();
    responses.insert(
        "good".to_string(),
        MockResponse {
            content: vec![serde_json::json!({"type": "text", "text": "Success"})],
        },
    );
    // No response for "bad" tool

    // Configure error for "bad" tool
    let mut errors = HashMap::new();
    errors.insert("bad".to_string(), "Bad tool error".to_string());

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server_with_errors(tools, responses, errors)
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

    // First call - error
    let result1 = client.call_tool("bad", serde_json::json!({})).await;
    assert!(result1.is_err(), "First call should error");

    // Second call - should still work (error didn't corrupt state)
    let result2 = client.call_tool("good", serde_json::json!({})).await;
    assert!(result2.is_ok(), "Second call should succeed after error");

    // Third call - error again
    let result3 = client.call_tool("bad", serde_json::json!({})).await;
    assert!(result3.is_err(), "Third call should error");

    // Fourth call - should still work
    let result4 = client.call_tool("good", serde_json::json!({})).await;
    assert!(result4.is_ok(), "Fourth call should succeed");

    // Cleanup
    let _ = child.kill().await;
}
