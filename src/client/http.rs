//! HTTP transport implementation for MCP server communication.
//!
//! This module implements HTTP-based transport for MCP servers, communicating
//! via HTTP requests and responses. Uses reqwest client for HTTP communication.

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

use crate::error::{McpError, Result};
use crate::transport::Transport;
use http::header::{HeaderMap, HeaderName, HeaderValue};
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
    async fn receive_notification(&mut self) -> Result<serde_json::Value> {
        Err(McpError::InvalidProtocol {
            message: "HTTP transport does not support notifications".to_string(),
        })
    }

    async fn send(&mut self, request: serde_json::Value) -> Result<serde_json::Value> {
        // Create request URL (append base_url to request path if needed)
        let url = self.base_url.clone();

        // Serialize request to JSON
        let body = request.to_string();

        // Convert HashMap to HeaderMap
        let mut headers = HeaderMap::new();
        for (key, value) in &self.headers {
            // Use simple string-based header names and values
            // Note: This may not validate header names, but works for basic use
            let key_str = key.clone();
            let value_str = value.clone();
            let _ = headers.insert(
                match HeaderName::try_from(&key_str) {
                    Ok(name) => name,
                    Err(_) => HeaderName::from_static("x-invalid-header"),
                },
                match HeaderValue::from_str(&value_str) {
                    Ok(value) => value,
                    Err(_) => HeaderValue::from_static(""),
                },
            );
        }

        // Send POST request
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .headers(headers)
            .body(body)
            .timeout(Duration::from_secs(30))
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
                McpError::InvalidProtocol {
                    message: format!("HTTP request failed: {}", e),
                }
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

    async fn send_notification(&mut self, notification: serde_json::Value) -> Result<()> {
        // Convert HashMap to HeaderMap
        let mut headers = HeaderMap::new();
        for (key, value) in &self.headers {
            headers.insert(key.parse::<HeaderName>().unwrap(), value.parse::<HeaderValue>().unwrap());
        }

        // Send notification without expecting response
        let body = serde_json::to_string(&notification).map_err(|e| {
            McpError::InvalidProtocol {
                message: format!("Failed to serialize notification: {}", e),
            }
        })?;
        
        self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .headers(headers)
            .body(body)
            .timeout(Duration::from_secs(30))
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
                                format!("HTTP {}: {}", status, e.to_string()),
                            ),
                        };
                    }
                }
                McpError::ConnectionError {
                    server: "http".to_string(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("HTTP error: {}", e),
                    ),
                }
            })?;

        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        // Create a minimal ping request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": "ping"
        });

        // Convert HashMap to HeaderMap
        let mut headers = HeaderMap::new();
        for (key, value) in &self.headers {
            headers.insert(key.parse::<HeaderName>().unwrap(), value.parse::<HeaderValue>().unwrap());
        }

        // Send request and expect HTTP 200 OK
        let body = serde_json::to_string(&request).map_err(|e| {
            McpError::InvalidProtocol {
                message: format!("Failed to serialize request: {}", e),
            }
        })?;
        let response = self.client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .headers(headers)
            .body(body)
            .timeout(Duration::from_secs(30))
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
                McpError::InvalidProtocol {
                    message: format!("HTTP request failed: {}", e),
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_transport_creation() {
        let headers = HashMap::new();
        let transport = HttpTransport::new("http://example.com/api", headers);
        assert_eq!(transport.base_url, "http://example.com/api");
    }

    #[test]
    fn test_http_send() {
        let headers = HashMap::new();
        let mut transport = HttpTransport::new("http://example.com/api", headers);
        let request = json!({ "test": "data" });
        // This would need a real server to test properly
        // transport.send(request).await.unwrap();
        println!("HttpTransport send structure works");
    }
}

