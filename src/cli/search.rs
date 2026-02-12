//! Search tools by name pattern command implementation.

use crate::client::ToolInfo;
use crate::daemon::protocol::{SearchMatch, SearchOutput};
use crate::error::Result;
use crate::format::{extract_params_from_schema, format_param_list, DetailLevel, OutputMode};
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_json, print_warning};
use crate::parallel::{list_tools_parallel, ParallelExecutor};
use colored::Colorize;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        Err(_) => glob::Pattern::new("*").unwrap(),
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
