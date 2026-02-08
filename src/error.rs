use thiserror::Error;

/// Main error type for MCP CLI
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

    #[error("Server '{}' not found", server)]
    ServerNotFound { server: String },

    // Tool errors (EXEC-04, DISC-03)
    #[error("Tool '{}' not found in server '{}'", tool, server)]
    ToolNotFound { tool: String, server: String },

    #[error("Invalid JSON arguments: {}", source)]
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

/// Exit codes (ERR-03)
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
        | McpError::PipeBusy { .. } => 1,
    }
}

/// Context helper for missing fields (CONFIG-04)
impl McpError {
    pub fn missing_field(server: &str, field: &'static str) -> Self {
        Self::MissingRequiredField {
            server: server.to_string(),
            field,
        }
    }

    pub fn config_read(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::ConfigReadError {
            path: path.to_path_buf(),
            source,
        }
    }

    pub fn connection_error(server: &str, source: std::io::Error) -> Self {
        Self::ConnectionError {
            server: server.to_string(),
            source,
        }
    }

    pub fn tool_not_found(tool: &str, server: &str) -> Self {
        Self::ToolNotFound {
            tool: tool.to_string(),
            server: server.to_string(),
        }
    }

    pub fn io_error(source: std::io::Error) -> Self {
        Self::IOError { source }
    }

    pub fn usage_error(message: impl Into<String>) -> Self {
        Self::UsageError {
            message: message.into(),
        }
    }

    pub fn ipc_error(message: impl Into<String>) -> Self {
        Self::IpcError {
            message: message.into(),
        }
    }

    #[cfg(unix)]
    pub fn socket_bind_error(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::SocketBindError {
            path: path.into(),
            source,
        }
    }

    #[cfg(unix)]
    pub fn connection_refused(path: impl Into<String>) -> Self {
        Self::ConnectionRefused { path: path.into() }
    }

    #[cfg(unix)]
    pub fn stale_socket(path: impl Into<String>) -> Self {
        Self::StaleSocket { path: path.into() }
    }

    #[cfg(windows)]
    pub fn pipe_creation_error(name: impl Into<String>, source: std::io::Error) -> Self {
        Self::PipeCreationError {
            name: name.into(),
            source,
        }
    }

    #[cfg(windows)]
    pub fn pipe_busy(path: impl Into<String>) -> Self {
        Self::PipeBusy { path: path.into() }
    }

    pub fn operation_cancelled(timeout: u64) -> Self {
        Self::OperationCancelled { timeout }
    }

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

// Result type alias (anyhow style)
pub type Result<T> = std::result::Result<T, McpError>;
