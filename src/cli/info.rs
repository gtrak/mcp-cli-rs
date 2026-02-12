//! Show server and tool information command implementation.

use crate::cli::DetailLevel;
use crate::config::ServerTransport;
use crate::daemon::protocol::{ParameterDetail, ServerDetailOutput, ToolDetailOutput};
use crate::error::{McpError, Result};
use crate::format::{extract_params_from_schema, format_param_list, OutputMode};
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_info, print_json};
use colored::Colorize;

/// Execute server info command.
///
/// Displays detailed information about a specific server.
/// Implements DISC-02: inspection of server details.
/// Implements TASK-03: colored output for error cases.
/// Implements OUTP-07, OUTP-08: JSON output mode
///
/// # Arguments
/// * `daemon` - Daemon IPC client
/// * `server_name` - Name of the server to inspect
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::ServerNotFound if server doesn't exist (ERR-02)
pub async fn cmd_server_info(
    daemon: Box<dyn ProtocolClient>,
    server_name: &str,
    output_mode: OutputMode,
) -> Result<()> {
    // Handle JSON mode separately
    if output_mode == OutputMode::Json {
        return cmd_server_info_json(daemon, server_name).await;
    }
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

/// Execute the server info command in JSON mode.
///
/// Outputs server configuration details as structured JSON for programmatic use.
/// Implements OUTP-07: --json flag support
/// Implements OUTP-08: consistent JSON schema with complete server details
async fn cmd_server_info_json(daemon: Box<dyn ProtocolClient>, server_name: &str) -> Result<()> {
    let config = daemon.config();
    let server = config
        .get_server(server_name)
        .ok_or_else(|| McpError::ServerNotFound {
            server: server_name.to_string(),
        })?;

    // Build transport details based on type
    let transport_details = match &server.transport {
        ServerTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            let mut details = serde_json::json!({
                "type": "stdio",
                "command": command,
            });
            if !args.is_empty() {
                details["args"] = serde_json::Value::Array(
                    args.iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                );
            }
            if !env.is_empty() {
                let env_map: serde_json::Value = env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                details["env"] = env_map;
            }
            if let Some(cwd_path) = cwd {
                details["cwd"] = serde_json::Value::String(cwd_path.clone());
            }
            details
        }
        ServerTransport::Http { url, headers } => {
            let mut details = serde_json::json!({
                "type": "http",
                "url": url,
            });
            if !headers.is_empty() {
                let headers_map: serde_json::Value = headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                details["headers"] = headers_map;
            }
            details
        }
    };

    let output = ServerDetailOutput {
        name: server.name.clone(),
        description: server.description.clone(),
        transport_type: server.transport.type_name().to_string(),
        transport: transport_details,
    };

    print_json(&output);
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

    let _server =
        daemon
            .config()
            .get_server(&server_name)
            .ok_or_else(|| McpError::ToolNotFound {
                tool: tool_name.clone(),
                server: server_name.clone(),
            })?;

    // Send ListTools request to daemon
    let tools = daemon.list_tools(&server_name).await?;

    let tool =
        tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| McpError::ToolNotFound {
                tool: tool_name.clone(),
                server: server_name.clone(),
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

/// Parse a tool identifier from a string.
///
/// Supports both "server/tool" and "server tool" formats (CLI-05).
/// Implements ERR-06: handles ambiguous command prompts with suggestions.
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
