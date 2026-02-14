//! Execute tool command implementation.

use crate::cli::formatters;
use crate::cli::models::CallResultModel;
use crate::error::{McpError, Result};
use crate::format::OutputMode;
use crate::ipc::ProtocolClient;
use crate::output::print_error;
use crate::retry::{RetryConfig, retry_with_backoff};
use futures_util::FutureExt;
use std::io::{self, Read};
use std::sync::Arc;

/// Parse command-line arguments into a JSON object.
///
/// Supports multiple formats:
/// - Empty args → empty object {}
/// - JSON only (starts with {) → parse as JSON (backward compatible)
/// - --key value → {"key": "value"}
/// - --key=value → {"key": "value"}
/// - --key {"a":1} → parse JSON value → {"key": {"a": 1}}
fn parse_arguments(args: Vec<String>) -> Result<serde_json::Value> {
    // Empty args → empty object (don't try to read stdin - that happens at a higher level)
    if args.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }

    // If first arg starts with '{', treat as JSON (backward compatible)
    if args.first().map(|s| s.starts_with('{')).unwrap_or(false) {
        let json_str = args.join(" ");
        return serde_json::from_str(&json_str).map_err(|e| McpError::InvalidJson { source: e });
    }

    // Parse as --key value pairs
    let mut map = serde_json::Map::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            return Err(McpError::usage_error(&format!(
                "Expected --key value pair, got: {}",
                arg
            )));
        }

        // Strip -- prefix
        let key_with_value = arg.trim_start_matches("--");

        // Check for = separator in the argument itself (--key=value)
        if let Some((k, v)) = key_with_value.split_once('=') {
            // --key=value format
            let value = parse_json_value(v)?;
            map.insert(k.to_string(), value);
            i += 1;
            continue;
        }

        // No = separator
        let key = key_with_value;
        
        // Check if next arg exists and is not a flag
        let has_value = i + 1 < args.len() && !args[i + 1].starts_with("--");
        
        if has_value {
            let next_arg = &args[i + 1];
            if next_arg.starts_with('{') {
                // Next arg is JSON → use as value directly
                let value = parse_json_value(next_arg)?;
                map.insert(key.to_string(), value);
                i += 2;
            } else {
                // --key value format
                map.insert(key.to_string(), serde_json::Value::String(next_arg.clone()));
                i += 2;
            }
        } else {
            // Boolean flag (no value or next arg is another flag)
            map.insert(key.to_string(), serde_json::Value::Bool(true));
            i += 1;
        }
    }

    Ok(serde_json::Value::Object(map))
}

/// Parse a value string into JSON, handling both plain strings and JSON.
fn parse_json_value(value: &str) -> Result<serde_json::Value> {
    let trimmed = value.trim();
    // Try to parse as JSON first
    if let Ok(parsed) = serde_json::from_str(trimmed) {
        Ok(parsed)
    } else {
        // Treat as plain string
        Ok(serde_json::Value::String(trimmed.to_string()))
    }
}

