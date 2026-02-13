//! Test fixtures for MCP tool call integration tests
//!
//! This module provides shared types, helper functions, and test data
//! for both stdio and HTTP mock MCP servers.
//!
//! # Re-exports
//! - `MockHttpServer` - In-process HTTP mock server
//! - `ToolDefinition` - Tool definition structure
//! - `MockResponse` - Tool response structure
//! - `start_mock_stdio` - Spawn stdio mock server process
//! - `start_mock_http` - Start HTTP mock server
//!
//! # Usage
//! ```rust
//! use tests::fixtures::{MockHttpServer, start_mock_stdio};
//!
//! // HTTP server
//! let (server, url) = MockHttpServer::start().await;
//!
//! // Stdio server
//! let (child, stdin, stdout) = start_mock_stdio().await;
//! ```

// Re-export mock HTTP server
pub mod mock_http_server;

// Re-export types from HTTP server for convenience
pub use mock_http_server::{MockHttpServer, MockResponse, ToolDefinition};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Mock server configuration for tests
#[derive(Debug, Clone)]
pub struct MockServerConfig {
    /// Tools to make available
    pub tools: Vec<ToolDefinition>,
    /// Pre-configured responses for tools
    pub responses: HashMap<String, MockResponse>,
    /// Pre-configured errors for tools
    pub errors: HashMap<String, String>,
}

impl Default for MockServerConfig {
    fn default() -> Self {
        Self {
            tools: default_tools(),
            responses: default_responses(),
            errors: HashMap::new(),
        }
    }
}

impl MockServerConfig {
    /// Create new configuration with default tools
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tool to the configuration
    pub fn with_tool(mut self, tool: ToolDefinition) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add a response for a tool
    pub fn with_response(mut self, tool_name: &str, response: MockResponse) -> Self {
        self.responses.insert(tool_name.to_string(), response);
        self
    }

    /// Add an error for a tool
    pub fn with_error(mut self, tool_name: &str, error: &str) -> Self {
        self.errors.insert(tool_name.to_string(), error.to_string());
        self
    }

    /// Convert to environment variables
    pub fn to_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if !self.tools.is_empty() {
            let tools_json = serde_json::to_string(&self.tools).unwrap();
            env.insert("MOCK_TOOLS".to_string(), tools_json);
        }

        if !self.responses.is_empty() {
            let responses_json = serde_json::to_string(&self.responses).unwrap();
            env.insert("MOCK_RESPONSES".to_string(), responses_json);
        }

        if !self.errors.is_empty() {
            let errors_json = serde_json::to_string(&self.errors).unwrap();
            env.insert("MOCK_ERRORS".to_string(), errors_json);
        }

        env
    }

    /// Apply configuration to current process environment
    pub fn apply(&self) {
        let env = self.to_env();
        for (key, value) in env {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }
}

/// Spawn a mock stdio MCP server process
///
/// # Returns
/// * `(Child, ChildStdin, BufReader<ChildStdout>)` - Process handle and I/O handles
///
/// # Example
/// ```rust
/// let (mut child, mut stdin, mut stdout) = start_mock_stdio().await;
/// // Send request via stdin
/// stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":1}\n").await?;
/// // Read response from stdout
/// let mut line = String::new();
/// stdout.read_line(&mut line).await?;
/// ```
pub async fn start_mock_stdio() -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().unwrap();

    // Find the mock server binary
    let mock_server_path = if exe_dir.join("mock-mcp-server.exe").exists() {
        exe_dir.join("mock-mcp-server.exe")
    } else if exe_dir.join("mock-mcp-server").exists() {
        exe_dir.join("mock-mcp-server")
    } else {
        // Try in target/debug directory
        let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

    let mut cmd = Command::new(mock_server_path);
    cmd.stdin(Stdio::piped())
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

/// Spawn a mock stdio MCP server with custom configuration
///
/// # Arguments
/// * `config` - Mock server configuration
///
/// # Returns
/// * `(Child, ChildStdin, BufReader<ChildStdout>)` - Process handles
///
/// # Example
/// ```rust
/// let config = MockServerConfig::new()
///     .with_error("fail", "Simulated failure");
/// let (child, stdin, stdout) = start_mock_stdio_with_config(&config).await?;
/// ```
pub async fn start_mock_stdio_with_config(
    config: &MockServerConfig,
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
        let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

    let mut cmd = Command::new(mock_server_path);

    // Apply environment variables
    let env_vars = config.to_env();
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    cmd.stdin(Stdio::piped())
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

/// Start HTTP mock server (convenience wrapper)
///
/// # Returns
/// * `(MockHttpServer, String)` - Server handle and URL
///
/// # Example
/// ```rust
/// let (server, url) = start_mock_http().await;
/// // Use url in HTTP transport config
/// server.shutdown().await;
/// ```
pub async fn start_mock_http() -> (MockHttpServer, String) {
    MockHttpServer::start().await
}

/// Get the path to a fixture file
///
/// # Arguments
/// * `name` - Relative path from tests/fixtures/ directory
///
/// # Returns
/// * `PathBuf` - Absolute path to fixture file
///
/// # Example
/// ```rust
/// let path = get_fixture_path("tools/simple.json");
/// ```
pub fn get_fixture_path(name: &str) -> PathBuf {
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cargo_dir.join("tests/fixtures").join(name)
}

/// Load JSON fixture file
///
/// # Arguments
/// * `name` - Relative path from tests/fixtures/ directory
///
/// # Returns
/// * `Result<Value>` - Parsed JSON value
///
/// # Example
/// ```rust
/// let tools: Vec<ToolDefinition> = load_fixture_json("tools/simple.json")?;
/// ```
pub fn load_fixture_json<T: serde::de::DeserializeOwned>(name: &str) -> anyhow::Result<T> {
    let path = get_fixture_path(name);
    let contents = std::fs::read_to_string(&path)?;
    let value = serde_json::from_str(&contents)?;
    Ok(value)
}

/// Default tools for testing
fn default_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo back the input message".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Message to echo"}
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
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }),
        },
        ToolDefinition {
            name: "fail".to_string(),
            description: "Always fails with an error".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reason": {"type": "string", "description": "Error reason"}
                }
            }),
        },
    ]
}

