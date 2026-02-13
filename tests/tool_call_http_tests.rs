//! End-to-end integration tests for HTTP transport tool calls
//!
//! These tests verify the full MCP protocol roundtrip over HTTP transport
//! from client to mock server and back, including initialization, tool calls
//! with arguments, and tools listing.
//!
//! Tests: TEST-03

use mcp_cli_rs::client::McpClient;
use mcp_cli_rs::client::http::HttpTransport;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;

// Import mock server from fixtures
use fixtures::{MockHttpServer, MockResponse, ToolDefinition};
use fixtures::mock_http_server::MockServerConfig;

mod fixtures;
use fixtures::mock_http_server;

/// Helper to run tests with a mock HTTP server.
/// Configuration is passed directly to avoid race conditions in parallel tests.
async fn with_mock_server<F, Fut>(config: mock_http_server::MockServerConfig, test: F)
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = ()>,
{
    let (server, url) = MockHttpServer::start(config).await;
    test(url).await;
    server.shutdown().await;
}

/// TEST-03: Basic tool call via HTTP transport
/// Verifies full roundtrip from client to mock HTTP server and back
#[tokio::test]
async fn test_http_basic_tool_call() {
    // Configure mock server with echo tool - passed directly to avoid race conditions
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
    // Template substitution: {message} will be replaced with actual argument value
    responses.insert(
        "echo".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Echo: {message}"
            })],
        },
    );

    // Create config and pass directly to server (no env vars - avoids race conditions)
    let config = MockServerConfig::from_parts(tools, responses, HashMap::new());

    with_mock_server(config, |url| async move {
        // Create HTTP transport and client
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // Initialize connection
        client
            .initialize()
            .await
            .expect("Failed to initialize HTTP connection");

        // Call echo tool
        let result = client
            .call_tool("echo", serde_json::json!({"message": "Hello HTTP"}))
            .await
            .expect("Failed to call echo tool via HTTP");

        // Verify response
        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .expect("Expected content array in HTTP response");
        assert_eq!(content.len(), 1);
        // The mock server uses template substitution: {message} -> "Hello HTTP"
        let text = content[0]
            .get("text")
            .and_then(|t| t.as_str())
            .expect("Expected text field");
        assert_eq!(text, "Echo: Hello HTTP", "Expected substituted response, got: {}", text);
        assert_eq!(
            result.get("isError").and_then(|e| e.as_bool()),
            Some(false)
        );
    }).await;
}

/// TEST-03b: Tool call with arguments via HTTP
/// Verifies JSON arguments are serialized correctly over HTTP POST
#[tokio::test]
async fn test_http_tool_call_with_args() {
    // Configure mock server with add and multiply tools - passed directly to avoid race conditions
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
    // Template substitution - {a}, {b} will be replaced with actual values
    responses.insert(
        "add".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Sum: {a} + {b} = {result}"
            })],
        },
    );
    responses.insert(
        "multiply".to_string(),
        MockResponse {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Product: {x} * {y} = {result}"
            })],
        },
    );

    // Create config and pass directly to server (no env vars - avoids race conditions)
    let config = MockServerConfig::from_parts(tools, responses, HashMap::new());

    with_mock_server(config, |url| async move {
        // Create HTTP transport and client
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // Initialize connection
        client
            .initialize()
            .await
            .expect("Failed to initialize HTTP connection");

        // Test add tool with arguments via HTTP POST
        let result = client
            .call_tool("add", serde_json::json!({"a": 10, "b": 20}))
            .await
            .expect("Failed to call add tool via HTTP");

        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .expect("Expected content array");
        assert_eq!(content.len(), 1);
        let text = content[0]
            .get("text")
            .and_then(|t| t.as_str())
            .expect("Expected text");
        // Template substitution replaces {a} and {b} with actual values
        assert!(text.contains("10"), "Expected response to contain '10', got: {}", text);
        assert!(text.contains("20"), "Expected response to contain '20', got: {}", text);

        // Test multiply tool with arguments
        let result = client
            .call_tool("multiply", serde_json::json!({"x": 3, "y": 5}))
            .await
            .expect("Failed to call multiply tool via HTTP");

        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .expect("Expected content array");
        assert_eq!(content.len(), 1);
        let text = content[0]
            .get("text")
            .and_then(|t| t.as_str())
            .expect("Expected text");
        assert!(text.contains("3"), "Expected response to contain '3', got: {}", text);
        assert!(text.contains("5"), "Expected response to contain '5', got: {}", text);
    }).await;
}

