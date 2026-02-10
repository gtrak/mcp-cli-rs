//! Configuration file loading utilities for MCP servers.

use crate::config::{Config, ServerConfig, ServerTransport};
use crate::error::McpError;
use std::path::Path;
use tokio::fs;
use tracing::debug;

/// Finds the MCP configuration file following the CONFIG-02 priority order.
///
/// Search order (from highest to lowest priority):
/// 1. MCP_CONFIG_PATH environment variable
/// 2. CLI -c/--config argument (if provided)
/// 3. ./mcp_servers.toml (current directory)
/// 4. ~/.mcp_servers.toml (home directory)
/// 5. ~/.config/mcp/mcp_servers.toml (config directory)
///
/// Returns the first existing config file path or None if not found.
///
/// # Arguments
/// * `cli_path` - Optional CLI -c/--config argument value
///
/// # Example
/// ```ignore
/// let config_path = find_config_path(Some("/custom/path/mcp_servers.toml"));
/// ```
pub async fn find_config_path(cli_path: Option<&str>) -> Option<String> {
    // Priority 1: MCP_CONFIG_PATH environment variable
    if let Ok(env_path) = std::env::var("MCP_CONFIG_PATH") {
        debug!("MCP_CONFIG_PATH found: {}", env_path);
        if Path::new(&env_path).exists() {
            return Some(env_path);
        }
        debug!("MCP_CONFIG_PATH path does not exist: {}", env_path);
    }

    // Priority 2: CLI -c/--config argument
    if let Some(path) = cli_path {
        debug!("CLI config argument found: {}", path);
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
        debug!("CLI config path does not exist: {}", path);
    }

    // Priority 3: ./mcp_servers.toml (current directory)
    let cwd_path = "mcp_servers.toml";
    debug!("Checking current directory config: {}", cwd_path);
    if Path::new(cwd_path).exists() {
        return Some(cwd_path.to_string());
    }

    // Priority 4: ~/.mcp_servers.toml (home directory)
    if let Some(home_dir) = dirs::home_dir() {
        let home_path = home_dir.join("mcp_servers.toml");
        let home_path_str = home_path.to_string_lossy().to_string();
        debug!("Checking home directory config: {}", home_path_str);
        if Path::new(&home_path).exists() {
            return Some(home_path_str);
        }
    }

    // Priority 5: ~/.config/mcp/mcp_servers.toml (config directory)
    if let Some(config_dir) = dirs::config_dir() {
        let config_path = config_dir.join("mcp").join("mcp_servers.toml");
        let config_path_str = config_path.to_string_lossy().to_string();
        debug!("Checking config directory config: {}", config_path_str);
        if Path::new(&config_path).exists() {
            return Some(config_path_str);
        }
    }

    debug!("No config file found in standard locations");
    None
}

