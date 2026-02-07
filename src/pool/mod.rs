//! Connection pool for server connections managed by daemon
//!
//! Full implementation in 02-04. This provides a placeholder trait
//! for daemon state.

use crate::client::ToolInfo;
use async_trait::async_trait;

/// Connection pool trait
#[async_trait]
pub trait ConnectionPoolInterface: Send + Sync {
    /// Execute a tool on a server
    fn execute_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::Value::Null
    }

    /// List available tools on a server
    fn list_tools(&self, server_name: &str) -> Vec<ToolInfo> {
        vec![]
    }

    /// List all configured servers
    fn list_servers(&self) -> Vec<String> {
        vec![]
    }
}

/// Dummy connection pool for stub implementation
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
