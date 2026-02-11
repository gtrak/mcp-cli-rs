//! Tool filtering in parallel discovery tests for Phase 4.
//!
//! Tests applying tool filtering logic to parallel server discovery.

use mcp_cli_rs::client::ToolInfo;
use mcp_cli_rs::config::{Config, ServerConfig, ServerTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filtering_disabled_tools() {
        // Test that tools are filtered when disabled_tools patterns are present
        let config = Config {
            servers: vec![ServerConfig {
                name: "server1".to_string(),
                transport: ServerTransport::Stdio {
                    command: "python".to_string(),
                    args: vec!["-m".to_string(), "server1".to_string()],
                    env: std::collections::HashMap::new(),
                    cwd: None,
                },
                description: None,
                allowed_tools: None,
                disabled_tools: Some(vec!["password_*".to_string(), "sudo_*".to_string()]),
            }],
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,
        };

        // Simulate listing tools from server1
        let tools = vec![
            ToolInfo {
                name: "list_tools".to_string(),
                description: Some("List all tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "password_set".to_string(),
                description: Some("Set password".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "password_check".to_string(),
                description: Some("Check password".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "search_tools".to_string(),
                description: Some("Search tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
        ];

        // Apply filtering logic (this is what we'll implement in parallel.rs)
        let server_config = &config.servers[0];
        let filtered_tools = mcp_cli_rs::parallel::filter_tools(tools.clone(), server_config);

        // Verify filtering works
        assert!(
            filtered_tools
                .iter()
                .all(|t| !t.name.contains("password") && !t.name.contains("sudo")),
            "All password and sudo tools should be filtered out"
        );
    }

    #[test]
    fn test_filtering_allowed_tools_only() {
        // Test that only allowed tools are returned when allowed_tools patterns are present
        let config = Config {
            servers: vec![ServerConfig {
                name: "server1".to_string(),
                transport: ServerTransport::Stdio {
                    command: "python".to_string(),
                    args: vec!["-m".to_string(), "server1".to_string()],
                    env: std::collections::HashMap::new(),
                    cwd: None,
                },
                description: None,
                allowed_tools: Some(vec!["list_*".to_string(), "search_*".to_string()]),
                disabled_tools: None,
            }],
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,
        };

        // Simulate listing tools from server1
        let tools = vec![
            ToolInfo {
                name: "list_tools".to_string(),
                description: Some("List all tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "search_tools".to_string(),
                description: Some("Search tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "password_set".to_string(),
                description: Some("Set password".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "unknown_tool".to_string(),
                description: Some("Unknown".to_string()),
                input_schema: serde_json::Value::Null,
            },
        ];

        // Apply filtering logic from parallel.rs
        let server_config = &config.servers[0];
        let filtered_tools = mcp_cli_rs::parallel::filter_tools(tools.clone(), server_config);

        // Verify only allowed tools are returned
        assert!(
            filtered_tools
                .iter()
                .all(|t| t.name.starts_with("list") || t.name.starts_with("search")),
            "Only list_ and search_ tools should be returned"
        );
    }

    #[test]
    fn test_filtering_disabled_tools_precedence_over_allowed() {
        // Test that disabled_tools takes precedence over allowed_tools
        let config = Config {
            servers: vec![ServerConfig {
                name: "server1".to_string(),
                transport: ServerTransport::Stdio {
                    command: "python".to_string(),
                    args: vec!["-m".to_string(), "server1".to_string()],
                    env: std::collections::HashMap::new(),
                    cwd: None,
                },
                description: None,
                allowed_tools: Some(vec!["*".to_string()]), // Allow all
                disabled_tools: Some(vec!["password_*".to_string()]), // But block password tools
            }],
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,
        };

        // Simulate listing tools from server1
        let tools = vec![
            ToolInfo {
                name: "list_tools".to_string(),
                description: Some("List all tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "password_set".to_string(),
                description: Some("Set password".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "search_tools".to_string(),
                description: Some("Search tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
        ];

        // Apply filtering logic from parallel.rs
        let server_config = &config.servers[0];
        let filtered_tools = mcp_cli_rs::parallel::filter_tools(tools, server_config);

        // Verify disabled_tools takes precedence
        assert!(
            !filtered_tools.iter().any(|t| t.name.contains("password")),
            "Password tools should be blocked even though allowed_tools has '*'"
        );
        assert!(
            filtered_tools
                .iter()
                .any(|t| t.name == "list_tools" || t.name == "search_tools"),
            "Non-password tools should be allowed"
        );
    }

    #[test]
    fn test_no_filtering_when_fields_empty() {
        // Test that all tools are returned when both fields are empty
        let config = Config {
            servers: vec![ServerConfig {
                name: "server1".to_string(),
                transport: ServerTransport::Stdio {
                    command: "python".to_string(),
                    args: vec!["-m".to_string(), "server1".to_string()],
                    env: std::collections::HashMap::new(),
                    cwd: None,
                },
                description: None,
                allowed_tools: None,
                disabled_tools: None,
            }],
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,
        };

        // Simulate listing tools from server1
        let tools = vec![
            ToolInfo {
                name: "list_tools".to_string(),
                description: Some("List all tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "password_set".to_string(),
                description: Some("Set password".to_string()),
                input_schema: serde_json::Value::Null,
            },
            ToolInfo {
                name: "search_tools".to_string(),
                description: Some("Search tools".to_string()),
                input_schema: serde_json::Value::Null,
            },
        ];

        // Apply filtering logic from parallel.rs
        let server_config = &config.servers[0];
        let filtered_tools = mcp_cli_rs::parallel::filter_tools(tools.clone(), server_config);

        // Verify all tools are returned (backward compatible)
        assert_eq!(
            filtered_tools.len(),
            tools.len(),
            "All tools should be returned when no filtering is configured"
        );
    }
}
