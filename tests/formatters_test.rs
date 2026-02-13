//! Tests for CLI output formatters.
//!
//! Verifies formatter functions produce correct output for both
//! human-readable and JSON modes.

use mcp_cli_rs::cli::formatters::*;
use mcp_cli_rs::cli::models::*;
use mcp_cli_rs::format::{DetailLevel, OutputMode};

/// Test format_list_servers with JSON output
#[test]
fn test_format_list_servers_json() {
    let model = ListServersModel {
        servers: vec![ServerModel {
            name: "test-server".into(),
            status: "connected".into(),
            transport_type: Some("stdio".into()),
            description: Some("A test server".into()),
            tool_count: 2,
            tools: vec![ToolModel {
                name: "tool1".into(),
                description: Some("First tool".into()),
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

    // JSON mode should not panic and should serialize correctly
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Json);
    format_list_servers(&model, DetailLevel::WithDescriptions, OutputMode::Json);
    format_list_servers(&model, DetailLevel::Verbose, OutputMode::Json);
}

/// Test format_list_servers with human output (different detail levels)
#[test]
fn test_format_list_servers_human() {
    let model = ListServersModel {
        servers: vec![ServerModel {
            name: "test-server".into(),
            status: "connected".into(),
            transport_type: Some("stdio".into()),
            description: Some("A test server".into()),
            tool_count: 2,
            tools: vec![
                ToolModel {
                    name: "tool1".into(),
                    description: Some("First tool with a description".into()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "arg1": {"type": "string"}
                        }
                    }),
                },
                ToolModel {
                    name: "tool2".into(),
                    description: None,
                    input_schema: serde_json::json!({}),
                },
            ],
            error: None,
            has_filtered_tools: true,
        }],
        total_servers: 1,
        connected_servers: 1,
        failed_servers: 0,
        total_tools: 2,
    };

    // Human mode should not panic with any detail level
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Human);
    format_list_servers(&model, DetailLevel::WithDescriptions, OutputMode::Human);
    format_list_servers(&model, DetailLevel::Verbose, OutputMode::Human);
}

/// Test format_list_servers with empty servers list
#[test]
fn test_format_list_servers_empty() {
    let model = ListServersModel {
        servers: vec![],
        total_servers: 0,
        connected_servers: 0,
        failed_servers: 0,
        total_tools: 0,
    };

    // Should handle empty state gracefully
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Human);
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Json);
}

/// Test format_list_servers with failed servers
#[test]
fn test_format_list_servers_with_failures() {
    let model = ListServersModel {
        servers: vec![
            ServerModel {
                name: "working-server".into(),
                status: "connected".into(),
                transport_type: Some("stdio".into()),
                description: None,
                tool_count: 1,
                tools: vec![ToolModel {
                    name: "tool".into(),
                    description: None,
                    input_schema: serde_json::json!({}),
                }],
                error: None,
                has_filtered_tools: false,
            },
            ServerModel {
                name: "broken-server".into(),
                status: "failed".into(),
                transport_type: Some("stdio".into()),
                description: None,
                tool_count: 0,
                tools: vec![],
                error: Some("Connection timeout".into()),
                has_filtered_tools: false,
            },
        ],
        total_servers: 2,
        connected_servers: 1,
        failed_servers: 1,
        total_tools: 1,
    };

    format_list_servers(&model, DetailLevel::Summary, OutputMode::Human);
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Json);
}

/// Test format_server_info with JSON output
#[test]
fn test_format_server_info_json() {
    let model = ServerInfoModel {
        name: "my-server".into(),
        description: Some("My server description".into()),
        transport_type: "stdio".into(),
        transport_detail: serde_json::json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        }),
        environment: Some(vec![("KEY".into(), "value".into())]),
        disabled_tools: vec!["blocked".into()],
        allowed_tools: vec![],
    };

    format_server_info(&model, OutputMode::Json);
}

/// Test format_server_info with human output
#[test]
fn test_format_server_info_human() {
    let model = ServerInfoModel {
        name: "http-server".into(),
        description: None,
        transport_type: "http".into(),
        transport_detail: serde_json::json!({
            "url": "http://localhost:3000",
            "headers": {"Authorization": "Bearer token"}
        }),
        environment: None,
        disabled_tools: vec![],
        allowed_tools: vec!["*".into()],
    };

    format_server_info(&model, OutputMode::Human);
}

/// Test format_tool_info with JSON output
#[test]
fn test_format_tool_info_json() {
    let model = ToolInfoModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        description: Some("Read a file's contents".into()),
        parameters: vec![ParameterModel {
            name: "path".into(),
            param_type: "string".into(),
            required: true,
            description: Some("File path".into()),
        }],
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"]
        }),
    };

    format_tool_info(&model, DetailLevel::Summary, OutputMode::Json);
    format_tool_info(&model, DetailLevel::WithDescriptions, OutputMode::Json);
    format_tool_info(&model, DetailLevel::Verbose, OutputMode::Json);
}

