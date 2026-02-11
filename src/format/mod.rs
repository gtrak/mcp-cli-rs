//! Help-style output formatting for MCP tool parameters.
//!
//! This module provides utilities for extracting and formatting parameter information
//! from JSON Schema, enabling consistent help-style output across all CLI commands.
//!
//! # Integration with Existing Code
//!
//! The formatting utilities work alongside `crate::output` for colored terminal output.
//! Use `output.rs` for printing messages with colors, and this module for structuring
//! tool parameter information in help-style formats.
//!
//! # Usage Examples
//!
//! ```rust,ignore
//! use crate::format::{extract_params_from_schema, format_param_list, DetailLevel};
//! use serde_json::Value;
//!
//! // Extract parameters from a tool's input schema
//! let schema: Value = /* JSON Schema */;
//! let params = extract_params_from_schema(&schema);
//!
//! // Format for display (summary view)
//! let summary = format_param_list(&params, DetailLevel::Summary);
//! // Output: "query <string> limit [number]"
//!
//! // Format with descriptions (-d flag)
//! let detailed = format_param_list(&params, DetailLevel::WithDescriptions);
//! ```

pub mod params;
pub mod schema;

// Re-export commonly used items
pub use params::{format_param_help, format_param_list, DetailLevel};
pub use schema::{extract_params_from_schema, ParameterInfo};

/// Output format mode for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Human-readable output with colors (when TTY)
    Human,
    /// Machine-readable JSON output
    Json,
}

impl OutputMode {
    /// Determine output mode from CLI flags and environment
    ///
    /// Priority:
    /// 1. --json flag forces JSON mode
    /// 2. If not TTY and not explicitly human, could consider JSON (but keep human as default)
    pub fn from_flags(json_flag: bool) -> Self {
        if json_flag {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }

    /// Check if this is JSON mode
    pub fn is_json(&self) -> bool {
        matches!(self, OutputMode::Json)
    }

    /// Check if this is human mode
    pub fn is_human(&self) -> bool {
        matches!(self, OutputMode::Human)
    }
}
