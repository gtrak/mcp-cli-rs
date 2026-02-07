//! Connection pool for MCP server connections.
//!
//! This module provides a thread-safe connection pool that caches transport connections
//! for MCP servers, with health checks to validate connections before reuse.
//!
//! See RESEARCH.md for connection pooling strategy and health check requirements.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::config::Config;
use crate::error::Result as StdResult;
use crate::transport::{Transport, TransportFactory};

/// Represents a pooled MCP server connection with metadata for tracking.
///
/// This struct wraps a transport connection with tracking information including
/// when it was created, last used, and health check failure count.
#[derive(Clone)]
pub struct PooledConnection {
    /// The underlying transport connection
    pub transport: Box<dyn Transport + Send + Sync>,
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
    async fn health_check(&mut self) -> StdResult<()> {
        let ping_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": "health-check"
        });

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
                Err(anyhow::anyhow!("Health check timeout for server {}", self.server_name))
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
    ///
    /// This method is atomic (mutex-protected) and handles all caching logic.
    pub async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        let mut connections = self.connections.lock().unwrap();
        let mut pooled_conn = connections
            .entry(server_name.to_string())
            .or_insert_with(|| {
                tracing::info!("Creating new connection for server: {}", server_name);
                PooledConnection {
                    transport: Box::new(self.create_transport_for_server(server_name)?),
                    server_name: server_name.to_string(),
                    created_at: Instant::now(),
                    last_used: Instant::now(),
                    health_check_failures: 0,
                }
            });

        // Check if connection is healthy
        if !pooled_conn.is_healthy() {
            tracing::warn!(
                "Connection for server {} is unhealthy ({} failures), recreating",
                server_name, pooled_conn.health_check_failures
            );
            // Remove unhealthy connection
            connections.remove(server_name);
            // Create new connection (fall through to creation below)
        } else {
            // Validate connection health
            if let Err(e) = pooled_conn.health_check().await {
                // Health check failed - increment failure count and recreate
                pooled_conn.health_check_failures += 1;
                tracing::warn!(
                    "Health check failed for server {} (failures: {}), recreating",
                    server_name, pooled_conn.health_check_failures
                );
                // Remove connection to trigger recreation
                connections.remove(server_name);
            } else {
                // Health check passed - update last used timestamp
                pooled_conn.touch();
                tracing::debug!("Reusing cached connection for server: {}", server_name);
            }
        }

        // Create new transport if connection was removed or didn't exist
        let mut new_connection = connections
            .entry(server_name.to_string())
            .or_insert_with(|| {
                tracing::info!("Creating new connection for server: {}", server_name);
                PooledConnection {
                    transport: Box::new(self.create_transport_for_server(server_name)?),
                    server_name: server_name.to_string(),
                    created_at: Instant::now(),
                    last_used: Instant::now(),
                    health_check_failures: 0,
                }
            });

        // Update last used timestamp
        new_connection.touch();
        Ok(new_connection.transport.clone())
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
            .ok_or_else(|| anyhow::anyhow!("Server not found in config: {}", server_name))?;

        // Create transport using factory
        let transport = server_config.transport_factory.create_transport(server_name);
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
    pub async fn get(&mut self, _server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        self.count += 1;
        tracing::debug!("Dummy connection pool: returning mock connection (count: {})", self.count);
        Ok(Box::new(crate::transport::StubTransport::new()))
    }

    pub fn remove(&mut self, _server_name: &str) {
        self.count -= 1;
    }

    pub fn clear(&mut self) {
        self.count = 0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_fingerprint() {
        let config = Config {
            servers: vec![],
        };
        let fp = config_fingerprint(&config);
        assert!(!fp.is_empty());
    }

    #[test]
    fn test_handle_request_ping() {
        let lifecycle = DaemonLifecycle::new(30);
        let config = Config { servers: vec![] };
        let state = DaemonState {
            config: Arc::new(config),
            config_fingerprint: String::new(),
            lifecycle,
            connection_pool: Arc::new(Mutex::new(crate::pool::ConnectionPool::new())),
        };

        let response = handle_request(DaemonRequest::Ping, &state);
        assert!(matches!(response, DaemonResponse::Pong));
    }

    #[test]
    fn test_handle_request_shutdown() {
        let lifecycle = DaemonLifecycle::new(30);
        let config = Config { servers: vec![] };
        let state = DaemonState {
            config: Arc::new(config),
            config_fingerprint: String::new(),
            lifecycle,
            connection_pool: Arc::new(Mutex::new(crate::pool::ConnectionPool::new())),
        };

        let response = handle_request(DaemonRequest::Shutdown, &state);
        assert!(matches!(response, DaemonResponse::ShutdownAck));
        assert!(!lifecycle.is_running());
    }
}
