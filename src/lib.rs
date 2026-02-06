//! MCP CLI Rust Rewrite
//!
//! Cross-platform MCP client with stdio and HTTP transport support.

pub mod error;
pub use error::{exit_code, McpError, Result};

// These modules will be created in subsequent plans
// pub mod client;
// pub mod config;
// pub mod cli;
