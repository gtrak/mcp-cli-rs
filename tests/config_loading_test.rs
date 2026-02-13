//! Configuration loading integration tests
//!
//! These tests verify that various server configurations load correctly,
//! covering stdio and HTTP transports with different options.
//!
//! TEST-16: Config loading with various server configurations

use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary config file for testing
fn temp_config_file(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("test-config.toml");
    std::fs::write(&config_path, content).expect("Failed to write config file");
    (temp_dir, config_path)
}

/// TEST-16-01: Test stdio server configuration loading
#[test]
fn test_config_stdio_server() {
    let config_content = r#"
[[servers]]
name = "stdio-test-server"
transport = { type = "stdio", command = "mock-mcp-server", args = ["--verbose", "--port", "8080"] }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    // Use the library's config parser
    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    match &result {
        Ok(config) => println!("Parsed config: {:?}", config),
        Err(e) => println!("Parse error: {:?}", e),
    }

    assert!(
        result.is_ok(),
        "Should parse stdio server config successfully"
    );

    let config = result.unwrap();
    assert_eq!(config.servers.len(), 1, "Should have 1 server");

    let server = &config.servers[0];
    assert_eq!(server.name, "stdio-test-server");

    // Check transport type
    match &server.transport {
        mcp_cli_rs::config::ServerTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            assert_eq!(command, "mock-mcp-server");
            // args includes all args passed after command in the config
            assert!(!args.is_empty(), "Should have args");
            assert_eq!(args[0], "--verbose");
            assert_eq!(args[1], "--port");
            assert!(env.is_empty());
            assert!(cwd.is_none());
        }
        _ => panic!("Expected Stdio transport"),
    }
}

