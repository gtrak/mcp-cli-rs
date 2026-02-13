//! Model types for CLI command outputs.
//!
//! This module provides structured data models for all CLI command outputs,
//! enabling separation between data collection and formatting. Models are
//! designed to support both human-readable and JSON output modes.
//!
//! # Architecture
//!
//! Commands populate these models with data, then pass them to formatters in
//! `crate::cli::formatters` to produce output. This separation enables:
//! - Consistent JSON serialization across all commands
//! - Single-source-of-truth for output structure
//! - Easier testing of command logic independent of formatting

use serde::{Deserialize, Serialize};

/// Model for list servers command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListServersModel {
    /// List of servers with their tool information
    pub servers: Vec<ServerModel>,
    /// Total number of configured servers
    pub total_servers: usize,
    /// Number of successfully connected servers
    pub connected_servers: usize,
    /// Number of servers that failed to connect
    pub failed_servers: usize,
    /// Total number of tools across all connected servers
    pub total_tools: usize,
}

/// Model for an individual server in the list output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerModel {
    /// Server name
    pub name: String,
    /// Connection status: "connected" or "failed"
    pub status: String,
    /// Transport type (stdio, http, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_type: Option<String>,
    /// Server description from config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of tools available on this server
    pub tool_count: usize,
    /// List of tools on this server
    pub tools: Vec<ToolModel>,
    /// Error message if connection failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Whether this server has filtered/disabled tools
    #[serde(skip_serializing_if = "is_false")]
    pub has_filtered_tools: bool,
}

/// Model for a tool within server listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolModel {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON schema for tool input parameters
    pub input_schema: serde_json::Value,
}

/// Model for server info command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfoModel {
    /// Server name
    pub name: String,
    /// Server description from config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Transport type (stdio, http, etc.)
    pub transport_type: String,
    /// Detailed transport configuration as JSON
    pub transport_detail: serde_json::Value,
    /// Environment variables (for stdio transport)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Vec<(String, String)>>,
    /// List of disabled tool patterns
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disabled_tools: Vec<String>,
    /// List of allowed tool patterns
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
}

/// Model for tool info command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfoModel {
    /// Server name containing this tool
    pub server_name: String,
    /// Tool name
    pub tool_name: String,
    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// List of parameters extracted from schema
    pub parameters: Vec<ParameterModel>,
    /// Full JSON schema for tool input
    pub input_schema: serde_json::Value,
}

/// Model for a parameter within tool info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterModel {
    /// Parameter name
    pub name: String,
    /// Parameter type (string, number, etc.)
    pub param_type: String,
    /// Whether the parameter is required
    pub required: bool,
    /// Parameter description from schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<&ParameterModel> for crate::format::ParameterInfo {
    fn from(model: &ParameterModel) -> Self {
        Self {
            name: model.name.clone(),
            param_type: model.param_type.clone(),
            description: model.description.clone(),
            required: model.required,
        }
    }
}

/// Model for tool call result output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResultModel {
    /// Server name where tool was executed
    pub server_name: String,
    /// Tool name that was executed
    pub tool_name: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Result value on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Execution time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
    /// Number of retry attempts made
    #[serde(skip_serializing_if = "is_zero")]
    pub retries: u32,
}

/// Model for search results output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultModel {
    /// Search pattern used
    pub pattern: String,
    /// List of matching tools
    pub matches: Vec<SearchMatchModel>,
    /// Total number of matches found
    pub total_matches: usize,
    /// Number of servers that were searched
    pub servers_searched: usize,
    /// List of servers that failed during search
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed_servers: Vec<String>,
}

/// Model for an individual search match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatchModel {
    /// Server name containing the matched tool
    pub server_name: String,
    /// Tool name that matched
    pub tool_name: String,
    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON schema for tool input parameters
    pub input_schema: serde_json::Value,
}

/// Helper function for serde skip_serializing_if
fn is_false(b: &bool) -> bool {
    !b
}

/// Helper function for serde skip_serializing_if
fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_servers_model_serialization() {
        let model = ListServersModel {
            servers: vec![],
            total_servers: 0,
            connected_servers: 0,
            failed_servers: 0,
            total_tools: 0,
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("total_servers"));
        assert!(json.contains("connected_servers"));
    }

    #[test]
    fn test_server_model_with_optional_fields() {
        let model = ServerModel {
            name: "test".to_string(),
            status: "connected".to_string(),
            transport_type: Some("stdio".to_string()),
            description: None,
            tool_count: 5,
            tools: vec![],
            error: None,
            has_filtered_tools: false,
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("test"));
        assert!(!json.contains("description")); // Should be skipped
    }

    #[test]
    fn test_tool_info_model_with_parameters() {
        let model = ToolInfoModel {
            server_name: "filesystem".to_string(),
            tool_name: "read_file".to_string(),
            description: Some("Reads a file".to_string()),
            parameters: vec![ParameterModel {
                name: "path".to_string(),
                param_type: "string".to_string(),
                required: true,
                description: Some("File path".to_string()),
            }],
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        };
        let json = serde_json::to_string_pretty(&model).unwrap();
        assert!(json.contains("filesystem"));
        assert!(json.contains("read_file"));
        assert!(json.contains("parameters"));
    }

    #[test]
    fn test_call_result_model_success() {
        let model = CallResultModel {
            server_name: "test".to_string(),
            tool_name: "echo".to_string(),
            success: true,
            result: Some(serde_json::json!({"message": "hello"})),
            error: None,
            execution_time_ms: Some(150),
            retries: 0,
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("success"));
        assert!(!json.contains("retries")); // Should be skipped when 0
    }

    #[test]
    fn test_search_result_model() {
        let model = SearchResultModel {
            pattern: "read*".to_string(),
            matches: vec![SearchMatchModel {
                server_name: "filesystem".to_string(),
                tool_name: "read_file".to_string(),
                description: Some("Reads files".to_string()),
                input_schema: serde_json::json!({}),
            }],
            total_matches: 1,
            servers_searched: 3,
            failed_servers: vec![],
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("read*"));
        assert!(json.contains("total_matches"));
        assert!(!json.contains("failed_servers")); // Should be skipped when empty
    }
}
