//! MCP CLI Rust Rewrite
//!
//! Cross-platform MCP client with stdio and HTTP transport support.

pub mod cli;
pub mod client;
pub use client::{McpClient, ToolInfo};
pub mod config;
pub mod error;
pub use error::{exit_code, McpError, Result as DaemonResult};

// Re-export modules for easy access
pub mod transport;
pub use transport::{Transport, TransportFactory};
pub use config::ServerTransport;

// Daemon module
pub mod daemon;
pub use daemon::{run_daemon, DaemonState};
pub mod ipc;
pub use ipc::{create_ipc_server, get_socket_path};

// Connection pool (stub, full impl in 02-04)
pub mod pool;
pub use pool::{ConnectionPoolInterface, DummyConnectionPool};
