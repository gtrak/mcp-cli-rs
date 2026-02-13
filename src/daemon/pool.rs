//! Connection pool for MCP server connections.
//!
//! This module provides a thread-safe connection pool that caches transport connections
//! for MCP servers, ensuring connections are reused across multiple requests.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;
use crate::daemon::protocol::ToolInfo;
use crate::error::McpError;
use crate::error::Result;
use crate::transport::{BoxedTransport, Transport};

/// Represents a pooled MCP server connection with metadata for tracking.
pub struct PooledConnection {
    pub transport: BoxedTransport,
    pub server_name: String,
    pub created_at: Instant,
    pub last_used: Instant,
    pub health_check_failures: u32,
}

impl PooledConnection {
    fn is_healthy(&self) -> bool {
        self.health_check_failures < MAX_HEALTH_FAILURES
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
    }
}

const MAX_HEALTH_FAILURES: u32 = 3;

/// Connection pool that caches transport connections by server name.
#[derive(Clone)]
pub struct ConnectionPool {
    connections: Arc<Mutex<HashMap<String, PooledConnection>>>,
    config: Arc<Config>,
}

impl ConnectionPool {
    pub fn new(config: Arc<Config>) -> Self {
        ConnectionPool {
            connections: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Take a connection from the pool for use
    pub async fn take(&self, server_name: &str) -> Result<Option<PooledConnection>> {
        tracing::debug!("take() called for server: {}", server_name);
        let mut connections = self.connections.lock().expect("Failed to acquire connection pool lock");
        tracing::debug!("Got lock, checking for existing connection");

        if let Some(mut conn) = connections.remove(server_name) {
            tracing::debug!("Found existing connection for: {}", server_name);
            if conn.is_healthy() {
                conn.touch();
                tracing::debug!("Connection is healthy, returning it");
                return Ok(Some(conn));
            }
            tracing::debug!("Connection is unhealthy, discarding");
        }

        tracing::debug!(
            "No existing connection, creating new one for: {}",
            server_name
        );
        drop(connections); // Release lock before creating transport

        let transport = self.create_transport(server_name)?;
        Ok(Some(PooledConnection {
            transport,
            server_name: server_name.to_string(),
            created_at: Instant::now(),
            last_used: Instant::now(),
            health_check_failures: 0,
        }))
    }

    /// Return a connection to the pool
    pub fn put_back(&self, conn: PooledConnection) {
        let server_name = conn.server_name.clone();
        let mut connections = self.connections.lock().expect("Failed to acquire connection pool lock");
        connections.insert(server_name, conn);
        tracing::debug!("Returned connection to pool");
    }

    /// Initialize MCP protocol on a connection (shared between execute and list_tools)
    async fn initialize_mcp_connection(transport: &mut BoxedTransport) -> Result<()> {
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        transport.send(init_request).await.map_err(|e| McpError::InvalidProtocol {
            message: format!("Initialize request failed: {}", e),
        })?;

        let initialized_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        transport.send_notification(initialized_notification).await.map_err(|e| McpError::InvalidProtocol {
            message: format!("Failed to send initialized notification: {}", e),
        })?;

        Ok(())
    }

    /// Execute a tool using cached or new connection
    pub async fn execute(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        tracing::debug!(
            "execute() called for server: {}, tool: {}",
            server_name,
            tool_name
        );
        let mut conn = match self.take(server_name).await? {
            Some(c) => c,
            None => {
                return Err(McpError::ServerNotFound {
                    server: server_name.to_string(),
                });
            }
        };
        tracing::debug!("Got connection, initializing MCP connection");

        Self::initialize_mcp_connection(&mut conn.transport).await?;

        tracing::debug!("MCP connection initialized, sending tools/call request");

        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": tool_name, "arguments": arguments }
        });

        let result = match conn.transport.send(mcp_request).await {
            Ok(response) => {
                if let Some(result) = response.get("result") {
                    Ok(result.clone())
                } else if let Some(error) = response.get("error") {
                    let msg = error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown");
                    Err(McpError::InvalidProtocol {
                        message: format!("Tool error: {}", msg),
                    })
                } else {
                    Err(McpError::InvalidProtocol {
                        message: "Invalid response".to_string(),
                    })
                }
            }
            Err(e) => Err(McpError::InvalidProtocol {
                message: format!("Transport error: {}", e),
            }),
        };

        self.put_back(conn);
        result
    }

