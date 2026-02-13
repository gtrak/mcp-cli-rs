//! Tests for CLI command models.
//!
//! Verifies model construction, serialization, and field coverage
//! for all daemon response models.

use mcp_cli_rs::cli::models::*;

/// Test ListServersModel construction and serialization
#[test]
fn test_list_servers_model_serialization() {
    let model = ListServersModel {
        servers: vec![ServerModel {
            name: "test-server".into(),
            status: "connected".into(),
            transport_type: Some("stdio".into()),
            description: Some("Test server description".into()),
            tool_count: 3,
            tools: vec![
                ToolModel {
                    name: "tool1".into(),
                    description: Some("First tool".into()),
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
            has_filtered_tools: false,
        }],
        total_servers: 1,
        connected_servers: 1,
        failed_servers: 0,
        total_tools: 3,
    };

    let json = serde_json::to_value(&model).unwrap();

    // Verify all fields are captured in JSON
    assert_eq!(json["total_servers"], 1);
    assert_eq!(json["connected_servers"], 1);
    assert_eq!(json["failed_servers"], 0);
    assert_eq!(json["total_tools"], 3);
    assert!(json["servers"].is_array());
    assert_eq!(json["servers"].as_array().unwrap().len(), 1);
    assert_eq!(json["servers"][0]["name"], "test-server");
    assert_eq!(json["servers"][0]["status"], "connected");
    assert_eq!(json["servers"][0]["transport_type"], "stdio");
    assert_eq!(json["servers"][0]["description"], "Test server description");
    assert_eq!(json["servers"][0]["tool_count"], 3);
    assert_eq!(json["servers"][0]["tools"].as_array().unwrap().len(), 2);
}

/// Test ListServersModel JSON round-trip
#[test]
fn test_list_servers_model_roundtrip() {
    let original = ListServersModel {
        servers: vec![ServerModel {
            name: "filesystem".into(),
            status: "connected".into(),
            transport_type: Some("stdio".into()),
            description: None,
            tool_count: 5,
            tools: vec![],
            error: None,
            has_filtered_tools: true,
        }],
        total_servers: 1,
        connected_servers: 1,
        failed_servers: 0,
        total_tools: 5,
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let deserialized: ListServersModel = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.total_servers, original.total_servers);
    assert_eq!(deserialized.connected_servers, original.connected_servers);
    assert_eq!(deserialized.servers.len(), original.servers.len());
    assert_eq!(deserialized.servers[0].name, original.servers[0].name);
    assert_eq!(
        deserialized.servers[0].has_filtered_tools,
        original.servers[0].has_filtered_tools
    );
}

/// Test ServerModel with failed status
#[test]
fn test_server_model_failed_status() {
    let model = ServerModel {
        name: "failing-server".into(),
        status: "failed".into(),
        transport_type: Some("stdio".into()),
        description: Some("A failing server".into()),
        tool_count: 0,
        tools: vec![],
        error: Some("Connection refused".into()),
        has_filtered_tools: false,
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["name"], "failing-server");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["error"], "Connection refused");
    // has_filtered_tools is false, so it's skipped in serialization
    assert!(json.get("has_filtered_tools").is_none());
}

/// Test ServerModel with optional fields skipped
#[test]
fn test_server_model_optional_fields_skipped() {
    let model = ServerModel {
        name: "minimal-server".into(),
        status: "connected".into(),
        transport_type: None,
        description: None,
        tool_count: 0,
        tools: vec![],
        error: None,
        has_filtered_tools: false,
    };

    let json = serde_json::to_string(&model).unwrap();

    // Optional fields should be skipped when None/false
    assert!(!json.contains("transport_type"));
    assert!(!json.contains("description"));
    assert!(!json.contains("error"));
    assert!(!json.contains("has_filtered_tools"));
}

/// Test ServerInfoModel construction and serialization
#[test]
fn test_server_info_model_serialization() {
    let model = ServerInfoModel {
        name: "my-server".into(),
        description: Some("My test server".into()),
        transport_type: "stdio".into(),
        transport_detail: serde_json::json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"],
            "cwd": "/home/user"
        }),
        environment: Some(vec![
            ("API_KEY".into(), "secret123".into()),
            ("DEBUG".into(), "true".into()),
        ]),
        disabled_tools: vec!["dangerous_tool".into()],
        allowed_tools: vec!["safe_tool".into()],
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["name"], "my-server");
    assert_eq!(json["description"], "My test server");
    assert_eq!(json["transport_type"], "stdio");
    assert!(json["transport_detail"].is_object());
    assert_eq!(json["transport_detail"]["command"], "npx");
    assert!(json["environment"].is_array());
    assert_eq!(json["environment"].as_array().unwrap().len(), 2);
    assert!(json["disabled_tools"].is_array());
    assert_eq!(json["disabled_tools"][0], "dangerous_tool");
}

