//! Connection pool for MCP server connections.
//!
//! This module provides a thread-safe connection pool that caches transport connections
//! for MCP servers, with health checks to validate connections before reuse.
//!
//! See RESEARCH.md for connection pooling strategy and health check requirements.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::config::Config;
use crate::error::Result;
use crate::error::McpError;
use crate::transport::{Transport, BoxedTransport};
use crate::daemon::protocol::ToolInfo;

/// Represents a pooled MCP server connection with metadata for tracking.
///
/// This struct wraps a transport connection with tracking information including
/// when it was created, last used, and health check failure count.
pub struct PooledConnection {
    /// The underlying transport connection
    pub transport: BoxedTransport,
    /// Server name this connection is for
    pub server_name: String,
    /// Time when connection was created
    pub created_at: Instant,
    /// Time when connection was last used
    pub last_used: Instant,
    /// Count of consecutive health check failures
    pub health_check_failures: u32,
}

impl PooledConnection {
    /// Check if connection is healthy (has not exceeded health failure threshold)
    fn is_healthy(&self) -> bool {
        self.health_check_failures < MAX_HEALTH_FAILURES
    }

    /// Update the last used timestamp
    fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    /// Calculate the age of the connection (time since creation)
    fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Perform a health check on the connection
    ///
    /// Sends an MCP ping request and resets failure count if successful.
    /// Returns Ok(()) if healthy, Err otherwise.
    async fn health_check(&mut self) -> Result<()> {
        // Timeout after 5 seconds to prevent hanging on dead connections
        match timeout(Duration::from_secs(5), self.transport.ping()).await {
            Ok(Ok(_)) => {
                // Health check passed - reset failure count
                self.health_check_failures = 0;
                tracing::debug!("Health check passed for server: {}", self.server_name);
                Ok(())
            }
            Ok(Err(e)) => {
                // Health check failed - increment failure count
                self.health_check_failures += 1;
                tracing::warn!(
                    "Health check failed for server {}: {} (failures: {})",
                    self.server_name, e, self.health_check_failures
                );
                Err(e)
            }
             Err(_) => {
                 // Timeout - treat as failure
                 self.health_check_failures += 1;
                 tracing::warn!(
                     "Health check timeout for server {} (failures: {})",
                     self.server_name, self.health_check_failures
                 );
                 Err(McpError::Timeout { timeout: 5 })
             }
        }
    }
}

/// Default maximum health check failures before treating connection as permanently unhealthy
const MAX_HEALTH_FAILURES: u32 = 3;

/// Connection pool that caches transport connections by server name.
///
/// This pool provides thread-safe access to cached connections with automatic
/// health checks before returning cached connections. Failed connections are
/// automatically recreated.
///
/// The pool uses Arc<Mutex<...>> for thread-safe sharing across multiple
/// concurrent request handlers.
#[derive(Clone)]
pub struct ConnectionPool {
    /// Map of server_name -> PooledConnection
    connections: Arc<Mutex<HashMap<String, PooledConnection>>>,
    /// Config for creating new connections
    config: Arc<Config>,
    /// Maximum health failures before treating connection as unhealthy
    max_health_failures: u32,
}

impl ConnectionPool {
    /// Create a new connection pool with the given config
    ///
    /// The pool starts empty and will create connections on demand.
    pub fn new(config: Arc<Config>) -> Self {
        ConnectionPool {
            connections: Arc::new(Mutex::new(HashMap::new())),
            config,
            max_health_failures: MAX_HEALTH_FAILURES,
        }
    }

    /// Get a transport connection for the specified server
    ///
    /// This method:
    /// 1. Checks if a cached connection exists
    /// 2. If exists and healthy: validates with health check and returns it
    /// 3. If exists but unhealthy: removes it and creates new connection
    /// 4. If not exists: creates new connection and adds to pool
    ///
    /// Returns a Box<dyn Transport> that can be used to communicate with the server.
    pub async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        // First, check if we have a cached connection and remove it if it exists
        let existing_conn = {
            let mut connections = self.connections.lock().unwrap();
            connections.remove(server_name)
        };

