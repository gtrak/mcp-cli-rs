//! Tool filtering configuration tests for Phase 4.
//!
//! Tests TOML parsing, validation, and tool filtering configuration fields.

use mcp_cli_rs::config::loader;
use mcp_cli_rs::config::{Config, ServerConfig, ServerTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing_with_allowed_tools() {
        let toml = r#"
[[servers]]
name = "my_server"
allowed_tools = ["*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        assert_eq!(server.name, "my_server");
        assert_eq!(server.allowed_tools, Some(vec!["*".to_string()]));
        assert!(server.disabled_tools.is_none());
    }

    #[test]
    fn test_config_parsing_with_disabled_tools() {
        let toml = r#"
[[servers]]
name = "my_server"
disabled_tools = ["password_*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        assert_eq!(server.name, "my_server");
        assert_eq!(server.disabled_tools, Some(vec!["password_*".to_string()]));
        assert!(server.allowed_tools.is_none());
    }

    #[test]
    fn test_config_parsing_with_both_allowed_and_disabled() {
        let toml = r#"
[[servers]]
name = "my_server"
allowed_tools = ["list_*", "search_*"]
disabled_tools = ["password_*", "sudo_*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        assert_eq!(server.name, "my_server");
        assert_eq!(
            server.allowed_tools,
            Some(vec!["list_*".to_string(), "search_*".to_string()])
        );
        assert_eq!(
            server.disabled_tools,
            Some(vec!["password_*".to_string(), "sudo_*".to_string()])
        );
    }

    #[test]
    fn test_config_parsing_without_tool_filtering() {
        let toml = r#"
[[servers]]
name = "my_server"

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        assert_eq!(server.name, "my_server");
        assert!(server.allowed_tools.is_none());
        assert!(server.disabled_tools.is_none());
    }

    #[test]
    fn test_config_validation_empty_tool_filtering() {
        let toml = r#"
[[servers]]
name = "my_server"
allowed_tools = []
disabled_tools = []

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        // This should pass validation - no error when both lists are empty
        let validation_error = loader::validate_server_config(server, "test.toml");

        // Empty lists are valid - they mean no filtering
        assert!(validation_error.is_ok());
    }

    #[test]
    fn test_config_validation_with_valid_tool_filtering() {
        let toml = r#"
[[servers]]
name = "my_server"
allowed_tools = ["*"]
disabled_tools = ["password_*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "mcp_server"]
"#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = config.servers.first().unwrap();

        assert!(loader::validate_server_config(server, "test.toml").is_ok());
    }

    #[test]
    fn test_config_validation_multiple_servers() {
        let toml = r#"
[[servers]]
name = "server1"
allowed_tools = ["*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "server1"]

[[servers]]
name = "server2"
disabled_tools = ["password_*"]

[servers.transport]
type = "stdio"
command = "python"
args = ["-m", "server2"]
"#;

        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.servers.len(), 2);

        let server1 = &config.servers[0];
        assert_eq!(server1.name, "server1");
        assert_eq!(server1.allowed_tools, Some(vec!["*".to_string()]));

        let server2 = &config.servers[1];
        assert_eq!(server2.name, "server2");
        assert_eq!(server2.disabled_tools, Some(vec!["password_*".to_string()]));
    }

    #[test]
    fn test_config_clone_with_tool_filtering() {
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
                disabled_tools: Some(vec!["password_*".to_string()]),
            }],
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,
        };

        // Clone should work with new fields
        let cloned = config.clone();
        assert_eq!(cloned.servers.len(), 1);
        assert_eq!(
            cloned.servers[0].allowed_tools,
            config.servers[0].allowed_tools
        );
        assert_eq!(
            cloned.servers[0].disabled_tools,
            config.servers[0].disabled_tools
        );
    }
}
