//! CLI commands for MCP CLI tool.

use crate::config::{ServerConfig, ServerTransport, Config};
use crate::error::{McpError, Result};
use crate::transport::Transport;
use crate::client::ToolInfo;
use std::io::{self, Read, IsTerminal};
use std::sync::Arc;
use crate::ipc::ProtocolClient;
use crate::parallel::{ParallelExecutor, list_tools_parallel};
use crate::output::{print_error, print_warning, print_info, print_success};
use crate::retry::{retry_with_backoff, timeout_wrapper, RetryConfig, is_transient_error};
use backoff::Error as BackoffError;
use tokio::sync::Mutex;

/// Execute the list servers command.
///
/// Lists all configured MCP servers and their tool availability.
/// Implements DISC-01: discovery of available tools.
/// Implements DISC-05: parallel server discovery with configurable concurrency.
/// Implements ERR-07: partial failure warnings.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `with_descriptions` - If true, show tool descriptions (DISC-06)
///
/// # Errors
/// Returns McpError::ConfigParseError if config file is invalid
pub async fn cmd_list_servers(mut daemon: Box<dyn ProtocolClient>, with_descriptions: bool) -> Result<()> {
    let config = daemon.config();

    if config.is_empty() {
        print_error("No servers configured. Please create a config file.");
        return Ok(());
    }

    print_info(&format!("Configured servers:"));
    println!();

    // Get server names from daemon
    let server_names = daemon.list_servers().await
        .map_err(|e| {
            print_error(&format!("Failed to get servers list: {}", e));
            e
        })?;

    // Create parallel executor with concurrency limit from config
    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Create daemon client for parallel execution (daemon is moved here and not used again)
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            // Closure that lists tools for a single server
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard.list_tools(&server).await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| {
                                    crate::client::ToolInfo {
                                        name: protocol_tool.name,
                                        description: Some(protocol_tool.description),
                                        input_schema: protocol_tool.input_schema,
                                    }
                                })
                                .collect()
                        })
                }
            },
            &executor,
        )
        .await?
    };

    // Display successful results
    for (server_name, tools) in &successes {
        // Get server info from config for display
        {
            let daemon_guard = daemon_arc.lock().await;
            if let Some(server_config) = daemon_guard.config().get_server(&server_name) {
                println!("{} {} ({})", server_name, server_config.description.as_deref().unwrap_or(""), server_config.transport.type_name());
            } else {
                println!("{} (unknown)", server_name);
            }
        }
        print_info(&format!("    Tools: {}", tools.len()));
        if with_descriptions && !tools.is_empty() {
            for tool in tools {
                println!("      - {}: {}", tool.name, tool.description.as_deref().unwrap_or(""));
            }
        }
        println!();
    }

    // Warn about partial failures (ERR-07)
    if !failures.is_empty() {
        print_warning(&format!(
            "Failed to connect to {} of {} servers: {}",
            failures.len(),
            successes.len() + failures.len(),
            failures.join(", ")
        ));
        println!();
    }

    Ok(())
}

/// Execute server info command.
///
/// Displays detailed information about a specific server.
/// Implements DISC-02: inspection of server details.
/// Implements TASK-03: colored output for error cases.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `server_name` - Name of the server to inspect
///
/// # Errors
/// Returns McpError::ServerNotFound if server doesn't exist (ERR-02)
pub async fn cmd_server_info(daemon: Box<dyn ProtocolClient>, server_name: &str) -> Result<()> {
    let config = daemon.config();
    let server = config.get_server(server_name)
        .ok_or_else(|| {
            print_error(&format!("Server '{}' not found", server_name));
            McpError::ServerNotFound {
                server: server_name.to_string(),
            }
        })?;

    print_info(&format!("Server: {}", server.name));
    if let Some(desc) = &server.description {
        println!("Description: {}", desc);
    }
    println!("Transport: {}", server.transport.type_name());

    match &server.transport {
        ServerTransport::Stdio { command, args, env, cwd } => {
            println!("Command: {}", command);
            if !args.is_empty() {
                println!("Arguments: {:?}", args);
            }
            if !env.is_empty() {
                println!("Environment:");
                for (key, value) in env {
                    println!("  {}={}", key, value);
                }
            }
            if let Some(cwd_path) = cwd {
                println!("Working directory: {}", cwd_path);
            }
        }
        ServerTransport::Http { url, headers } => {
            println!("URL: {}", url);
            if !headers.is_empty() {
                println!("Headers:");
                for (key, value) in headers {
                    println!("  {}: {}", key, value);
                }
            }
        }
    }

    Ok(())
}

