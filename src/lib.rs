//! MCP CLI Rust Rewrite
//!
//! Cross-platform MCP client with stdio and HTTP transport support.

pub mod cli;
pub mod client;
pub use client::{McpClient, ToolInfo};
pub mod config;
pub mod error;
pub use error::{exit_code, McpError, Result};

// Re-export modules for easy access
pub mod transport;
pub use transport::{Transport, TransportFactory};
pub use config::ServerTransport;
pub use cli::commands::AppContext;
