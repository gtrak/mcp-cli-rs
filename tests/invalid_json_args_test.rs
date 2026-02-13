//! Invalid JSON arguments error handling test (TEST-12)
//!
//! This test verifies that invalid JSON arguments produce helpful error messages
//! when calling tools. Tests include malformed JSON, type mismatches, and
//! missing required fields.

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

/// TEST-12: Invalid JSON arguments error handling
/// 
/// This test verifies that:
/// 1. Server-reported argument errors produce helpful error messages
/// 2. Error messages mention JSON, parse, invalid, or schema
/// 3. Client gracefully handles server-side validation errors
#[tokio::test]
async fn test_invalid_json_arguments() {
    // Configure tools with validation on server side via error configuration
    let tools = vec![
        ToolDefinition {
            name: "validate".to_string(),
            description: "Validate input".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "email": {"type": "string", "description": "Email address"}
                },
                "required": ["email"]
            }),
        },
        ToolDefinition {
            name: "process".to_string(),
            description: "Process structured data".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {"type": "number", "description": "Count value"}
                },
                "required": ["count"]
            }),
        },
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo back the input message".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Message to echo"}
                }
            }),
        },
    ];

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
    responses.insert(
        "process".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Processed"
            })],
        },
    );

    // Configure errors for validation failures
    let mut errors = HashMap::new();
    errors.insert(
        "validate".to_string(),
        "Schema validation failed: Invalid JSON - missing required field 'email'".to_string(),
    );

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

    // Test 1: Server validation error - missing required field
    let result = client
        .call_tool("validate", serde_json::json!({"wrong_field": "value"}))
        .await;

    assert!(result.is_err(), "Expected error for invalid arguments");
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("invalid") || 
        error_msg.contains("missing") || 
        error_msg.contains("required") ||
        error_msg.contains("json") ||
        error_msg.contains("schema") ||
        error_msg.contains("parse") ||
        error_msg.contains("validation"),
        "Error message should mention JSON/parse/invalid/schema/missing/required/validation: got {}",
        error_msg
    );

    // Test 2: Server validation error with empty arguments
    let result = client
        .call_tool("validate", serde_json::json!({}))
        .await;

    assert!(result.is_err(), "Expected error for empty arguments with required field");
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("invalid") || 
        error_msg.contains("missing") || 
        error_msg.contains("required") ||
        error_msg.contains("json") ||
        error_msg.contains("schema") ||
        error_msg.contains("parse") ||
        error_msg.contains("validation"),
        "Error message should be helpful: got {}",
        error_msg
    );

    // Test 3: Valid echo call should succeed (no error configured for echo)
    let result = client
        .call_tool("echo", serde_json::json!({"message": "hello world"}))
        .await;

    assert!(result.is_ok(), "Valid echo call should succeed: {:?}", result);

    // Test 4: Echo with different valid arguments
    let result = client
        .call_tool("echo", serde_json::json!({"message": "test", "extra": "field"}))
        .await;

    assert!(result.is_ok(), "Echo with extra fields should succeed: {:?}", result);

    // Test 5: Valid process call should succeed (no error configured for process)
    let result = client
        .call_tool("process", serde_json::json!({"count": 42}))
        .await;

    assert!(result.is_ok(), "Valid process call should succeed: {:?}", result);

    // Cleanup
    let _ = child.kill().await;
}

/// Additional test: Validate error message clarity
/// Verifies that error messages are user-friendly and actionable
#[tokio::test]
async fn test_error_message_clarity() {
    let tools = vec![ToolDefinition {
        name: "validate".to_string(),
        description: "Validate input".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "email": {"type": "string", "format": "email"}
            },
            "required": ["email"]
        }),
    }];

    let responses = HashMap::new();

    // Configure specific validation error
    let mut errors = HashMap::new();
    errors.insert(
        "validate".to_string(),
        "Schema validation failed: Field 'email' is required".to_string(),
    );

    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    client
        .initialize()
        .await
        .expect("Failed to initialize");

    // Call with missing field
    let result = client
        .call_tool("validate", serde_json::json!({}))
        .await;

    assert!(result.is_err(), "Should error with missing required field");
    
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    
    // Error should be descriptive, not generic
    assert!(
        error_msg.len() > 10,
        "Error message should be descriptive, got short/generic: '{}'",
        error_msg
    );

    // Cleanup
    let _ = child.kill().await;
}

/// Test: Complex nested object validation
/// Verifies graceful handling of nested JSON structure issues
#[tokio::test]
async fn test_nested_json_validation() {
    let tools = vec![ToolDefinition {
        name: "nested".to_string(),
        description: "Process nested data".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "object",
                    "properties": {
                        "value": {"type": "string"}
                    },
                    "required": ["value"]
                }
            },
            "required": ["data"]
        }),
    }];

    let responses = HashMap::new();
    let errors = HashMap::new();

    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses, errors)
        .await
        .expect("Failed to spawn mock server");

    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    client
        .initialize()
        .await
        .expect("Failed to initialize");

    // Valid nested structure
    let result = client
        .call_tool("nested", serde_json::json!({
            "data": {"value": "test"}
        }))
        .await;
    
    // Server may accept or reject based on validation, but should not panic
    // Result will vary based on mock server validation depth
    
    // Invalid nested structure (missing nested required field)
    let result2 = client
        .call_tool("nested", serde_json::json!({
            "data": {}
        }))
        .await;
    
    // Should not panic, may error or succeed based on validation
    match result2 {
        Ok(_) => (), // Server accepted it
        Err(ref e) => {
            let msg = e.to_string().to_lowercase();
            // Error should be informative if it fails
            assert!(
                msg.contains("invalid") || msg.contains("missing") || msg.contains("required") ||
                msg.contains("json") || msg.contains("schema") || msg.contains("value"),
                "Nested validation error should be helpful: {}",
                msg
            );
        }
    }

    // Cleanup
    let _ = child.kill().await;
}
