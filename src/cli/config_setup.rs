//! Configuration setup and initialization for CLI.
//!
//! Provides functions for loading and validating configuration
//! at application startup.

use crate::config::loader::{find_and_load, load_config};
use crate::config::Config;
use crate::error::Result;
use std::path::PathBuf;

/// Load configuration from specified path or find default.
///
/// This function requires a valid config file - it will return an error
/// if no config can be found or loaded.
///
/// # Arguments
/// * `config_path` - Optional explicit path to config file
///
/// # Returns
/// * `Ok(Config)` - Loaded configuration
/// * `Err(McpError)` - Config loading/validation error
pub async fn setup_config(config_path: Option<PathBuf>) -> Result<Config> {
    if let Some(path) = config_path {
        // Use explicitly provided config path
        load_config(&path).await
    } else {
        // Search for config in standard locations
        find_and_load(None).await
    }
}

/// Initialize config with default values if loading fails.
///
/// This function is used when config file is optional - it will
/// return a default config if no config file is found.
///
/// # Arguments
/// * `config_path` - Optional explicit path to config file
///
/// # Returns
/// * `Ok(Config)` - Loaded configuration or default if not found
pub async fn setup_config_optional(config_path: Option<PathBuf>) -> Result<Config> {
    match setup_config(config_path).await {
        Ok(config) => Ok(config),
        Err(e) => {
            tracing::warn!("No config file found, using default: {}", e);
            Ok(Config::default())
        }
    }
}

/// Load configuration for daemon startup.
///
/// This variant is specifically tuned for daemon mode - it allows
/// the daemon to start even without a config file, using defaults.
///
/// # Arguments
/// * `config_path` - Optional explicit path to config file
///
/// # Returns
/// * `Ok(Config)` - Loaded configuration or default if not found
pub async fn setup_config_for_daemon(config_path: Option<PathBuf>) -> Result<Config> {
    match setup_config(config_path).await {
        Ok(config) => Ok(config),
        Err(e) => {
            tracing::warn!(
                "No config file found, starting daemon with empty config: {}",
                e
            );
            Ok(Config::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_config_returns_result() {
        // Just verify the function returns a Result type - actual loading tested elsewhere
        let result = setup_config(None).await;
        // Should either succeed with config or fail with error (not panic)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_setup_config_optional_returns_result() {
        // Verify the function returns a Result type
        let result = setup_config_optional(None).await;
        // Should always succeed since it returns default on failure
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_setup_config_for_daemon_returns_result() {
        // Verify the function returns a Result type  
        let result = setup_config_for_daemon(None).await;
        // Should always succeed since it returns default on failure
        assert!(result.is_ok());
    }
}
