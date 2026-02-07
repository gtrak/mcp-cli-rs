//! Configuration module for MCP server definitions.
//!
//! This module provides types and utilities for parsing MCP server configurations
//! from TOML files, supporting both stdio and HTTP transports.
//!
//! Requires the `std::sync::LazyLock` crate feature for static HashMaps.

use std::sync::LazyLock;
use serde::{Deserialize, Serialize};

use crate::McpError;
use std::collections::HashMap;

/// Transport protocol for MCP server connections.
///
/// Supports both local stdio execution and remote HTTP connections.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
#[derive(Debug, Clone, Deserialize, Serialize)]
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

impl ServerConfig {
    /// Create a transport for this server configuration.
    ///
    /// This implements TransportFactory trait to bridge config and transport layers.
    /// Implements Task 4 of Plan 01-04.
    pub fn create_transport(
        &self,
        _server_name: &str,
    ) -> std::result::Result<Box<dyn crate::transport::Transport + Send + Sync>, McpError> {
        match &self.transport {
            ServerTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                let transport =
                    crate::client::stdio::StdioTransport::new(command, args, env, cwd.as_deref())?;
                Ok(Box::new(transport))
            }
            ServerTransport::Http { url, headers } => Ok(Box::new(
                crate::client::http::HttpTransport::new(url, headers.clone()),
            )),
        }
    }
}

/// Overall MCP configuration containing multiple server definitions.
///
/// This is the root config structure parsed from TOML files.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
        self.servers_by_name().get(name).map(|v| &**v)
    }

    /// Checks if the configuration has any servers defined.
    ///
    /// This is used to display CONFIG-05 warnings when no servers are configured.
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}

impl ServerTransport {
    /// Get the transport type name.
    pub fn type_name(&self) -> &str {
        match self {
            ServerTransport::Stdio { .. } => "stdio",
            ServerTransport::Http { .. } => "http",
        }
    }

    /// Extract the command for stdio transport.
    pub fn command(&self) -> &str {
        match self {
            ServerTransport::Stdio { command, .. } => command,
            ServerTransport::Http { .. } => "",
        }
    }

    /// Extract the arguments for stdio transport.
    pub fn args(&self) -> &[String] {
        match self {
            ServerTransport::Stdio { args, .. } => args,
            ServerTransport::Http { .. } => &[],
        }
    }

    /// Extract environment variables for stdio transport.
    pub fn env(&self) -> &HashMap<String, String> {
        match self {
            ServerTransport::Stdio { env, .. } => env,
            ServerTransport::Http { .. } => {
                static EMPTY: LazyLock<HashMap<String, String>> = LazyLock::new(HashMap::new);
                &EMPTY
            }
        }
    }

    /// Extract working directory for stdio transport.
    pub fn cwd(&self) -> Option<&String> {
        match self {
            ServerTransport::Stdio { cwd, .. } => cwd.as_ref(),
            ServerTransport::Http { .. } => None,
        }
    }

    /// Extract the URL for HTTP transport.
    pub fn url(&self) -> &str {
        match self {
            ServerTransport::Stdio { .. } => "",
            ServerTransport::Http { url, .. } => url,
        }
    }

    /// Extract headers for HTTP transport.
    pub fn headers(&self) -> &HashMap<String, String> {
        match self {
            ServerTransport::Stdio { .. } => {
                static EMPTY: LazyLock<HashMap<String, String>> = LazyLock::new(HashMap::new);
                &EMPTY
            }
            ServerTransport::Http { headers, .. } => headers,
        }
    }
}

pub mod loader;
