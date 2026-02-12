//! Execute tool command implementation.

use crate::daemon::protocol::{ExecutionMetadata, ToolError, ToolResult};
use crate::error::{McpError, Result};
use crate::format::OutputMode;
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_json};
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
    // Handle JSON mode separately
    if output_mode == OutputMode::Json {
        return cmd_call_tool_json(daemon, tool_id, args_json).await;
    }

    let (server_name, tool_name) = crate::cli::info::parse_tool_id(tool_id)?;

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
                print_error(&format!(
                    "Tool '{}' on server '{}' is disabled (blocked by patterns: {})",
                    tool_name, server_name, patterns_str
                ));
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
    let result = retry_with_backoff(operation, &retry_config).await;

    match result {
        Ok(result) => {
            // Format and display the result (EXEC-03)
            format_and_display_result(&result, &server_name);

            // Print success message with colored output (TASK-03)
            println!();
            print_error(&format!(
                "Tool '{}' executed successfully on server '{}'",
                tool_name, server_name
            ));

            Ok(())
        }
        Err(McpError::MaxRetriesExceeded { attempts }) => {
            print_error(&format!(
                "Tool execution failed after {} retry attempts. Last error: {}",
                attempts, "No additional information available"
            ));
            Err(McpError::MaxRetriesExceeded { attempts })
        }
        Err(McpError::OperationCancelled { timeout }) => {
            print_error(&format!(
                "Tool execution cancelled after {}s timeout",
                timeout
            ));
            Err(McpError::OperationCancelled { timeout })
        }
        Err(e) => {
            print_error(&format!("Tool execution failed: {}", e));
            Err(e)
        }
    }
}

/// Execute the call tool command in JSON mode.
///
/// Outputs tool execution result as structured JSON for programmatic use.
/// Implements OUTP-07: --json flag support
/// Implements OUTP-08: consistent JSON schema with complete execution results
/// Implements OUTP-10: error responses are valid JSON
async fn cmd_call_tool_json(
    daemon: Box<dyn ProtocolClient>,
    tool_id: &str,
    args_json: Option<&str>,
) -> Result<()> {
    let (server_name, tool_name) = crate::cli::info::parse_tool_id(tool_id)?;

    // Get current timestamp for metadata (RFC 3339 format approximation)
    let timestamp = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();

        // Simple conversion to approximate RFC 3339 format
        // This is good enough for the use case without adding chrono dependency
        format!("{}s", secs)
    };

    // Parse arguments (inline or from stdin)
    let result_parse: std::result::Result<serde_json::Value, McpError> = match args_json {
        Some(args) => serde_json::from_str(args).map_err(|e| McpError::InvalidJson { source: e }),
        None => {
            // Read from stdin if not provided
            if std::io::stdin().is_terminal() {
                let output = ToolResult {
                    server: server_name.clone(),
                    tool: tool_name.clone(),
                    status: "error".to_string(),
                    result: None,
                    error: Some(ToolError {
                        message: "No arguments provided. Pass JSON arguments as a command-line argument, or pipe JSON to stdin.".to_string(),
                        code: Some(400),
                    }),
                    metadata: ExecutionMetadata {
                        timestamp: timestamp.clone(),
                        retry_count: Some(0),
                    },
                };
                print_json(&output);
                return Ok(());
            }

            let input = read_stdin_async()?;
            serde_json::from_str(&input).map_err(|e| McpError::InvalidJson { source: e })
        }
    };

    let arguments = match result_parse {
        Ok(args) => args,
        Err(e) => {
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolError {
                    message: format!("Failed to parse arguments: {}", e),
                    code: Some(400),
                }),
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(0),
                },
            };
            print_json(&output);
            return Ok(());
        }
    };

    // Check if server exists
    let config = daemon.config();
    let _server = config.get_server(&server_name).ok_or_else(|| {
        let output = ToolResult {
            server: server_name.clone(),
            tool: tool_name.clone(),
            status: "error".to_string(),
            result: None,
            error: Some(ToolError {
                message: format!("Server '{}' not found", server_name),
                code: Some(404),
            }),
            metadata: ExecutionMetadata {
                timestamp: timestamp.clone(),
                retry_count: Some(0),
            },
        };
        print_json(&output);
        McpError::ServerNotFound {
            server: server_name.clone(),
        }
    })?;

    // Check if tool is disabled (FILT-04)
    let server_config = config.get_server(&server_name);
    if let Some(server_config) = server_config
        && let Some(disabled_patterns) = &server_config.disabled_tools
    {
        let is_disabled = crate::cli::filter::tools_match_any(&tool_name, disabled_patterns);
        if is_disabled.is_some() {
            let patterns_str = disabled_patterns.join(", ");
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolError {
                    message: format!(
                        "Tool '{}' on server '{}' is disabled (blocked by patterns: {})",
                        tool_name, server_name, patterns_str
                    ),
                    code: Some(403),
                }),
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(0),
                },
            };
            print_json(&output);
            return Err(McpError::UsageError {
                message: "Tool execution blocked by disabled_tools configuration".to_string(),
            });
        }
    }

    // Execute tool with retry logic
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
    let result = retry_with_backoff(operation, &retry_config).await;

    match result {
        Ok(tool_result) => {
            // Output JSON result
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "success".to_string(),
                result: Some(tool_result),
                error: None,
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(0),
                },
            };
            print_json(&output);
            Ok(())
        }
        Err(McpError::MaxRetriesExceeded { attempts }) => {
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolError {
                    message: format!("Tool execution failed after {} retry attempts", attempts),
                    code: Some(503),
                }),
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(attempts),
                },
            };
            print_json(&output);
            Err(McpError::MaxRetriesExceeded { attempts })
        }
        Err(McpError::OperationCancelled { timeout }) => {
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolError {
                    message: format!("Tool execution cancelled after {}s timeout", timeout),
                    code: Some(408),
                }),
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(0),
                },
            };
            print_json(&output);
            Err(McpError::OperationCancelled { timeout })
        }
        Err(e) => {
            let output = ToolResult {
                server: server_name.clone(),
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolError {
                    message: format!("Tool execution failed: {}", e),
                    code: None,
                }),
                metadata: ExecutionMetadata {
                    timestamp: timestamp.clone(),
                    retry_count: Some(0),
                },
            };
            print_json(&output);
            Err(e)
        }
    }
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

