//! CLI commands for MCP CLI tool.

use crate::client::McpClient;
use crate::config::{Config, ServerConfig, ServerTransport};
use crate::error::{McpError, Result};
use crate::transport::Transport;
use std::io::{self, Read, IsTerminal};

/// Context for CLI operations.
///
/// Contains the loaded configuration and provides access to server configurations.
pub struct AppContext {
    pub config: Config,
}

impl AppContext {
    /// Create a new AppContext from a configuration.
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

/// Execute the list servers command.
///
/// Lists all configured MCP servers and their tool availability.
/// Implements DISC-01: discovery of available tools.
///
/// # Arguments
/// * `ctx` - Application context
/// * `with_descriptions` - If true, show tool descriptions (DISC-06)
///
/// # Errors
/// Returns McpError::ConfigParseError if config file is invalid
pub async fn cmd_list_servers(ctx: &AppContext, with_descriptions: bool) -> Result<()> {
    if ctx.config.is_empty() {
        println!("No servers configured. Please create a config file.");
        return Ok(());
    }

    println!("Configured servers:");
    println!();

    let _servers_by_name = ctx.config.servers_by_name();

    for server in &ctx.config.servers {
        println!("{}. {} ({})", server.name, server.description.as_deref().unwrap_or(""), server.transport.type_name());

        // Try to connect and list tools for each server
        let transport = create_transport_for_server(server).unwrap();
        let mut client = McpClient::new(server.name.clone(), transport);
        match client.list_tools().await {
            Ok(tools) => {
                println!("  - {} tool(s)", tools.len());
                if with_descriptions && !tools.is_empty() {
                    println!("    Tools:");
                    for tool in &tools {
                        println!("      - {}: {}", tool.name, tool.description.as_deref().unwrap_or(""));
                    }
                }
            }
            Err(e) => println!("  - Failed to list tools: {}", e),
        }
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
/// * `ctx` - Application context
/// * `server_name` - Name of the server to inspect
///
/// # Errors
/// Returns McpError::ServerNotFound if server doesn't exist (ERR-02)
pub async fn cmd_server_info(ctx: &AppContext, server_name: &str) -> Result<()> {
    let server = ctx.config.get_server(server_name)
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
/// * `ctx` - Application context
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
///
/// # Errors
/// Returns McpError::ToolNotFound if tool doesn't exist (ERR-02)
/// Returns McpError::AmbiguousCommand if tool_id format is unclear (ERR-06)
pub async fn cmd_tool_info(ctx: &AppContext, tool_id: &str) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let server = ctx.config.get_server(&server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.clone(),
        })?;

    let transport = create_transport_for_server(server)?;

    let mut client = McpClient::new(server_name.clone(), transport);
    let tools = client.list_tools().await?;

    let tool = tools.iter()
        .find(|t| t.name == tool_name)
        .ok_or_else(|| McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        })?;

    println!("Tool: {}", tool.name);
    if let Some(desc) = &tool.description {
        println!("Description: {}", desc);
    }
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
/// * `ctx` - Application context
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `args_json` - JSON arguments as a string, or None to read from stdin
///
/// # Errors
/// Returns McpError::InvalidProtocol for malformed response
/// Returns McpError::Timeout if timeout exceeded (EXEC-06)
pub async fn cmd_call_tool(ctx: &AppContext, tool_id: &str, args_json: Option<&str>) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let server = ctx.config.get_server(&server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.clone(),
        })?;

    let transport = create_transport_for_server(server)?;
    let mut client = McpClient::new(server_name.clone(), transport);

    // Parse arguments
    let arguments = match args_json {
        Some(args) => {
            serde_json::from_str(args)
                .map_err(|e| McpError::InvalidJson { source: e })?
        }
        None => {
            // Read from stdin if not provided
            if io::stdin().is_terminal() {
                println!("No arguments provided. Pass JSON arguments as a command-line argument, or pipe JSON to stdin.");
                return Ok(());
            }

            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| {
                    McpError::ConfigReadError {
                        path: std::path::PathBuf::from("stdin"),
                        source: e,
                    }
                })?;

            if input.trim().is_empty() {
                println!("Stdin is empty. Pass JSON arguments as a command-line argument or pipe JSON to stdin.");
                return Ok(());
            }

            serde_json::from_str(&input)
                .map_err(|e| McpError::InvalidJson { source: e })?
        }
    };

    // Execute the tool call with timeout (EXEC-06)
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1800), // 30 minute timeout from ROADMAP.md
        client.call_tool(&tool_name, arguments)
    ).await
        .map_err(|_| McpError::Timeout { timeout: 1800 })?
        .map_err(|e| {
            McpError::InvalidProtocol {
                message: format!("Server returned error: {}", e),
            }
        })?;

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
/// * `ctx` - Application context
/// * `pattern` - Glob pattern to search for (e.g., "*", "search*", "tool-*")
///
/// # Errors
/// Returns empty result if no tools match
pub async fn cmd_search_tools(ctx: &AppContext, pattern: &str) -> Result<()> {
    if ctx.config.is_empty() {
        println!("No servers configured. Please create a config file.");
        return Ok(());
    }

    println!("Searching for tools matching '{}':", pattern);

    let mut matches_found = false;

    for server in &ctx.config.servers {
        println!("Server: {} ({}):", server.name, server.transport.type_name());

        let transport = create_transport_for_server(server).unwrap();
        let mut client = McpClient::new(server.name.clone(), transport);
        match client.list_tools().await {
            Ok(tools) => {
                // Match tool names against the glob pattern
                let matched_tools: Vec<_> = tools.iter()
                    .filter(|tool| {
                        let tool_name = &tool.name;
                        // Use globset for pattern matching instead of glob
                        match glob::Pattern::new(pattern) {
                            Ok(pattern) => pattern.matches(tool_name),
                            Err(_) => tool_name.contains(pattern),
                        }
                    })
                    .collect();

                if !matched_tools.is_empty() {
                    matches_found = true;
                    println!("  - {} tool(s):", matched_tools.len());
                    for tool in matched_tools {
                        println!("      - {}: {}", tool.name, tool.description.as_deref().unwrap_or(""));
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