/// Test ServerInfoModel with empty vectors (should be skipped)
#[test]
fn test_server_info_model_empty_skipped() {
    let model = ServerInfoModel {
        name: "simple-server".into(),
        description: None,
        transport_type: "http".into(),
        transport_detail: serde_json::json!({"url": "http://localhost:3000"}),
        environment: None,
        disabled_tools: vec![],
        allowed_tools: vec![],
    };

    let json_str = serde_json::to_string(&model).unwrap();

    // Empty vectors and None should be skipped
    assert!(!json_str.contains("disabled_tools"));
    assert!(!json_str.contains("allowed_tools"));
    assert!(!json_str.contains("environment"));
    assert!(!json_str.contains("description"));
}

/// Test ToolInfoModel construction and serialization
#[test]
fn test_tool_info_model_serialization() {
    let model = ToolInfoModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        description: Some("Reads the contents of a file".into()),
        parameters: vec![
            ParameterModel {
                name: "path".into(),
                param_type: "string".into(),
                required: true,
                description: Some("Path to the file".into()),
            },
            ParameterModel {
                name: "encoding".into(),
                param_type: "string".into(),
                required: false,
                description: None,
            },
        ],
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file"},
                "encoding": {"type": "string"}
            },
            "required": ["path"]
        }),
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["server_name"], "filesystem");
    assert_eq!(json["tool_name"], "read_file");
    assert_eq!(json["description"], "Reads the contents of a file");
    assert!(json["parameters"].is_array());
    assert_eq!(json["parameters"].as_array().unwrap().len(), 2);
    assert_eq!(json["parameters"][0]["name"], "path");
    assert_eq!(json["parameters"][0]["required"], true);
    assert_eq!(json["parameters"][1]["name"], "encoding");
    assert_eq!(json["parameters"][1]["required"], false);
}

/// Test ToolInfoModel JSON round-trip
#[test]
fn test_tool_info_model_roundtrip() {
    let original = ToolInfoModel {
        server_name: "test".into(),
        tool_name: "echo".into(),
        description: None,
        parameters: vec![],
        input_schema: serde_json::json!({"type": "object"}),
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let deserialized: ToolInfoModel = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.server_name, original.server_name);
    assert_eq!(deserialized.tool_name, original.tool_name);
    assert!(deserialized.description.is_none());
}

/// Test ParameterModel conversion to ParameterInfo
#[test]
fn test_parameter_model_conversion() {
    let param = ParameterModel {
        name: "test_param".into(),
        param_type: "number".into(),
        required: true,
        description: Some("A test parameter".into()),
    };

    let info: mcp_cli_rs::format::ParameterInfo = (&param).into();

    assert_eq!(info.name, "test_param");
    assert_eq!(info.param_type, "number");
    assert_eq!(info.required, true);
    assert_eq!(info.description, Some("A test parameter".into()));
}

/// Test CallResultModel success case
#[test]
fn test_call_result_model_success() {
    let model = CallResultModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        success: true,
        result: Some(serde_json::json!({
            "content": [{"type": "text", "text": "file contents"}]
        })),
        error: None,
        execution_time_ms: Some(150),
        retries: 0,
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["server_name"], "filesystem");
    assert_eq!(json["tool_name"], "read_file");
    assert_eq!(json["success"], true);
    assert!(json["result"].is_object());
    assert_eq!(json["execution_time_ms"], 150);
    // retries=0 should be skipped
    assert!(json.get("retries").is_none());
    assert!(json.get("error").is_none());
}

/// Test CallResultModel failure case
#[test]
fn test_call_result_model_failure() {
    let model = CallResultModel {
        server_name: "filesystem".into(),
        tool_name: "read_file".into(),
        success: false,
        result: None,
        error: Some("File not found: /path/to/file".into()),
        execution_time_ms: None,
        retries: 2,
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["success"], false);
    assert_eq!(json["error"], "File not found: /path/to/file");
    assert_eq!(json["retries"], 2);
    // execution_time_ms=None should be skipped
    assert!(json.get("execution_time_ms").is_none());
    // result=None should be skipped
    assert!(json.get("result").is_none());
}

/// Test CallResultModel JSON round-trip
#[test]
fn test_call_result_model_roundtrip() {
    let original = CallResultModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        success: true,
        result: Some(serde_json::json!({"data": [1, 2, 3]})),
        error: None,
        execution_time_ms: Some(100),
        retries: 1,
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let deserialized: CallResultModel = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.server_name, original.server_name);
    assert_eq!(deserialized.success, original.success);
    assert_eq!(deserialized.result, original.result);
    assert_eq!(deserialized.retries, original.retries);
}

