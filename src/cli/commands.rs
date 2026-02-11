//! CLI commands for MCP CLI tool.

use crate::cli::DetailLevel;
use crate::client::ToolInfo;
use crate::config::{ServerConfig, ServerTransport};
use crate::daemon::protocol::{ListOutput, ParameterDetail, SearchOutput, SearchMatch, ServerInfo, ToolDetailOutput};
use crate::error::{McpError, Result};
use crate::format::{extract_params_from_schema, format_param_list, OutputMode};
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_info, print_json, print_warning};
use crate::parallel::{ParallelExecutor, list_tools_parallel};
use crate::retry::{RetryConfig, retry_with_backoff};
use crate::transport::Transport;
use colored::Colorize;
use futures_util::FutureExt;
use std::io::{self, IsTerminal, Read};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Execute the list servers command.
///
/// Lists all configured MCP servers and their tool availability with visual hierarchy.
/// Implements DISC-01: discovery of available tools.
/// Implements DISC-05: parallel server discovery with configurable concurrency.
/// Implements ERR-07: partial failure warnings.
/// Implements OUTP-01, OUTP-03, OUTP-04, OUTP-11, OUTP-12, OUTP-13, OUTP-15, OUTP-18
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `detail_level` - Level of detail for tool listings (ignored in JSON mode)
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::ConfigParseError if config file is invalid
pub async fn cmd_list_servers(
    mut daemon: Box<dyn ProtocolClient>,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    // Handle JSON mode separately
    if output_mode == OutputMode::Json {
        return cmd_list_servers_json(daemon).await;
    }
    let config = daemon.config();

    // Empty state handling (OUTP-15)
    if config.is_empty() {
        println!("{}", "No servers configured".bold());
        println!("{}", "─".repeat(50).dimmed());
        println!();
        println!("To get started, create a configuration file:");
        println!();
        println!("  {}", "mcp_servers.toml".cyan());
        println!();
        println!("Example configuration:");
        println!(
            "{}",
            r#"
[[servers]]
name = "filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/files"]
"#
            .dimmed()
        );
        return Ok(());
    }

    // Get server names from daemon
    let server_names = daemon.list_servers().await.map_err(|e| {
        print_error(&format!("Failed to get servers list: {}", e));
        e
    })?;

    // Create parallel executor with concurrency limit from config
    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Create daemon client for parallel execution
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel with filtering
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard
                        .list_tools(&server)
                        .await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| crate::client::ToolInfo {
                                    name: protocol_tool.name,
                                    description: Some(protocol_tool.description),
                                    input_schema: protocol_tool.input_schema,
                                })
                                .collect()
                        })
                }
            },
            &executor,
            config.as_ref(),
        )
        .await?
    };

    // Header (OUTP-03, OUTP-04)
    let connected_count = successes.len();
    let failed_count = failures.len();
    println!(
        "{} {}",
        "MCP Servers".bold(),
        format!("({} connected, {} failed)", connected_count, failed_count).dimmed()
    );
    println!("{}", "─".repeat(50).dimmed());
    println!();

    // Display successful results with visual hierarchy
    for (server_name, tools) in &successes {
        // Get server info from config
        let server_config = {
            let daemon_guard = daemon_arc.lock().await;
            daemon_guard.config().get_server(server_name).cloned()
        };

        // Server header with status indicator (OUTP-13)
        let has_filtered_tools = server_config
            .as_ref()
            .map(|s| s.disabled_tools.as_ref().is_some_and(|d| !d.is_empty()))
            .unwrap_or(false);

        let status_icon = if has_filtered_tools {
            "⚠".yellow()
        } else {
            "✓".green()
        };

        if let Some(ref server_config) = server_config {
            println!(
                "{} {} {}",
                status_icon,
                server_name.bold(),
                format!("({})", server_config.transport.type_name()).dimmed()
            );
        } else {
            println!(
                "{} {} {}",
                status_icon,
                server_name.bold(),
                "(unknown)".dimmed()
            );
        }
        println!("{}", "═".repeat(50).dimmed());

        // Server description (OUTP-11)
        if let Some(ref server_config) = server_config {
            let desc = server_config
                .description
                .as_deref()
                .unwrap_or("No description");
            println!("Description: {}", desc);
        }
        println!("Tools: {}", tools.len());
        println!();

        // Tool listings based on detail level (OUTP-01, OUTP-02)
        if !tools.is_empty() {
            match detail_level {
                DetailLevel::Summary => {
                    // Default view: tool name, description, parameter overview
                    for tool in tools {
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        let truncated_desc = if desc.len() > 60 {
                            format!("{}...", &desc[..57])
                        } else {
                            desc.to_string()
                        };

                        // Parameter overview
                        let params = extract_params_from_schema(&tool.input_schema);
                        let param_str = format_param_list(&params, detail_level);

                        println!("  • {}: {}", tool.name.bold(), truncated_desc);
                        println!("    Usage: {} {}", tool.name, param_str.dimmed());
                    }
                    println!();
                    println!(
                        "{}",
                        "Use 'mcp info <server>/<tool>' for full schema".dimmed()
                    );
                }
                DetailLevel::WithDescriptions => {
                    // Detailed view: full descriptions and parameter details
                    for tool in tools {
                        println!("  {}", tool.name.bold());
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        println!("    Description: {}", desc);

                        let params = extract_params_from_schema(&tool.input_schema);
                        if !params.is_empty() {
                            println!("    Parameters:");
                            for param in &params {
                                let type_str = if param.required {
                                    format!("<{}>", param.param_type)
                                } else {
                                    format!("[{}]", param.param_type)
                                };
                                let req_str = if param.required {
                                    "Required"
                                } else {
                                    "Optional"
                                };

                                if let Some(ref param_desc) = param.description {
                                    println!(
                                        "      {} {}  {}. {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed(),
                                        param_desc
                                    );
                                } else {
                                    println!(
                                        "      {} {}  {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed()
                                    );
                                }
                            }
                        }
                        println!();
                    }
                }
                DetailLevel::Verbose => {
                    // Verbose view: everything plus full schema
                    for tool in tools {
                        println!("  {}", tool.name.bold());
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        println!("    Description: {}", desc);

                        let params = extract_params_from_schema(&tool.input_schema);
                        if !params.is_empty() {
                            println!("    Parameters:");
                            for param in &params {
                                let type_str = if param.required {
                                    format!("<{}>", param.param_type)
                                } else {
                                    format!("[{}]", param.param_type)
                                };
                                let req_str = if param.required {
                                    "Required"
                                } else {
                                    "Optional"
                                };

                                if let Some(ref param_desc) = param.description {
                                    println!(
                                        "      {} {}  {}. {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed(),
                                        param_desc
                                    );
                                } else {
                                    println!(
                                        "      {} {}  {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed()
                                    );
                                }
                            }
                        }
                        println!("    Schema:");
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&tool.input_schema)
                                .unwrap_or_default()
                                .dimmed()
                        );
                        println!();
                    }
                }
            }
        } else {
            // Empty state for no tools on server (OUTP-15)
            println!("  {}", "No tools available on this server".dimmed());
        }

        println!();
    }

    // Partial failure reporting (OUTP-18)
    if !failures.is_empty() {
        println!(
            "{} {}",
            "⚠".yellow(),
            format!("Connection Issues ({} servers)", failures.len()).bold()
        );
        println!("{}", "─".repeat(50).dimmed());
        for server_name in &failures {
            println!(
                "  {} {}: {}",
                "✗".red(),
                server_name,
                "Connection failed".dimmed()
            );
        }
        println!();
    }

    // Filter warning
    let has_disabled_tools = config
        .servers
        .iter()
        .any(|s| s.disabled_tools.as_ref().is_some_and(|d| !d.is_empty()));
    let has_allowed_tools = config
        .servers
        .iter()
        .any(|s| s.allowed_tools.as_ref().is_some_and(|a| !a.is_empty()));

    if has_disabled_tools && !has_allowed_tools {
        println!(
            "{} {}",
            "⚠".yellow(),
            "Note: Some tools are disabled by configuration".dimmed()
        );
        println!();
    }

    Ok(())
}