    /// List tools using cached or new connection
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<ToolInfo>> {
        tracing::debug!("list_tools() called for server: {}", server_name);
        let mut conn = match self.take(server_name).await? {
            Some(c) => c,
            None => {
                return Err(McpError::ServerNotFound {
                    server: server_name.to_string(),
                });
            }
        };
        tracing::debug!("Got connection, initializing MCP connection");

        Self::initialize_mcp_connection(&mut conn.transport).await?;

        tracing::debug!("MCP connection initialized, sending tools/list request");

        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        let result = match conn.transport.send(mcp_request).await {
            Ok(response) => {
                tracing::debug!("Got response: {:?}", response);
                if let Some(result) = response.get("result") {
                    let tools: Vec<ToolInfo> = if let Some(tools_array) =
                        result.get("tools").and_then(|t| t.as_array())
                    {
                        tools_array
                            .iter()
                            .filter_map(|tool| {
                                let name = tool.get("name")?.as_str()?.to_string();
                                let description = tool.get("description")?.as_str()?.to_string();
                                let input_schema =
                                    tool.get("inputSchema").cloned().unwrap_or_else(|| {
                                        serde_json::Value::Object(serde_json::Map::new())
                                    });
                                Some(ToolInfo {
                                    name,
                                    description,
                                    input_schema,
                                })
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    Ok(tools)
                } else if let Some(error) = response.get("error") {
                    let msg = error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown");
                    Err(McpError::InvalidProtocol {
                        message: format!("List error: {}", msg),
                    })
                } else {
                    Err(McpError::InvalidProtocol {
                        message: "Invalid response".to_string(),
                    })
                }
            }
            Err(e) => Err(McpError::InvalidProtocol {
                message: format!("Transport error: {}", e),
            }),
        };

        self.put_back(conn);
        result
    }

    fn create_transport(&self, server_name: &str) -> Result<BoxedTransport> {
        tracing::debug!("Creating transport for server: {}", server_name);
        let server_config = self
            .config
            .servers
            .iter()
            .find(|s| s.name.as_str() == server_name)
            .ok_or_else(|| McpError::ServerNotFound {
                server: server_name.to_string(),
            })?;

        tracing::debug!("Found server config: name={}", server_config.name);
        let result = server_config.create_transport(server_name);
        match &result {
            Ok(_) => tracing::debug!("Transport created successfully"),
            Err(e) => tracing::debug!("Transport creation failed: {}", e),
        }
        result.map_err(|e| McpError::ConnectionError {
            server: server_name.to_string(),
            source: std::io::Error::other(e.to_string()),
        })
    }

    pub fn clear(&self) {
        let mut connections = self.connections.lock().expect("Failed to acquire connection pool lock");
        connections.clear();
    }

    pub fn count(&self) -> usize {
        self.connections.lock().expect("Failed to acquire connection pool lock").len()
    }
}

#[async_trait::async_trait]
pub trait ConnectionPoolInterface: Send + Sync {
    async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>>;
    fn remove(&self, server_name: &str);
    fn clear(&self);
    fn count(&self) -> usize;
}

#[async_trait::async_trait]
impl ConnectionPoolInterface for ConnectionPool {
    async fn get(&self, server_name: &str) -> Result<Box<dyn Transport + Send + Sync>> {
        if let Some(conn) = self.take(server_name).await? {
            Ok(conn.transport)
        } else {
            Err(McpError::ServerNotFound {
                server: server_name.to_string(),
            })
        }
    }

    fn remove(&self, server_name: &str) {
        let mut connections = self.connections.lock().expect("Failed to acquire connection pool lock");
        connections.remove(server_name);
    }

    fn clear(&self) {
        self.clear();
    }

    fn count(&self) -> usize {
        self.count()
    }
}

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
        Ok(Box::new(DummyTransport))
    }
    pub fn remove(&self, _server_name: &str) {}
    pub fn clear(&self) {}
    pub fn count(&self) -> usize {
        self.count
    }
}

impl Default for DummyConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

struct DummyTransport;

#[async_trait]
impl crate::transport::Transport for DummyTransport {
    async fn send(
        &mut self,
        _request: serde_json::Value,
    ) -> crate::error::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": "success"}))
    }
    async fn send_notification(
        &mut self,
        _notification: serde_json::Value,
    ) -> crate::error::Result<()> {
        Ok(())
    }
    async fn receive_notification(&mut self) -> crate::error::Result<serde_json::Value> {
        Ok(serde_json::json!({"method": "notifications/initialized"}))
    }
    async fn ping(&self) -> crate::error::Result<()> {
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
    fn test_pool_new() {
        let config = Arc::new(Config::default());
        let pool = ConnectionPool::new(config);
        assert_eq!(pool.count(), 0);
    }
}
