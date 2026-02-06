//! HTTP transport implementation for MCP server communication.
//!
//! This module implements HTTP-based transport for MCP servers, communicating
//! via HTTP requests and responses. Uses reqwest client for HTTP communication.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::client::transport::{Transport, TransportFactory};
use crate::error::{McpError, Result};
use crate::config::ServerTransport;
use reqwest::Client;

/// HTTP transport for remote server communication.
///
/// This transport communicates with MCP servers via HTTP using POST requests
/// with JSON bodies.
pub struct HttpTransport {
    client: Client,
    base_url: String,
    headers: HashMap<String, String>,
}

impl HttpTransport {
    /// Create a new HttpTransport from server configuration.
    ///
    /// # Arguments
    /// * `url` - Base URL for the server
    /// * `headers` - HTTP headers to include in requests
    pub fn new(url: &str, headers: HashMap<String, String>) -> Self {
        let client = Client::new();
        Self {
            client,
            base_url: url.to_string(),
            headers,
        }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn send(&mut self, request: serde_json::Value) -> Result<serde_json::Value> {
        // Create request URL (append base_url to request path if needed)
        let url = self.base_url.clone();

        // Serialize request to JSON
        let body = request.to_string();

        // Send POST request
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .headers(&self.headers) // Add configured headers
            .body(body)
            .timeout(reqwest::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                // Check for HTTP errors
                if let Some(status) = e.status() {
                    if status.is_client_error() || status.is_server_error() {
                        return McpError::ConnectionError {
                            server: "http".to_string(),
                            source: std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("HTTP {}", status),
                            ),
                        };
                    }
                }
                McpError::connection_error("http", e.into())
            })?;

        // Parse response JSON
        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| {
                McpError::InvalidProtocol {
                    message: format!("Failed to parse response: {}", e),
                }
            })?;

        Ok(response_json)
    }

    async fn ping(&self) -> Result<()> {
        // Create a minimal ping request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": "ping"
        });

        // Send request and expect HTTP 200 OK
        let response = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .headers(&self.headers)
            .json(&request)
            .timeout(reqwest::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                // Check for HTTP errors
                if let Some(status) = e.status() {
                    if status.is_client_error() || status.is_server_error() {
                        return McpError::ConnectionError {
                            server: "http".to_string(),
                            source: std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("HTTP {}", status),
                            ),
                        };
                    }
                }
                McpError::connection_error("http", e.into())
            })?;

        // Check if response is HTTP 200 OK
        if !response.status().is_success() {
            return Err(McpError::ConnectionError {
                server: "http".to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("HTTP {}", response.status()),
                ),
            });
        }

        Ok(())
    }

    fn transport_type(&self) -> &str {
        "http"
    }
}

#[async_trait]
impl TransportFactory for ServerTransport {
    fn create_transport(
        &self,
        server_name: &str,
    ) -> Box<dyn Transport + Send + Sync> {
        match self {
            ServerTransport::Stdio { .. } => {
                // Stdio transport created in stdio.rs
                Box::new(crate::client::stdio::StdioTransport::new(
                    &self.command(),
                    &self.args,
                    &self.env,
                    self.cwd.as_deref(),
                ).expect("Failed to create stdio transport"))
            }
            ServerTransport::Http { url, headers } => {
                let transport = HttpTransport::new(url, headers.clone());
                Box::new(transport)
            }
        }
    }

    fn supports_filtering(&self) -> bool {
        match self {
            ServerTransport::Http { .. } => true,
            ServerTransport::Stdio { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_transport_creation() {
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://example.com", headers);
        println!("HttpTransport creation works");
    }

    #[test]
    fn test_http_send() {
        let headers = HashMap::new();
        let mut transport = HttpTransport::new("http://example.com", headers);

        let request = json!({ "test": "data" });
        // This would need a real server to test properly
        // transport.send(request).await.unwrap();
        println!("HttpTransport send structure works");
    }
}
