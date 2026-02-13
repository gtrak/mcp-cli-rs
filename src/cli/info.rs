//! Show server and tool information command implementation.

use crate::cli::models::{ParameterModel, ServerInfoModel, ToolInfoModel};
use crate::cli::DetailLevel;
use crate::config::ServerTransport;
use crate::error::{McpError, Result};
use crate::format::{extract_params_from_schema, OutputMode};
use crate::cli::formatters;
use crate::ipc::ProtocolClient;
use crate::output::print_error;

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
    let model = query_server_info(daemon, server_name).await?;
    formatters::format_server_info(&model, output_mode);
    Ok(())
}

/// Query daemon to build server info model.
async fn query_server_info(
    daemon: Box<dyn ProtocolClient>,
    server_name: &str,
) -> Result<ServerInfoModel> {
    let config = daemon.config();
    let server = config.get_server(server_name).ok_or_else(|| {
        print_error(&format!("Server '{}' not found", server_name));
        McpError::ServerNotFound {
            server: server_name.to_string(),
        }
    })?;

    // Build transport details based on type
    let transport_detail = match &server.transport {
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

    let environment = match &server.transport {
        ServerTransport::Stdio { env, .. } => {
            if env.is_empty() {
                None
            } else {
                Some(env.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            }
        }
        _ => None,
    };

    let disabled_tools = server.disabled_tools.clone().unwrap_or_default();
    let allowed_tools = server.allowed_tools.clone().unwrap_or_default();

    Ok(ServerInfoModel {
        name: server.name.clone(),
        description: server.description.clone(),
        transport_type: server.transport.type_name().to_string(),
        transport_detail,
        environment,
        disabled_tools,
        allowed_tools,
    })
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
/// * `detail_level` - Level of detail for display
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
    let model = query_tool_info(&mut daemon, tool_id).await?;
    formatters::format_tool_info(&model, detail_level, output_mode);
    Ok(())
}

/// Query daemon to build tool info model.
async fn query_tool_info(
    daemon: &mut Box<dyn ProtocolClient>,
    tool_id: &str,
) -> Result<ToolInfoModel> {
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

    // Extract parameters from schema
    let params = extract_params_from_schema(&tool.input_schema);
    let parameters: Vec<ParameterModel> = params
        .into_iter()
        .map(|param| ParameterModel {
            name: param.name,
            param_type: param.param_type,
            required: param.required,
            description: param.description,
        })
        .collect();

    Ok(ToolInfoModel {
        server_name: server_name.clone(),
        tool_name: tool_name.clone(),
        description: if tool.description.is_empty() {
            None
        } else {
            Some(tool.description.clone())
        },
        parameters,
        input_schema: tool.input_schema.clone(),
    })
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

    #[test]
    fn test_server_info_model_building() {
        let model = ServerInfoModel {
            name: "test".to_string(),
            description: Some("Test server".to_string()),
            transport_type: "stdio".to_string(),
            transport_detail: serde_json::json!({"command": "test"}),
            environment: None,
            disabled_tools: vec![],
            allowed_tools: vec![],
        };

        assert_eq!(model.name, "test");
        assert_eq!(model.transport_type, "stdio");
    }

    #[test]
    fn test_tool_info_model_building() {
        let model = ToolInfoModel {
            server_name: "test-server".to_string(),
            tool_name: "test-tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: vec![ParameterModel {
                name: "param1".to_string(),
                param_type: "string".to_string(),
                required: true,
                description: Some("A parameter".to_string()),
            }],
            input_schema: serde_json::json!({"type": "object"}),
        };

        assert_eq!(model.server_name, "test-server");
        assert_eq!(model.tool_name, "test-tool");
        assert_eq!(model.parameters.len(), 1);
    }
}
