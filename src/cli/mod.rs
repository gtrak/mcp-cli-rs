//! CLI module for MCP CLI tool.
//!
//! This module provides the command definitions and handler functions
//! for the MCP CLI application.

// Module declarations - keep these public for tests and binary
pub mod call;
pub mod command_router;
pub mod commands;
pub mod config_setup;
pub mod daemon;
pub mod daemon_lifecycle;
pub mod entry;
pub mod filter;
pub mod formatters;
pub mod info;
pub mod list;
pub mod models;
pub mod search;

// DetailLevel is used internally for output formatting
pub use crate::format::DetailLevel;

// Internal re-exports removed - modules are accessed directly:
// - entry module accessed as cli::entry::main from main.rs
// - formatters accessed as cli::formatters from tests
// - models accessed as cli::models from tests
// - filter accessed as cli::filter from tests
// - command_router accessed as cli::command_router from entry.rs
// - daemon_lifecycle accessed as cli::daemon_lifecycle from command_router.rs
