//! CLI command orchestration module.
//!
//! This module re-exports command functions from specialized modules
//! for backward compatibility. New code should import directly from
//! the specific command modules (list, info, call, search).

// Re-export all command functions from specialized modules
pub use crate::cli::call::cmd_call_tool;
pub use crate::cli::info::{cmd_server_info, cmd_tool_info, parse_tool_id};
pub use crate::cli::list::cmd_list_servers;
pub use crate::cli::search::cmd_search_tools;

// Re-export DetailLevel for convenience
pub use crate::cli::DetailLevel;

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
            crate::error::McpError::AmbiguousCommand { .. }
        ));
    }
}
