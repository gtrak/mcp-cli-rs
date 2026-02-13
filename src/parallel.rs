//! Parallel execution utilities for MCP operations.
//!
//! Provides concurrent processing of multiple servers with configurable
//! concurrency limits. Implements DISC-05: parallel server discovery.

use crate::cli::filter::tools_match_any;
use crate::client::ToolInfo;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use futures_util::stream::{self, StreamExt};
use std::sync::Arc;
use tokio::sync::Semaphore;

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
        Self { concurrency_limit }
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

/// Filter tools based on configuration rules.
///
/// Applies filtering precedence: disabled_tools takes precedence over allowed_tools.
/// No filtering applied when both fields are empty or None.
///
/// # Arguments
/// * `tools` - List of tools to filter
/// * `server_config` - Server configuration to apply filtering rules
///
/// # Returns
/// Filtered list of tools that pass the filtering rules
///
/// # Filtering Logic
/// 1. If disabled_tools is non-empty: filter out tools matching disabled patterns
/// 2. Precedence: disabled > allowed (if a tool is disabled, it's blocked even if allowed)
/// 3. If allowed_tools is non-empty: return only tools matching allowed patterns
/// 4. If both fields empty: return all tools unchanged (backward compatible)
///
/// # Example
/// ```rust,ignore
/// let config = ServerConfig { disabled_tools: Some(vec!["password_*".into()]) };
/// let tools = list_tools_parallel(..., &config).await?;
/// ```
pub fn filter_tools(tools: Vec<ToolInfo>, server_config: &ServerConfig) -> Vec<ToolInfo> {
    let disabled_patterns = server_config.disabled_tools.as_deref().unwrap_or_default();
    let allowed_patterns = server_config.allowed_tools.as_deref().unwrap_or_default();

    // If no filtering rules, return all tools (backward compatible)
    if disabled_patterns.is_empty() && allowed_patterns.is_empty() {
        return tools;
    }

    tools
        .iter()
        .filter(|tool| {
            // Precedence: disabled > allowed
            if !disabled_patterns.is_empty() {
                // Check if tool matches disabled patterns first
                match tools_match_any(&tool.name, disabled_patterns) {
                    Some(_) => false, // Disabled (precedes allowed)
                    None => {
                        // Not disabled, check allowed patterns
                        if !allowed_patterns.is_empty() {
                            tools_match_any(&tool.name, allowed_patterns).is_some()
                        } else {
                            true // No allowed patterns, so it's allowed
                        }
                    }
                }
            } else {
                // No disabled patterns, check allowed patterns
                if !allowed_patterns.is_empty() {
                    tools_match_any(&tool.name, allowed_patterns).is_some()
                } else {
                    true // Both empty, return all tools
                }
            }
        })
        .cloned()
        .collect()
}

/// List tools from multiple servers in parallel.
///
/// Processes servers concurrently up to the concurrency limit, tracking
/// successes and failures separately. Implements DISC-05 and prepares ERR-07.
/// Applies tool filtering based on server configuration.
///
/// # Arguments
/// * `server_names` - List of server names to process
/// * `list_fn` - Async function to list tools for a server
/// * `executor` - ParallelExecutor to control concurrency
/// * `config` - Configuration to apply tool filtering rules
///
/// # Returns
/// Tuple of (successes, failures) where:
/// - successes: Vec<(String, Vec<ToolInfo>)> of server name and filtered tools
/// - failures: Vec<String> of server names that failed
///
/// # Example
/// ```rust,ignore
/// let (successes, failures) = list_tools_parallel(
///     server_names,
///     |server| async move { daemon.list_tools(&server).await },
///     executor,
///     &config,
/// ).await?;
/// ```
pub async fn list_tools_parallel<F, Fut>(
    server_names: Vec<String>,
    list_fn: F,
    executor: &ParallelExecutor,
    config: &Config,
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
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("Failed to acquire semaphore permit - semaphore should not be closed");

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
            Ok((server_name, raw_tools)) => {
                // Apply filtering based on server config
                let server_config = config.get_server(&server_name);
                if let Some(server_config) = server_config {
                    let filtered_tools = filter_tools(raw_tools, server_config);
                    successes.push((server_name, filtered_tools));
                } else {
                    // Server not found in config, return raw tools
                    successes.push((server_name, raw_tools));
                }
            }
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
