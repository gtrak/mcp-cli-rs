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

pub mod schema;
pub mod params;

// Re-export commonly used items
pub use schema::{extract_params_from_schema, ParameterInfo};
pub use params::{format_param_list, format_param_help, DetailLevel};
