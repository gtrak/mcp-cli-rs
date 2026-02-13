//! Output formatters for CLI command results.
//!
//! This module provides formatting functions that convert model data into
//! human-readable or JSON output. Each formatter handles both output modes,
//! ensuring consistent presentation across all CLI commands.
//!
//! # Architecture
//!
//! Commands populate models (from `crate::cli::models`) and pass them to these
//! formatters along with an `OutputMode`. The formatter handles presentation
//! independently of data collection.

use crate::cli::models::*;
use crate::format::{extract_params_from_schema, format_param_list, DetailLevel};
use crate::output::print_json;
use colored::Colorize;

/// Format list servers output.
///
/// Displays server list with visual hierarchy and tool information.
/// Matches the output format from list.rs cmd_list_servers.
pub fn format_list_servers(
    model: &ListServersModel,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) {
    match output_mode {
        OutputMode::Human => format_list_servers_human(model, detail_level),
        OutputMode::Json => print_json(model),
    }
}

/// Format list servers for human-readable output.
fn format_list_servers_human(model: &ListServersModel, detail_level: DetailLevel) {
    // Handle empty state (OUTP-15)
    if model.total_servers == 0 {
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
        return;
    }

    // Header (OUTP-03, OUTP-04)
    println!(
        "{} {}",
        "MCP Servers".bold(),
        format!(
            "({} connected, {} failed)",
            model.connected_servers, model.failed_servers
        )
        .dimmed()
    );
    println!("{}", "─".repeat(50).dimmed());
    println!();

    // Display servers with visual hierarchy
    for server in &model.servers {
        // Skip failed servers in the main listing (they go in partial failure section)
        if server.status == "failed" {
            continue;
        }

        // Server header with status indicator (OUTP-13)
        let status_icon = if server.has_filtered_tools {
            "⚠".yellow()
        } else {
            "✓".green()
        };

        let transport_name = server.transport_type.as_deref().unwrap_or("unknown");

        println!(
            "{} {} {}",
            status_icon,
            server.name.bold(),
            format!("({})", transport_name).dimmed()
        );
        println!("{}", "═".repeat(50).dimmed());

        // Server description (OUTP-11)
        if let Some(ref desc) = server.description {
            println!("Description: {}", desc);
        }
        println!("Tools: {}", server.tool_count);
        println!();

        // Tool listings based on detail level (OUTP-01, OUTP-02)
        if !server.tools.is_empty() {
            match detail_level {
                DetailLevel::Summary => {
                    // Default view: tool name, description, parameter overview
                    for tool in &server.tools {
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
                    for tool in &server.tools {
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
                    for tool in &server.tools {
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
    let failed_servers: Vec<_> = model
        .servers
        .iter()
        .filter(|s| s.status == "failed")
        .collect();

    if !failed_servers.is_empty() {
        println!(
            "{} {}",
            "⚠".yellow(),
            format!("Connection Issues ({} servers)", failed_servers.len()).bold()
        );
        println!("{}", "─".repeat(50).dimmed());
        for server in &failed_servers {
            println!(
                "  {} {}: {}",
                "✗".red(),
                server.name,
                server
                    .error
                    .as_deref()
                    .unwrap_or("Connection failed")
                    .dimmed()
            );
        }
        println!();
    }
}

/// Format server info output.
///
/// Displays detailed information about a specific server.
/// Matches the output format from info.rs cmd_server_info.
pub fn format_server_info(model: &ServerInfoModel, output_mode: OutputMode) {
    match output_mode {
        OutputMode::Human => format_server_info_human(model),
        OutputMode::Json => print_json(model),
    }
}

/// Format server info for human-readable output.
fn format_server_info_human(model: &ServerInfoModel) {
    println!("{} {}", "Server:".bold(), model.name);
    if let Some(ref desc) = model.description {
        println!("Description: {}", desc);
    }
    println!("Transport: {}", model.transport_type);

    // Display transport details based on type
    if let Some(obj) = model.transport_detail.as_object() {
        // Stdio transport details
        if let Some(cmd) = obj.get("command").and_then(|v| v.as_str()) {
            println!("Command: {}", cmd);
        }
        if let Some(args) = obj.get("args").and_then(|v| v.as_array())
            && !args.is_empty()
        {
            let args_str: Vec<String> = args
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            println!("Arguments: {:?}", args_str);
        }
        if let Some(env) = obj.get("env").and_then(|v| v.as_object())
            && !env.is_empty()
        {
            println!("Environment:");
            for (key, value) in env {
                println!("  {}={}", key, value.as_str().unwrap_or_default());
            }
        }
        if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
            println!("Working directory: {}", cwd);
        }

        // HTTP transport details
        if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
            println!("URL: {}", url);
        }
        if let Some(headers) = obj.get("headers").and_then(|v| v.as_object())
            && !headers.is_empty()
        {
            println!("Headers:");
            for (key, value) in headers {
                println!("  {}: {}", key, value.as_str().unwrap_or_default());
            }
        }
    }
}

/// Format tool info output.
///
/// Displays detailed information about a specific tool including its JSON Schema.
/// Matches the output format from info.rs cmd_tool_info.
pub fn format_tool_info(model: &ToolInfoModel, detail_level: DetailLevel, output_mode: OutputMode) {
    match output_mode {
        OutputMode::Human => format_tool_info_human(model, detail_level),
        OutputMode::Json => print_json(model),
    }
}

/// Format tool info for human-readable output.
fn format_tool_info_human(model: &ToolInfoModel, detail_level: DetailLevel) {
    // Header with visual hierarchy
    println!("{} {}", "Tool:".bold(), model.tool_name.bold());
    println!("{}", "═".repeat(50).dimmed());
    println!(
        "{} {} {}",
        "Server:".bold(),
        model.server_name,
        "(server)".dimmed()
    );
    println!();

    // Description (OUTP-11)
    let description = model
        .description
        .as_deref()
        .unwrap_or("No description available");
    println!("{} {}", "Description:".bold(), description);
    println!();

    // Format based on detail level (OUTP-02)
    match detail_level {
        DetailLevel::Summary => {
            // Parameter overview
            if model.parameters.is_empty() {
                println!("{}", "This tool takes no parameters".dimmed());
            } else {
                let params: Vec<crate::format::ParameterInfo> =
                    model.parameters.iter().map(|p| p.into()).collect();
                let param_str = format_param_list(&params, detail_level);
                println!("{} {}", "Parameters:".bold(), param_str);
            }
            println!();
            println!(
                "{}",
                format!("Usage: mcp call {}/{}", model.server_name, model.tool_name).dimmed()
            );
            println!(
                "{}",
                "Use -d for parameter details, -v for full schema".dimmed()
            );
        }
        DetailLevel::WithDescriptions => {
            // Detailed parameter list
            if model.parameters.is_empty() {
                println!("{}", "Parameters: none".dimmed());
            } else {
                println!("{}", "Parameters:".bold());
                for param in &model.parameters {
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
                format_args!(
                    "mcp call {}/{} '{{...}}'",
                    model.server_name, model.tool_name
                )
            );
        }
        DetailLevel::Verbose => {
            // Full details including schema
            if model.parameters.is_empty() {
                println!("{}", "Parameters: none".dimmed());
            } else {
                println!("{}", "Parameters:".bold());
                for param in &model.parameters {
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
                serde_json::to_string_pretty(&model.input_schema)
                    .unwrap_or_default()
                    .dimmed()
            );
            println!();
            println!(
                "{} {}",
                "Usage:".bold(),
                format_args!(
                    "mcp call {}/{} '{{...}}'",
                    model.server_name, model.tool_name
                )
            );
        }
    }
}

/// Format tool call result output.
///
/// Displays tool execution results with success/error formatting.
/// Matches the output format from call.rs format_and_display_result.
pub fn format_call_result(model: &CallResultModel, output_mode: OutputMode) {
    match output_mode {
        OutputMode::Human => format_call_result_human(model),
        OutputMode::Json => print_json(model),
    }
}

/// Format call result for human-readable output.
fn format_call_result_human(model: &CallResultModel) {
    if model.success {
        // Format successful result
        if let Some(ref result) = model.result {
            if result.is_object() {
                let result_obj = result.as_object().unwrap();

                // Check if result indicates an error
                if let Some(error) = result_obj.get("error") {
                    println!("Error from server '{}':", model.server_name);
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
                                    "image" => item_obj.get("data")?.as_str().map(|d| {
                                        format!("(image data: {} bytes, type: unknown)", d.len())
                                    }),
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
                            serde_json::to_string_pretty(content)
                                .unwrap_or_else(|_| result.to_string())
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

        println!();
        println!(
            "{} Tool '{}' executed successfully on server '{}'",
            "✓".green(),
            model.tool_name,
            model.server_name
        );
    } else if let Some(ref error) = model.error {
        // Format error result
        println!("{} {}", "✗".red(), "Tool execution failed".bold());
        println!("  Error: {}", error);
    }
}

/// Format search results output.
///
/// Displays search results with context-rich information.
/// Matches the output format from search.rs cmd_search_tools.
pub fn format_search_results(
    model: &SearchResultModel,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) {
    match output_mode {
        OutputMode::Human => format_search_results_human(model, detail_level),
        OutputMode::Json => print_json(model),
    }
}

/// Format search results for human-readable output.
fn format_search_results_human(model: &SearchResultModel, detail_level: DetailLevel) {
    // Handle empty pattern
    if model.pattern.trim().is_empty() {
        println!("{}", "No search pattern provided".bold());
        println!();
        println!("Usage: mcp grep <pattern>");
        println!("Examples:");
        println!("  mcp grep 'read*'      # Tools starting with 'read'");
        println!("  mcp grep '*file*'     # Tools containing 'file'");
        println!("  mcp grep '*'          # All tools");
        return;
    }

    // Search header
    println!(
        "{} {}",
        "Search Results for".bold(),
        format!("'{}'", model.pattern).cyan()
    );
    println!("{}", "═".repeat(50).dimmed());

    // Group matches by server for display
    let mut current_server: Option<&str> = None;
    let mut server_match_count = 0;

    for (i, match_item) in model.matches.iter().enumerate() {
        // New server section
        if current_server != Some(match_item.server_name.as_str()) {
            if i > 0 {
                println!(); // Add blank line between servers
            }
            current_server = Some(&match_item.server_name);
            server_match_count += 1;

            // Count matches for this server
            let count = model
                .matches
                .iter()
                .filter(|m| m.server_name == match_item.server_name)
                .count();

            println!(
                "{} {} {}",
                match_item.server_name.bold(),
                "(server)".dimmed(),
                format!("- {} tool(s)", count).dimmed()
            );
            println!("{}", "─".repeat(50).dimmed());
        }

        // Display tool based on detail level
        match detail_level {
            DetailLevel::Summary => {
                let desc = match_item
                    .description
                    .as_deref()
                    .unwrap_or("No description");
                let truncated_desc = if desc.len() > 60 {
                    format!("{}...", &desc[..57])
                } else {
                    desc.to_string()
                };

                let params = extract_params_from_schema(&match_item.input_schema);
                let param_str = format_param_list(&params, detail_level);

                println!("  • {}: {}", match_item.tool_name.bold(), truncated_desc);
                println!("    Usage: {} {}", match_item.tool_name, param_str.dimmed());
            }
            DetailLevel::WithDescriptions => {
                println!("  {}", match_item.tool_name.bold());
                let desc = match_item
                    .description
                    .as_deref()
                    .unwrap_or("No description");
                println!("    Description: {}", desc);

                let params = extract_params_from_schema(&match_item.input_schema);
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
                println!("  {}", match_item.tool_name.bold());
                let desc = match_item
                    .description
                    .as_deref()
                    .unwrap_or("No description");
                println!("    Description: {}", desc);

                let params = extract_params_from_schema(&match_item.input_schema);
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
                    serde_json::to_string(&match_item.input_schema)
                        .unwrap_or_default()
                        .dimmed()
                );
                println!();
            }
        }
    }

    // Summary footer
    println!();
    println!("{}", "─".repeat(50).dimmed());
    if model.total_matches == 0 {
        println!(
            "{} {}",
            "✗".red(),
            format_args!("No tools matching '{}' found", model.pattern)
        );
        println!();
        println!("Suggestions:");
        println!("  • Try a broader pattern (e.g., '*' for all tools)");
        println!("  • Use wildcards: 'read*' for tools starting with 'read'");
        println!("  • Use '*file*' for tools containing 'file'");
    } else {
        println!(
            "Found {} matching tool(s) across {} server(s)",
            model.total_matches.to_string().bold(),
            server_match_count.to_string().bold()
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
    if !model.failed_servers.is_empty() {
        println!();
        println!(
            "{} {}",
            "⚠".yellow(),
            format!(
                "Search limited - {} server(s) unavailable",
                model.failed_servers.len()
            )
            .dimmed()
        );
    }
}

/// Output mode for formatters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Human-readable output with colors
    Human,
    /// Machine-readable JSON output
    Json,
}

impl OutputMode {
    /// Create output mode from boolean JSON flag.
    pub fn from_json_flag(json: bool) -> Self {
        if json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_mode_creation() {
        assert_eq!(OutputMode::from_json_flag(true), OutputMode::Json);
        assert_eq!(OutputMode::from_json_flag(false), OutputMode::Human);
    }

    #[test]
    fn test_list_servers_model_formatting() {
        let model = ListServersModel {
            servers: vec![ServerModel {
                name: "test".to_string(),
                status: "connected".to_string(),
                transport_type: Some("stdio".to_string()),
                description: Some("Test server".to_string()),
                tool_count: 2,
                tools: vec![ToolModel {
                    name: "tool1".to_string(),
                    description: Some("Tool one".to_string()),
                    input_schema: serde_json::json!({}),
                }],
                error: None,
                has_filtered_tools: false,
            }],
            total_servers: 1,
            connected_servers: 1,
            failed_servers: 0,
            total_tools: 2,
        };

        // Test JSON output doesn't panic
        format_list_servers(&model, DetailLevel::Summary, OutputMode::Json);

        // Test human output doesn't panic
        format_list_servers(&model, DetailLevel::Summary, OutputMode::Human);
    }

    #[test]
    fn test_server_info_model_formatting() {
        let model = ServerInfoModel {
            name: "test".to_string(),
            description: Some("Test".to_string()),
            transport_type: "stdio".to_string(),
            transport_detail: serde_json::json!({"command": "cmd"}),
            environment: None,
            disabled_tools: vec![],
            allowed_tools: vec![],
        };

        format_server_info(&model, OutputMode::Json);
        format_server_info(&model, OutputMode::Human);
    }

    #[test]
    fn test_tool_info_model_formatting() {
        let model = ToolInfoModel {
            server_name: "srv".to_string(),
            tool_name: "tool".to_string(),
            description: Some("Test tool".to_string()),
            parameters: vec![],
            input_schema: serde_json::json!({}),
        };

        format_tool_info(&model, DetailLevel::Summary, OutputMode::Json);
        format_tool_info(&model, DetailLevel::Summary, OutputMode::Human);
    }

    #[test]
    fn test_call_result_model_formatting() {
        let model = CallResultModel {
            server_name: "srv".to_string(),
            tool_name: "tool".to_string(),
            success: true,
            result: Some(serde_json::json!({"data": "value"})),
            error: None,
            execution_time_ms: Some(100),
            retries: 0,
        };

        format_call_result(&model, OutputMode::Json);
        format_call_result(&model, OutputMode::Human);
    }

    #[test]
    fn test_search_result_model_formatting() {
        let model = SearchResultModel {
            pattern: "test".to_string(),
            matches: vec![SearchMatchModel {
                server_name: "srv".to_string(),
                tool_name: "test_tool".to_string(),
                description: Some("A test".to_string()),
                input_schema: serde_json::json!({}),
            }],
            total_matches: 1,
            servers_searched: 1,
            failed_servers: vec![],
        };

        format_search_results(&model, DetailLevel::Summary, OutputMode::Json);
        format_search_results(&model, DetailLevel::Summary, OutputMode::Human);
    }
}
