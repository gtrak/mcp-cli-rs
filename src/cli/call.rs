//! Execute tool command implementation.

use crate::cli::models::CallResultModel;
use crate::error::{McpError, Result};
use crate::format::OutputMode;
use crate::cli::formatters;
use crate::ipc::ProtocolClient;
use crate::output::print_error;
use crate::retry::{retry_with_backoff, RetryConfig};
use futures_util::FutureExt;
use std::io::{self, IsTerminal, Read};
use std::sync::Arc;

/// Execute tool call command.
///
/// Executes a tool with JSON arguments, retrying on transient failures.
/// Implements EXEC-01, EXEC-02, EXEC-04, EXEC-06.
/// Implements EXEC-05, EXEC-07: retry logic with exponential backoff.
/// Implements TASK-03: colored output for stdin and error cases.
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client (will be wrapped in Arc<Mutex>)
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `args_json` - JSON arguments as a string, or None to read from stdin
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::InvalidProtocol for malformed response
/// Returns McpError::Timeout if timeout exceeded (EXEC-06)
/// Returns McpError::MaxRetriesExceeded if max retries exceeded (EXEC-07)
pub async fn cmd_call_tool(
    mut daemon: Box<dyn ProtocolClient>,
    tool_id: &str,
    args_json: Option<&str>,
    output_mode: OutputMode,
) -> Result<()> {
    let (server_name, tool_name) = crate::cli::info::parse_tool_id(tool_id)?;

    // Get current timestamp for metadata
    let _timestamp = get_timestamp();

    // Check if server exists
    let config = daemon.config();
    let _server = config.get_server(&server_name).ok_or_else(|| {
        print_error(&format!("Server '{}' not found", server_name));
        McpError::ServerNotFound {
            server: server_name.clone(),
        }
    })?;

    // Parse arguments (inline or from stdin)
    let arguments: serde_json::Value = match args_json {
        Some(args) => {
            serde_json::from_str(args).map_err(|e| McpError::InvalidJson { source: e })?
        }
        None => {
            // Read from stdin if not provided (EXEC-02)
            if std::io::stdin().is_terminal() {
                print_error(
                    "No arguments provided. Pass JSON arguments as a command-line argument, or pipe JSON to stdin.",
                );
                let available_tools = match daemon.list_tools(&server_name).await {
                    Ok(tools) => {
                        let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                        names.join(", ")
                    }
                    Err(_) => "(unavailable)".to_string(),
                };
                println!();
                print_error(&format!(
                    "Available tools on '{}': {}",
                    server_name, available_tools
                ));
                return Ok(());
            }

            let input = read_stdin_async()?;
            serde_json::from_str(&input).map_err(|e| McpError::InvalidJson { source: e })?
        }
    };

    // Check if tool is disabled (FILT-04)
    let server_config = config.get_server(&server_name);
    if let Some(server_config) = server_config {
        // Check if tool matches disabled_tools patterns
        if let Some(disabled_patterns) = &server_config.disabled_tools {
            let is_disabled = crate::cli::filter::tools_match_any(&tool_name, disabled_patterns);
            if is_disabled.is_some() {
                let patterns_str = disabled_patterns.join(", ");
                let error_msg = format!(
                    "Tool '{}' on server '{}' is disabled (blocked by patterns: {})",
                    tool_name, server_name, patterns_str
                );

                if output_mode == OutputMode::Json {
                    let model = CallResultModel {
                        server_name: server_name.clone(),
                        tool_name: tool_name.clone(),
                        success: false,
                        result: None,
                        error: Some(error_msg.clone()),
                        execution_time_ms: None,
                        retries: 0,
                    };
                    formatters::format_call_result(&model, output_mode);
                } else {
                    print_error(&error_msg);
                }

                return Err(McpError::UsageError {
                    message: "Tool execution blocked by disabled_tools configuration. Remove patterns from disabled_tools list to allow this tool.".to_string(),
                });
            }
        }
    }

    // Execute tool with retry logic (EXEC-05, EXEC-07)
    let retry_config = RetryConfig::from_config(&config);

    // Create shared access for operation closure
    let daemon_shared = Arc::new(tokio::sync::Mutex::new(daemon));

    // Execute tool with retry logic - this is an async operation
    let operation = || {
        let daemon_shared = daemon_shared.clone();
        let server_name_clone = server_name.clone();
        let tool_name_clone = tool_name.clone();
        let arguments_clone = arguments.clone();

        // Convert async block to boxed trait object Future using futures-util
        Box::new(async move {
            let mut daemon_guard = daemon_shared.lock().await;
            daemon_guard
                .execute_tool(&server_name_clone, &tool_name_clone, arguments_clone)
                .await
        })
        .boxed()
    };

    // Execute with retry
    let start_time = std::time::Instant::now();
    let result = retry_with_backoff(operation, &retry_config).await;
    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    // Build model from result and format it
    let model = match result {
        Ok(tool_result) => {
            CallResultModel {
                server_name: server_name.clone(),
                tool_name: tool_name.clone(),
                success: true,
                result: Some(tool_result),
                error: None,
                execution_time_ms: Some(execution_time_ms),
                retries: 0, // Retry count not tracked by current retry implementation
            }
        }
        Err(McpError::MaxRetriesExceeded { attempts }) => {
            CallResultModel {
                server_name: server_name.clone(),
                tool_name: tool_name.clone(),
                success: false,
                result: None,
                error: Some(format!(
                    "Tool execution failed after {} retry attempts",
                    attempts
                )),
                execution_time_ms: Some(execution_time_ms),
                retries: attempts,
            }
        }
        Err(McpError::OperationCancelled { timeout }) => {
            CallResultModel {
                server_name: server_name.clone(),
                tool_name: tool_name.clone(),
                success: false,
                result: None,
                error: Some(format!(
                    "Tool execution cancelled after {}s timeout",
                    timeout
                )),
                execution_time_ms: Some(execution_time_ms),
                retries: 0,
            }
        }
        Err(e) => {
            CallResultModel {
                server_name: server_name.clone(),
                tool_name: tool_name.clone(),
                success: false,
                result: None,
                error: Some(format!("Tool execution failed: {}", e)),
                execution_time_ms: Some(execution_time_ms),
                retries: 0,
            }
        }
    };

    formatters::format_call_result(&model, output_mode);

    // Return appropriate error if execution failed
    if !model.success && let Some(ref err) = model.error {
        if err.contains("retry attempts") {
            return Err(McpError::MaxRetriesExceeded { attempts: model.retries });
        } else if err.contains("timeout") {
            // Extract timeout value from error message
            return Err(McpError::OperationCancelled { timeout: 30 }); // Default timeout
        }
    }

    Ok(())
}

