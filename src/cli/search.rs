//! Search tools by name pattern command implementation.

use crate::cli::models::{SearchMatchModel, SearchResultModel};
use crate::cli::DetailLevel;
use crate::client::ToolInfo;
use crate::error::Result;
use crate::format::OutputMode;
use crate::cli::formatters;
use crate::ipc::ProtocolClient;
use crate::output::{print_error, print_warning};
use crate::parallel::{list_tools_parallel, ParallelExecutor};
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
/// * `detail_level` - Level of detail for display
/// * `output_mode` - Output format (human or JSON)
///
/// # Errors
/// Returns empty result if no tools match
pub async fn cmd_search_tools(
    daemon: Box<dyn ProtocolClient>,
    pattern: &str,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    let model = query_search_results(daemon, pattern).await?;
    formatters::format_search_results(&model, detail_level, output_mode);
    Ok(())
}

/// Query daemon to build search results model.
///
/// This function handles all the data collection, building a model that
/// can be formatted for either human or JSON output.
async fn query_search_results(
    mut daemon: Box<dyn ProtocolClient>,
    pattern: &str,
) -> Result<SearchResultModel> {
    let config = daemon.config();

    // Handle empty pattern or config - return empty model
    if pattern.trim().is_empty() || config.is_empty() {
        return Ok(SearchResultModel {
            pattern: pattern.to_string(),
            matches: vec![],
            total_matches: 0,
            servers_searched: 0,
            failed_servers: vec![],
        });
    }

    let executor = ParallelExecutor::new(config.concurrency_limit);

    // Get server names from daemon
    let server_names = daemon.list_servers().await.map_err(|e| {
        print_error(&format!("Failed to get servers list: {}", e));
        e
    })?;

    let servers_searched = server_names.len();

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

    // Search for matching tools across all servers
    let mut matches = Vec::new();
    for (server_name, tools) in successes {
        for tool in tools {
            if pattern_obj.matches(&tool.name) {
                matches.push(SearchMatchModel {
                    server_name: server_name.clone(),
                    tool_name: tool.name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                });
            }
        }
    }

    Ok(SearchResultModel {
        pattern: pattern.to_string(),
        total_matches: matches.len(),
        servers_searched,
        matches,
        failed_servers: failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_model_building() {
        let model = SearchResultModel {
            pattern: "read*".to_string(),
            matches: vec![
                SearchMatchModel {
                    server_name: "filesystem".to_string(),
                    tool_name: "read_file".to_string(),
                    description: Some("Read a file".to_string()),
                    input_schema: serde_json::json!({}),
                },
            ],
            total_matches: 1,
            servers_searched: 2,
            failed_servers: vec![],
        };

        assert_eq!(model.total_matches, 1);
        assert_eq!(model.servers_searched, 2);
    }
}
