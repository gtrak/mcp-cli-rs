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
}

/// Exit codes (ERR-03)
pub fn exit_code(error: &McpError) -> i32 {
    match error {
        McpError::ServerNotFound { .. }
        | McpError::ToolNotFound { .. }
        | McpError::ConfigReadError { .. }
        | McpError::ConfigParseError { .. }
        | McpError::MissingRequiredField { .. }
        | McpError::InvalidJson { .. }
        | McpError::AmbiguousCommand { .. } => 1, // Client error

        McpError::InvalidProtocol { .. } => 2, // Server error

        McpError::ConnectionError { .. } | McpError::Timeout { .. } => 3, // Network error
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
}

// Result type alias (anyhow style)
pub type Result<T> = std::result::Result<T, McpError>;
