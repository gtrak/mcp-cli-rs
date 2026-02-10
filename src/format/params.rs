//! Parameter formatting utilities for help-style output.
//!
//! This module provides functions to format parameter information
//! in CLI-friendly formats with progressive detail levels.

use super::schema::ParameterInfo;

/// Detail level for parameter display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailLevel {
    /// Summary view - just parameter names and types
    Summary,
    /// With descriptions - parameter names, types, and descriptions
    WithDescriptions,
    /// Verbose view - full details including defaults and examples
    Verbose,
}

/// Format a list of parameters for display.
///
/// # Arguments
/// * `params` - Slice of ParameterInfo to format
/// * `level` - Detail level to use
///
/// # Returns
/// Formatted string suitable for CLI output
///
/// # Examples
/// ```
/// use mcp_cli_rs::format::{ParameterInfo, format_param_list, DetailLevel};
///
/// let params = vec![
///     ParameterInfo {
///         name: "query".to_string(),
///         param_type: "string".to_string(),
///         description: Some("Search query".to_string()),
///         required: true,
///     },
/// ];
///
/// let output = format_param_list(&params, DetailLevel::Summary);
/// assert_eq!(output, "query <string>");
/// ```
pub fn format_param_list(params: &[ParameterInfo], level: DetailLevel) -> String {
    match level {
        DetailLevel::Summary => format_summary(params),
        DetailLevel::WithDescriptions => format_with_descriptions(params),
        DetailLevel::Verbose => format_verbose(params),
    }
}

/// Format a single parameter for help-style display.
///
/// # Arguments
/// * `param` - The parameter to format
/// * `level` - Detail level to use
///
/// # Returns
/// Formatted string for the parameter
///
/// # Examples
/// ```
/// use mcp_cli_rs::format::{ParameterInfo, format_param_help, DetailLevel};
///
/// let param = ParameterInfo {
///     name: "limit".to_string(),
///     param_type: "number".to_string(),
///     description: Some("Maximum items".to_string()),
///     required: false,
/// };
///
/// let output = format_param_help(&param, DetailLevel::Summary);
/// assert_eq!(output, "limit [number]");
/// ```
pub fn format_param_help(param: &ParameterInfo, level: DetailLevel) -> String {
    match level {
        DetailLevel::Summary => format_single_summary(param),
        DetailLevel::WithDescriptions => format_single_detailed(param),
        DetailLevel::Verbose => format_single_verbose(param),
    }
}

// Internal formatting functions

fn format_summary(params: &[ParameterInfo]) -> String {
    if params.is_empty() {
        return "(no parameters)".to_string();
    }

    let parts: Vec<String> = params.iter().map(format_single_summary).collect();
    let result = parts.join(" ");

    // Truncate if too long
    if result.len() > 80 {
        let truncated: Vec<String> = parts.iter().take(3).cloned().collect();
        format!("{} ...", truncated.join(" "))
    } else {
        result
    }
}

fn format_single_summary(param: &ParameterInfo) -> String {
    let type_display = type_to_display(&param.param_type);
    if param.required {
        format!("{} <{}>", param.name, type_display)
    } else {
        format!("{} [{}]", param.name, type_display)
    }
}

fn format_with_descriptions(params: &[ParameterInfo]) -> String {
    if params.is_empty() {
        return "(no parameters)".to_string();
    }

    let lines: Vec<String> = params.iter().map(format_single_detailed).collect();
    lines.join("\n")
}

fn format_single_detailed(param: &ParameterInfo) -> String {
    let type_display = type_to_display(&param.param_type);
    let req_indicator = if param.required {
        "required"
    } else {
        "optional"
    };

    let base = format!("  {} <{}>  [{}]", param.name, type_display, req_indicator);

    if let Some(desc) = &param.description {
        let wrapped = wrap_description(desc, 60, 8);
        format!("{}\n{}", base, wrapped)
    } else {
        base
    }
}

fn format_verbose(params: &[ParameterInfo]) -> String {
    if params.is_empty() {
        return "(no parameters)".to_string();
    }

    // For now, verbose is same as with descriptions
    // Can be extended to include defaults, enums, examples
    format_with_descriptions(params)
}

fn format_single_verbose(param: &ParameterInfo) -> String {
    // For now, same as detailed
    format_single_detailed(param)
}

fn type_to_display(type_str: &str) -> &str {
    match type_str {
        "string" => "string",
        "number" => "number",
        "integer" => "integer",
        "boolean" => "boolean",
        "object" => "object",
        "array" => "array",
        "null" => "null",
        _ => "any",
    }
}

fn wrap_description(text: &str, width: usize, indent: usize) -> String {
    let indent_str = " ".repeat(indent);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(format!("{}{}", indent_str, current_line));
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(format!("{}{}", indent_str, current_line));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_param(
        name: &str,
        param_type: &str,
        required: bool,
        description: Option<&str>,
    ) -> ParameterInfo {
        ParameterInfo {
            name: name.to_string(),
            param_type: param_type.to_string(),
            description: description.map(|s| s.to_string()),
            required,
        }
    }

    #[test]
    fn test_format_single_required() {
        let param = create_param("query", "string", true, None);
        assert_eq!(format_single_summary(&param), "query <string>");
    }

    #[test]
    fn test_format_single_optional() {
        let param = create_param("limit", "number", false, None);
        assert_eq!(format_single_summary(&param), "limit [number]");
    }

    #[test]
    fn test_format_summary_empty() {
        let params: Vec<ParameterInfo> = vec![];
        assert_eq!(format_summary(&params), "(no parameters)");
    }

    #[test]
    fn test_format_summary_single() {
        let params = vec![create_param("query", "string", true, None)];
        assert_eq!(format_summary(&params), "query <string>");
    }

    #[test]
    fn test_format_summary_multiple() {
        let params = vec![
            create_param("query", "string", true, None),
            create_param("limit", "number", false, None),
            create_param("enabled", "boolean", false, None),
        ];
        let result = format_summary(&params);
        assert_eq!(result, "query <string> limit [number] enabled [boolean]");
    }

    #[test]
    fn test_format_summary_truncation() {
        // Create many parameters to trigger truncation
        let params: Vec<ParameterInfo> = (0..10)
            .map(|i| create_param(&format!("param{}", i), "string", i < 2, None))
            .collect();
        let result = format_summary(&params);
        assert!(result.contains("..."));
        assert!(result.starts_with("param0 <string>"));
    }

    #[test]
    fn test_format_with_descriptions() {
        let params = vec![create_param(
            "query",
            "string",
            true,
            Some("The search query string to look for"),
        )];
        let result = format_with_descriptions(&params);
        assert!(result.contains("query <string>"));
        assert!(result.contains("[required]"));
        assert!(result.contains("The search query"));
    }

    #[test]
    fn test_detail_level_variants() {
        let param = create_param("test", "string", true, None);

        assert_eq!(
            format_param_help(&param, DetailLevel::Summary),
            "test <string>"
        );
        assert!(format_param_help(&param, DetailLevel::WithDescriptions).contains("test"));
        assert!(format_param_help(&param, DetailLevel::Verbose).contains("test"));
    }

    #[test]
    fn test_wrap_description() {
        let text = "This is a very long description that should be wrapped at the specified width";
        let wrapped = wrap_description(text, 30, 4);
        let lines: Vec<&str> = wrapped.lines().collect();

        // Should be multiple lines
        assert!(lines.len() > 1);

        // Each line should start with indentation
        for line in &lines {
            assert!(line.starts_with("    "));
        }
    }
}
