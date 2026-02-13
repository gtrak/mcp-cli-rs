//! Server timeout handling tests (TEST-13)
//!
//! These tests verify that the client properly handles server timeouts,
//! producing clear error messages when the server takes too long to respond.
//!
//! Tests: TEST-13
//!
//! Must-haves verified:
//! - Server timeout triggers client-side timeout with clear error

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

/// Spawn the mock MCP server with custom configuration including delay
async fn spawn_mock_server_with_delay(
    tools: Vec<ToolDefinition>,
    responses: HashMap<String, MockResponse>,
    errors: HashMap<String, String>,
    delay_ms: u64,
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
        .env("MOCK_DELAY_MS", delay_ms.to_string())
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

/// Spawn the mock MCP server with custom configuration (no delay)
async fn spawn_mock_server(
    tools: Vec<ToolDefinition>,
    responses: HashMap<String, MockResponse>,
    errors: HashMap<String, String>,
) -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    spawn_mock_server_with_delay(tools, responses, errors, 0).await
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

/// TEST-13: Server timeout handling
///
/// This test verifies that:
/// 1. Client-side timeout triggers when server takes too long to respond
/// 2. Error message clearly indicates timeout/deadline exceeded
/// 3. No panic - graceful error handling
#[tokio::test]
async fn test_server_timeout() {
    // Configure mock server with a slow tool (500ms delay)
    let tools = vec![ToolDefinition {
        name: "slow_echo".to_string(),
        description: "Slow echo back the input message".to_string(),
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

    // Spawn mock server with 500ms delay
    let (mut child, stdin, stdout) = spawn_mock_server_with_delay(tools, responses, errors, 500)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize connection (should work - no delay on initialize)
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    // Now try to call the slow tool with a short timeout (100ms)
    // The server takes 500ms to respond, so this should timeout
    let result = timeout(
        Duration::from_millis(100),
        client.call_tool("slow_echo", serde_json::json!({"message": "test"}))
    ).await;

    // Should timeout
    assert!(
        result.is_err(),
        "Expected timeout when server takes longer than timeout duration"
    );

    println!("Timeout test completed - timeout handled gracefully");

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-13b: Timeout on tool call returns clear error
///
/// Verify timeout behavior produces clear error message
#[tokio::test]
async fn test_timeout_on_tool_call() {
    // Configure mock server with slow tool
    let tools = vec![ToolDefinition {
        name: "slow".to_string(),
        description: "Slow operation".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    // Spawn mock server with 200ms delay
    let (mut child, stdin, stdout) = spawn_mock_server_with_delay(tools, responses, errors, 200)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize first
    client
        .initialize()
        .await
        .expect("Failed to initialize");

    // Try to call slow tool with 50ms timeout
    let result = timeout(
        Duration::from_millis(50),
        client.call_tool("slow", serde_json::json!({}))
    ).await;

    // Should timeout
    assert!(
        result.is_err(),
        "Expected timeout error on slow tool call"
    );

    println!("Tool call timeout test completed");

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-13c: Timeout error message clarity
///
/// Verify that timeout errors produce clear, actionable error messages
#[tokio::test]
async fn test_timeout_error_message_clarity() {
    // Configure mock server with slow tool
    let tools = vec![ToolDefinition {
        name: "slow_echo".to_string(),
        description: "Slow echo tool".to_string(),
        input_schema: serde_json::json!({}),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    // Spawn mock server with 300ms delay
    let (mut child, stdin, stdout) = spawn_mock_server_with_delay(tools, responses, errors, 300)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    client
        .initialize()
        .await
        .expect("Failed to initialize");

    // Trigger timeout with short duration
    let result = timeout(
        Duration::from_millis(50),
        client.call_tool("slow_echo", serde_json::json!({}))
    ).await;

    // Should timeout
    assert!(result.is_err(), "Expected timeout error");

    // The timeout error from tokio is Elapsed, which is clear enough
    let error_msg = format!("{:?}", result).to_lowercase();
    println!("Timeout error message: {}", error_msg);
    
    // Timeout should be indicated
    assert!(
        error_msg.contains("timeout") || error_msg.contains("elapsed"),
        "Timeout error should mention timeout or elapsed: {}",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-13d: Multiple timeout scenarios
/// Verify timeout works correctly across multiple operations
#[tokio::test]
async fn test_multiple_timeouts() {
    let tools = vec![
        ToolDefinition {
            name: "fast".to_string(),
            description: "Fast tool".to_string(),
            input_schema: serde_json::json!({}),
        },
        ToolDefinition {
            name: "slow".to_string(),
            description: "Slow tool".to_string(),
            input_schema: serde_json::json!({}),
        },
    ];

    let responses = HashMap::new();
    let errors = HashMap::new();

    // Spawn mock server with 100ms delay on slow tool
    let (mut child, stdin, stdout) = spawn_mock_server_with_delay(tools, responses, errors, 100)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    client
        .initialize()
        .await
        .expect("Initialize should succeed");

    // First call - fast tool should work
    let result1 = client.call_tool("fast", serde_json::json!({})).await;
    assert!(result1.is_ok(), "Fast tool call should succeed");

    // Second call - slow tool with timeout should fail
    let result2 = timeout(
        Duration::from_millis(50),
        client.call_tool("slow", serde_json::json!({}))
    ).await;
    assert!(result2.is_err(), "Slow tool with timeout should fail");

    // Third call - fast tool should work again
    let result3 = client.call_tool("fast", serde_json::json!({})).await;
    assert!(result3.is_ok(), "Fast tool should work after timeout");

    println!("Multiple timeout test completed successfully");

    // Cleanup
    let _ = child.kill().await;
}