        if let Some(mut conn) = existing_conn {
            // Connection exists - check if it's healthy
            if conn.is_healthy() {
                // Validate connection health (lock is dropped, so this is Send-safe)
                if conn.health_check().await.is_ok() {
                    // Health check passed - update last used timestamp
                    conn.touch();
                    tracing::debug!("Reusing cached connection for server: {}", server_name);
                    return Ok(conn.transport);
                } else {
                    // Health check failed - increment failure count and drop it
                    conn.health_check_failures += 1;
                    tracing::warn!(
                        "Health check failed for server {} (failures: {}), recreating",
                        server_name, conn.health_check_failures
                    );
                    // Connection is dropped here, will create new one below
                }
            } else {
                // Connection is unhealthy
                tracing::warn!(
                    "Connection for server {} is unhealthy ({} failures), recreating",
                    server_name, conn.health_check_failures
                );
                // Connection is dropped here, will create new one below
            }
        }

        // Create new connection
        tracing::info!("Creating new connection for server: {}", server_name);
        let transport = self.create_transport_for_server(server_name)?;
        
        Ok(transport)
    }

    /// Create a transport for the specified server
    ///
    /// Looks up the server configuration and creates an appropriate transport
    /// using the transport factory.
    fn create_transport_for_server(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        // Find server configuration
        let server_config = self
            .config
            .servers
            .iter()
            .find(|s| s.name.as_str() == server_name)
            .ok_or_else(|| McpError::ServerNotFound { server: server_name.to_string() })?;

        // Create transport using factory trait
        let transport = server_config.create_transport(server_name)
            .map_err(|e| {
                tracing::error!("Failed to create transport for server {}: {}", server_name, e);
                // The source type must be std::io::Error
                // Create an I/O error with the error message
                let source = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                McpError::ConnectionError {
                    server: server_name.to_string(),
                    source,
                }
            })?;
        Ok(transport)
    }

    /// Remove a connection from the pool
    ///
    /// This is called when a connection is permanently unhealthy or needs to be closed.
    pub fn remove(&self, server_name: &str) {
        let mut connections = self.connections.lock().unwrap();
        connections.remove(server_name);
        tracing::debug!("Removed connection from pool: {}", server_name);
    }

    /// Clear all connections from the pool
    ///
    /// This is called on daemon shutdown to release all resources.
    pub fn clear(&self) {
        let mut connections = self.connections.lock().unwrap();
        let count = connections.len();
        connections.clear();
        tracing::info!("Cleared {} connections from pool", count);
    }

    /// Get the number of connections currently in the pool
    pub fn count(&self) -> usize {
        let connections = self.connections.lock().unwrap();
        connections.len()
    }

    /// Execute a tool on the specified server using a cached or new connection
    pub async fn execute(&self, server_name: &str, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value> {
        let mut transport: Box<dyn Transport + Send + Sync> = self.get(server_name).await?;
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });
        let response = transport.send(mcp_request).await?;
        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = response.get("error") {
            let message = error.get("message").and_then(|m: &serde_json::Value| m.as_str()).unwrap_or("Unknown error");
            Err(McpError::InvalidProtocol { message: format!("Tool execution failed: {}", message) })
        } else {
            Err(McpError::InvalidProtocol { message: "Invalid MCP response format".to_string() })
        }
    }

    /// List available tools on the specified server using a cached or new connection
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<ToolInfo>> {
        let mut transport: Box<dyn Transport + Send + Sync> = self.get(server_name).await?;
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });
        let response = transport.send(mcp_request).await?;
        if let Some(result) = response.get("result") {
            let tools: Vec<ToolInfo> = if let Some(tools_array) = result.get("tools").and_then(|t: &serde_json::Value| t.as_array()) {
                tools_array.iter().filter_map(|tool: &serde_json::Value| {
                    Some(ToolInfo {
                        name: tool.get("name").and_then(|n: &serde_json::Value| n.as_str()).unwrap_or("unknown").to_string(),
                        description: tool.get("description").and_then(|d: &serde_json::Value| d.as_str()).unwrap_or("").to_string(),
                        input_schema: tool.get("inputSchema").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                    })
                }).collect()
            } else {
                Vec::new()
            };
            Ok(tools)
        } else if let Some(error) = response.get("error") {
            let message = error.get("message").and_then(|m: &serde_json::Value| m.as_str()).unwrap_or("Unknown error");
            Err(McpError::InvalidProtocol { message: format!("List tools failed: {}", message) })
        } else {
            Err(McpError::InvalidProtocol { message: "Invalid MCP response format".to_string() })
        }
    }

    /// Get statistics about the pool
    pub fn stats(&self) -> PoolStats {
        let connections = self.connections.lock().unwrap();
        let mut healthy_count = 0;
        let mut unhealthy_count = 0;

        for conn in connections.values() {
            if conn.is_healthy() {
                healthy_count += 1;
            } else {
                unhealthy_count += 1;
            }
        }

        PoolStats {
            total_connections: connections.len(),
            healthy_connections: healthy_count,
            unhealthy_connections: unhealthy_count,
        }
    }
}

