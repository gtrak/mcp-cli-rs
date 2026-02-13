//! Mock MCP HTTP server for testing tool execution via HTTP transport
//!
//! This module provides an in-process HTTP mock MCP server for testing.
//! Unlike the stdio mock (which is a binary), this runs in-process during tests.
//!
//! Usage:
//! ```rust
//! let (server, url) = MockHttpServer::start().await;
//! // Use url in HTTP transport config
//! server.shutdown().await;
//! ```

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

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

/// Mock HTTP server for MCP protocol testing
pub struct MockHttpServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockHttpServer {
    /// Start the mock HTTP server on a random port
    ///
    /// Returns the server instance and the URL to connect to
    pub async fn start() -> (Self, String) {
        let state = Arc::new(RwLock::new(MockServerState::new()));

        // Bind to localhost with random port (let OS assign: 127.0.0.1:0)
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind");
        let bound_addr = listener.local_addr().expect("Failed to get local addr");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Create the service
        let make_svc = make_service_fn(move |_conn| {
            let state = state.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let state = state.clone();
                    handle_request(req, state)
                }))
            }
        });

        // Create server with shutdown signal
        let server = Server::bind(&bound_addr).serve(make_svc);
        let graceful = server.with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

        // Spawn server in background
        tokio::spawn(async move {
            if let Err(e) = graceful.await {
                eprintln!("Server error: {}", e);
            }
        });

        let url = format!("http://{}", bound_addr);

        (
            MockHttpServer {
                addr: bound_addr,
                shutdown_tx: Some(shutdown_tx),
            },
            url,
        )
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Give server time to shutdown
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

/// Handle HTTP requests
async fn handle_request(
    req: Request<Body>,
    state: Arc<RwLock<MockServerState>>,
) -> Result<Response<Body>, Infallible> {
    // Only accept POST requests
    if req.method() != Method::POST {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("Method not allowed"))
            .unwrap());
    }

    // Parse request body
    let body_bytes = match hyper::body::to_bytes(req.into_body()).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Failed to read body: {}", e)))
                .unwrap());
        }
    };

    let body_str = String::from_utf8_lossy(&body_bytes);

    // Parse JSON-RPC request
    let rpc_request: JsonRpcRequest = match serde_json::from_str(&body_str) {
        Ok(req) => req,
        Err(e) => {
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
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&error_response).unwrap()))
                .unwrap());
        }
    };

    // Handle the request
    let rpc_response = handle_rpc_request(&rpc_request, state).await;

    // Check if it's an error response for HTTP status
    let http_status = if rpc_response.error.is_some() {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    };

    Ok(Response::builder()
        .status(http_status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&rpc_response).unwrap()))
        .unwrap())
}

/// Handle JSON-RPC requests
async fn handle_rpc_request(
    request: &JsonRpcRequest,
    state: Arc<RwLock<MockServerState>>,
) -> JsonRpcResponse {
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
        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!(null)),
            error: None,
            id: request.id.clone(),
        },
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

async fn handle_initialize(
    request: &JsonRpcRequest,
    state: Arc<RwLock<MockServerState>>,
) -> JsonRpcResponse {
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

    let mut state = state.write().await;
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

async fn handle_tools_list(
    request: &JsonRpcRequest,
    state: Arc<RwLock<MockServerState>>,
) -> JsonRpcResponse {
    let state = state.read().await;

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

async fn handle_tools_call(
    request: &JsonRpcRequest,
    state: Arc<RwLock<MockServerState>>,
) -> JsonRpcResponse {
    let state = state.read().await;

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
        let substituted: Vec<Value> = mock_response
            .content
            .iter()
            .map(|item| {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    let mut result = text.to_string();
                    if let Some(obj) = arguments.as_object() {
                        for (key, value) in obj {
                            let placeholder = format!("{{{}}}", key);
                            let value_str = value
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| value.to_string());
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
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(serde_json::json!(null)),
        error: None,
        id: request.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_http_server_start() {
        let (server, url) = MockHttpServer::start().await;
        assert!(url.starts_with("http://127.0.0.1:"));
        assert_eq!(server.url(), url);
        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_ping() {
        let (server, url) = MockHttpServer::start().await;

        // Send ping request
        let client = reqwest::Client::new();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": 1
        });

        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .expect("Failed to send request");

        assert!(response.status().is_success());

        let body: Value = response.json().await.expect("Failed to parse response");
        assert_eq!(body["jsonrpc"], "2.0");
        assert!(body["result"].is_null());

        server.shutdown().await;
    }
}
