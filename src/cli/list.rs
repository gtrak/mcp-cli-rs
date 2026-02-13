//! List servers and tools command implementation.

use crate::cli::DetailLevel;
use crate::cli::formatters;
use crate::cli::models::{ListServersModel, ServerModel, ToolModel};
use crate::client::ToolInfo;
use crate::error::Result;
use crate::format::OutputMode;
use crate::ipc::ProtocolClient;
use crate::output::print_error;
use crate::parallel::{ParallelExecutor, list_tools_parallel};
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
/// * `detail_level` - Level of detail for tool listings
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns McpError::ConfigParseError if config file is invalid
pub async fn cmd_list_servers(
    daemon: Box<dyn ProtocolClient>,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    let model = query_list_servers(daemon).await?;
    formatters::format_list_servers(&model, detail_level, output_mode);
    Ok(())
}

/// Query daemon to build list servers model.
///
/// This function handles all the data collection, building a model that
/// can be formatted for either human or JSON output.
async fn query_list_servers(mut daemon: Box<dyn ProtocolClient>) -> Result<ListServersModel> {
    let config = daemon.config();

    // Handle empty config - return empty model
    if config.is_empty() {
        return Ok(ListServersModel {
            servers: vec![],
            total_servers: 0,
            connected_servers: 0,
            failed_servers: 0,
            total_tools: 0,
        });
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
                                .map(|protocol_tool| ToolInfo {
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

    // Build model from results
    let mut servers = Vec::new();
    let mut total_tools = 0;

    // Process successful servers
    for (server_name, tools) in successes {
        let server_config = {
            let daemon_guard = daemon_arc.lock().await;
            daemon_guard.config().get_server(&server_name).cloned()
        };

        let has_filtered_tools = server_config
            .as_ref()
            .map(|s| s.disabled_tools.as_ref().is_some_and(|d| !d.is_empty()))
            .unwrap_or(false);

        let tool_count = tools.len();
        total_tools += tool_count;

        let tool_models: Vec<ToolModel> = tools
            .into_iter()
            .map(|tool| ToolModel {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
            })
            .collect();

        let transport_type = server_config
            .as_ref()
            .map(|s| s.transport.type_name().to_string());

        let description = server_config.as_ref().and_then(|s| s.description.clone());

        servers.push(ServerModel {
            name: server_name,
            status: "connected".to_string(),
            transport_type,
            description,
            tool_count,
            tools: tool_models,
            error: None,
            has_filtered_tools,
        });
    }

    // Process failed servers
    for server_name in failures {
        servers.push(ServerModel {
            name: server_name,
            status: "failed".to_string(),
            transport_type: None,
            description: None,
            tool_count: 0,
            tools: vec![],
            error: Some("Connection failed".to_string()),
            has_filtered_tools: false,
        });
    }

    let total_servers = servers.len();
    let connected_servers = servers.iter().filter(|s| s.status == "connected").count();
    let failed_servers = servers.iter().filter(|s| s.status == "failed").count();

    Ok(ListServersModel {
        servers,
        total_servers,
        connected_servers,
        failed_servers,
        total_tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_servers_model_building() {
        let model = ListServersModel {
            servers: vec![ServerModel {
                name: "test-server".to_string(),
                status: "connected".to_string(),
                transport_type: Some("stdio".to_string()),
                description: Some("Test server".to_string()),
                tool_count: 2,
                tools: vec![ToolModel {
                    name: "tool1".to_string(),
                    description: Some("Tool 1".to_string()),
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

        assert_eq!(model.total_servers, 1);
        assert_eq!(model.connected_servers, 1);
        assert_eq!(model.total_tools, 2);
    }
}