/// Execute the list servers command in JSON mode.
///
/// Outputs all servers and tools as structured JSON for programmatic use.
/// Implements OUTP-07: --json flag support
/// Implements OUTP-08: consistent JSON schema with complete tool metadata
async fn cmd_list_servers_json(mut daemon: Box<dyn ProtocolClient>) -> Result<()> {
    let config = daemon.config();

    // Handle empty config
    if config.is_empty() {
        let output = ListOutput {
            servers: vec![],
            total_servers: 0,
            connected_servers: 0,
            failed_servers: 0,
            total_tools: 0,
        };
        print_json(&output);
        return Ok(());
    }

    // Get server names from daemon
    let server_names = daemon.list_servers().await.map_err(|e| {
        print_error(&format!("Failed to get servers list: {}", e));
        e
    })?;

    // Create parallel executor with concurrency limit from config
    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Create daemon client for parallel execution
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard
                        .list_tools(&server)
                        .await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| crate::client::ToolInfo {
                                    name: protocol_tool.name,
                                    description: Some(protocol_tool.description),
                                    input_schema: protocol_tool.input_schema,
                                })
                                .collect()
                        })
                }
            },
            &executor,
            config.as_ref(),
        )
        .await?
    };

    // Build JSON output
    let mut servers = Vec::new();
    let mut total_tools = 0;

    // Process successful servers
    for (server_name, tools) in successes {
        let _server_config = {
            let daemon_guard = daemon_arc.lock().await;
            daemon_guard.config().get_server(&server_name).cloned()
        };

        // Convert client ToolInfo to protocol ToolInfo for serialization
        let protocol_tools: Vec<crate::daemon::protocol::ToolInfo> = tools
            .into_iter()
            .map(|t| crate::daemon::protocol::ToolInfo {
                name: t.name,
                description: t.description.unwrap_or_default(),
                input_schema: t.input_schema,
            })
            .collect();

        let server_info = ServerInfo {
            name: server_name.clone(),
            status: "connected".to_string(),
            tool_count: protocol_tools.len(),
            tools: protocol_tools,
            error: None,
        };

        total_tools += server_info.tool_count;
        servers.push(server_info);
    }

    // Process failed servers
    for server_name in failures {
        let server_info = ServerInfo {
            name: server_name,
            status: "failed".to_string(),
            tool_count: 0,
            tools: vec![],
            error: Some("Connection failed".to_string()),
        };
        servers.push(server_info);
    }

    let output = ListOutput {
        total_servers: servers.len(),
        connected_servers: servers.iter().filter(|s| s.status == "connected").count(),
        failed_servers: servers.iter().filter(|s| s.status == "failed").count(),
        total_tools,
        servers,
    };

    print_json(&output);
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
    let server = config.get_server(server_name).ok_or_else(|| {
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
        ServerTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
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
/// Implements OUTP-05, OUTP-11, OUTP-14: consistent formatting and descriptions
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `tool_id` - Tool identifier in format "server/tool" or "server tool"
/// * `detail_level` - Level of detail for display (ignored in JSON mode)
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::ToolNotFound if tool doesn't exist (ERR-02)
/// Returns McpError::AmbiguousCommand if tool_id format is unclear (ERR-06)
pub async fn cmd_tool_info(
    mut daemon: Box<dyn ProtocolClient>,
    tool_id: &str,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    // Handle JSON mode separately
    if output_mode == OutputMode::Json {
        return cmd_tool_info_json(daemon, tool_id).await;
    }
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let _server = daemon.config().get_server(&server_name).ok_or_else(|| {
        print_error(&format!(
            "Tool '{}' not found on server '{}'",
            tool_name, server_name
        ));
        McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        }
    })?;

    // Send ListTools request to daemon
    let tools = daemon.list_tools(&server_name).await?;

    let tool = tools.iter().find(|t| t.name == tool_name).ok_or_else(|| {
        print_error(&format!(
            "Tool '{}' not found on server '{}'",
            tool_name, server_name
        ));
        McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        }
    })?;

    // Get server config for transport info
    let config = daemon.config();
    let server_config = config.get_server(&server_name);
    let transport_name = server_config
        .map(|s| s.transport.type_name())
        .unwrap_or("unknown");

    // Header with visual hierarchy
    println!("{} {}", "Tool:".bold(), tool.name.bold());
    println!("{}", "═".repeat(50).dimmed());
    println!(
        "{} {} {}",
        "Server:".bold(),
        server_name,
        format!("({})", transport_name).dimmed()
    );
    println!();

    // Description (OUTP-11)
    let description = if tool.description.is_empty() {
        "No description available"
    } else {
        &tool.description
    };
    println!("{} {}", "Description:".bold(), description);
    println!();

    // Extract parameters from schema
    let params = extract_params_from_schema(&tool.input_schema);

    // Format based on detail level (OUTP-02)
    match detail_level {
        DetailLevel::Summary => {
            // Parameter overview
            if params.is_empty() {
                println!("{}", "This tool takes no parameters".dimmed());
            } else {
                let param_str = format_param_list(&params, detail_level);
                println!("{} {}", "Parameters:".bold(), param_str);
            }
            println!();
            println!(
                "{}",
                format!("Usage: mcp call {}/{} [args]", server_name, tool_name).dimmed()
            );
            println!(
                "{}",
                "Use -d for parameter details, -v for full schema".dimmed()
            );
        }
        DetailLevel::WithDescriptions => {
            // Detailed parameter list
            if params.is_empty() {
                println!("{}", "Parameters: none".dimmed());
            } else {
                println!("{}", "Parameters:".bold());
                for param in &params {
                    let type_str = if param.required {
                        format!("<{}>", param.param_type)
                    } else {
                        format!("[{}]", param.param_type)
                    };
                    let req_str = if param.required {
                        "Required"
                    } else {
                        "Optional"
                    };

                    if let Some(ref param_desc) = param.description {
                        println!(
                            "  {} {}  {}. {}",
                            param.name.cyan(),
                            type_str.dimmed(),
                            req_str.dimmed(),
                            param_desc
                        );
                    } else {
                        println!(
                            "  {} {}  {}",
                            param.name.cyan(),
                            type_str.dimmed(),
                            req_str.dimmed()
                        );
                    }
                }
            }
            println!();
            println!(
                "{} {}",
                "Usage:".bold(),
                format_args!("mcp call {}/{} '{{...}}'", server_name, tool_name)
            );
        }
        DetailLevel::Verbose => {
            // Full details including schema
            if params.is_empty() {
                println!("{}", "Parameters: none".dimmed());
            } else {
                println!("{}", "Parameters:".bold());
                for param in &params {
                    let type_str = if param.required {
                        format!("<{}>", param.param_type)
                    } else {
                        format!("[{}]", param.param_type)
                    };
                    let req_str = if param.required {
                        "Required"
                    } else {
                        "Optional"
                    };

                    if let Some(ref param_desc) = param.description {
                        println!(
                            "  {} {}  {}. {}",
                            param.name.cyan(),
                            type_str.dimmed(),
                            req_str.dimmed(),
                            param_desc
                        );
                    } else {
                        println!(
                            "  {} {}  {}",
                            param.name.cyan(),
                            type_str.dimmed(),
                            req_str.dimmed()
                        );
                    }
                }
            }
            println!();
            println!("{}", "JSON Schema:".bold());
            println!(
                "{}",
                serde_json::to_string_pretty(&tool.input_schema)
                    .unwrap_or_default()
                    .dimmed()
            );
            println!();
            println!(
                "{} {}",
                "Usage:".bold(),
                format_args!("mcp call {}/{} '{{...}}'", server_name, tool_name)
            );
        }
    }

    Ok(())
}