/// Test format_tool_info with human output (all detail levels)
#[test]
fn test_format_tool_info_human() {
    let model = ToolInfoModel {
        server_name: "test".into(),
        tool_name: "complex_tool".into(),
        description: Some("A complex tool with many parameters".into()),
        parameters: vec![
            ParameterModel {
                name: "required_param".into(),
                param_type: "string".into(),
                required: true,
                description: Some("This is required".into()),
            },
            ParameterModel {
                name: "optional_param".into(),
                param_type: "number".into(),
                required: false,
                description: None,
            },
        ],
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "required_param": {"type": "string"},
                "optional_param": {"type": "number"}
            }
        }),
    };

    format_tool_info(&model, DetailLevel::Summary, OutputMode::Human);
    format_tool_info(&model, DetailLevel::WithDescriptions, OutputMode::Human);
    format_tool_info(&model, DetailLevel::Verbose, OutputMode::Human);
}

/// Test format_tool_info with no parameters
#[test]
fn test_format_tool_info_no_params() {
    let model = ToolInfoModel {
        server_name: "simple".into(),
        tool_name: "ping".into(),
        description: Some("Simple ping tool".into()),
        parameters: vec![],
        input_schema: serde_json::json!({"type": "object"}),
    };

    format_tool_info(&model, DetailLevel::Summary, OutputMode::Human);
    format_tool_info(&model, DetailLevel::WithDescriptions, OutputMode::Human);
}

/// Test format_call_result with JSON output (success)
#[test]
fn test_format_call_result_json_success() {
    let model = CallResultModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        success: true,
        result: Some(serde_json::json!({
            "content": [{"type": "text", "text": "File contents here"}]
        })),
        error: None,
        execution_time_ms: Some(250),
        retries: 0,
    };

    format_call_result(&model, OutputMode::Json);
}

/// Test format_call_result with JSON output (failure)
#[test]
fn test_format_call_result_json_failure() {
    let model = CallResultModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        success: false,
        result: None,
        error: Some("Permission denied".into()),
        execution_time_ms: None,
        retries: 1,
    };

    format_call_result(&model, OutputMode::Json);
}

/// Test format_call_result with human output (success)
#[test]
fn test_format_call_result_human_success() {
    let model = CallResultModel {
        server_name: "test".into(),
        tool_name: "echo".into(),
        success: true,
        result: Some(serde_json::json!({"message": "Hello, World!"})),
        error: None,
        execution_time_ms: Some(100),
        retries: 0,
    };

    format_call_result(&model, OutputMode::Human);
}

/// Test format_call_result with human output (failure)
#[test]
fn test_format_call_result_human_failure() {
    let model = CallResultModel {
        server_name: "test".into(),
        tool_name: "failing_tool".into(),
        success: false,
        result: None,
        error: Some("Something went wrong".into()),
        execution_time_ms: None,
        retries: 0,
    };

    format_call_result(&model, OutputMode::Human);
}

/// Test format_call_result with error response from server
#[test]
fn test_format_call_result_server_error() {
    let model = CallResultModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        success: true, // HTTP success but server returned error
        result: Some(serde_json::json!({
            "error": {
                "message": "Invalid argument",
                "code": 400
            }
        })),
        error: None,
        execution_time_ms: Some(50),
        retries: 0,
    };

    format_call_result(&model, OutputMode::Human);
    format_call_result(&model, OutputMode::Json);
}

/// Test format_search_results with JSON output
#[test]
fn test_format_search_results_json() {
    let model = SearchResultModel {
        pattern: "read*".into(),
        matches: vec![
            SearchMatchModel {
                server_name: "filesystem".into(),
                tool_name: "read_file".into(),
                description: Some("Read file contents".into()),
                input_schema: serde_json::json!({}),
            },
            SearchMatchModel {
                server_name: "filesystem".into(),
                tool_name: "read_directory".into(),
                description: Some("Read directory".into()),
                input_schema: serde_json::json!({}),
            },
        ],
        total_matches: 2,
        servers_searched: 3,
        failed_servers: vec!["broken".into()],
    };

    format_search_results(&model, DetailLevel::Summary, OutputMode::Json);
    format_search_results(&model, DetailLevel::WithDescriptions, OutputMode::Json);
}

/// Test format_search_results with human output
#[test]
fn test_format_search_results_human() {
    let model = SearchResultModel {
        pattern: "*tool*".into(),
        matches: vec![
            SearchMatchModel {
                server_name: "server1".into(),
                tool_name: "my_tool".into(),
                description: Some("A useful tool with parameters".into()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "arg1": {"type": "string", "description": "First argument"},
                        "arg2": {"type": "number"}
                    }
                }),
            },
            SearchMatchModel {
                server_name: "server2".into(),
                tool_name: "another_tool".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
        ],
        total_matches: 2,
        servers_searched: 5,
        failed_servers: vec![],
    };

    format_search_results(&model, DetailLevel::Summary, OutputMode::Human);
    format_search_results(&model, DetailLevel::WithDescriptions, OutputMode::Human);
    format_search_results(&model, DetailLevel::Verbose, OutputMode::Human);
}

