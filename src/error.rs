//! Error types for MCP CLI operations.
//!
//! This module defines [`McpError`], a comprehensive error enum covering
//! configuration, connection, protocol, IPC, and CLI usage errors.
//! Each variant includes structured context for actionable error messages.
//!
//! The [`exit_code`] function maps errors to process exit codes (1=client,
//! 2=server, 3=network/IO) for scripting compatibility.
//!
//! # Usage
//!
//! ```rust
//! use mcp_cli_rs::error::{McpError, Result};
//!
//! fn find_server(name: &str) -> Result<String> {
//!     if name.is_empty() {
//!         return Err(McpError::usage_error("server name required"));
//!     }
//!     Ok(name.to_string())
//! }
//!
//! assert!(find_server("").is_err());
//! assert!(find_server("my-server").is_ok());
//! ```

use thiserror::Error;

/// Main error type for MCP CLI.
///
/// Covers all error categories: configuration, connection, protocol,
/// tool execution, daemon, IPC, and CLI usage errors. Uses `thiserror`
/// for `Display` and `Error` trait implementations.
#[derive(Error, Debug)]
pub enum McpError {
    // Configuration errors (CONFIG-01, CONFIG-04)
    #[error("Failed to read config file '{}': {}", path.display(), source)]
    ConfigReadError {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid TOML in config file '{}': {}", path.display(), source)]
    ConfigParseError {
        path: std::path::PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error(
        "Missing required field '{}' in server configuration for '{}'",
        field,
        server
    )]
    MissingRequiredField { server: String, field: &'static str },

    // Connection errors (CONN-01, CONN-02, CONN-03)
    #[error("Failed to connect to server '{}': {}", server, source)]
    ConnectionError {
        server: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Server '{}' not found. Available servers: {}", server, servers.join(", "))]
    ServerNotFound {
        server: String,
        servers: Vec<String>,
    },

    // Tool errors (EXEC-04, DISC-03)
    #[error("Tool '{}' not found in server '{}'", tool, server)]
    ToolNotFound { tool: String, server: String },

    // Daemon errors (DAEMON-04)
    #[error("Daemon not running: {}", message)]
    DaemonNotRunning { message: String },

    #[error(
        "Invalid JSON arguments: {}. Expected format: {{'\"key\"': \"value\"}}",
        source
    )]
    InvalidJson {
        #[from]
        source: serde_json::Error,
    },

    // Protocol errors (XP-03)
    #[error("Invalid MCP protocol message: {}", message)]
    InvalidProtocol { message: String },

    #[error("Timeout waiting for server response ({}s timeout)", timeout)]
    Timeout { timeout: u64 },

    // CLI errors (ERR-06)
    #[error("Ambiguous command: {}", hint)]
    AmbiguousCommand { hint: String },

    // CLI usage errors (ERR-05, ERR-06)
    #[error("Usage error: {}", message)]
    UsageError { message: String },

    // IO errors
    #[error("IO error: {}", source)]
    IOError {
        #[source]
        source: std::io::Error,
    },

    // IPC errors (IPC-01, IPC-02, IPC-03)
    #[error("IPC error: {}", message)]
    IpcError { message: String },

    #[cfg(unix)]
    #[error("Socket bind failed at '{}': {}", path, source)]
    SocketBindError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[cfg(unix)]
    #[error("Connection refused to socket '{}'", path)]
    ConnectionRefused { path: String },

    #[cfg(unix)]
    #[error("Stale socket file found at '{}'", path)]
    StaleSocket { path: String },

    #[cfg(windows)]
    #[error("Pipe creation failed at '{}': {}", name, source)]
    PipeCreationError {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[cfg(windows)]
    #[error("Named pipe instance busy at '{}'", path)]
    PipeBusy { path: String },

    // Retry and timeout errors (EXEC-05, EXEC-06, EXEC-07)
    #[error("Operation cancelled (timeout: {}s)", timeout)]
    OperationCancelled { timeout: u64 },

    #[error("Max retry attempts ({}) exceeded", attempts)]
    MaxRetriesExceeded { attempts: u32 },
}

/// Exit codes for CLI process (ERR-03).
///
/// Maps error types to numeric exit codes:
/// - `1` — Client/usage errors (bad input, missing resources)
/// - `2` — Server/protocol errors
/// - `3` — Network or IO errors
#[cfg(unix)]
pub fn exit_code(error: &McpError) -> i32 {
    match error {
        McpError::ServerNotFound { .. }
        | McpError::ToolNotFound { .. }
        | McpError::ConfigReadError { .. }
        | McpError::ConfigParseError { .. }
        | McpError::MissingRequiredField { .. }
        | McpError::InvalidJson { .. }
        | McpError::AmbiguousCommand { .. }
        | McpError::UsageError { .. }
        | McpError::OperationCancelled { .. }
        | McpError::MaxRetriesExceeded { .. } => 1, // Client error

        McpError::InvalidProtocol { .. } => 2, // Server error

        McpError::ConnectionError { .. } | McpError::Timeout { .. } | McpError::IOError { .. } => 3, // Network or IO error

        // IPC errors also return client error code
        McpError::IpcError { .. }
        | McpError::SocketBindError { .. }
        | McpError::ConnectionRefused { .. }
        | McpError::StaleSocket { .. } => 1,
    }
}

/// Exit codes for CLI process (ERR-03, Windows variant).
///
/// Same mapping as Unix but includes Windows-specific IPC errors.
#[cfg(windows)]
pub fn exit_code(error: &McpError) -> i32 {
    match error {
        McpError::ServerNotFound { .. }
        | McpError::ToolNotFound { .. }
        | McpError::ConfigReadError { .. }
        | McpError::ConfigParseError { .. }
        | McpError::MissingRequiredField { .. }
        | McpError::InvalidJson { .. }
        | McpError::AmbiguousCommand { .. }
        | McpError::UsageError { .. }
        | McpError::OperationCancelled { .. }
        | McpError::MaxRetriesExceeded { .. } => 1, // Client error

        McpError::InvalidProtocol { .. } => 2, // Server error

        McpError::ConnectionError { .. } | McpError::Timeout { .. } | McpError::IOError { .. } => 3, // Network or IO error

        // IPC errors also return client error code
        McpError::IpcError { .. }
        | McpError::PipeCreationError { .. }
        | McpError::PipeBusy { .. }
        | McpError::DaemonNotRunning { .. } => 1,
    }
}

/// Context helper for missing fields (CONFIG-04).
impl McpError {
    /// Create a [`MissingRequiredField`](McpError::MissingRequiredField) error.
    pub fn missing_field(server: &str, field: &'static str) -> Self {
        Self::MissingRequiredField {
            server: server.to_string(),
            field,
        }
    }

    /// Create a [`ConfigReadError`](McpError::ConfigReadError) from a path and IO error.
    pub fn config_read(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::ConfigReadError {
            path: path.to_path_buf(),
            source,
        }
    }

    /// Error when daemon is not running (DAEMON-04)
    pub fn daemon_not_running(message: impl Into<String>) -> Self {
        Self::DaemonNotRunning {
            message: message.into(),
        }
    }

    /// Create a [`ServerNotFound`](McpError::ServerNotFound) error with available servers.
    pub fn server_not_found(server: &str, servers: Vec<String>) -> Self {
        Self::ServerNotFound {
            server: server.to_string(),
            servers,
        }
    }

    /// Create a [`ConnectionError`](McpError::ConnectionError) for a server.
    pub fn connection_error(server: &str, source: std::io::Error) -> Self {
        Self::ConnectionError {
            server: server.to_string(),
            source,
        }
    }

    /// Create a [`ToolNotFound`](McpError::ToolNotFound) error.
    pub fn tool_not_found(tool: &str, server: &str) -> Self {
        Self::ToolNotFound {
            tool: tool.to_string(),
            server: server.to_string(),
        }
    }

    /// Create an [`IOError`](McpError::IOError) from a `std::io::Error`.
    pub fn io_error(source: std::io::Error) -> Self {
        Self::IOError { source }
    }

    /// Create a [`UsageError`](McpError::UsageError) for CLI misuse.
    pub fn usage_error(message: impl Into<String>) -> Self {
        Self::UsageError {
            message: message.into(),
        }
    }

    /// Create an [`IpcError`](McpError::IpcError).
    pub fn ipc_error(message: impl Into<String>) -> Self {
        Self::IpcError {
            message: message.into(),
        }
    }

    /// Create a [`SocketBindError`](McpError::SocketBindError) (Unix only).
    #[cfg(unix)]
    pub fn socket_bind_error(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::SocketBindError {
            path: path.into(),
            source,
        }
    }

    /// Create a [`ConnectionRefused`](McpError::ConnectionRefused) error (Unix only).
    #[cfg(unix)]
    pub fn connection_refused(path: impl Into<String>) -> Self {
        Self::ConnectionRefused { path: path.into() }
    }

    /// Create a [`StaleSocket`](McpError::StaleSocket) error (Unix only).
    #[cfg(unix)]
    pub fn stale_socket(path: impl Into<String>) -> Self {
        Self::StaleSocket { path: path.into() }
    }

    /// Create a [`PipeCreationError`](McpError::PipeCreationError) (Windows only).
    #[cfg(windows)]
    pub fn pipe_creation_error(name: impl Into<String>, source: std::io::Error) -> Self {
        Self::PipeCreationError {
            name: name.into(),
            source,
        }
    }

    /// Create a [`PipeBusy`](McpError::PipeBusy) error (Windows only).
    #[cfg(windows)]
    pub fn pipe_busy(path: impl Into<String>) -> Self {
        Self::PipeBusy { path: path.into() }
    }

    /// Create an [`OperationCancelled`](McpError::OperationCancelled) error.
    pub fn operation_cancelled(timeout: u64) -> Self {
        Self::OperationCancelled { timeout }
    }

    /// Create a [`MaxRetriesExceeded`](McpError::MaxRetriesExceeded) error.
    pub fn max_retries_exceeded(attempts: u32) -> Self {
        Self::MaxRetriesExceeded { attempts }
    }
}

// From implementations for error conversion
impl From<std::io::Error> for McpError {
    fn from(error: std::io::Error) -> Self {
        Self::IOError { source: error }
    }
}

/// Result type alias using [`McpError`] as the error type.
pub type Result<T> = std::result::Result<T, McpError>;
