//! CLI commands for MCP CLI tool.

use crate::config::{Config, ServerConfig, ServerTransport};
use crate::error::{McpError, Result};
use crate::transport::Transport;
use std::io::{self, Read, IsTerminal};
use std::sync::{Arc, Mutex};
use crate::ipc::ProtocolClient;
use crate::parallel::{ParallelExecutor, list_tools_parallel};
use crate::output::{print_error, print_warning, print_info};

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
pub async fn cmd_list_servers(mut daemon: Box<dyn crate::ipc::ProtocolClient>, with_descriptions: bool) -> Result<()> {
    if daemon.config().is_empty() {
        print_error("No servers configured. Please create a config file.");
        return Ok(());
    }

    print_info("Configured servers:");
    println!();

    // Get server names from daemon
    let server_names = match daemon.list_servers().await {
        Ok(names) => names,
        Err(e) => {
            print_error(&format!("Failed to get servers list: {}", e));
            return Err(e);
        }
    };

    // Create Arc<Mutex<>> for shared mutable access across threads
    let daemon_arc = std::sync::Arc::new(Mutex::new(daemon));

    // Create parallel executor with concurrency limit from config
    let executor = ParallelExecutor::new(daemon.config().concurrency_limit);

    // List tools from all servers in parallel using list_tools_parallel
    let (successes, failures) = list_tools_parallel(
        server_names,
        // Closure that lists tools for a single server (using Arc<Mutex<>>)
        |server: String| async move {
            let daemon = daemon_arc.lock();
            daemon.list_tools(&server).await
                .map_err(|e| {
                    // Log individual failures but continue with other servers
                    tracing::warn!("Failed to list tools for {}: {}", server, e);
                    e
                })
                .map(|protocol_tools| {
                    // Convert protocol::ToolInfo to client::ToolInfo
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
        },
        &executor,
    )
    .await?;

    // Display successful results
    for (server_name, tools) in successes {
        // Get server info from config for display
        if let Some(server_config) = daemon.config().get_server(&server_name) {
            println!("{} {} ({})", server_name, server_config.description.as_deref().unwrap_or(""), server_config.transport.type_name());
        } else {
            println!("{} (unknown)", server_name);
        }

        print_info(&format!("    Tools: {}", tools.len()));
        if with_descriptions && !tools.is_empty() {
            for tool in &tools {
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
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `server_name` - Name of the server to inspect
///
/// # Errors
/// Returns McpError::ServerNotFound if server doesn't exist (ERR-02)
pub async fn cmd_server_info(daemon: Box<dyn crate::ipc::ProtocolClient>, server_name: &str) -> Result<()> {
    let config = daemon.config();
    let server = config.get_server(server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.to_string(),
        })?;

    println!("Server: {}", server.name);
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
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
///
/// # Errors
/// Returns McpError::ToolNotFound if tool doesn't exist (ERR-02)
/// Returns McpError::AmbiguousCommand if tool_id format is unclear (ERR-06)
pub async fn cmd_tool_info(mut daemon: Box<dyn crate::ipc::ProtocolClient>, tool_id: &str) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let server = daemon.config().get_server(&server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.clone(),
        })?;

    // Send ListTools request to daemon
    let tools = daemon.list_tools(&server_name).await?;

    let tool = tools.iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        })?;

    println!("Tool: {}", tool.name);
    println!("Description: {}", tool.description);
    println!("Input schema (JSON Schema):");
    println!("{}", serde_json::to_string_pretty(&tool.input_schema)?);

    Ok(())
}

/// Execute tool call command.
///
/// Executes a tool with JSON arguments.
/// Implements EXEC-01, EXEC-02, EXEC-04, EXEC-06.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `args_json` - JSON arguments as a string, or None to read from stdin
///
/// # Errors
/// Returns McpError::InvalidProtocol for malformed response
/// Returns McpError::Timeout if timeout exceeded (EXEC-06)
pub async fn cmd_call_tool(mut daemon: Box<dyn crate::ipc::ProtocolClient>, tool_id: &str, args_json: Option<&str>) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let _server = daemon.config().get_server(&server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.clone(),
        })?;

    // Send ExecuteTool request to daemon
    let result = match args_json {
        Some(args) => {
            let arguments = serde_json::from_str(args)
                .map_err(|e| McpError::InvalidJson { source: e })?;
            daemon.execute_tool(&server_name, &tool_name, arguments).await?
        }
        None => {
            // Read from stdin if not provided
            if io::stdin().is_terminal() {
                println!("No arguments provided. Pass JSON arguments as a command-line argument, or pipe JSON to stdin.");
                return Ok(());
            }

            let input = read_stdin_async()?;

            let arguments = serde_json::from_str(&input)
                .map_err(|e| McpError::InvalidJson { source: e })?;
            daemon.execute_tool(&server_name, &tool_name, arguments).await?
        }
    };

    // Format the result (EXEC-03)
    format_and_display_result(&result, server_name.as_str());

    Ok(())
}

/// Execute search tools command.
///
/// Search for tools using glob patterns.
/// Implements DISC-04: search of tools using glob patterns.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `pattern` - Glob pattern to search for (e.g., "*", "search*", "tool-*")
///
/// # Errors
/// Returns empty result if no tools match
pub async fn cmd_search_tools(mut daemon: Box<dyn crate::ipc::ProtocolClient>, pattern: &str) -> Result<()> {
    if daemon.config().is_empty() {
        println!("No servers configured. Please create a config file.");
        return Ok(());
    }

    println!("Searching for tools matching '{}':", pattern);

    let mut matches_found = false;

    for server in &daemon.config().servers {
        println!("Server: {} ({}):", server.name, server.transport.type_name());

        // Send ListServers request to daemon
        match daemon.list_servers().await {
            Ok(server_names) => {
                for server_name in &server_names {
                    println!("Server: {}:", server_name);

                    // Send ListTools request for each server
                    match daemon.list_tools(server_name).await {
                        Ok(tools) => {
                            // Match tool names against the glob pattern
                            let matched_tools: Vec<_> = tools.iter()
                                .filter(|tool| {
                                    let tool_name = &tool.name;
                                    // Use globset for pattern matching instead of glob
                                    match glob::Pattern::new(pattern) {
                                        Ok(pattern_obj) => pattern_obj.matches(tool_name),
                                        Err(_) => tool_name.contains(pattern),
                                    }
                                })
                                .collect();

                            if !matched_tools.is_empty() {
                                matches_found = true;
                                println!("  - {} tool(s):", matched_tools.len());
                                for tool in matched_tools {
                                    println!("      - {}: {}", tool.name, tool.description);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
    }

    if !matches_found {
        println!("No matching tools found.");
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
