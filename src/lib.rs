#![cfg_attr(not(test), warn(unused_must_use))]

// MCP CLI Rust Rewrite
//!
// Cross-platform MCP client with stdio and HTTP transport support.

pub mod error;
pub use error::{McpError, Result, exit_code};

// These modules will be created in subsequent plans
// pub mod client;
// pub mod config;
// pub mod cli;
