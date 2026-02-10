//! CLI module for MCP CLI tool.
//!
//! This module provides the command definitions and handler functions
//! for the MCP CLI application.

pub mod commands;
pub mod daemon;
pub mod filter;

// Re-export DetailLevel from format module
pub use crate::format::DetailLevel;

pub use commands::{
    cmd_list_servers,
    cmd_server_info,
    cmd_tool_info,
    cmd_call_tool,
    cmd_search_tools,
    parse_tool_id,
};
