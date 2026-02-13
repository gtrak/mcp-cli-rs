//! Configuration types and parsing for MCP server definitions.
//!
//! This module provides types and utilities for parsing MCP server configurations
//! from TOML files, supporting both stdio and HTTP transports.
//!
//! # Module Structure
//!
//! - **types** — Core types: [`Config`], [`ServerConfig`], [`ServerTransport`]
//! - **parser** — TOML parsing logic ([`parse_toml`])
//! - **validator** — Configuration validation ([`validate_config`], [`validate_server_config`])
//! - [`loader`] — File loading and config discovery utilities
//!
//! # Usage
//!
//! ```rust
//! use mcp_cli_rs::config::{Config, ServerConfig, ServerTransport, parse_toml};
//! use std::path::Path;
//!
//! // Parse a TOML configuration string
//! let toml = r#"
//! [[servers]]
//! name = "my-server"
//! [servers.transport]
//! type = "stdio"
//! command = "npx"
//! args = ["-y", "@modelcontextprotocol/server-everything"]
//! "#;
//!
//! let config = parse_toml(toml, Path::new("example.toml")).expect("valid TOML");
//! assert_eq!(config.servers.len(), 1);
//! assert_eq!(config.servers[0].name, "my-server");
//! ```

// Re-export all public items for backward compatibility
pub use crate::config::parser::parse_toml;
pub use crate::config::validator::{validate_config, validate_server_config};

// Re-export types (backward compatible)
pub use crate::config::types::{Config, ServerConfig, ServerTransport};

// Keep loader module for file loading utilities
pub mod loader;

// Re-export for internal use within config module
pub(crate) mod parser;
pub(crate) mod types;
pub(crate) mod validator;
