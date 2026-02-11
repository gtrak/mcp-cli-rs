//! Transport abstraction for MCP server communication.
//!
//! This module provides a trait-based abstraction for communicating with MCP servers
//! over different transports (stdio, HTTP). This enables the client to switch between
//! transports without code changes.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;

/// Transport protocol for MCP server communication.
///
/// This trait provides a common interface for sending JSON-RPC messages to MCP servers
/// regardless of the underlying transport (stdio, HTTP, etc.).
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a JSON-RPC request and get the response.
    ///
    /// This method serializes the request to JSON, sends it to the server,
    /// and returns the parsed response.
    async fn send(&mut self, request: Value) -> Result<Value>;

    /// Send a JSON-RPC notification without expecting a response.
    ///
    /// This method serializes notification to JSON, sends it to server,
    /// and doesn't wait for a response. Used for notifications like
    /// "notifications/initialized" in MCP protocol.
    async fn send_notification(&mut self, notification: Value) -> Result<()>;

    /// Receive a JSON-RPC notification from the server.
    ///
    /// This method reads unsolicited notifications sent by the server,
    /// such as "notifications/initialized" after an initialize request.
    /// Used in Phase 1 for MCP protocol initialization (INIT-01).
    async fn receive_notification(&mut self) -> Result<Value>;

    /// Check if the connection is healthy.
    ///
    /// This method sends a minimal "ping" request and expects a response.
    /// Used in Phase 2 for connection health checks (CONN-06).
    async fn ping(&self) -> Result<()>;

    /// Get the transport type for debugging.
    ///
    /// Returns a string identifying the transport type (e.g., "stdio", "http").
    fn transport_type(&self) -> &str;
}

/// Trait extension for transport factory methods.
///
/// This trait allows server configurations to be converted to transport instances.
#[async_trait]
pub trait TransportFactory: Send + Sync {
    /// Convert a ServerTransport configuration to an actual Transport instance.
    ///
    /// This method is implemented for ServerTransport enum to create stdio or
    /// HTTP transport instances based on the configuration.
    fn create_transport(&self, server_name: &str) -> Box<dyn Transport + Send + Sync>;

    /// Check if transport requires tool filtering support (Phase 4).
    fn supports_filtering(&self) -> bool {
        false
    }
}

/// Type alias for boxed, thread-safe transport connections.
///
/// This simplifies the signature of methods that return BoxedTransport.
pub type BoxedTransport = Box<dyn Transport + Send + Sync>;

#[cfg(test)]
mod tests {
    #[test]
    fn test_transport_type() {
        // This is a placeholder test - actual transport implementations will be in stdio.rs and http.rs
        println!("Transport trait implemented");
    }
}

// TransportFactory trait is already defined above, no re-export needed
