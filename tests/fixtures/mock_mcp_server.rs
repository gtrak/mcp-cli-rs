//! Mock MCP server for testing tool execution via stdio transport
//!
//! This binary implements a minimal MCP server that communicates via stdin/stdout
//! using newline-delimited JSON. It supports the full MCP protocol initialization
//! handshake and responds to tools/list, tools/call, and ping methods.
//!
//! Configuration via environment variables:
//! - MOCK_TOOLS: JSON array of ToolDefinition objects defining available tools
//! - MOCK_RESPONSES: JSON object mapping tool_name -> response content
//! - MOCK_ERRORS: JSON object mapping tool_name -> error message
//!
//! Usage:
//!   cargo run --bin mock-mcp-server
//!   Or spawned as subprocess by integration tests

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// Tool definition matching MCP protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Mock response for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    pub content: Vec<Value>,
}

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    id: Value,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Value,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Clone, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Server state holding configuration
struct MockServerState {
    tools: Vec<ToolDefinition>,
    responses: HashMap<String, MockResponse>,
    errors: HashMap<String, String>,
    initialized: bool,
}

impl MockServerState {
    fn new() -> Self {
        let tools = Self::load_tools_from_env();
        let responses = Self::load_responses_from_env();
        let errors = Self::load_errors_from_env();

        Self {
            tools,
            responses,
            errors,
            initialized: false,
        }
    }

    fn load_tools_from_env() -> Vec<ToolDefinition> {
        if let Ok(tools_json) = std::env::var("MOCK_TOOLS") {
            serde_json::from_str(&tools_json).unwrap_or_else(|_| Self::default_tools())
        } else {
            Self::default_tools()
        }
    }

    fn load_responses_from_env() -> HashMap<String, MockResponse> {
        if let Ok(responses_json) = std::env::var("MOCK_RESPONSES") {
            serde_json::from_str(&responses_json).unwrap_or_default()
        } else {
            Self::default_responses()
        }
    }

    fn load_errors_from_env() -> HashMap<String, String> {
        if let Ok(errors_json) = std::env::var("MOCK_ERRORS") {
            serde_json::from_str(&errors_json).unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

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
}

#[tokio::main]
async fn main() {
    // Initialize tracing for debugging
    tracing_subscriber::fmt::init();

    let mut state = MockServerState::new();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = tokio::io::BufReader::new(stdin);
    let mut lines = reader.lines();

    tracing::info!("Mock MCP server started, awaiting requests...");

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(request) => {
                let response = handle_request(&request, &mut state).await;
                let response_json = serde_json::to_string(&response).unwrap();
                if let Err(e) = stdout.write_all(response_json.as_bytes()).await {
                    tracing::error!("Failed to write response: {}", e);
                    break;
                }
                if let Err(e) = stdout.write_all(b"\n").await {
                    tracing::error!("Failed to write newline: {}", e);
                    break;
                }
                if let Err(e) = stdout.flush().await {
                    tracing::error!("Failed to flush stdout: {}", e);
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Failed to parse JSON-RPC request: {}", e);
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                    id: Value::Null,
                };
                let response_json = serde_json::to_string(&error_response).unwrap();
                let _ = stdout.write_all(response_json.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
            }
        }
    }

    tracing::info!("Mock MCP server shutting down");
}

async fn handle_request(request: &JsonRpcRequest, state: &mut MockServerState) -> JsonRpcResponse {
    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request: jsonrpc must be 2.0".to_string(),
                data: None,
            }),
            id: request.id.clone(),
        };
    }

    match request.method.as_str() {
        "initialize" => handle_initialize(request, state).await,
        "notifications/initialized" => {
            // Notification has no response
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!(null)),
                error: None,
                id: request.id.clone(),
            }
        }
        "tools/list" => handle_tools_list(request, state).await,
        "tools/call" => handle_tools_call(request, state).await,
        "ping" => handle_ping(request).await,
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
            id: request.id.clone(),
        },
    }
}

async fn handle_initialize(request: &JsonRpcRequest, state: &mut MockServerState) -> JsonRpcResponse {
    tracing::info!("Handling initialize request");

    // Validate protocol version
    let protocol_version = request
        .params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if protocol_version != "2024-11-05" {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: format!("Unsupported protocol version: {}", protocol_version),
                data: None,
            }),
            id: request.id.clone(),
        };
    }

    state.initialized = true;

    let result = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "mock-mcp-server",
            "version": "0.1.0"
        },
        "capabilities": {
            "tools": {}
        }
    });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: request.id.clone(),
    }
}

async fn handle_tools_list(request: &JsonRpcRequest, state: &MockServerState) -> JsonRpcResponse {
    tracing::info!("Handling tools/list request");

    if !state.initialized {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32001,
                message: "Server not initialized".to_string(),
                data: None,
            }),
            id: request.id.clone(),
        };
    }

    let tools: Vec<Value> = state
        .tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema
            })
        })
        .collect();

    let result = serde_json::json!({ "tools": tools });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: request.id.clone(),
    }
}

async fn handle_tools_call(request: &JsonRpcRequest, state: &MockServerState) -> JsonRpcResponse {
    tracing::info!("Handling tools/call request");

    if !state.initialized {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32001,
                message: "Server not initialized".to_string(),
                data: None,
            }),
            id: request.id.clone(),
        };
    }

    let tool_name = request
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let arguments = request.params.get("arguments").cloned().unwrap_or_default();

    // Check for configured error
    if let Some(error_msg) = state.errors.get(tool_name) {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32002,
                message: error_msg.clone(),
                data: Some(arguments),
            }),
            id: request.id.clone(),
        };
    }

    // Check if tool exists
    let tool_exists = state.tools.iter().any(|t| t.name == tool_name);
    if !tool_exists {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32003,
                message: format!("Tool not found: {}", tool_name),
                data: None,
            }),
            id: request.id.clone(),
        };
    }

    // Generate response based on configured response or default
    let content = if let Some(mock_response) = state.responses.get(tool_name) {
        // Template substitution in response
        let substituted: Vec<Value> = mock_response
            .content
            .iter()
            .map(|item| {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    let mut result = text.to_string();
                    if let Some(obj) = arguments.as_object() {
                        for (key, value) in obj {
                            let placeholder = format!("{{{}}}", key);
                            let value_str = value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string());
                            result = result.replace(&placeholder, &value_str);
                        }
                    }
                    serde_json::json!({"type": "text", "text": result})
                } else {
                    item.clone()
                }
            })
            .collect();
        substituted
    } else {
        vec![serde_json::json!({
            "type": "text",
            "text": format!("Executed tool '{}' with arguments: {}", tool_name, arguments)
        })]
    };

    let result = serde_json::json!({
        "content": content,
        "isError": false
    });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: request.id.clone(),
    }
}

async fn handle_ping(request: &JsonRpcRequest) -> JsonRpcResponse {
    tracing::info!("Handling ping request");

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!(null)),
        error: None,
        id: request.id.clone(),
    }
}
