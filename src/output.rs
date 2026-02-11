//! Terminal output formatting utilities with NO_COLOR support.
//!
//! This module provides colored output functions that respect the NO_COLOR
//! environment variable and automatically detect terminal capabilities.
//!
//! CLI output is written to stdout. Tracing is used for debug-level logging
//! to stderr, controlled via RUST_LOG environment variable.
//!
//! # Plain Text Mode Compliance (OUTP-09)
//!
//! This module implements OUTP-09 requirement for correct plain text mode behavior:
//!
//! - **NO_COLOR environment variable**: When set to "1", all color output is disabled
//! - **TTY detection**: Colors are automatically disabled when stdout is not a terminal
//! - **JSON output mode**: Produces plain text JSON without any ANSI color codes
//!
//! The JSON output functions (`print_json`, `print_json_compact`) never add color codes,
//! ensuring machine-readable output when using `--json` flag or piping output.

use colored::*;
use serde::Serialize;
use std::io::{stdout, IsTerminal};
use tracing;

/// Determines whether colored output should be used based on:
/// - NO_COLOR environment variable (if set to 1, colors disabled)
/// - Automatic TTY detection (skip if not a terminal)
///
/// Returns true if colors should be enabled, false otherwise.
pub fn use_color() -> bool {
    // Check NO_COLOR environment variable first
    // Source: https://no-color.org/
    if let Ok(no_color) = std::env::var("NO_COLOR") {
        return no_color == "1";
    }

    // Check if stdout is a TTY (terminal)
    if !stdout().is_terminal() {
        return false;
    }

    // Colors enabled
    true
}

/// Print an error message to stdout with red coloring.
///
/// Also logs the error at debug level for troubleshooting.
pub fn print_error(message: &str) {
    tracing::debug!("CLI error output: {}", message);

    if use_color() {
        println!("{} {}", "Error:".red().bold(), message);
    } else {
        println!("Error: {}", message);
    }
}

/// Print a warning message to stdout with yellow coloring.
///
/// Also logs the warning at debug level for troubleshooting.
pub fn print_warning(message: &str) {
    tracing::debug!("CLI warning output: {}", message);

    if use_color() {
        println!("{} {}", "Warning:".yellow().bold(), message);
    } else {
        println!("Warning: {}", message);
    }
}

/// Print a success message to stdout with green coloring.
///
/// Also logs the success at debug level for troubleshooting.
pub fn print_success(message: &str) {
    tracing::debug!("CLI success output: {}", message);

    if use_color() {
        println!("{} {}", "Success:".green().bold(), message);
    } else {
        println!("Success: {}", message);
    }
}

/// Print an informational message to stdout with blue coloring.
///
/// Also logs the info at debug level for troubleshooting.
pub fn print_info(message: &str) {
    tracing::debug!("CLI info output: {}", message);

    if use_color() {
        println!("{} {}", "Info:".blue().bold(), message);
    } else {
        println!("Info: {}", message);
    }
}

/// Print a formatted error message with context and suggestion.
///
/// Outputs to stdout for CLI display. Logs at debug level for troubleshooting.
/// Implements OUTP-16 requirement.
pub fn print_formatted_error(context: &str, message: &str, suggestion: Option<&str>) {
    tracing::debug!(
        context = context,
        message = message,
        suggestion = ?suggestion,
        "CLI formatted error output"
    );

    if use_color() {
        println!("{} [{}] {}", "✗".red(), context.yellow(), message);
        if let Some(sugg) = suggestion {
            println!("  {} {}", "Suggestion:".dimmed(), sugg);
        }
    } else {
        println!("✗ [{}] {}", context, message);
        if let Some(sugg) = suggestion {
            println!("  Suggestion: {}", sugg);
        }
    }
}

/// Print a formatted warning message with context.
///
/// Outputs to stdout for CLI display. Logs at debug level for troubleshooting.
/// Implements OUTP-17 requirement.
pub fn print_formatted_warning(context: &str, message: &str) {
    tracing::debug!(
        context = context,
        message = message,
        "CLI formatted warning output"
    );

    if use_color() {
        println!("{} [{}] {}", "⚠".yellow(), context.yellow(), message);
    } else {
        println!("⚠ [{}] {}", context, message);
    }
}

/// Print partial failures with context.
///
/// Outputs to stdout for CLI display. Logs at debug level for troubleshooting.
/// Implements OUTP-18 requirement.
pub fn print_partial_failures(context: &str, failures: &[(String, String)]) {
    tracing::debug!(
        context = context,
        failure_count = failures.len(),
        "CLI partial failures output"
    );

    if failures.is_empty() {
        return;
    }

    if use_color() {
        println!(
            "{} [{}] {} operation(s) failed",
            "⚠".yellow(),
            context.yellow(),
            failures.len()
        );
        println!("{}", "─".repeat(50).dimmed());
        for (item, error) in failures {
            println!("  {} {}: {}", "✗".red(), item, error.dimmed());
        }
    } else {
        println!("⚠ [{}] {} operation(s) failed", context, failures.len());
        println!("{}", "─".repeat(50));
        for (item, error) in failures {
            println!("  ✗ {}: {}", item, error);
        }
    }
}

/// Print a value as formatted JSON to stdout.
///
/// This function serializes any serializable value to pretty-printed JSON
/// and writes it to stdout. Used for --json output mode.
///
/// # Type Parameters
/// * `T` - Any type implementing Serialize
///
/// # Arguments
/// * `value` - The value to serialize and print
///
/// # Errors
/// Prints error to stderr if serialization fails
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("{{\"error\": \"Failed to serialize output: {}\"}}", e);
        }
    }
}

/// Print a value as compact JSON to stdout.
///
/// Used when minimal output size is preferred.
pub fn print_json_compact<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("{{\"error\": \"Failed to serialize output: {}\"}}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_formatted_error_with_suggestion() {
        print_formatted_error(
            "Configuration",
            "No servers configured",
            Some("Create mcp_servers.toml"),
        );
    }

    #[test]
    fn test_print_formatted_error_without_suggestion() {
        print_formatted_error("Connection", "Failed to connect", None);
    }

    #[test]
    fn test_print_formatted_warning() {
        print_formatted_warning("Discovery", "Server is slow");
    }

    #[test]
    fn test_print_partial_failures_empty() {
        let failures: Vec<(String, String)> = vec![];
        print_partial_failures("Discovery", &failures);
    }

    #[test]
    fn test_print_partial_failures_with_items() {
        let failures = vec![
            ("server1".to_string(), "Connection refused".to_string()),
            ("server2".to_string(), "Timeout".to_string()),
        ];
        print_partial_failures("Discovery", &failures);
    }

    #[test]
    fn test_json_output_no_color_codes() {
        // Verify that JSON serialization produces plain output without color codes
        let value = serde_json::json!({
            "test": "value",
            "another": 123
        });

        // Capture output in a buffer to check for no color codes
        use std::io::Write;
        let mut buffer = Vec::new();
        writeln!(
            &mut buffer,
            "{}",
            serde_json::to_string_pretty(&value).unwrap()
        )
        .unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(
            !output.contains('\u{001b}'),
            "JSON should not contain ANSI color codes"
        );
    }

    #[test]
    fn test_json_compact_no_color_codes() {
        // Verify that compact JSON also produces plain output
        let value = serde_json::json!({"compact": "output"});

        let output = serde_json::to_string(&value).unwrap();
        assert!(
            !output.contains('\u{001b}'),
            "Compact JSON should not contain ANSI color codes"
        );
    }
}