/// Execute the tool info command in JSON mode.
///
/// Outputs complete tool information including schema as structured JSON.
/// Implements OUTP-07: --json flag support
/// Implements OUTP-08: consistent JSON schema with complete tool metadata
async fn cmd_tool_info_json(mut daemon: Box<dyn ProtocolClient>, tool_id: &str) -> Result<()> {
    let (server_name, tool_name) = parse_tool_id(tool_id)?;

    let _server = daemon.config().get_server(&server_name).ok_or_else(|| {
        McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        }
    })?;

    // Send ListTools request to daemon
    let tools = daemon.list_tools(&server_name).await?;

    let tool = tools.iter().find(|t| t.name == tool_name).ok_or_else(|| {
        McpError::ToolNotFound {
            tool: tool_name.clone(),
            server: server_name.clone(),
        }
    })?;

    // Get server config for transport info
    let config = daemon.config();
    let server_config = config.get_server(&server_name);
    let transport_name = server_config
        .map(|s| s.transport.type_name())
        .unwrap_or("unknown");

    // Extract parameters from schema
    let params = extract_params_from_schema(&tool.input_schema);
    let parameters: Vec<ParameterDetail> = params
        .into_iter()
        .map(|param| ParameterDetail {
            name: param.name,
            param_type: param.param_type,
            required: param.required,
            description: param.description,
        })
        .collect();

    let output = ToolDetailOutput {
        name: tool_name.clone(),
        description: tool.description.clone(),
        server: server_name.clone(),
        transport: transport_name.to_string(),
        parameters,
        input_schema: tool.input_schema.clone(),
    };

    print_json(&output);
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
/// Execute the call tool command.
///
/// Calls a tool on a configured MCP server with JSON arguments.
/// Implements EXEC-01: tool execution with inline or stdin input.
/// Implements EXEC-02: argument passing via command-line argument or stdin pipe.
/// Implements FILT-04: disabledTools blocking with clear error messages.
/// Implements FILT-03: disabledTools precedence over allowedTools when both defined.
/// Implements EXEC-05: retry logic with exponential backoff for transient errors.
/// Implements EXEC-07: operation timeout enforcement.
/// Implements EXEC-06: timeout enforcement for overall operations.
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `tool_id` - Server tool identifier in format "server.tool" or "server/tool"
/// * `args_json` - JSON arguments as command-line argument or None for stdin pipe
///
/// # Errors
/// Returns McpError::ServerNotFound if server not configured
/// Returns McpError::UsageError if tool is disabled (FILT-04)
/// Returns McpError::InvalidJson if arguments parsing fails
/// Returns error on tool execution failure
pub async fn cmd_call_tool(
    mut daemon: Box<dyn ProtocolClient>,
    tool_id: &str,
    args_json: Option<&str>,
) -> Result<()> {
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
                print_info(&format!(
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
            print_info(&format!(
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

/// Execute search tools command.
///
/// Search for tools using glob patterns with parallel server discovery.
/// Implements DISC-04: search of tools using glob patterns.
/// Implements OUTP-14: context-rich search results
/// Implements OUTP-05: consistent formatting
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `pattern` - Glob pattern to search for (e.g., "*", "search*", "tool-*")
/// * `detail_level` - Level of detail for display (ignored in JSON mode)
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns empty result if no tools match
pub async fn cmd_search_tools(
    mut daemon: Box<dyn ProtocolClient>,
    pattern: &str,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    // Handle JSON mode separately
    if output_mode == OutputMode::Json {
        return cmd_search_tools_json(daemon, pattern).await;
    }
    let config = daemon.config();

    // Empty pattern handling
    if pattern.trim().is_empty() {
        println!("{}", "No search pattern provided".bold());
        println!();
        println!("Usage: mcp grep <pattern>");
        println!("Examples:");
        println!("  mcp grep 'read*'      # Tools starting with 'read'");
        println!("  mcp grep '*file*'     # Tools containing 'file'");
        println!("  mcp grep '*'          # All tools");
        return Ok(());
    }

    if config.is_empty() {
        println!("{}", "No servers configured".bold());
        println!("{}", "─".repeat(50).dimmed());
        println!();
        println!("To get started, create a configuration file:");
        println!();
        println!("  {}", "mcp_servers.toml".cyan());
        return Ok(());
    }

    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Get server names from daemon
    let server_names = daemon.list_servers().await.map_err(|e| {
        print_error(&format!("Failed to get servers list: {}", e));
        e
    })?;

    // Create daemon client for parallel execution
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel with filtering
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard
                        .list_tools(&server)
                        .await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| crate::client::ToolInfo {
                                    name: protocol_tool.name,
                                    description: Some(protocol_tool.description),
                                    input_schema: protocol_tool.input_schema,
                                })
                                .collect()
                        })
                }
            },
            &executor,
            config.as_ref(),
        )
        .await?
    };

    // Parse glob pattern
    let pattern_obj = match glob::Pattern::new(pattern) {
        Ok(p) => p,
        Err(_) => {
            print_warning(&format!(
                "Invalid glob pattern '{}' - using substring matching",
                pattern
            ));
            glob::Pattern::new("*").unwrap()
        }
    };

    // Search header
    println!(
        "{} {}",
        "Search Results for".bold(),
        format!("'{}'", pattern).cyan()
    );
    println!("{}", "═".repeat(50).dimmed());

    // Track matches across servers
    let mut total_matches = 0;
    let mut servers_with_matches = 0;

    // Display matching tools by server
    for (server_name, tools) in &successes {
        // Get server config for transport info
        let server_config = {
            let daemon_guard = daemon_arc.lock().await;
            daemon_guard.config().get_server(server_name).cloned()
        };

        // Filter matching tools
        let matched_tools: Vec<_> = tools
            .iter()
            .filter(|tool| pattern_obj.matches(&tool.name))
            .collect();

        if !matched_tools.is_empty() {
            servers_with_matches += 1;
            total_matches += matched_tools.len();

            // Server header with context
            let transport_name = server_config
                .as_ref()
                .map(|s| s.transport.type_name())
                .unwrap_or("unknown");

            if servers_with_matches > 1 {
                println!();
            }

            println!(
                "{} {} {}",
                server_name.bold(),
                format!("({})", transport_name).dimmed(),
                format!("- {} tool(s)", matched_tools.len()).dimmed()
            );
            println!("{}", "─".repeat(50).dimmed());

            // Display tools based on detail level
            for tool in matched_tools {
                match detail_level {
                    DetailLevel::Summary => {
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        let truncated_desc = if desc.len() > 60 {
                            format!("{}...", &desc[..57])
                        } else {
                            desc.to_string()
                        };

                        let params = extract_params_from_schema(&tool.input_schema);
                        let param_str = format_param_list(&params, detail_level);

                        println!("  • {}: {}", tool.name.bold(), truncated_desc);
                        println!("    Usage: {} {}", tool.name, param_str.dimmed());
                    }
                    DetailLevel::WithDescriptions => {
                        println!("  {}", tool.name.bold());
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        println!("    Description: {}", desc);

                        let params = extract_params_from_schema(&tool.input_schema);
                        if !params.is_empty() {
                            println!("    Parameters:");
                            for param in &params {
                                let type_str = if param.required {
                                    format!("<{}>", param.param_type)
                                } else {
                                    format!("[{}]", param.param_type)
                                };
                                let req_str = if param.required {
                                    "Required"
                                } else {
                                    "Optional"
                                };

                                if let Some(ref param_desc) = param.description {
                                    println!(
                                        "      {} {}  {}. {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed(),
                                        param_desc
                                    );
                                } else {
                                    println!(
                                        "      {} {}  {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed()
                                    );
                                }
                            }
                        }
                        println!();
                    }
                    DetailLevel::Verbose => {
                        println!("  {}", tool.name.bold());
                        let desc = tool.description.as_deref().unwrap_or("No description");
                        println!("    Description: {}", desc);

                        let params = extract_params_from_schema(&tool.input_schema);
                        if !params.is_empty() {
                            println!("    Parameters:");
                            for param in &params {
                                let type_str = if param.required {
                                    format!("<{}>", param.param_type)
                                } else {
                                    format!("[{}]", param.param_type)
                                };
                                let req_str = if param.required {
                                    "Required"
                                } else {
                                    "Optional"
                                };

                                if let Some(ref param_desc) = param.description {
                                    println!(
                                        "      {} {}  {}. {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed(),
                                        param_desc
                                    );
                                } else {
                                    println!(
                                        "      {} {}  {}",
                                        param.name.cyan(),
                                        type_str.dimmed(),
                                        req_str.dimmed()
                                    );
                                }
                            }
                        }
                        println!(
                            "    Schema: {}",
                            serde_json::to_string(&tool.input_schema)
                                .unwrap_or_default()
                                .dimmed()
                        );
                        println!();
                    }
                }
            }
        }
    }

    // Summary footer
    println!();
    println!("{}", "─".repeat(50).dimmed());
    if total_matches == 0 {
        println!(
            "{} {}",
            "✗".red(),
            format_args!("No tools matching '{}' found", pattern)
        );
        println!();
        println!("Suggestions:");
        println!("  • Try a broader pattern (e.g., '*' for all tools)");
        println!("  • Use wildcards: 'read*' for tools starting with 'read'");
        println!("  • Use '*file*' for tools containing 'file'");
    } else {
        println!(
            "Found {} matching tool(s) across {} server(s)",
            total_matches.to_string().bold(),
            servers_with_matches.to_string().bold()
        );
        println!();
        println!(
            "{}",
            "Use 'mcp info <server>/<tool>' for detailed information"
                .to_string()
                .dimmed()
        );
    }

    // Partial failure reporting
    if !failures.is_empty() {
        println!();
        println!(
            "{} {}",
            "⚠".yellow(),
            format!("Search limited - {} server(s) unavailable", failures.len()).dimmed()
        );
    }

    Ok(())
}