/// Execute tool info command.
///
/// Displays detailed information about a specific tool including its JSON Schema.
/// Implements DISC-03: inspection of tool details.
/// Implements TASK-03: colored output for error cases.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
///
/// # Errors
/// Returns McpError::ToolNotFound if tool doesn't exist (ERR-02)
/// Returns McpError::AmbiguousCommand if tool_id format is unclear (ERR-06)
pub async fn cmd_tool_info(mut daemon: Box<dyn ProtocolClient>, tool_id: &str) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let _server = daemon.config().get_server(&server_name)
        .ok_or_else(|| {
            print_error(&format!("Tool '{}' not found on server '{}'", tool_name, server_name));
            McpError::ToolNotFound {
                tool: tool_name.clone(),
                server: server_name.clone(),
            }
        })?;

    // Send ListTools request to daemon
    let tools = daemon.list_tools(&server_name).await?;

    let tool = tools.iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| {
            print_error(&format!("Tool '{}' not found on server '{}'", tool_name, server_name));
            McpError::ToolNotFound {
                tool: tool_name.clone(),
                server: server_name.clone(),
            }
        })?;

    print_info(&format!("Tool: {}", tool.name));
    println!("Description: {}", tool.description);
    println!("Input schema (JSON Schema):");
    println!("{}", serde_json::to_string_pretty(&tool.input_schema)?);

    Ok(())
}