/// Execute tool call command.
///
/// Executes a tool with JSON arguments, retrying on transient failures.
/// Implements EXEC-01, EXEC-02, EXEC-04, EXEC-06.
/// Implements EXEC-05, EXEC-07: retry logic with exponential backoff.
/// Implements TASK-03: colored output for stdin and error cases.
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client (will be wrapped in `Arc<Mutex>`)
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `args` - Arguments as Vec<String>, supports: JSON, --key value, --key=value, --key {"a":1}
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::InvalidProtocol for malformed response
/// Returns McpError::Timeout if timeout exceeded (EXEC-06)
/// Returns McpError::MaxRetriesExceeded if max retries exceeded (EXEC-07)
pub async fn cmd_call_tool(
    mut daemon: Box<dyn ProtocolClient>,
    tool_id: &str,
    args: Vec<String>,
    output_mode: OutputMode,
) -> Result<()> {
    let (server_name, tool_name) = crate::cli::info::parse_tool_id(tool_id)?;

    // Get current timestamp for metadata
    let _timestamp = get_timestamp();

    // Check if server exists
    let config = daemon.config();
    let _server = config.get_server(&server_name).ok_or_else(|| {
        print_error(&format!("Server '{}' not found", server_name));
        let servers: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();
        McpError::ServerNotFound {
            server: server_name.clone(),
            servers,
        }
    })?;

    // Parse arguments - supports JSON, --key value, --key=value, --key {"a":1}
    let arguments: serde_json::Value = match parse_arguments(args) {
        Ok(parsed) => parsed,
        Err(e) => {
            print_error(&format!("Failed to parse arguments: {}", e));
            return Err(e);
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
        Err(McpError::MaxRetriesExceeded { attempts }) => CallResultModel {
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
        },
        Err(McpError::OperationCancelled { timeout }) => CallResultModel {
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
        },
        Err(e) => CallResultModel {
            server_name: server_name.clone(),
            tool_name: tool_name.clone(),
            success: false,
            result: None,
            error: Some(format!("Tool execution failed: {}", e)),
            execution_time_ms: Some(execution_time_ms),
            retries: 0,
        },
    };

    formatters::format_call_result(&model, output_mode);

    // Return appropriate error if execution failed
    if !model.success
        && let Some(ref err) = model.error
    {
        if err.contains("retry attempts") {
            return Err(McpError::MaxRetriesExceeded {
                attempts: model.retries,
            });
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

    // Tests for parse_arguments function (ARGS-01, ARGS-02, ARGS-03, ARGS-04)

    #[test]
    fn test_parse_arguments_empty() {
        // Empty args → empty object {}
        let result = parse_arguments(vec![]).unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn test_parse_arguments_json_backward_compatible() {
        // JSON only → parse as JSON (ARGS-04: backward compatible)
        let result = parse_arguments(vec!["{\"key\": \"value\"}".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_parse_arguments_key_value() {
        // --key value → {"key": "value"} (ARGS-01)
        let result = parse_arguments(vec!["--key".to_string(), "value".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_parse_arguments_key_equals_value() {
        // --key=value → {"key": "value"} (ARGS-02)
        let result = parse_arguments(vec!["--key=value".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_parse_arguments_key_json_value() {
        // --key {"a":1} → {"key": {"a": 1}} (ARGS-03)
        let result = parse_arguments(vec!["--key".to_string(), "{\"a\":1}".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": {"a": 1}}));
    }

    #[test]
    fn test_parse_arguments_multiple_flags() {
        // Multiple flags
        let result = parse_arguments(vec![
            "--path".to_string(),
            "/tmp/file.txt".to_string(),
            "--verbose".to_string(),
            "true".to_string(),
        ]).unwrap();
        assert_eq!(result, serde_json::json!({"path": "/tmp/file.txt", "verbose": "true"}));
    }

    #[test]
    fn test_parse_arguments_boolean_flag() {
        // Boolean flags (next arg is another flag)
        let result = parse_arguments(vec![
            "--flag1".to_string(),
            "--flag2".to_string(),
        ]).unwrap();
        // Both flags should be boolean true
        assert_eq!(result, serde_json::json!({"flag1": true, "flag2": true}));
    }

    #[test]
    fn test_parse_arguments_json_with_nesting() {
        // JSON with nested objects
        let result = parse_arguments(vec!["{\"a\": {\"b\": 2}}".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"a": {"b": 2}}));
    }

    #[test]
    fn test_parse_arguments_key_equals_json_value() {
        // --key={"a":1} → {"key": {"a": 1}}
        let result = parse_arguments(vec!["--key={\"a\":1}".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": {"a": 1}}));
    }

    #[test]
    fn test_parse_arguments_single_boolean_flag() {
        // Single boolean flag (no value) should be true
        let result = parse_arguments(vec!["--key".to_string()]).unwrap();
        assert_eq!(result, serde_json::json!({"key": true}));
    }

    #[test]
    fn test_parse_arguments_error_invalid_format() {
        // Non-flag argument should error
        let result = parse_arguments(vec!["not-a-flag".to_string()]);
        assert!(result.is_err());
    }
}
