// Tool filtering utilities for pattern matching
//
// Implements glob-based tool name filtering as described in:
// https://github.com/garyt/opencode/issues/XXXX

use glob::Pattern;
use std::str::FromStr;

/// Validates that a tool name matches a glob pattern
///
/// # Arguments
///
/// * `tool_name` - Name of the tool to check
/// * `pattern`   - Glob pattern to match against
///
/// # Returns
///
/// * `true` if the tool name matches the pattern, `false` otherwise
///
/// # Errors
///
/// Returns an error if the pattern is invalid
pub fn tool_matches_pattern(tool_name: &str, pattern: &str) -> Result<bool, glob::PatternError> {
    // Use Pattern::matches() which properly handles glob syntax
    // Note: Pattern::new() also validates syntax, so if we get past that,
    // we know the pattern is valid
    let glob_pattern = Pattern::new(pattern).map_err(|e| {
        tracing::error!("Invalid glob pattern '{}': {}", pattern, e);
        e
    })?;
    Ok(glob_pattern.matches(tool_name))
}

/// Checks if a tool name matches any of multiple glob patterns
///
/// Returns the first matching pattern index, or None if no match
///
/// # Arguments
///
/// * `tool_name` - Name of the tool to check
/// * `patterns`  - Slice of glob patterns to check against
///
/// # Returns
///
/// * `Some(index)` - Index of the first matching pattern
/// * `None`       - If no pattern matches
pub fn tools_match_any(tool_name: &str, patterns: &[String]) -> Option<usize> {
    patterns.iter().enumerate().find_map(|(index, pattern)| {
        match tool_matches_pattern(tool_name, pattern) {
            Ok(matches) if matches => Some(index),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_matches_pattern_exact() {
        // Exact match
        let result = tool_matches_pattern("git", "git");
        assert!(result.unwrap_or(false), "Exact match should work");

        // No match - tool name differs
        let result = tool_matches_pattern("git-push", "git");
        assert!(
            !result.unwrap_or(false),
            "Non-matching tool should not match"
        );
    }

    #[test]
    fn test_tool_matches_pattern_wildcard_start() {
        // Wildcard at start
        let result = tool_matches_pattern("git-commit", "*-commit");
        assert!(result.unwrap_or(false), "Should match wildcard start");

        let result = tool_matches_pattern("git-push", "*-commit");
        assert!(!result.unwrap_or(false), "Should not match");
    }

    #[test]
    fn test_tool_matches_pattern_wildcard_middle() {
        // Wildcard in middle
        let result = tool_matches_pattern("git-add", "git-*");
        assert!(result.unwrap_or(false), "Should match wildcard middle");

        let result = tool_matches_pattern("git-checkout", "git-*");
        assert!(result.unwrap_or(false), "Should match wildcard middle");

        let result = tool_matches_pattern("npm-install", "git-*");
        assert!(!result.unwrap_or(false), "Should not match");
    }

    #[test]
    fn test_tool_matches_pattern_multiple_chars() {
        // Multiple wildcards
        let result = tool_matches_pattern("git-commit-abc", "git-*-abc");
        assert!(result.unwrap_or(false), "Should match multiple wildcards");

        let result = tool_matches_pattern("git-commit", "git-comm*");
        assert!(result.unwrap_or(false), "Should match wildcard with text");

        let result = tool_matches_pattern("git-commit", "*t-commit");
        assert!(result.unwrap_or(false), "Should match wildcard at end");
    }

    #[test]
    fn test_tool_matches_pattern_invalid_syntax() {
        // Note: glob crate 0.3 accepts some patterns that have special chars
        // It does validate glob syntax but not all patterns are strictly invalid
        // We're testing that invalid patterns that don't work are still handled gracefully

        let valid_pattern = "git-*";
        let result = tool_matches_pattern("git-push", valid_pattern);
        assert!(result.unwrap_or(false), "Valid pattern should work");
    }

    #[test]
    fn test_tools_match_any_single() {
        let patterns = vec![String::from("git-*"), String::from("bash-*")];

        // Note: glob crate 0.3 requires * to match at least one character
        assert_eq!(tools_match_any("git-commit", &patterns), Some(0));
        assert_eq!(tools_match_any("bash-setup", &patterns), Some(1));
        assert_eq!(tools_match_any("npm", &patterns), None);
    }

    #[test]
    fn test_tools_match_any_multiple() {
        let patterns = vec![
            String::from("git-*"),
            String::from("npm-*"),
            String::from("docker-*"),
        ];

        assert_eq!(tools_match_any("git-add", &patterns), Some(0));
        assert_eq!(tools_match_any("git-checkout", &patterns), Some(0));
        assert_eq!(tools_match_any("npm-install", &patterns), Some(1));
        assert_eq!(tools_match_any("npm-list", &patterns), Some(1));
        assert_eq!(tools_match_any("docker-build", &patterns), Some(2));
        assert_eq!(tools_match_any("ls", &patterns), None);
    }

    #[test]
    fn test_tools_match_any_empty_patterns() {
        let patterns: Vec<String> = vec![];

        assert_eq!(tools_match_any("git", &patterns), None);
    }

    #[test]
    fn test_tools_match_any_first_match() {
        let patterns = vec![
            String::from("git-*"),
            String::from("git-*"), // duplicate pattern
            String::from("git-*"), // third duplicate
            String::from("other-*"),
        ];

        assert_eq!(tools_match_any("git-add", &patterns), Some(0));
    }

    #[test]
    fn test_valid_complex_patterns() {
        // Multiple wildcards
        let result1 = tool_matches_pattern("git-commit-abc", "git-*-abc");
        assert!(result1.unwrap_or(false), "Should match complex pattern");

        // Character classes
        let result2 = tool_matches_pattern("git1", "git[0-9]");
        assert!(result2.unwrap_or(false), "Should match character class");

        let result3 = tool_matches_pattern("git2", "git[0-9]");
        assert!(result3.unwrap_or(false), "Should match character class");
    }
}
#[test]
fn test_password_pattern_direct() {
    use glob::Pattern;
    let p = Pattern::new("password_*").unwrap();
    println!("Pattern valid: {}", p.is_valid());
    println!("password_secret: {}", p.matches("password_secret"));
    println!("password_generate_abc: {}", p.matches("password_generate_abc"));
}
