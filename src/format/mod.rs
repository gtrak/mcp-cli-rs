//! Help-style output formatting for MCP tool parameters.
//!
//! This module provides utilities for extracting and formatting parameter information
//! from JSON Schema, enabling consistent help-style output across all CLI commands.
//!
//! # Module Structure
//!
//! - [`params`] — Parameter formatting with [`DetailLevel`] control
//! - [`schema`] — JSON Schema extraction into [`ParameterInfo`] structs
//! - [`OutputMode`] — Human vs JSON output mode selection
//!
//! # Integration
//!
//! Use [`crate::output`] for colored terminal messages (errors, warnings, info).
//! Use this module for structuring tool parameter information in help-style formats.
//!
//! # Examples
//!
//! ```rust
//! use mcp_cli_rs::format::{OutputMode, DetailLevel};
//!
//! // Determine output mode from CLI flags
//! let mode = OutputMode::from_flags(false);
//! assert!(mode.is_human());
//!
//! // DetailLevel controls how much parameter info is shown
//! let level = DetailLevel::Summary;
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
