//! Terminal output formatting utilities with NO_COLOR support.
//!
//! This module provides colored output functions that respect the NO_COLOR
//! environment variable and automatically detect terminal capabilities.
//!
//! These utilities are used throughout Phase 3 for performance warnings,
//! error handling, and user feedback. Implements ERR-04 requirement.

use colored::*;
use std::io::{self, stderr, IsTerminal, Write};

/// Determines whether colored output should be used based on:
/// - NO_COLOR environment variable (if set to 1, colors disabled)
/// - Automatic TTY detection (skip if not a terminal)
///
/// Returns true if colors should be enabled, false otherwise.
///
/// # Examples
/// ```ignore
/// if use_color() {
///     print_success("Operation completed successfully");
/// }
/// ```
pub fn use_color() -> bool {
    // Check NO_COLOR environment variable first
    // Source: https://no-color.org/
    if let Some(no_color) = std::env::var("NO_COLOR").ok() {
        return no_color == "1";
    }

    // Check if stderr is a TTY (terminal)
    // We check stderr because many CLI tools use stderr for errors/warnings
    if !stderr().is_terminal() {
        return false;
    }

    // Colors enabled
    true
}

/// Print an error message with red coloring.
///
/// This function is used for critical errors that prevent normal operation.
/// It respects NO_COLOR and TTY detection, so it will not display colors
/// on redirected output.
///
/// # Parameters
/// - `message`: The error message to print (supports embedded newlines)
///
/// # Examples
/// ```ignore
/// print_error("Failed to connect to MCP server");
/// ```
pub fn print_error(message: &str) {
    if use_color() {
        eprint!("{} {}", "Error:".red().bold(), message);
    } else {
        eprint!("Error: {}", message);
    }
}

/// Print a warning message with yellow coloring.
///
/// This function is used for non-critical warnings that don't prevent operation.
/// It respects NO_COLOR and TTY detection, so it will not display colors
/// on redirected output.
///
/// # Parameters
/// - `message`: The warning message to print (supports embedded newlines)
///
/// # Examples
/// ```ignore
/// print_warning("Server is slow to respond");
/// ```
pub fn print_warning(message: &str) {
    if use_color() {
        eprint!("{} {}", "Warning:".yellow().bold(), message);
    } else {
        eprint!("Warning: {}", message);
    }
}

/// Print a success message with green coloring.
///
/// This function is used for successful operation reports and completion messages.
/// It respects NO_COLOR and TTY detection, so it will not display colors
/// on redirected output.
///
/// # Parameters
/// - `message`: The success message to print (supports embedded newlines)
///
/// # Examples
/// ```ignore
/// print_success("Configuration loaded successfully");
/// ```
pub fn print_success(message: &str) {
    if use_color() {
        eprint!("{} {}", "Success:".green().bold(), message);
    } else {
        eprint!("Success: {}", message);
    }
}

/// Print an informational message with blue coloring.
///
/// This function is used for informational messages and progress updates.
/// It respects NO_COLOR and TTY detection, so it will not display colors
/// on redirected output.
///
/// # Parameters
/// - `message`: The info message to print (supports embedded newlines)
///
/// # Examples
/// ```ignore
/// print_info("Loading MCP servers from configuration");
/// ```
pub fn print_info(message: &str) {
    if use_color() {
        eprint!("{} {}", "Info:".blue().bold(), message);
    } else {
        eprint!("Info: {}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_color_with_no_color() {
        unsafe {
            std::env::set_var("NO_COLOR", "1");
            assert_eq!(use_color(), false);
            std::env::remove_var("NO_COLOR");
        }
    }

    #[test]
    fn test_use_color_without_no_color() {
        unsafe {
            std::env::remove_var("NO_COLOR");
            assert_eq!(use_color(), true);
        }
    }
}