/// TEST-03c: Tools list via HTTP transport
/// Verifies tools/list works correctly over HTTP
#[tokio::test]
async fn test_http_tools_list() {
    // Configure mock server with multiple tools - passed directly to avoid race conditions
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

    // Create config and pass directly to server (no env vars - avoids race conditions)
    let config = MockServerConfig::from_parts(tools, HashMap::new(), HashMap::new());

    with_mock_server(config, |url| async move {
        // Create HTTP transport and client
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // List tools via HTTP
        let tools_list = client
            .list_tools()
            .await
            .expect("Failed to list tools via HTTP");

        // Verify returned tools
        assert_eq!(tools_list.len(), 3, "Expected 3 tools, got: {}", tools_list.len());

        let tool_names: Vec<String> = tools_list.iter().map(|t| t.name.clone()).collect();
        assert!(tool_names.contains(&"echo".to_string()));
        assert!(tool_names.contains(&"add".to_string()));
        assert!(tool_names.contains(&"search".to_string()));

        // Verify tool info structure
        let echo_tool = tools_list.iter().find(|t| t.name == "echo").unwrap();
        assert!(echo_tool.description.is_some());
        assert!(!echo_tool.input_schema.as_object().unwrap().is_empty());
    }).await;
}

/// TEST-03d: Initialize handshake via HTTP
/// Verifies MCP protocol initialization works over HTTP
#[tokio::test]
async fn test_http_initialize_handshake() {
    // Create config with empty tools and pass directly to server
    let config = MockServerConfig::from_parts(vec![], HashMap::new(), HashMap::new());

    with_mock_server(config, |url| async move {
        // Create HTTP transport
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // Initialize - this sends initialize request and receives response
        client
            .initialize()
            .await
            .expect("Failed to initialize HTTP connection");

        // After initialization, we should be able to call tools
        // This implicitly verifies the handshake worked
        let _tools = client
            .list_tools()
            .await
            .expect("Should be able to list tools after initialization");
    }).await;
}

/// TEST-03e: HTTP transport error handling
/// Verifies graceful handling of HTTP errors
#[tokio::test]
async fn test_http_transport_error_handling() {
    // Test connection refused error
    let bad_url = "http://127.0.0.1:1"; // Port 1 is unlikely to be used
    let transport = HttpTransport::new(bad_url, HashMap::new());
    let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

    let result = client.initialize().await;
    assert!(
        result.is_err(),
        "Expected connection error for bad URL"
    );

    // Test tool not found error via mock - create config with empty tools
    let config = MockServerConfig::from_parts(vec![], HashMap::new(), HashMap::new());

    with_mock_server(config, |url| async move {
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        client
            .initialize()
            .await
            .expect("Should initialize successfully");

        // Try to call a non-existent tool
        let result = client
            .call_tool("nonexistent", serde_json::json!({}))
            .await;

        assert!(
            result.is_err(),
            "Expected error for non-existent tool"
        );
    }).await;
}

/// TEST-03f: Custom headers passthrough
/// Verifies custom HTTP headers are sent with requests
#[tokio::test]
async fn test_http_headers_passthrough() {
    // Create config with empty tools
    let config = MockServerConfig::from_parts(vec![], HashMap::new(), HashMap::new());

    with_mock_server(config, |url| async move {
        // Create transport with custom headers
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "test-value".to_string());
        headers.insert("Authorization".to_string(), "Bearer test-token".to_string());

        let transport = HttpTransport::new(&url, headers);
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // Initialize with custom headers
        // Note: Mock server currently doesn't validate headers,
        // but this tests that HttpTransport accepts and uses them
        client
            .initialize()
            .await
            .expect("Should initialize with custom headers");

        // Verify operations still work
        let _tools = client
            .list_tools()
            .await
            .expect("Should list tools with custom headers");
    }).await;
}

/// TEST-03g: Complex nested arguments via HTTP
/// Verifies complex JSON arguments work correctly over HTTP
#[tokio::test]
async fn test_http_complex_nested_arguments() {
    // Configure mock server with complex tool - passed directly to avoid race conditions
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
                "text": "Complex data processed via HTTP"
            })],
        },
    );

    // Create config and pass directly to server (no env vars - avoids race conditions)
    let config = MockServerConfig::from_parts(tools, responses, HashMap::new());

    with_mock_server(config, |url| async move {
        // Create HTTP transport and client
        let transport = HttpTransport::new(&url, HashMap::new());
        let mut client = McpClient::new("test-http-server".to_string(), Box::new(transport));

        // Initialize
        client
            .initialize()
            .await
            .expect("Failed to initialize");

        // Call with complex nested arguments
        let complex_args = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30
            },
            "tags": ["rust", "mcp", "http-test"],
            "metadata": {
                "version": "1.0",
                "timestamp": 1234567890
            }
        });

        let result = client
            .call_tool("process", complex_args)
            .await
            .expect("Failed to call process tool via HTTP");

        // Verify response
        assert!(result.get("content").is_some());
        assert_eq!(
            result.get("isError").and_then(|e| e.as_bool()),
            Some(false)
        );
    }).await;
}