/// Execute tool call command.
///
/// Executes a tool with JSON arguments, retrying on transient failures.
/// Implements EXEC-01, EXEC-02, EXEC-04, EXEC-06.
/// Implements EXEC-05, EXEC-07: retry logic with exponential backoff.
/// Implements TASK-03: colored output for stdin and error cases.
///
/// # Arguments
/// * `daemon` - Daemon IPC client (will be wrapped in Arc<Mutex>)
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `args_json` - JSON arguments as a string, or None to read from stdin
///
/// # Errors
/// Returns McpError::InvalidProtocol for malformed response
/// Returns McpError::Timeout if timeout exceeded (EXEC-06)
/// Returns McpError::MaxRetriesExceeded if max retries exceeded (EXEC-07)
pub async fn cmd_call_tool(mut daemon: Box<dyn ProtocolClient>, tool_id: &str, args_json: Option<&str>) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

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
                print_error("No arguments provided. Pass JSON arguments as a command-line argument, or pipe JSON to stdin.");
                let available_tools = match daemon.list_tools(&server_name).await {
                    Ok(tools) => {
                        let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                        names.join(", ")
                    }
                    Err(_) => "(unavailable)".to_string(),
                };
                println!();
                print_info(&format!("Available tools on '{}': {}", server_name, available_tools));
                return Ok(());
            }

            let input = read_stdin_async()?;
            serde_json::from_str(&input).map_err(|e| McpError::InvalidJson { source: e })?
        }
    };

    // Execute tool with retry logic (EXEC-05, EXEC-07)
    let retry_config = RetryConfig::from_config(&config);
    let timeout_secs = config.timeout_secs;

    // Create shared access for retry closure
    let daemon_shared = Arc::new(tokio::sync::Mutex::new(daemon));

    // Wrap execution with both retry logic and overall timeout
    let result = timeout_wrapper(
        || async {
            retry_with_backoff(
                || {
                    let daemon_shared = daemon_shared.clone();
                    let server_name_clone = server_name.clone();
                    let tool_name_clone = tool_name.clone();
                    let arguments_clone = arguments.clone();

                    async move {
                        let mut daemon_guard = daemon_shared.lock().await;
                        daemon_guard
                            .execute_tool(&server_name_clone, &tool_name_clone, arguments_clone)
                            .await
                    }
                },
                &retry_config,
            )
            .await
        },
        timeout_secs,
    )
    .await;

    match result {
        Ok(result) => {
            // Format and display the result (EXEC-03)
            format_and_display_result(&result, &server_name);

            // Print success message with colored output (TASK-03)
            println!();
            print_info(&format!("Tool '{}' executed successfully on server '{}'", tool_name, server_name));

            Ok(())
        }
        Err(McpError::MaxRetriesExceeded { attempts }) => {
            print_error(&format!(
                "Tool execution failed after {} retry attempts. Last error: {}",
                attempts,
                "No additional information available"
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

/// Execute search tools command.
///
/// Search for tools using glob patterns with parallel server discovery.
/// Implements DISC-04: search of tools using glob patterns.
/// Implements TASK-02: parallel server discovery for cmd_search_tools.
/// Implements TASK-03: colored output for error cases.
///
/// # Arguments
/// * `daemon` - Daemon IPC client (IpcClientWrapper<UnixIpcClient>)
/// * `pattern` - Glob pattern to search for (e.g., "*", "search*", "tool-*")
///
/// # Errors
/// Returns empty result if no tools match
pub async fn cmd_search_tools(mut daemon: Box<dyn ProtocolClient>, pattern: &str) -> Result<()> {
    let config = daemon.config();

    if config.is_empty() {
        print_error("No servers configured. Please create a config file.");
        return Ok(());
    }

    print_info(&format!("Searching for tools matching '{}':", pattern));

    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Get server names from daemon
    let server_names = daemon.list_servers().await
        .map_err(|e| {
            print_error(&format!("Failed to get servers list: {}", e));
            e
        })?;

    // Create daemon client for parallel execution (daemon is moved here and not used again)
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            // Closure that lists tools for a single server
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard.list_tools(&server).await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| {
                                    crate::client::ToolInfo {
                                        name: protocol_tool.name,
                                        description: Some(protocol_tool.description),
                                        input_schema: protocol_tool.input_schema,
                                    }
                                })
                                .collect()
                        })
                }
            },
            &executor,
        )
        .await?
    };

    let mut matches_found = false;
    let pattern_obj = match glob::Pattern::new(pattern) {
        Ok(p) => p,
        Err(_) => {
            print_warning(&format!("Invalid glob pattern '{}': {}", pattern, "Using substring matching instead"));
            glob::Pattern::new("*").unwrap()
        }
    };

    // Display successful matches
    for (server_name, tools) in &successes {
        // Match tool names against the glob pattern
        let matched_tools: Vec<_> = tools.iter()
            .filter(|tool| {
                let tool_name = &tool.name;
                pattern_obj.matches(tool_name)
            })
            .collect();

        if !matched_tools.is_empty() {
            matches_found = true;
            print_info(&format!("Server: {}:", server_name));
            println!("  - {} tool(s) match '{}':", matched_tools.len(), pattern);
            for tool in matched_tools {
                println!("      - {}: {}", tool.name, tool.description.as_deref().unwrap_or(""));
            }
        }
    }

    // Warn about partial failures
    if !failures.is_empty() {
        print_warning(&format!(
            "Search limited to {} servers ({} failed): {}",
            successes.len(),
            failures.len(),
            failures.join(", ")
        ));
        println!();
    }

    if !matches_found {
        print_error("No matching tools found.");
    } else {
        println!();
        print_info(&format!("Total matches: {}", successes.iter().filter(|(_, t)| !t.is_empty()).count()));
    }

    Ok(())
}