/// Format and display tool execution result.
///
/// Implements EXEC-03: formatting tool results as readable text.
///
/// # Arguments
/// * `result` - The tool result to format
/// * `server_name` - Server name for context
pub fn format_and_display_result(result: &serde_json::Value, server_name: &str) {
    if result.is_object() {
        let result_obj = result.as_object().unwrap();

        // Check if result indicates an error
        if let Some(error) = result_obj.get("error") {
            println!("Error from server '{}':", server_name);
            if let Some(msg) = error.get("message").and_then(|m| m.as_str()) {
                println!("  {}", msg);
            }
            if let Some(code) = error.get("code").and_then(|c| c.as_u64()) {
                println!("  Code: {}", code);
            }
            return;
        }

        // Extract text content from result
        let content = result_obj.get("result");

        if let Some(content) = content {
            if let Some(text_array) = content.get("content").and_then(|c| c.as_array()) {
                let text_lines: Vec<String> = text_array
                    .iter()
                    .filter_map(|item| {
                        let item_obj = item.as_object()?;
                        item_obj.get("type")?.as_str().and_then(|t| match t {
                            "text" => item_obj.get("text")?.as_str().map(|s| s.to_string()),
                            "image" => item_obj
                                .get("data")?
                                .as_str()
                                .map(|d| format!("(image data: {} bytes, type: unknown)", d.len())),
                            "resource" => item_obj
                                .get("uri")?
                                .as_str()
                                .map(|u| format!("(resource: {})", u)),
                            _ => None,
                        })
                    })
                    .collect();

                if text_lines.is_empty() {
                    println!(
                        "Result: {}",
                        serde_json::to_string_pretty(content)
                            .unwrap_or_else(|_| result.to_string())
                    );
                } else {
                    println!("Result:");
                    for line in text_lines {
                        println!("  {}", line);
                    }
                }
            } else {
                println!(
                    "Result: {}",
                    serde_json::to_string_pretty(content).unwrap_or_else(|_| result.to_string())
                );
            }
        } else {
            println!(
                "Result: {}",
                serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
            );
        }
    } else {
        println!("Result: {}", result);
    }
}
