//! Configuration module for MCP server definitions.
//!
//! This module provides types and utilities for parsing MCP server configurations
//! from TOML files, supporting both stdio and HTTP transports.
//!
//! The module is organized into focused sub-modules:
//! - [`types`](types::index.html) - Core config types (ServerConfig, ServerTransport, Config)
//! - [`parser`](parser::index.html) - TOML parsing logic
//! - [`validator`](validator::index.html) - Configuration validation
//! - [`loader`](loader::index.html) - File loading and discovery utilities

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
