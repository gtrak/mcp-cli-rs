//! Tool filtering and call integration tests
//!
//! These tests verify the integration between tool filtering (search/list)
//! and tool execution (call) functionality.
//!
//! TEST-17: Tool filtering + call integration

use mcp_cli_rs::client::McpClient;
use mcp_cli_rs::transport::Transport;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Tool definition for mock server configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

/// Mock response configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// TEST-17-01: Test filtering tools by name pattern, then calling a specific tool
/// This simulates the workflow of: search/filter tools -> identify target -> execute
#[tokio::test]
async fn test_filter_then_call_tool() {
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
            name: "echo_reverse".to_string(),
            description: "Echo back the input message in reverse".to_string(),
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
    ];

    let mut responses = HashMap::new();
    responses.insert(
        "echo".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "echo: Hello"
            })],
        },
    );
    responses.insert(
        "echo_reverse".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "echo_reverse: olleH"
            })],
        },
    );
    responses.insert(
        "add".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "3"
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

    // List all tools
    let all_tools = client
        .list_tools()
        .await
        .expect("Failed to list tools");

    println!("Available tools: {:?}", all_tools);

    // Filter for tools containing "echo"
    let filtered: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.contains("echo"))
        .collect();

    println!("Filtered tools (containing 'echo'): {:?}", filtered);

    // Verify we found exactly 2 echo tools
    assert_eq!(filtered.len(), 2, "Should find 2 tools matching 'echo'");

    // Call the first filtered tool (echo)
    let result = client
        .call_tool("echo", serde_json::json!({"message": "Hello"}))
        .await
        .expect("Failed to call echo tool");

    // Verify result
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content.len(), 1);
    assert_eq!(
        content[0].get("text").and_then(|t| t.as_str()),
        Some("echo: Hello")
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-17-02: Test filtering that matches multiple tools - verify disambiguation works
#[tokio::test]
async fn test_filter_multiple_tools() {
    // Configure mock server with multiple tools that match a pattern
    let tools = vec![
        ToolDefinition {
            name: "file_read".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "file_write".to_string(),
            description: "Write to a file".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "file_delete".to_string(),
            description: "Delete a file".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "db_query".to_string(),
            description: "Query database".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
    ];

    let mut responses = HashMap::new();
    responses.insert(
        "file_read".to_string(),
        MockResponse {
            content: vec![serde_json::json!({"type": "text", "text": "file content"})],
        },
    );

    // Spawn mock server
    let (mut child, stdin, stdout) = spawn_mock_server(tools, responses)
        .await
        .expect("Failed to spawn mock server");

    // Create transport and client
    let transport = TestStdioTransport { stdin, stdout };
    let mut client = McpClient::new("test-server".to_string(), Box::new(transport));

    // Initialize
    client.initialize().await.expect("Failed to initialize");

    // List tools
    let all_tools = client.list_tools().await.expect("Failed to list tools");

    // Filter for "file_" prefix
    let file_tools: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.starts_with("file_"))
        .collect();

    println!("File tools: {:?}", file_tools);

    // Should find 3 file tools
    assert_eq!(file_tools.len(), 3, "Should find 3 file_ tools");

    // Verify we can call one of them
    let result = client
        .call_tool("file_read", serde_json::json!({"path": "/test.txt"}))
        .await
        .expect("Failed to call file_read");

    let content = result.get("content").and_then(|c| c.as_array());
    assert!(content.is_some());

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-17-03: Test filter that matches no tools - graceful handling
#[tokio::test]
async fn test_filter_no_match() {
    let tools = vec![
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
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

    // Initialize
    client.initialize().await.expect("Failed to initialize");

    // List tools
    let all_tools = client.list_tools().await.expect("Failed to list tools");

    // Filter for non-existent pattern
    let filtered: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.contains("nonexistent_tool_xyz"))
        .collect();

    // Should be empty
    assert!(filtered.is_empty(), "Filter should return empty for no match");

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-17-04: Test combined search and call workflow
/// This simulates the CLI workflow: search -> filter -> call
#[tokio::test]
async fn test_search_and_call() {
    // Configure mock server with various tools
    let tools = vec![
        ToolDefinition {
            name: "greet".to_string(),
            description: "Greet a user".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "calculate_sum".to_string(),
            description: "Calculate sum of numbers".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "numbers": {"type": "array", "items": {"type": "number"}}
                }
            }),
        },
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for location".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        },
    ];

    let mut responses = HashMap::new();
    responses.insert(
        "greet".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Hello, World!"
            })],
        },
    );
    responses.insert(
        "calculate_sum".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "15"
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

    // Initialize
    client.initialize().await.expect("Failed to initialize");

    // Step 1: List all tools
    let all_tools = client.list_tools().await.expect("Failed to list tools");
    println!("All available tools: {:?}", all_tools);

    // Step 2: Search for tools with "greet" in name
    let search_results: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.contains("greet"))
        .collect();

    assert_eq!(search_results.len(), 1, "Should find greet tool");
    assert_eq!(search_results[0].name, "greet");

    // Step 3: Call the searched tool
    let result = client
        .call_tool("greet", serde_json::json!({"name": "World"}))
        .await
        .expect("Failed to call greet tool");

    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content.len(), 1);
    assert_eq!(
        content[0].get("text").and_then(|t| t.as_str()),
        Some("Hello, World!")
    );

    // Step 4: Now search for calculate and call it
    let calc_tools: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.contains("sum"))
        .collect();

    assert_eq!(calc_tools.len(), 1);
    assert_eq!(calc_tools[0].name, "calculate_sum");

    let result2 = client
        .call_tool("calculate_sum", serde_json::json!({"numbers": [1, 2, 3, 4, 5]}))
        .await
        .expect("Failed to call calculate_sum");

    let content2 = result2
        .get("content")
        .and_then(|c| c.as_array())
        .expect("Expected content array");
    assert_eq!(content2.len(), 1);
    assert_eq!(
        content2[0].get("text").and_then(|t| t.as_str()),
        Some("15")
    );

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-17-05: Test tool filtering by description (not just name)
#[tokio::test]
async fn test_filter_by_description() {
    let tools = vec![
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read content from a file on disk".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file on disk".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "api_request".to_string(),
            description: "Make an HTTP API request".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
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

    // Initialize
    client.initialize().await.expect("Failed to initialize");

    // List tools
    let all_tools = client.list_tools().await.expect("Failed to list tools");

    // Filter by description containing "file"
    let file_related: Vec<_> = all_tools
        .iter()
        .filter(|t| t.description.as_ref().map_or(false, |d| d.contains("file")))
        .collect();

    println!("File-related tools: {:?}", file_related);

    // Should find read_file and write_file
    assert_eq!(file_related.len(), 2);

    // Filter by description containing "HTTP"
    let http_related: Vec<_> = all_tools
        .iter()
        .filter(|t| t.description.as_ref().map_or(false, |d| d.contains("HTTP")))
        .collect();

    assert_eq!(http_related.len(), 1);
    assert_eq!(http_related[0].name, "api_request");

    // Cleanup
    let _ = child.kill().await;
}

/// TEST-17-06: Test case-insensitive filtering
#[tokio::test]
async fn test_filter_case_insensitive() {
    let tools = vec![
        ToolDefinition {
            name: "GetUser".to_string(),
            description: "Get user by ID".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "create_user".to_string(),
            description: "Create a new user".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        },
        ToolDefinition {
            name: "DELETE_item".to_string(),
            description: "Delete an item".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
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

    // Initialize
    client.initialize().await.expect("Failed to initialize");

    // List tools
    let all_tools = client.list_tools().await.expect("Failed to list tools");

    // Filter with lowercase "user" - should match both GetUser and create_user
    let user_tools: Vec<_> = all_tools
        .iter()
        .filter(|t| t.name.to_lowercase().contains("user"))
        .collect();

    println!("User tools (case-insensitive): {:?}", user_tools);

    // Should find both GetUser and create_user
    assert_eq!(user_tools.len(), 2);

    // Cleanup
    let _ = child.kill().await;
}