/// Execute the search tools command in JSON mode.
///
/// Outputs search results as structured JSON for programmatic use.
/// Implements OUTP-07: --json flag support
/// Implements OUTP-08: consistent JSON schema with complete tool metadata
async fn cmd_search_tools_json(mut daemon: Box<dyn ProtocolClient>, pattern: &str) -> Result<()> {
    let config = daemon.config();

    // Handle empty pattern or config
    if pattern.trim().is_empty() || config.is_empty() {
        let output = SearchOutput {
            pattern: pattern.to_string(),
            total_matches: 0,
            match_count: 0,
            matches: vec![],
            failed_servers: vec![],
        };
        print_json(&output);
        return Ok(());
    }

    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Get server names from daemon
    let server_names = daemon.list_servers().await.map_err(|e| {
        print_error(&format!("Failed to get servers list: {}", e));
        e
    })?;

    // Create daemon client for parallel execution
    let daemon_arc = Arc::new(Mutex::new(daemon));

    // List tools from all servers in parallel
    let (successes, failures): (Vec<(String, Vec<ToolInfo>)>, Vec<String>) = {
        list_tools_parallel(
            server_names,
            |server| {
                let daemon_arc = daemon_arc.clone();
                async move {
                    let mut daemon_guard = daemon_arc.lock().await;
                    daemon_guard
                        .list_tools(&server)
                        .await
                        .map_err(|e| {
                            tracing::warn!("Failed to list tools for {}: {}", server, e);
                            e
                        })
                        .map(|protocol_tools| {
                            protocol_tools
                                .into_iter()
                                .map(|protocol_tool| crate::client::ToolInfo {
                                    name: protocol_tool.name,
                                    description: Some(protocol_tool.description),
                                    input_schema: protocol_tool.input_schema,
                                })
                                .collect()
                        })
                }
            },
            &executor,
            config.as_ref(),
        )
        .await?
    };

    // Parse glob pattern
    let pattern_obj = match glob::Pattern::new(pattern) {
        Ok(p) => p,
        Err(_) => {
            glob::Pattern::new("*").unwrap()
        }
    };

    // Search for matching tools across all servers
    let mut matches = Vec::new();
    for (server_name, tools) in successes {
        for tool in tools {
            if pattern_obj.matches(&tool.name) {
                matches.push(SearchMatch {
                    server: server_name.clone(),
                    name: tool.name,
                    description: tool.description.unwrap_or_default(),
                });
            }
        }
    }

    let output = SearchOutput {
        pattern: pattern.to_string(),
        total_matches: matches.len(),
        match_count: matches.len(),
        matches,
        failed_servers: failures,
    };

    print_json(&output);
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
        hint: format!(
            "Tool identifier '{}' is ambiguous. Use format 'server/tool' or 'server tool'.",
            tool_id
        ),
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
pub fn create_transport_for_server(
    server: &ServerConfig,
) -> std::result::Result<Box<dyn Transport + Send + Sync>, McpError> {
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
        assert!(matches!(
            result.unwrap_err(),
            McpError::AmbiguousCommand { .. }
        ));
    }
}