/// Test SearchResultModel construction and serialization
#[test]
fn test_search_result_model_serialization() {
    let model = SearchResultModel {
        pattern: "read*".into(),
        matches: vec![
            SearchMatchModel {
                server_name: "filesystem".into(),
                tool_name: "read_file".into(),
                description: Some("Read a file".into()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}}
                }),
            },
            SearchMatchModel {
                server_name: "filesystem".into(),
                tool_name: "read_directory".into(),
                description: Some("Read directory contents".into()),
                input_schema: serde_json::json!({}),
            },
        ],
        total_matches: 2,
        servers_searched: 3,
        failed_servers: vec!["broken-server".into()],
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["pattern"], "read*");
    assert_eq!(json["total_matches"], 2);
    assert_eq!(json["servers_searched"], 3);
    assert!(json["matches"].is_array());
    assert_eq!(json["matches"].as_array().unwrap().len(), 2);
    assert_eq!(json["matches"][0]["server_name"], "filesystem");
    assert_eq!(json["matches"][0]["tool_name"], "read_file");
    assert!(json["failed_servers"].is_array());
    assert_eq!(json["failed_servers"][0], "broken-server");
}

/// Test SearchResultModel empty results
#[test]
fn test_search_result_model_empty() {
    let model = SearchResultModel {
        pattern: "nonexistent*".into(),
        matches: vec![],
        total_matches: 0,
        servers_searched: 5,
        failed_servers: vec![],
    };

    let json_str = serde_json::to_string(&model).unwrap();

    assert!(json_str.contains("nonexistent*"));
    assert!(json_str.contains("\"total_matches\":0"));
    // Empty failed_servers should be skipped
    assert!(!json_str.contains("failed_servers"));
}

/// Test SearchResultModel JSON round-trip
#[test]
fn test_search_result_model_roundtrip() {
    let original = SearchResultModel {
        pattern: "*file*".into(),
        matches: vec![SearchMatchModel {
            server_name: "test".into(),
            tool_name: "file_tool".into(),
            description: None,
            input_schema: serde_json::json!({}),
        }],
        total_matches: 1,
        servers_searched: 2,
        failed_servers: vec!["down-server".into()],
    };

    let json_str = serde_json::to_string(&original).unwrap();
    let deserialized: SearchResultModel = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.pattern, original.pattern);
    assert_eq!(deserialized.total_matches, original.total_matches);
    assert_eq!(deserialized.servers_searched, original.servers_searched);
    assert_eq!(deserialized.failed_servers.len(), 1);
    assert_eq!(deserialized.matches.len(), 1);
    assert_eq!(deserialized.matches[0].tool_name, "file_tool");
}

/// Test empty ListServersModel (no servers configured)
#[test]
fn test_list_servers_model_empty() {
    let model = ListServersModel {
        servers: vec![],
        total_servers: 0,
        connected_servers: 0,
        failed_servers: 0,
        total_tools: 0,
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["total_servers"], 0);
    assert!(json["servers"].as_array().unwrap().is_empty());
}

/// Test ToolModel serialization
#[test]
fn test_tool_model_serialization() {
    let model = ToolModel {
        name: "my_tool".into(),
        description: Some("A helpful tool".into()),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["input"]
        }),
    };

    let json = serde_json::to_value(&model).unwrap();

    assert_eq!(json["name"], "my_tool");
    assert_eq!(json["description"], "A helpful tool");
    assert!(json["input_schema"].is_object());
}

/// Test all model types can be serialized without panic
#[test]
fn test_all_models_serializable() {
    // ListServersModel
    let _ = serde_json::to_string(&ListServersModel {
        servers: vec![],
        total_servers: 0,
        connected_servers: 0,
        failed_servers: 0,
        total_tools: 0,
    })
    .unwrap();

    // ServerInfoModel
    let _ = serde_json::to_string(&ServerInfoModel {
        name: "test".into(),
        description: None,
        transport_type: "stdio".into(),
        transport_detail: serde_json::json!({}),
        environment: None,
        disabled_tools: vec![],
        allowed_tools: vec![],
    })
    .unwrap();

    // ToolInfoModel
    let _ = serde_json::to_string(&ToolInfoModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        description: None,
        parameters: vec![],
        input_schema: serde_json::json!({}),
    })
    .unwrap();

    // CallResultModel
    let _ = serde_json::to_string(&CallResultModel {
        server_name: "test".into(),
        tool_name: "tool".into(),
        success: true,
        result: None,
        error: None,
        execution_time_ms: None,
        retries: 0,
    })
    .unwrap();

    // SearchResultModel
    let _ = serde_json::to_string(&SearchResultModel {
        pattern: "*".into(),
        matches: vec![],
        total_matches: 0,
        servers_searched: 0,
        failed_servers: vec![],
    })
    .unwrap();
}