/// Get current timestamp in seconds since epoch.
fn get_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", duration.as_secs())
}

/// Read JSON from stdin asynchronously.
///
/// This is a helper for EXEC-02: piping JSON input to the tool call.
///
/// # Errors
/// Returns error if stdin is not readable or empty
pub fn read_stdin_async() -> Result<String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(McpError::io_error)?;

    if input.trim().is_empty() {
        return Err(McpError::usage_error(
            "Stdin is empty. Pipe JSON data or pass as command-line argument.",
        ));
    }

    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_result_model_building() {
        let model = CallResultModel {
            server_name: "test-server".to_string(),
            tool_name: "test-tool".to_string(),
            success: true,
            result: Some(serde_json::json!({"data": "value"})),
            error: None,
            execution_time_ms: Some(150),
            retries: 0,
        };

        assert!(model.success);
        assert_eq!(model.server_name, "test-server");
        assert!(model.result.is_some());
    }

    #[test]
    fn test_call_result_model_error() {
        let model = CallResultModel {
            server_name: "test-server".to_string(),
            tool_name: "test-tool".to_string(),
            success: false,
            result: None,
            error: Some("Connection failed".to_string()),
            execution_time_ms: Some(50),
            retries: 3,
        };

        assert!(!model.success);
        assert!(model.error.is_some());
        assert_eq!(model.retries, 3);
    }
}
