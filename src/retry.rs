//! Retry logic with exponential backoff for transient errors.
//!
//! Provides automatic retry with configurable limits and exponential backoff.
//! Implements EXEC-05, EXEC-06, EXEC-07.

use backoff::{ExponentialBackoff, ExponentialBackoffBuilder, future::retry};
use backoff::Error as BackoffError;
use std::time::Duration;
use tokio::time::timeout;
use crate::error::McpError;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (EXEC-07).
    pub max_attempts: u32,

    /// Base delay in milliseconds for exponential backoff (EXEC-07).
    pub base_delay_ms: u64,

    /// Maximum delay in milliseconds between retries.
    pub max_delay_ms: u64,
}

impl RetryConfig {
    /// Create RetryConfig from Config struct.
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            max_attempts: config.retry_max,
            base_delay_ms: config.retry_delay_ms,
            max_delay_ms: 30_000, // 30 seconds cap (research recommendation)
        }
    }

    /// Get the backoff configuration.
    fn backoff(&self) -> ExponentialBackoff {
        ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(self.base_delay_ms))
            .with_max_interval(Duration::from_millis(self.max_delay_ms))
            .with_multiplier(2.0)
            .with_randomization_factor(0.5)
            .build()
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
        }
    }
}

/// Check if an error is transient (should be retried).
///
/// Transient errors: Timeout, ConnectionError, IOError (EXEC-05).
/// Permanent errors: InvalidJson, InvalidProtocol, ToolNotFound, ServerNotFound.
fn is_transient_error(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Timeout { .. }
            | McpError::ConnectionError { .. }
            | McpError::IOError { .. }
            | McpError::IpcError { .. }
    )
}

pub async fn retry_with_backoff<F, T, Fut>(operation: F, config: &RetryConfig) -> std::result::Result<T, McpError>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = std::result::Result<T, McpError>> + Send,
{
    let backoff = config.backoff();

    retry(backoff, || async {
        match operation().await {
            Ok(value) => Ok(value),
            Err(error) if is_transient_error(&error) => {
                // Mark transient error - using From trait for automatic conversion
                Err(BackoffError::from(error))
            }
            Err(error) => {
                // Permanent error - don't retry
                Err(BackoffError::permanent(error))
            }
        }
    })
    .await
    .map_err(|e| {
        // The backoff errors have already been converted to McpError by this point
        // Just return the McpError directly (should have been converted)
        e
    })
}

pub async fn timeout_wrapper<F, T, Fut>(operation: F, timeout_secs: u64) -> std::result::Result<T, McpError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, McpError>>,
{
    let duration = Duration::from_secs(timeout_secs);

    match timeout(duration, operation()).await {
        Ok(result) => result,
        Err(_) => {
            tracing::error!("Operation timed out after {}s", timeout_secs);
            Err(McpError::operation_cancelled(timeout_secs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30_000);
    }

    #[test]
    fn test_is_transient_error() {
        assert!(is_transient_error(&McpError::Timeout { timeout: 1 }));
        assert!(is_transient_error(&McpError::IOError {
            source: std::io::Error::new(std::io::ErrorKind::TimedOut, "test"),
        }));
        // Permanent error - should not be transient
        assert!(!is_transient_error(&McpError::InvalidJson {
            source: serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid json"
            )),
        }));
    }

    #[test]
    fn test_is_transient_error_connection() {
        assert!(is_transient_error(&McpError::ConnectionError {
            server: "localhost:8080".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "test"),
        }));
    }

    #[test]
    fn test_is_transient_error_ipc() {
        assert!(is_transient_error(&McpError::IpcError {
            message: "IPC error".to_string(),
        }));
    }
}
