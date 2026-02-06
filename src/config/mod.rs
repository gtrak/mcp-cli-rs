//! Configuration module for MCP server definitions.
//!
//! This module provides types and utilities for parsing MCP server configurations
//! from TOML files, supporting both stdio and HTTP transports.

use serde::Deserialize;
use std::collections::HashMap;

/// Transport protocol for MCP server connections.
///
/// Supports both local stdio execution and remote HTTP connections.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ServerTransport {
    /// Server runs locally via stdio communication.
    ///
    /// The server process is spawned with the specified command, arguments,
    /// environment variables, and working directory.
    #[serde(rename = "stdio")]
    Stdio {
        /// Command to execute (required for stdio transport).
        #[serde(default)]
        command: String,

        /// Command arguments (optional, defaults to empty slice).
        #[serde(default)]
        args: Vec<String>,

        /// Environment variables to pass to the server process.
        #[serde(default)]
        env: HashMap<String, String>,

        /// Working directory for the server process.
        #[serde(default)]
        cwd: Option<String>,
    },

    /// Server accessed remotely via HTTP.
    ///
    /// The server is contacted at the specified URL with optional headers.
    #[serde(rename = "http")]
    Http {
        /// Server URL (required for HTTP transport).
        #[serde(default)]
        url: String,

        /// HTTP headers to include in requests.
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// Configuration for a single MCP server.
///
/// Represents a configured MCP server with optional tool filtering capabilities.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Unique server identifier.
    pub name: String,

    /// Transport protocol configuration.
    pub transport: ServerTransport,

    /// Optional human-readable description of the server.
    #[serde(default)]
    pub description: Option<String>,

    /// Optional list of tool names allowed to be used by this server.
    /// This is used in Phase 4 for tool filtering.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,

    /// Optional list of tool names disabled for this server.
    /// This is used in Phase 4 for tool filtering.
    #[serde(default)]
    pub disabled_tools: Option<Vec<String>>,
}

/// Overall MCP configuration containing multiple server definitions.
///
/// This is the root config structure parsed from TOML files.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// List of MCP servers to configure.
    pub servers: Vec<ServerConfig>,
}

impl Config {
    /// Returns a HashMap mapping server names to their configurations.
    ///
    /// This enables O(1) lookups by server name.
    pub fn servers_by_name(&self) -> HashMap<String, &ServerConfig> {
        self.servers
            .iter()
            .map(|server| (server.name.clone(), server))
            .collect()
    }

    /// Retrieves a server configuration by name.
    ///
    /// Returns None if no server with the given name exists.
    pub fn get_server(&self, name: &str) -> Option<&ServerConfig> {
        self.servers_by_name().get(name)
    }

    /// Checks if the configuration has any servers defined.
    ///
    /// This is used to display CONFIG-05 warnings when no servers are configured.
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}
