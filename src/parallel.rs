//! Parallel execution utilities for MCP operations.
//!
//! Provides concurrent processing of multiple servers with configurable
//! concurrency limits. Implements DISC-05: parallel server discovery.

use futures_util::stream::{self, StreamExt};
use tokio::sync::Semaphore;
use std::sync::Arc;
use crate::client::ToolInfo;
use crate::error::Result;

/// Parallel executor for MCP operations with concurrency control.
///
/// Limits concurrent operations to prevent resource exhaustion while
/// maximizing throughput through parallel processing.
pub struct ParallelExecutor {
    /// Maximum concurrent operations.
    concurrency_limit: usize,
}

impl ParallelExecutor {
    /// Create a new ParallelExecutor with the specified concurrency limit.
    ///
    /// # Arguments
    /// * `concurrency_limit` - Maximum number of concurrent operations
    ///
    /// # Default
    /// Uses DISC-05 default: 5 concurrent operations
    pub fn new(concurrency_limit: usize) -> Self {
        Self {
            concurrency_limit,
        }
    }

    /// Get the concurrency limit.
    pub fn concurrency_limit(&self) -> usize {
        self.concurrency_limit
    }
}

impl Default for ParallelExecutor {
    fn default() -> Self {
        Self::new(5) // DISC-05 default: 5 concurrent operations
    }
}

/// List tools from multiple servers in parallel.
///
/// Processes servers concurrently up to the concurrency limit, tracking
/// successes and failures separately. Implements DISC-05 and prepares ERR-07.
///
/// # Arguments
/// * `server_names` - List of server names to process
/// * `list_fn` - Async function to list tools for a server
/// * `executor` - ParallelExecutor to control concurrency
///
/// # Returns
/// Tuple of (successes, failures) where:
/// - successes: Vec<(String, Vec<ToolInfo>)> of server name and tools
/// - failures: Vec<String> of server names that failed
///
/// # Example
/// ```rust,ignore
/// let (successes, failures) = list_tools_parallel(
///     server_names,
///     |server| async move { daemon.list_tools(&server).await },
///     executor,
/// ).await?;
/// ```
pub async fn list_tools_parallel<F, Fut>(
    server_names: Vec<String>,
    list_fn: F,
    executor: &ParallelExecutor,
) -> Result<(Vec<(String, Vec<ToolInfo>)>, Vec<String>)>
where
    F: Fn(String) -> Fut + Send + Sync + Clone,
    Fut: std::future::Future<Output = Result<Vec<ToolInfo>>> + Send,
{
    // Create semaphore to limit concurrent operations (research: buffer_unordered pattern)
    let semaphore = Arc::new(Semaphore::new(executor.concurrency_limit()));

    // Process each server with concurrency control using stream combinators
    let results: Vec<_> = stream::iter(server_names)
        .map(move |server_name: String| {
            let semaphore = semaphore.clone();
            let list_fn = list_fn.clone();

            async move {
                // Acquire permit before starting operation (prevents resource exhaustion)
                let _permit = semaphore.acquire().await.unwrap();

                // Execute the list function for this server
                match list_fn(server_name.clone()).await {
                    Ok(tools) => Ok((server_name, tools)),
                    Err(_) => Err(server_name.to_string()), // Return failed server name as string
                }
            }
        })
        .buffer_unordered(executor.concurrency_limit()) // Enforce concurrency limit
        .collect()
        .await;

    // Separate successes and failures for error reporting (ERR-07 preparation)
    let mut successes = Vec::new();
    let mut failures = Vec::new();

    for result in results {
        match result {
            Ok(success) => successes.push(success),
            Err(server_name) => failures.push(server_name), // server_name is now String
        }
    }

    Ok((successes, failures))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_executor_default() {
        let executor = ParallelExecutor::default();
        assert_eq!(executor.concurrency_limit(), 5);
    }

    #[test]
    fn test_parallel_executor_custom_limit() {
        let executor = ParallelExecutor::new(10);
        assert_eq!(executor.concurrency_limit(), 10);
    }
}