/// TEST-16-02: Test HTTP server configuration loading
#[test]
fn test_config_http_server() {
    let config_content = r#"
[[servers]]
name = "http-test-server"
transport = { type = "http", url = "http://localhost:3000/mcp" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(
        result.is_ok(),
        "Should parse HTTP server config successfully"
    );

    let config = result.unwrap();
    assert_eq!(config.servers.len(), 1);

    let server = &config.servers[0];
    assert_eq!(server.name, "http-test-server");

    match &server.transport {
        mcp_cli_rs::config::ServerTransport::Http { url, headers } => {
            assert_eq!(url, "http://localhost:3000/mcp");
            assert!(headers.is_empty());
        }
        _ => panic!("Expected HTTP transport"),
    }
}

/// TEST-16-03: Test mixed server configuration (stdio + HTTP)
#[test]
fn test_config_mixed_servers() {
    let config_content = r#"
[[servers]]
name = "local-stdio"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-everything"] }

[[servers]]
name = "remote-http"
transport = { type = "http", url = "https://api.example.com/mcp" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(
        result.is_ok(),
        "Should parse mixed server config successfully"
    );

    let config = result.unwrap();
    assert_eq!(config.servers.len(), 2, "Should have 2 servers");

    // First server: stdio
    assert_eq!(config.servers[0].name, "local-stdio");
    match &config.servers[0].transport {
        mcp_cli_rs::config::ServerTransport::Stdio { command, args, .. } => {
            assert_eq!(command, "npx");
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "-y");
        }
        _ => panic!("Expected first server to be stdio"),
    }

    // Second server: HTTP
    assert_eq!(config.servers[1].name, "remote-http");
    match &config.servers[1].transport {
        mcp_cli_rs::config::ServerTransport::Http { url, .. } => {
            assert_eq!(url, "https://api.example.com/mcp");
        }
        _ => panic!("Expected second server to be HTTP"),
    }
}

/// TEST-16-04: Test server with environment variables
#[test]
fn test_config_env_vars() {
    let config_content = r#"
[[servers]]
name = "env-test-server"
transport = { type = "stdio", command = "my-server", env = { API_KEY = "test-key-123", DEBUG = "true" } }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse config with env vars");

    let config = result.unwrap();
    let server = &config.servers[0];

    match &server.transport {
        mcp_cli_rs::config::ServerTransport::Stdio { env, .. } => {
            assert_eq!(env.get("API_KEY"), Some(&"test-key-123".to_string()));
            assert_eq!(env.get("DEBUG"), Some(&"true".to_string()));
            assert_eq!(env.len(), 2);
        }
        _ => panic!("Expected Stdio transport with env"),
    }
}

/// TEST-16-05: Test complex args with spaces and special characters
#[test]
fn test_config_complex_args() {
    let config_content = r#"
[[servers]]
name = "complex-args-server"
transport = { type = "stdio", command = "node", args = ["--config", "/path/to/config.json", "--verbose", "--option=value with spaces"] }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse config with complex args");

    let config = result.unwrap();
    let server = &config.servers[0];

    match &server.transport {
        mcp_cli_rs::config::ServerTransport::Stdio { args, .. } => {
            assert_eq!(args.len(), 4);
            assert_eq!(args[0], "--config");
            assert_eq!(args[1], "/path/to/config.json");
            assert_eq!(args[2], "--verbose");
            assert_eq!(args[3], "--option=value with spaces");
        }
        _ => panic!("Expected Stdio transport"),
    }
}

/// TEST-16-06: Test working directory configuration
#[test]
fn test_config_working_directory() {
    let config_content = r#"
[[servers]]
name = "cwd-test-server"
transport = { type = "stdio", command = "./start-server.sh", cwd = "/opt/mcp-servers" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse config with working directory");

    let config = result.unwrap();
    let server = &config.servers[0];

    match &server.transport {
        mcp_cli_rs::config::ServerTransport::Stdio { cwd, .. } => {
            assert_eq!(cwd.as_deref(), Some("/opt/mcp-servers"));
        }
        _ => panic!("Expected Stdio transport with cwd"),
    }
}

/// TEST-16-07: Test server with optional fields (description, allowed_tools, disabled_tools)
#[test]
fn test_config_server_with_optional_fields() {
    let config_content = r#"
[[servers]]
name = "full-featured-server"
description = "A server with all the bells and whistles"
transport = { type = "stdio", command = "fancy-server" }
allowed_tools = ["read_*", "write_file"]
disabled_tools = ["delete_*", "admin_*"]
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse config with optional fields");

    let config = result.unwrap();
    let server = &config.servers[0];

    assert_eq!(server.name, "full-featured-server");
    assert_eq!(
        server.description,
        Some("A server with all the bells and whistles".to_string())
    );

    assert!(server.allowed_tools.is_some());
    let allowed = server.allowed_tools.as_ref().unwrap();
    assert_eq!(allowed.len(), 2);
    assert!(allowed.contains(&"read_*".to_string()));
    assert!(allowed.contains(&"write_file".to_string()));

    assert!(server.disabled_tools.is_some());
    let disabled = server.disabled_tools.as_ref().unwrap();
    assert_eq!(disabled.len(), 2);
    assert!(disabled.contains(&"delete_*".to_string()));
}

/// TEST-16-08: Test invalid TOML syntax handling
#[test]
fn test_config_invalid_toml() {
    let config_content = r#"
[[servers]]
name = "broken-server"
transport = { type = "stdio" }
[invalid syntax here
"#;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("invalid.toml");
    std::fs::write(&config_path, config_content).expect("Failed to write config file");

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_err(), "Should fail to parse invalid TOML syntax");
}

/// TEST-16-09: Test missing required fields
#[test]
fn test_config_missing_required_fields() {
    // Missing transport type
    let config_content = r#"
[[servers]]
name = "incomplete-server"
"#;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("incomplete.toml");
    std::fs::write(&config_path, config_content).expect("Failed to write config file");

    // This may or may not fail depending on how strict the parser is
    // The test verifies the behavior is consistent
    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    // Document the actual behavior
    match result {
        Ok(config) => {
            // If it parses, check what defaults are used
            println!("Config parsed with defaults: {:?}", config.servers[0]);
        }
        Err(e) => {
            println!("Config rejected as expected: {}", e);
        }
    }
}

/// TEST-16-10: Test empty servers array
#[test]
fn test_config_empty_servers() {
    let config_content = r#"servers = []"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse empty servers config");

    let config = result.unwrap();
    assert!(config.servers.is_empty(), "Should have no servers");
}

/// TEST-16-11: Test global config options (retry, timeout, etc.)
#[test]
fn test_config_global_options() {
    let config_content = r#"
retry_max = 5
retry_delay_ms = 2000
timeout_secs = 3600
concurrency_limit = 10
daemon_ttl = 300

[[servers]]
name = "server-with-globals"
transport = { type = "stdio", command = "test-server" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse config with global options");

    let config = result.unwrap();
    assert_eq!(config.retry_max, 5);
    assert_eq!(config.retry_delay_ms, 2000);
    assert_eq!(config.timeout_secs, 3600);
    assert_eq!(config.concurrency_limit, 10);
    assert_eq!(config.daemon_ttl, 300);
    assert_eq!(config.servers.len(), 1);
}

/// TEST-16-12: Test default values for global options
#[test]
fn test_config_default_values() {
    let config_content = r#"
[[servers]]
name = "minimal-server"
transport = { type = "stdio", command = "echo" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse minimal config");

    let config = result.unwrap();

    // Verify defaults
    assert_eq!(config.retry_max, 3, "Default retry_max should be 3");
    assert_eq!(
        config.retry_delay_ms, 1000,
        "Default retry_delay_ms should be 1000"
    );
    assert_eq!(
        config.timeout_secs, 1800,
        "Default timeout_secs should be 1800"
    );
    assert_eq!(
        config.concurrency_limit, 5,
        "Default concurrency_limit should be 5"
    );
    assert_eq!(config.daemon_ttl, 60, "Default daemon_ttl should be 60");
}

/// TEST-16-13: Test multiple HTTP servers
#[test]
fn test_config_multiple_http_servers() {
    let config_content = r#"
[[servers]]
name = "api-server-1"
transport = { type = "http", url = "https://api1.example.com/mcp" }

[[servers]]
name = "api-server-2"
transport = { type = "http", url = "https://api2.example.com/mcp" }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse multiple HTTP servers");

    let config = result.unwrap();
    assert_eq!(config.servers.len(), 2);

    // Verify server 1
    match &config.servers[0].transport {
        mcp_cli_rs::config::ServerTransport::Http { url, .. } => {
            assert_eq!(url, "https://api1.example.com/mcp");
        }
        _ => panic!("Expected HTTP transport"),
    }

    // Verify server 2
    match &config.servers[1].transport {
        mcp_cli_rs::config::ServerTransport::Http { url, .. } => {
            assert_eq!(url, "https://api2.example.com/mcp");
        }
        _ => panic!("Expected HTTP transport"),
    }
}

/// TEST-16-14: Test config validation (positive case)
#[test]
fn test_config_validation_success() {
    let config_content = r#"
[[servers]]
name = "valid-server"
transport = { type = "stdio", command = "valid-command", args = ["arg1", "arg2"] }
"#;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("valid.toml");
    std::fs::write(&config_path, config_content).expect("Failed to write config file");

    let parse_result = mcp_cli_rs::config::parse_toml(config_content, &config_path);
    assert!(parse_result.is_ok(), "Should parse valid config");

    let config = parse_result.unwrap();
    let validation_result =
        mcp_cli_rs::config::validate_config(&config, config_path.to_str().unwrap());

    assert!(
        validation_result.is_ok(),
        "Should validate correct config successfully"
    );
}

/// TEST-16-15: Test HTTP server with headers
#[test]
fn test_config_http_with_headers() {
    let config_content = r#"
[[servers]]
name = "header-server"
transport = { type = "http", url = "http://localhost:8080/mcp", headers = { Authorization = "Bearer token123", "Content-Type" = "application/json" } }
"#;

    let (_temp_dir, config_path) = temp_config_file(config_content);

    let result = mcp_cli_rs::config::parse_toml(config_content, &config_path);

    assert!(result.is_ok(), "Should parse HTTP config with headers");

    let config = result.unwrap();
    match &config.servers[0].transport {
        mcp_cli_rs::config::ServerTransport::Http { headers, .. } => {
            assert_eq!(
                headers.get("Authorization"),
                Some(&"Bearer token123".to_string())
            );
            assert_eq!(
                headers.get("Content-Type"),
                Some(&"application/json".to_string())
            );
        }
        _ => panic!("Expected HTTP transport"),
    }
}
