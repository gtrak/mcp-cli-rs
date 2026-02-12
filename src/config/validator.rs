//! Configuration validation logic for MCP servers.
//!
//! This module provides validation functions to ensure server configurations
//! are valid before being used.

use std::path::Path;

use crate::config::{Config, ServerConfig, ServerTransport};
use crate::error::McpError;
use tracing::debug;

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
            debug!(
                "Server '{}' stdio config validated (command: {})",
                server.name, command
            );
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

            debug!(
                "Server '{}' HTTP config validated (url: {})",
                server.name, url
            );
        }
    }

    // Phase 4: Validate tool filtering configuration
    // Ensure at least one of allowed_tools or disabled_tools is provided
    let has_allowed = server
        .allowed_tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty());
    let has_disabled = server
        .disabled_tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty());

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
        validate_server_config(server, config_path)?
    }
    debug!(
        "All server configurations in {} validated successfully",
        config_path
    );
    Ok(())
}
