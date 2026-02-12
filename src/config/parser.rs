//! TOML configuration parsing for MCP servers.
//!
//! This module provides functions for parsing TOML configuration files
//! into Config structures.

use std::path::Path;

use crate::config::Config;
use crate::error::McpError;
use tracing::debug;

/// Parses a TOML string into a Config structure.
///
/// # Arguments
/// * `content` - TOML string content to parse
/// * `path` - Path to the config file (for error reporting)
///
/// # Returns
/// * `Ok(Config)` if parsing succeeds
/// * `Err(McpError)` if parsing fails
pub fn parse_toml(content: &str, path: &Path) -> Result<Config, McpError> {
    let config: Config = toml::from_str(content).map_err(|e| {
        debug!("Failed to parse TOML from {}: {}", path.display(), e);
        McpError::ConfigParseError {
            path: path.to_path_buf(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        }
    })?;

    debug!("TOML parsed successfully from {}", path.display());
    Ok(config)
}