/// Validates a single server configuration.
///
/// Checks that required fields are present and valid according to CONFIG-04:
/// - Stdio transport: command field must not be empty
/// - HTTP transport: url field must not be empty and must be a valid URL
///
/// # Arguments
/// * `server` - Server configuration to validate
/// * `config_path` - Path to config file (for error reporting)
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err(McpError)` with detailed message if validation fails
pub fn validate_server_config(server: &ServerConfig, config_path: &str) -> Result<(), McpError> {
    match &server.transport {
        ServerTransport::Stdio { command, .. } => {
            if command.is_empty() {
                return Err(McpError::MissingRequiredField {
                    server: server.name.clone(),
                    field: "command",
                });
            }
            debug!("Server '{}' stdio config validated (command: {})", server.name, command);
        }
        ServerTransport::Http { url, .. } => {
            if url.is_empty() {
                return Err(McpError::MissingRequiredField {
                    server: server.name.clone(),
                    field: "url",
                });
            }

            // Validate URL format
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(McpError::ConfigParseError {
                    path: Path::new(config_path).to_path_buf(),
                    source: Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Server '{}' has invalid URL '{}': must start with http:// or https://",
                            server.name, url
                        ),
                    )),
                });
            }

            debug!("Server '{}' HTTP config validated (url: {})", server.name, url);
        }
    }

    // Phase 4: Validate tool filtering configuration
    // Ensure at least one of allowed_tools or disabled_tools is provided
    let has_allowed = server.allowed_tools.as_ref().map_or(false, |tools| !tools.is_empty());
    let has_disabled = server.disabled_tools.as_ref().map_or(false, |tools| !tools.is_empty());

    if !has_allowed && !has_disabled {
        debug!(
            "Server '{}' has no tool filtering specified (both allowedTools and disabledTools empty)",
            server.name
        );
        // This is not an error - allows backward compatibility with no filtering
    }

    Ok(())
}

/// Validates all server configurations in the config.
///
/// # Arguments
/// * `config` - Configuration to validate
/// * `config_path` - Path to config file (for error reporting)
///
/// # Returns
/// * `Ok(())` if all validations pass
/// * `Err(McpError)` if any validation fails
pub fn validate_config(config: &Config, config_path: &str) -> Result<(), McpError> {
    for server in &config.servers {
        if let Err(e) = validate_server_config(server, config_path) {
            return Err(e);
        }
    }
    debug!("All server configurations in {} validated successfully", config_path);
    Ok(())
}

/// Loads and parses the MCP configuration from a file.
///
/// This is an async function using tokio::fs for non-blocking file operations (XP-03).
///
/// # Arguments
/// * `path` - Path to the config file
///
/// # Returns
/// * `Ok(Config)` if parsing succeeds
/// * `Err(McpError)` if file read or parse fails
///
/// # Behavior
/// - Reads the entire file content asynchronously
/// - Parses TOML using the toml crate (v0.8)
/// - Validates all server configurations
/// - Displays warning if no servers configured (CONFIG-05)
pub async fn load_config(path: &std::path::Path) -> Result<Config, McpError> {
    // Read file content asynchronously (XP-03: use tokio::fs)
    let content = fs::read_to_string(path).await.map_err(|e| {
        debug!("Failed to read config file {}: {}", path.display(), e);
        McpError::config_read(path, e)
    })?;

    debug!("Config file loaded successfully: {}", path.display());

    // Parse TOML
    let config: Config = toml::from_str(&content).map_err(|e| {
        debug!("Failed to parse TOML from {}: {}", path.display(), e);
        McpError::ConfigParseError {
            path: path.to_path_buf(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        }
    })?;

    // Validate all server configurations
    validate_config(&config, &path.to_string_lossy())?;

    // CONFIG-05: Display warning if no servers configured
    if config.is_empty() {
        tracing::warn!("Config file '{}' contains no server definitions", path.display());
    } else {
        debug!(
            "Config file '{}' parsed successfully with {} server(s)",
            path.display(),
            config.servers.len()
        );
    }

    Ok(config)
}

/// Synchronous version of load_config for use in tests and sync contexts.
///
/// # Arguments
/// * `path` - Path to the configuration file
///
/// # Returns
/// * `Ok(Config)` if config parsed successfully
/// * `Err(McpError)` if parsing fails
pub fn load_config_sync(path: &std::path::Path) -> Result<Config, McpError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        debug!("Failed to read config file {}: {}", path.display(), e);
        McpError::config_read(path, e)
    })?;

    debug!("Config file loaded successfully: {}", path.display());

    let config: Config = toml::from_str(&content).map_err(|e| {
        debug!("Failed to parse TOML from {}: {}", path.display(), e);
        McpError::ConfigParseError {
            path: path.to_path_buf(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        }
    })?;

    validate_config(&config, &path.to_string_lossy())?;

    if config.is_empty() {
        tracing::warn!("Config file '{}' contains no server definitions", path.display());
    } else {
        debug!(
            "Config file '{}' parsed successfully with {} server(s)",
            path.display(),
            config.servers.len()
        );
    }

    Ok(config)
}

/// Combines config discovery and loading into a single operation.
///
/// This is the main entry point for loading configuration from all possible sources.
///
/// # Arguments
/// * `cli_path` - Optional CLI -c/--config argument value
///
/// # Returns
/// * `Ok(Config)` if config found and parsed successfully
/// * `Err(McpError)` if config not found or parsing fails
///
/// # Behavior
/// 1. Searches for config file using priority order (CONFIG-02)
/// 2. If found, loads and parses TOML
/// 3. Validates all server configurations
/// 4. Returns config or error with helpful message
pub async fn find_and_load(cli_path: Option<&str>) -> Result<Config, McpError> {
    // Find the config file
    let config_path = find_config_path(cli_path).await;

    if config_path.is_none() {
        // CONFIG-04: Clear error message for missing config
        return Err(McpError::ConfigReadError {
            path: Path::new("mcp_servers.toml").to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "MCP configuration file not found. Configuration search order:\n\
                 1. MCP_CONFIG_PATH environment variable\n\
                 2. CLI -c/--config argument\n\
                 3. ./mcp_servers.toml (current directory)\n\
                 4. ~/.mcp_servers.toml (home directory)\n\
                 5. ~/.config/mcp/mcp_servers.toml (config directory)",
            ),
        });
    }

    let config_path_str = config_path.unwrap();
    let config_path = Path::new(&config_path_str);

    // Load and parse the config
    load_config(config_path).await
}
