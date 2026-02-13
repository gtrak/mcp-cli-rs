//! Mock HTTP server that fails first N requests for retry logic testing
//!
//! This module provides an in-process HTTP mock server that returns 503 Service Unavailable
//! for the first N requests, then succeeds. Used to test exponential backoff retry logic.
//!
//! Usage:
//! ```rust
//! let (server, url) = spawn_failing_server(3).await; // Fails first 3 requests
//! // Use url in HTTP transport - will retry with backoff
//! server.shutdown().await;
//! ```

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

/// Server state tracking failures
struct FailingServerState {
    fail_count: AtomicUsize,
    fail_first_n: usize,
    initialized: bool,
}

impl FailingServerState {
    fn new(fail_first_n: usize) -> Self {
        Self {
            fail_count: AtomicUsize::new(0),
            fail_first_n,
            initialized: false,
        }
    }

    /// Check if we should fail this request
    fn should_fail(&self) -> bool {
        let current = self.fail_count.fetch_add(1, Ordering::SeqCst);
        current < self.fail_first_n
    }

    /// Get current failure count
    fn failure_count(&self) -> usize {
        self.fail_count.load(Ordering::SeqCst)
    }
}

/// Mock HTTP server that fails first N requests
pub struct MockFailingServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockFailingServer {
    /// Start a server that fails the first N requests
    ///
    /// # Arguments
    /// * `fail_first_n` - Number of initial requests to fail with 503
    ///
    /// # Returns
    /// * `(MockFailingServer, String)` - Server handle and base URL
    pub async fn start(fail_first_n: usize) -> (Self, String) {
        let state = Arc::new(RwLock::new(FailingServerState::new(fail_first_n)));

        // Bind to localhost with random port
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

        // Create server from existing TcpListener
        let server = Server::from_tcp(listener.into_std().expect("Failed to convert listener"))
            .expect("Failed to create server from listener")
            .serve(make_svc);
        let graceful = server.with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

        // Spawn server in background
        tokio::spawn(async move {
            if let Err(e) = graceful.await {
                eprintln!("Server error: {}", e);
            }
        });

        // Give the server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let url = format!("http://{}", bound_addr);

        (
            MockFailingServer {
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

/// Spawn a mock server that fails the first N requests
///
/// Convenience function that creates and starts the server.
pub async fn spawn_failing_server(fail_first_n: usize) -> (MockFailingServer, String) {
    MockFailingServer::start(fail_first_n).await
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

/// Handle HTTP requests
async fn handle_request(
    req: Request<Body>,
    state: Arc<RwLock<FailingServerState>>,
) -> Result<Response<Body>, Infallible> {
    let should_fail = {
        let state = state.read().await;
        state.should_fail()
    };

    // Fail with 503 if we haven't reached the threshold
    if should_fail {
        let error_response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32099,
                message: "Service temporarily unavailable (simulated transient error)".to_string(),
                data: Some(serde_json::json!({
                    "type": "transient",
                    "retry_after": "100ms"
                })),
            }),
            id: Value::Null,
        };

        return Ok(Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .header("Retry-After", "1")
            .body(Body::from(serde_json::to_string(&error_response).unwrap()))
            .unwrap());
    }

    // Parse request body for actual handling
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

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&rpc_response).unwrap()))
        .unwrap())
}

/// Handle JSON-RPC requests
async fn handle_rpc_request(
    request: &JsonRpcRequest,
    _state: Arc<RwLock<FailingServerState>>,
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
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "mock-failing-server",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            })),
            error: None,
            id: request.id.clone(),
        },
        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!(null)),
            error: None,
            id: request.id.clone(),
        },
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!(null)),
            error: None,
            id: request.id.clone(),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_failing_server_start() {
        let (server, url) = spawn_failing_server(2).await;
        assert!(url.starts_with("http://127.0.0.1:"));
        assert_eq!(server.url(), url);
        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_failing_server_counts() {
        let (server, url) = spawn_failing_server(2).await;

        let client = reqwest::Client::new();

        // First request should fail with 503
        let response1 = client
            .post(&url)
            .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response1.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Second request should fail with 503
        let response2 = client
            .post(&url)
            .body(r#"{"jsonrpc":"2.0","method":"ping","id":2}"#)
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response2.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Third request should succeed
        let response3 = client
            .post(&url)
            .body(r#"{"jsonrpc":"2.0","method":"ping","id":3}"#)
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response3.status(), StatusCode::OK);

        server.shutdown().await;
    }
}
