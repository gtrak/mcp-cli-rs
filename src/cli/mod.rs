//! CLI command handling for the MCP CLI tool.
//!
//! This module provides the command definitions, routing, and handler functions
//! for the `mcp-cli` binary. It is the entry point for all user-facing commands
//! including server discovery, tool execution, and daemon management.
//!
//! # Module Structure
//!
//! - [`entry`] — CLI entry point (`main()` and argument parsing)
//! - [`command_router`] — Dispatches parsed commands to handlers
//! - [`daemon_lifecycle`] — Daemon start/stop/auto-spawn logic
//! - [`config_setup`] — Configuration loading helpers
//! - [`commands`] — Individual command implementations (list, call, info, search)
//! - [`models`] — Shared data models for command output
//! - [`formatters`] — Human/JSON output formatting for command results
//! - [`filter`] — Tool filtering by name/description patterns
//!
//! # Usage
//!
//! This module is primarily used by `main.rs` to bootstrap the CLI:
//!
//! ```rust,ignore
//! use mcp_cli_rs::cli::entry;
//!
//! // Called from main.rs thin wrapper
//! entry::main().await;
//! ```

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
