//! MCP client for communicating with MCP servers.
//!
//! This module provides the main client for interacting with MCP servers,
//! including tool listing, execution, and protocol handling.

pub mod http;
pub mod stdio;

use crate::config::Config;
use crate::error::{McpError, Result};
use crate::transport::{Transport, TransportFactory};
use serde_json::Value;

/// Information about a tool available on a MCP server.
///
/// Contains the tool name, description, and input schema (JSON Schema).
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Name of the tool.
    pub name: String,

    /// Optional human-readable description of the tool.
    pub description: Option<String>,

    /// JSON Schema for tool input parameters.
    pub input_schema: Value,
}

/// MCP client for communicating with servers.
///
/// Encapsulates a transport connection and provides high-level methods
/// for interacting with MCP servers (e.g., listing tools).
pub struct McpClient {
    /// Transport connection to the server.
    transport: Box<dyn Transport + Send + Sync>,

    /// Server identifier for error messages.
    server_name: String,
}

impl McpClient {
    /// Create a new MCP client with the given transport.
    ///
    /// # Arguments
    /// * `server_name` - Server identifier for error messages
    /// * `transport` - Transport connection to the server
    pub fn new(server_name: String, transport: Box<dyn Transport + Send + Sync>) -> Self {
        Self {
            transport,
            server_name,
        }
    }

    /// Initialize the MCP server connection.
    ///
    /// Sends initialize request with client capabilities and receives server capabilities.
    /// The server automatically sends notifications/initialized - we don't need to wait for it.
    pub async fn initialize(&mut self) -> Result<()> {
        let init_request = Self::json_rpc_request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        );

        let response = self.transport.send(init_request).await?;

        let _result = response["result"]
            .as_object()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Expected result in initialize response".to_string(),
            })?;

        Ok(())
    }

    /// List available tools from the server.
    ///
    /// This implements DISC-01: discovery of available tools.
    /// Returns a vector of ToolInfo structs representing available tools.
    ///
    /// # Errors
    /// Returns McpError::InvalidProtocol if response is malformed
    /// Returns McpError::Timeout if server doesn't respond
    pub async fn list_tools(&mut self) -> Result<Vec<ToolInfo>> {
        self.initialize().await?;

        let request = Self::json_rpc_request("tools/list", serde_json::json!({}));
        let response = self.transport.send(request).await?;

        // Parse response
        let result = response["result"]
            .as_object()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Expected result object in response".to_string(),
            })?;

        let tools_array = result["tools"]
            .as_array()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Expected tools array in result".to_string(),
            })?;

        // Convert tools array to ToolInfo structs
        let tools: Vec<ToolInfo> = tools_array
            .iter()
            .filter_map(|tool| {
                let name = tool["name"].as_str()?.to_string();
                let description = tool["description"].as_str().map(|s| s.to_string());
                let input_schema = tool["input_schema"].clone();

                Some(ToolInfo {
                    name,
                    description,
                    input_schema,
                })
            })
            .collect();

        Ok(tools)
    }

    /// Call a tool on the server.
    ///
    /// This is a stub implementation for now (TODO for Plan 04: EXEC-03).
    /// Returns an error indicating it's not yet implemented.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to call
    /// * `arguments` - Arguments for the tool (JSON Schema validated)
    ///
    /// # Errors
    /// Returns McpError::InvalidOperation for now (to be implemented in Plan 04)
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<Value> {
        // Build JSON-RPC request for "tools/call"
        let request = Self::json_rpc_request(
            "tools/call",
            serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            }),
        );

        // Send request via transport
        let response = self.transport.send(request).await?;

        // Parse response
        let result = response["result"]
            .as_object()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Expected result object in response".to_string(),
            })?;

        Ok(serde_json::Value::Object(result.clone()))
    }

    /// Create a JSON-RPC 2.0 request.
    ///
    /// This helper method builds standardized JSON-RPC requests for MCP protocol.
    ///
    /// # Arguments
    /// * `method` - Method name (e.g., "tools/list")
    /// * `params` - Method parameters
    ///
    /// # Returns
    /// JSON-RPC 2.0 formatted request object
    fn json_rpc_request(method: &str, params: Value) -> Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": Self::generate_request_id()
        })
    }

    /// Generate a unique request ID.
    ///
    /// Uses a simple timestamp-based ID for uniqueness.
    fn generate_request_id() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

impl std::fmt::Display for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "McpClient({})", self.server_name)
    }
}

impl TransportFactory for Config {
    fn create_transport(&self, server_name: &str) -> Box<dyn Transport + Send + Sync> {
        let server = self
            .get_server(server_name)
            .expect("Server not found in config");

        server.transport.create_transport(server_name)
    }

    fn supports_filtering(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_tool_info_struct() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            }
        });
        let tool = ToolInfo {
            name: "search".to_string(),
            description: Some("Search for information".to_string()),
            input_schema: schema.clone(),
        };
        println!("ToolInfo created: {}", tool.name);
    }

    #[test]
    fn test_json_rpc_request() {
        let request = McpClient::json_rpc_request("tools/list", serde_json::json!({}));
        println!("JSON-RPC request: {}", request);
    }
}