/// Default responses for tools
fn default_responses() -> HashMap<String, MockResponse> {
    let mut responses = HashMap::new();

    responses.insert(
        "echo".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Echo: {message}"
            })],
        },
    );

    responses.insert(
        "add".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Result: {result}"
            })],
        },
    );

    responses
}

/// Simple tool call request builder
#[derive(Debug, Serialize)]
pub struct ToolCallRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: ToolCallParams,
    pub id: u64,
}

/// Parameters for tool/call method
#[derive(Debug, Serialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: Value,
}

impl ToolCallRequest {
    /// Create a new tool call request
    pub fn new(tool_name: &str, arguments: Value, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: ToolCallParams {
                name: tool_name.to_string(),
                arguments,
            },
            id,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// Initialize request builder
#[derive(Debug, Serialize)]
pub struct InitializeRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: InitializeParams,
    pub id: u64,
}

/// Parameters for initialize method
#[derive(Debug, Serialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: Value,
    pub client_info: Value,
}

impl InitializeRequest {
    /// Create a new initialize request
    pub fn new(id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                capabilities: serde_json::json!({"tools": {}}),
                client_info: serde_json::json!({
                    "name": "mcp-cli-rs-test",
                    "version": "0.1.0"
                }),
            },
            id,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// Tools list request builder
#[derive(Debug, Serialize)]
pub struct ToolsListRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: u64,
}

impl ToolsListRequest {
    /// Create a new tools/list request
    pub fn new(id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: serde_json::json!({}),
            id,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// Ping request builder
#[derive(Debug, Serialize)]
pub struct PingRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: u64,
}

impl PingRequest {
    /// Create a new ping request
    pub fn new(id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: serde_json::json!(null),
            id,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// JSON-RPC response parser
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tools() {
        let tools = default_tools();
        assert_eq!(tools.len(), 3);
        assert!(tools.iter().any(|t| t.name == "echo"));
        assert!(tools.iter().any(|t| t.name == "add"));
        assert!(tools.iter().any(|t| t.name == "fail"));
    }

    #[test]
    fn test_mock_server_config() {
        let config = MockServerConfig::new()
            .with_error("fail", "Test error")
            .with_tool(ToolDefinition {
                name: "custom".to_string(),
                description: "Custom tool".to_string(),
                input_schema: serde_json::json!({}),
            });

        assert!(config.errors.contains_key("fail"));
        assert_eq!(config.tools.len(), 4); // 3 default + 1 custom
    }

    #[test]
    fn test_request_builders() {
        let init = InitializeRequest::new(1);
        let json = init.to_json();
        assert!(json.contains("initialize"));
        assert!(json.contains("2024-11-05"));

        let ping = PingRequest::new(2);
        let json = ping.to_json();
        assert!(json.contains("ping"));

        let tools_list = ToolsListRequest::new(3);
        let json = tools_list.to_json();
        assert!(json.contains("tools/list"));

        let tool_call = ToolCallRequest::new("echo", serde_json::json!({"message": "test"}), 4);
        let json = tool_call.to_json();
        assert!(json.contains("tools/call"));
        assert!(json.contains("echo"));
    }

    #[test]
    fn test_config_to_env() {
        let config = MockServerConfig::new().with_error("fail", "Simulated error");
        let env = config.to_env();

        assert!(env.contains_key("MOCK_TOOLS"));
        assert!(env.contains_key("MOCK_RESPONSES"));
        assert!(env.contains_key("MOCK_ERRORS"));

        let errors: HashMap<String, String> =
            serde_json::from_str(&env["MOCK_ERRORS"]).expect("Failed to parse MOCK_ERRORS");
        assert_eq!(errors["fail"], "Simulated error");
    }
}