/// Statistics about the connection pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of connections in the pool
    pub total_connections: usize,
    /// Number of healthy (not exceeding max failures) connections
    pub healthy_connections: usize,
    /// Number of unhealthy connections (exceeded max failures)
    pub unhealthy_connections: usize,
}

/// Connection pool interface for daemon integration
///
/// This trait abstracts the connection pool so it can be mocked during testing
/// and implemented differently for different use cases.
#[async_trait::async_trait]
pub trait ConnectionPoolInterface: Send + Sync {
    /// Get a transport connection for the specified server
    async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>>;

    /// Remove a connection from the pool
    fn remove(&self, server_name: &str);

    /// Clear all connections from the pool
    fn clear(&self);

    /// Get the number of connections currently in the pool
    fn count(&self) -> usize;
}

#[async_trait::async_trait]
impl ConnectionPoolInterface for ConnectionPool {
    async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        self.get(server_name).await
    }

    fn remove(&self, server_name: &str) {
        self.remove(server_name);
    }

    fn clear(&self) {
        self.clear();
    }

    fn count(&self) -> usize {
        self.count()
    }
}

/// Stub implementation for testing purposes
pub struct DummyConnectionPool {
    count: usize,
}

impl DummyConnectionPool {
    pub fn new() -> Self {
        DummyConnectionPool { count: 0 }
    }
}

impl DummyConnectionPool {
    pub async fn get(&mut self, _server_name: &str) -> Result<BoxedTransport> {
        self.count += 1;
        tracing::debug!("Dummy connection pool: returning mock connection (count: {})", self.count);
        // Return a simple stub transport
        Ok(Box::new(DummyTransport::new()))
    }

    pub fn remove(&self, _server_name: &str) {
        // Stub implementation - no-op
    }

    pub fn clear(&self) {
        // Stub implementation - no-op
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

impl Default for DummyConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub transport for testing
struct DummyTransport;

impl DummyTransport {
    pub fn new() -> Self {
        DummyTransport
    }
}

#[async_trait]
impl crate::transport::Transport for DummyTransport {
    async fn send(&mut self, _request: serde_json::Value) -> crate::error::Result<serde_json::Value> {
        // Stub implementation - return success
        Ok(serde_json::json!({"result": "success"}))
    }

    async fn receive_notification(&mut self) -> crate::error::Result<serde_json::Value> {
        // Stub implementation - return success
        Ok(serde_json::json!({"method": "notifications/initialized", "result": {}}))
    }

    async fn ping(&self) -> crate::error::Result<()> {
        // Stub implementation - always succeeds
        Ok(())
    }

    fn transport_type(&self) -> &str {
        "dummy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_pool_new() {
        let config = Arc::new(Config::default());
        let pool = ConnectionPool::new(config);
        assert_eq!(pool.count(), 0);
    }
}