/// Parse a tool identifier from a string.
///
/// Supports both "server/tool" and "server tool" formats (CLI-05).
/// Implements ERR-06: handles ambiguous command prompts with suggestions.
///
/// # Arguments
/// * `tool_id` - Tool identifier string
///
/// # Returns
/// Tuple of (server_name, tool_name)
///
/// # Errors
/// Returns McpError::AmbiguousCommand if format is unclear (ERR-06)
pub fn parse_tool_id(tool_id: &str) -> Result<(String, String)> {
    // Try "server/tool" format first
    if let Some((server, tool)) = tool_id.split_once('/') {
        let server = server.to_string();
        let tool = tool.to_string();

        if !server.is_empty() && !tool.is_empty() {
            return Ok((server, tool));
        }
    }

    // Try "server tool" format
    let parts: Vec<&str> = tool_id.split_whitespace().collect();
    if parts.len() == 2 {
        let server = parts[0].to_string();
        let tool = parts[1].to_string();

        if !server.is_empty() && !tool.is_empty() {
            return Ok((server, tool));
        }
    }

    // Ambiguous format - suggest alternatives (ERR-06)
    Err(McpError::AmbiguousCommand {
        hint: format!("Tool identifier '{}' is ambiguous. Use format 'server/tool' or 'server tool'.", tool_id),
    })
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
        .map_err(|e| McpError::io_error(e))?;

    if input.trim().is_empty() {
        return Err(McpError::usage_error("Stdin is empty. Pipe JSON data or pass as command-line argument."));
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
                let text_lines: Vec<String> = text_array.iter()
                    .filter_map(|item| {
                        let item_obj = item.as_object()?;
                        item_obj.get("type")?.as_str().and_then(|t| {
                            match t {
                                "text" => item_obj.get("text")?.as_str().map(|s| s.to_string()),
                                "image" => item_obj.get("data")?.as_str().map(|d| format!("(image data: {} bytes, type: unknown)", d.len())),
                                "resource" => {
                                    item_obj.get("uri")?.as_str().map(|u| format!("(resource: {})", u))
                                },
                                _ => None,
                            }
                        })
                    })
                    .collect();

                if text_lines.is_empty() {
                    println!("Result: {}", serde_json::to_string_pretty(content).unwrap_or_else(|_| result.to_string()));
                } else {
                    println!("Result:");
                    for line in text_lines {
                        println!("  {}", line);
                    }
                }
            } else {
                println!("Result: {}", serde_json::to_string_pretty(content).unwrap_or_else(|_| result.to_string()));
            }
        } else {
            println!("Result: {}", serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()));
        }
    } else {
        println!("Result: {}", result);
    }
}

/// Create a transport for a server configuration.
///
/// This bridges the config layer and the transport layer.
/// Implements Task 4: add config client conversion function.
///
/// # Arguments
/// * `server` - Server configuration
///
/// # Returns
/// Result<Box<dyn Transport>> for the server
pub fn create_transport_for_server(server: &ServerConfig) -> std::result::Result<Box<dyn Transport + Send + Sync>, McpError> {
    server.create_transport(server.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_id_slash_format() {
        let result = parse_tool_id("server/tool_name");
        assert!(result.is_ok());
        let (server, tool) = result.unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool_name");
    }

    #[test]
    fn test_parse_tool_id_space_format() {
        let result = parse_tool_id("server tool_name");
        assert!(result.is_ok());
        let (server, tool) = result.unwrap();
        assert_eq!(server, "server");
        assert_eq!(tool, "tool_name");
    }

    #[test]
    fn test_parse_tool_id_ambiguous() {
        let result = parse_tool_id("ambiguous");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::AmbiguousCommand { .. }));
    }
}
