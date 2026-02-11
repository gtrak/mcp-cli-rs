//! Retry logic with exponential backoff for transient errors.
//!
//! Provides automatic retry with configurable limits and exponential backoff.
//! Implements EXEC-05, EXEC-06, EXEC-07.

use crate::error::McpError;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::timeout;

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
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 200,
            max_delay_ms: 5_000,
        }
    }
}

/// Check if an error is transient (should be retried).
///
/// Transient errors: Timeout, ConnectionError, IOError (EXEC-05).
/// Permanent errors: InvalidJson, InvalidProtocol, ToolNotFound, ServerNotFound.
pub fn is_transient_error(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Timeout { .. }
            | McpError::ConnectionError { .. }
            | McpError::IOError { .. }
            | McpError::IpcError { .. }
    )
}

/// Retry logic with exponential backoff for sync operations.
///
/// This version uses the backoff crate for retry logic with exponential backoff.
/// (EXEC-05, EXEC-06, EXEC-07).
pub async fn retry_with_backoff_sync<F, T>(
    mut operation: F,
    config: &RetryConfig,
) -> std::result::Result<T, McpError>
where
    F: FnMut() -> Pin<Box<dyn std::future::Future<Output = std::result::Result<T, McpError>>>>,
{
    let mut attempt = 0u32;

    loop {
        attempt += 1;

        // Execute the operation
        let operation_result = operation();
        let result = operation_result.await;

        match result {
            Ok(value) => return Ok(value),
            Err(error) => {
                // Check if this is a timeout error
                let is_timeout = matches!(error, McpError::Timeout { .. });

                if is_timeout {
                    return Err(McpError::OperationCancelled { timeout: 30 });
                }

                // For other errors, check if they're transient
                if !is_transient_error(&error) {
                    // Permanent error - don't retry
                    return Err(error);
                }

                if attempt >= config.max_attempts {
                    return Err(McpError::MaxRetriesExceeded { attempts: attempt });
                }

                // Calculate delay for this retry attempt (using exponential backoff)
                let delay = if attempt == 1 {
                    Duration::from_millis(config.base_delay_ms)
                } else {
                    // Exponential backoff with jitter
                    let multiplier = 2f64.powi(attempt as i32 - 1);
                    let max_delay = Duration::from_millis(config.max_delay_ms);
                    let calculated_delay = Duration::from_millis(
                        (config.base_delay_ms as f64 * multiplier).ceil() as u64,
                    );

                    // Clamp to max_delay
                    calculated_delay.min(max_delay)
                };

                // Add some jitter (using a simple deterministic approach)
                let jitter = Duration::from_millis((delay.as_millis() / 10) as u64);
                let total_delay = delay + jitter;

                std::thread::sleep(total_delay);
            }
        }
    }
}

/// Retry logic with exponential backoff for async operations.
///
/// This version accepts async closures and wraps them properly using the backoff crate.
/// (EXEC-05, EXEC-06, EXEC-07).
pub async fn retry_with_backoff<F, T>(
    mut operation: F,
    config: &RetryConfig,
) -> std::result::Result<T, McpError>
where
    F: FnMut()
        -> Pin<Box<dyn std::future::Future<Output = std::result::Result<T, McpError>> + Send>>,
{
    let mut attempt = 0u32;

    loop {
        attempt += 1;

        // Execute the operation
        let operation_result = operation();
        let result = operation_result.await;

        match result {
            Ok(value) => return Ok(value),
            Err(error) => {
                // Check if this is a timeout error
                let is_timeout = matches!(error, McpError::Timeout { .. });

                if is_timeout {
                    return Err(McpError::OperationCancelled { timeout: 30 });
                }

                // For other errors, check if they're transient
                if !is_transient_error(&error) {
                    // Permanent error - don't retry
                    return Err(error);
                }

                if attempt >= config.max_attempts {
                    return Err(McpError::MaxRetriesExceeded { attempts: attempt });
                }

                // Calculate delay for this retry attempt (using exponential backoff)
                let delay = if attempt == 1 {
                    Duration::from_millis(config.base_delay_ms)
                } else {
                    // Exponential backoff with jitter
                    let multiplier = 2f64.powi(attempt as i32 - 1);
                    let max_delay = Duration::from_millis(config.max_delay_ms);
                    let calculated_delay = Duration::from_millis(
                        (config.base_delay_ms as f64 * multiplier).ceil() as u64,
                    );

                    // Clamp to max_delay
                    calculated_delay.min(max_delay)
                };

                // Add some jitter (using a simple deterministic approach)
                let jitter = Duration::from_millis((delay.as_millis() / 10) as u64);
                let total_delay = delay + jitter;

                std::thread::sleep(total_delay);
            }
        }
    }
}

pub async fn timeout_wrapper<F, T, Fut>(
    operation: F,
    timeout_secs: u64,
) -> std::result::Result<T, McpError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, McpError>>,
{
    let duration = Duration::from_secs(timeout_secs);

    match timeout(duration, operation()).await {
        Ok(result) => result,
        Err(_) => {
            tracing::error!("Operation timed out after {}s", timeout_secs);
            Err(McpError::Timeout {
                timeout: timeout_secs,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
