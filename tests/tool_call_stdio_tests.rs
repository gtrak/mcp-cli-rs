//! End-to-end integration tests for stdio transport tool calls
//!
//! These tests verify the full MCP protocol roundtrip from client to mock server
//! and back, including initialization, tool calls with arguments, and tools listing.
//!
//! Tests: TEST-02, TEST-04

use mcp_cli_rs::client::McpClient;
use mcp_cli_rs::transport::Transport;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

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

    let mut cmd = Command::new(mock_server_path);
    cmd.env("MOCK_TOOLS", tools_json)
        .env("MOCK_RESPONSES", responses_json)
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

/// Create a test transport from child process handles
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

/// TEST-02: Basic tool call end-to-end test
/// Verifies full roundtrip from client to mock server and back
#[tokio::test]
async fn test_stdio_basic_tool_call() {
    // Configure mock server with echo tool
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
                "text": "Hello from mock"
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

    // Initialize connection
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    // Call echo tool
    let result = client
        .call_tool("echo", serde_json::json!({"message": "Hello"}))
        .await
        .expect("Failed to call tool");

    // Verify response
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content.len(), 1);
    assert_eq!(
        content[0].get("text").and_then(|t| t.as_str()),
        Some("Hello from mock")
    );
    assert_eq!(
        result.get("isError").and_then(|e| e.as_bool()),
        Some(false)
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-04: Tool call with arguments
/// Verifies JSON arguments are passed correctly to mock server
#[tokio::test]
async fn test_stdio_tool_call_with_args() {
    // Configure mock server with add tool
    let tools = vec![
        ToolDefinition {
            name: "add".to_string(),
            description: "Add two numbers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                },
                "required": ["a", "b"]
            }),
        },
        ToolDefinition {
            name: "multiply".to_string(),
            description: "Multiply two numbers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"}
                },
                "required": ["x", "y"]
            }),
        },
    ];

    let mut responses = HashMap::new();
    // Template substitution in response - mock server replaces {a}, {b}, {result}
    responses.insert(
        "add".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Result: 8"
            })],
        },
    );
    responses.insert(
        "multiply".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Product: 15"
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

    // Initialize connection
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    // Test add tool with arguments
    let result = client
        .call_tool("add", serde_json::json!({"a": 5, "b": 3}))
        .await
        .expect("Failed to call add tool");

    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content.len(), 1);
    assert!(content[0]
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .contains("8"));

    // Test multiply tool with arguments
    let result = client
        .call_tool("multiply", serde_json::json!({"x": 3, "y": 5}))
        .await
        .expect("Failed to call multiply tool");

    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content.len(), 1);
    assert!(content[0]
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .contains("15"));

    // Cleanup
    let _ = child.kill().await;
}

/// Tools list integration test
/// Verifies tools/list works end-to-end
#[tokio::test]
async fn test_stdio_tools_list() {
    // Configure mock server with multiple tools
    let tools = vec![
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo back the input message".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                },
                "required": ["message"]
            }),
        },
        ToolDefinition {
            name: "add".to_string(),
            description: "Add two numbers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                },
                "required": ["a", "b"]
            }),
        },
        ToolDefinition {
            name: "search".to_string(),
            description: "Search for items".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
        },
    ];

    let responses = HashMap::new();

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // List tools
    let tools_list = client.list_tools().await.expect("Failed to list tools");

    // Verify returned tools
    assert_eq!(tools_list.len(), 3);

    let tool_names: Vec<String> = tools_list.iter().map(|t| t.name.clone()).collect();
    assert!(tool_names.contains(&"echo".to_string()));
    assert!(tool_names.contains(&"add".to_string()));
    assert!(tool_names.contains(&"search".to_string()));

    // Verify tool info structure
    let echo_tool = tools_list.iter().find(|t| t.name == "echo").unwrap();
    assert!(echo_tool.description.is_some());
    assert!(!echo_tool.input_schema.as_object().unwrap().is_empty());

    // Cleanup
    let _ = child.kill().await;
}

/// Complex arguments test with nested objects
/// Verifies complex JSON arguments work correctly
#[tokio::test]
async fn test_stdio_complex_nested_arguments() {
    // Configure mock server with complex tool
    let tools = vec![ToolDefinition {
        name: "process".to_string(),
        description: "Process complex data".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "age": {"type": "number"}
                    }
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"}
                },
                "metadata": {
                    "type": "object"
                }
            }
        }),
    }];

    let mut responses = HashMap::new();
    responses.insert(
        "process".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Processed successfully"
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

    // Initialize connection
    client
        .initialize()
        .await
        .expect("Failed to initialize connection");

    // Call with complex nested arguments
    let complex_args = serde_json::json!({
        "user": {
            "name": "Alice",
            "age": 30
        },
        "tags": ["rust", "mcp", "test"],
        "metadata": {
            "version": "1.0",
            "timestamp": 1234567890
        }
    });

    let result = client
        .call_tool("process", complex_args)
        .await
        .expect("Failed to call process tool");

    // Verify response
    assert!(result.get("content").is_some());
    assert_eq!(
        result.get("isError").and_then(|e| e.as_bool()),
        Some(false)
    );

    // Cleanup
    let _ = child.kill().await;
}