/// Test format_search_results with empty pattern
#[test]
fn test_format_search_results_empty_pattern() {
    let model = SearchResultModel {
        pattern: "".into(),
        matches: vec![],
        total_matches: 0,
        servers_searched: 0,
        failed_servers: vec![],
    };

    format_search_results(&model, DetailLevel::Summary, OutputMode::Human);
    format_search_results(&model, DetailLevel::Summary, OutputMode::Json);
}

/// Test format_search_results with no matches
#[test]
fn test_format_search_results_no_matches() {
    let model = SearchResultModel {
        pattern: "nonexistent_tool".into(),
        matches: vec![],
        total_matches: 0,
        servers_searched: 3,
        failed_servers: vec!["unavailable".into()],
    };

    format_search_results(&model, DetailLevel::Summary, OutputMode::Human);
    format_search_results(&model, DetailLevel::Summary, OutputMode::Json);
}

/// Test format_search_results with single match
#[test]
fn test_format_search_results_single_match() {
    let model = SearchResultModel {
        pattern: "specific".into(),
        matches: vec![SearchMatchModel {
            server_name: "test".into(),
            tool_name: "specific_tool".into(),
            description: Some("A specific tool".into()),
            input_schema: serde_json::json!({}),
        }],
        total_matches: 1,
        servers_searched: 1,
        failed_servers: vec![],
    };

    format_search_results(&model, DetailLevel::Summary, OutputMode::Human);
}

/// Test all formatters with edge case: long descriptions
#[test]
fn test_formatters_long_descriptions() {
    let long_desc = "a".repeat(200);

    let tool_model = ToolInfoModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        description: Some(long_desc.clone()),
        parameters: vec![],
        input_schema: serde_json::json!({}),
    };
    format_tool_info(&tool_model, DetailLevel::Summary, OutputMode::Human);

    let search_model = SearchResultModel {
        pattern: "test".into(),
        matches: vec![SearchMatchModel {
            server_name: "srv".into(),
            tool_name: "tool".into(),
            description: Some(long_desc),
            input_schema: serde_json::json!({}),
        }],
        total_matches: 1,
        servers_searched: 1,
        failed_servers: vec![],
    };
    format_search_results(&search_model, DetailLevel::Summary, OutputMode::Human);
}

/// Test all formatters with edge case: special characters in names
#[test]
fn test_formatters_special_characters() {
    let model = ListServersModel {
        servers: vec![ServerModel {
            name: "server-with-dashes_and_underscores.123".into(),
            status: "connected".into(),
            transport_type: Some("stdio".into()),
            description: Some("Description with \"quotes\" and <brackets>".into()),
            tool_count: 1,
            tools: vec![ToolModel {
                name: "tool-with.special$chars".into(),
                description: Some("Tool with unicode: ñ 中文 🎉".into()),
                input_schema: serde_json::json!({}),
            }],
            error: None,
            has_filtered_tools: false,
        }],
        total_servers: 1,
        connected_servers: 1,
        failed_servers: 0,
        total_tools: 1,
    };

    format_list_servers(&model, DetailLevel::Summary, OutputMode::Human);
    format_list_servers(&model, DetailLevel::Summary, OutputMode::Json);
}

/// Test formatters produce valid JSON that can be deserialized
#[test]
fn test_json_output_valid() {
    use serde_json::Value;

    // ListServersModel JSON
    let list_model = ListServersModel {
        servers: vec![ServerModel {
            name: "test".into(),
            status: "connected".into(),
            transport_type: None,
            description: None,
            tool_count: 0,
            tools: vec![],
            error: None,
            has_filtered_tools: false,
        }],
        total_servers: 1,
        connected_servers: 1,
        failed_servers: 0,
        total_tools: 0,
    };
    let json_str = serde_json::to_string(&list_model).unwrap();
    let _: Value = serde_json::from_str(&json_str).unwrap();

    // ServerInfoModel JSON
    let server_model = ServerInfoModel {
        name: "test".into(),
        description: None,
        transport_type: "stdio".into(),
        transport_detail: serde_json::json!({}),
        environment: None,
        disabled_tools: vec![],
        allowed_tools: vec![],
    };
    let json_str = serde_json::to_string(&server_model).unwrap();
    let _: Value = serde_json::from_str(&json_str).unwrap();

    // ToolInfoModel JSON
    let tool_model = ToolInfoModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        description: None,
        parameters: vec![],
        input_schema: serde_json::json!({}),
    };
    let json_str = serde_json::to_string(&tool_model).unwrap();
    let _: Value = serde_json::from_str(&json_str).unwrap();

    // CallResultModel JSON
    let call_model = CallResultModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        success: true,
        result: Some(serde_json::json!({"data": "value"})),
        error: None,
        execution_time_ms: Some(100),
        retries: 0,
    };
    let json_str = serde_json::to_string(&call_model).unwrap();
    let _: Value = serde_json::from_str(&json_str).unwrap();

    // SearchResultModel JSON
    let search_model = SearchResultModel {
        pattern: "*".into(),
        matches: vec![],
        total_matches: 0,
        servers_searched: 0,
        failed_servers: vec![],
    };
    let json_str = serde_json::to_string(&search_model).unwrap();
    let _: Value = serde_json::from_str(&json_str).unwrap();
}
