//! CLI module for MCP CLI tool.
//!
//! This module provides the command definitions and handler functions
//! for the MCP CLI application.

pub mod call;
pub mod commands;
pub mod command_router;
pub mod config_setup;
pub mod daemon;
pub mod daemon_lifecycle;
pub mod entry;
pub mod filter;
pub mod info;
pub mod list;
pub mod search;

// Re-export from entry module (CLI entry point)
pub use entry::{Cli, init_tracing, main as entry_main};

// Re-export from command_router
pub use command_router::{Commands, dispatch_command, execute_command, get_run_mode, RunMode};

// Re-export DetailLevel from format module
pub use crate::format::DetailLevel;

// Re-export command functions from specialized modules
pub use call::cmd_call_tool;
pub use commands::{
    cmd_call_tool as old_cmd_call_tool, cmd_list_servers as old_cmd_list_servers,
    cmd_search_tools as old_cmd_search_tools, cmd_server_info as old_cmd_server_info,
    cmd_tool_info as old_cmd_tool_info, parse_tool_id,
};
pub use daemon_lifecycle::{
    connect_or_spawn_daemon, connect_to_daemon, DirectProtocolClient,
    create_direct_client, create_auto_daemon_client, create_require_daemon_client,
};
pub use info::{cmd_server_info, cmd_tool_info};
pub use list::{cmd_list_servers, cmd_list_servers_json};
pub use search::cmd_search_tools;
