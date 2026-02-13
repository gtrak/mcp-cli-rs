//! Connection pool interface for MCP server connections.
//!
//! This module defines the [`ConnectionPoolInterface`] trait used by the daemon
//! to manage persistent connections to MCP servers. The actual connection pool
//! implementation lives in [`crate::daemon::pool`]; this module provides the
//! trait abstraction and a [`DummyConnectionPool`] for testing.
//!
//! # Usage
//!
//! ```rust,ignore
//! use mcp_cli_rs::pool::{ConnectionPoolInterface, DummyConnectionPool};
//!
//! let pool = DummyConnectionPool::new();
//! let servers = pool.list_servers();
//! assert!(servers.is_empty());
//! ```

use crate::client::ToolInfo;
use async_trait::async_trait;

/// Connection pool trait
#[async_trait]
pub trait ConnectionPoolInterface: Send + Sync {
    /// Execute a tool on a server
    fn execute_tool(
        &self,
        _server_name: &str,
        _tool_name: &str,
        _arguments: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::Value::Null
    }

    /// List available tools on a server
    fn list_tools(&self, _server_name: &str) -> Vec<ToolInfo> {
        vec![]
    }

    /// List all configured servers
    fn list_servers(&self) -> Vec<String> {
        vec![]
    }
}

/// Dummy connection pool for stub implementation
#[derive(Default)]
pub struct DummyConnectionPool;

impl DummyConnectionPool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ConnectionPoolInterface for DummyConnectionPool {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_pool() {
        let pool = DummyConnectionPool::new();
        let args = serde_json::json!({});
        let result = pool.execute_tool("test", "test_tool", args);
        assert!(result.is_null());

        let tools = pool.list_tools("test");
        assert!(tools.is_empty());

        let servers = pool.list_servers();
        assert!(servers.is_empty());
    }
}
