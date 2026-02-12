//! List servers and tools command implementation.

use crate::cli::DetailLevel;
use crate::client::ToolInfo;
use crate::daemon::protocol::{ListOutput, ServerInfo};
use crate::error::Result;
use crate::format::{OutputMode, extract_params_from_schema, format_param_list};
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_json};
use crate::parallel::{list_tools_parallel, ParallelExecutor};
use colored::Colorize;
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
pub async fn cmd_list_servers_json(mut daemon: Box<dyn ProtocolClient>) -> Result<()> {
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
