//! JSON Schema parsing for parameter extraction.
//!
//! This module provides utilities to parse JSON Schema and extract
//! parameter information for display purposes.

use serde_json::Value;

/// Information about a single parameter extracted from JSON Schema.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterInfo {
    /// Parameter name
    pub name: String,
    /// Parameter type (string, number, boolean, object, array, any)
    pub param_type: String,
    /// Optional description from schema
    pub description: Option<String>,
    /// Whether the parameter is required
    pub required: bool,
}

/// Extract parameter information from a JSON Schema.
///
/// Parses the "properties" and "required" fields from a JSON Schema
/// to extract parameter names, types, descriptions, and required status.
///
/// # Arguments
/// * `schema` - JSON Schema as a serde_json::Value
///
/// # Returns
/// Vector of ParameterInfo, sorted by required first, then by name
///
/// # Examples
/// ```
/// use serde_json::json;
/// use mcp_cli_rs::format::schema::extract_params_from_schema;
///
/// let schema = json!({
///     "type": "object",
///     "properties": {
///         "name": { "type": "string", "description": "The name" },
///         "age": { "type": "number" }
///     },
///     "required": ["name"]
/// });
///
/// let params = extract_params_from_schema(&schema);
/// assert_eq!(params.len(), 2);
/// assert_eq!(params[0].name, "name");  // Required comes first
/// assert!(params[0].required);
/// ```
pub fn extract_params_from_schema(schema: &Value) -> Vec<ParameterInfo> {
    let mut params = Vec::new();

    // Get properties object
    let properties = match schema.get("properties") {
        Some(Value::Object(props)) => props,
        _ => return params, // Empty or invalid schema
    };

    // Get required fields array
    let required_fields: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    // Extract parameter info from each property
    for (name, prop_schema) in properties {
        let param_type = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("any")
            .to_string();

        let description = prop_schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        let required = required_fields.contains(&name.as_str());

        params.push(ParameterInfo {
            name: name.clone(),
            param_type,
            description,
            required,
        });
    }

    // Sort: required first, then by name
    params.sort_by(|a, b| match (a.required, b.required) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_empty_schema() {
        let schema = json!({});
        let params = extract_params_from_schema(&schema);
        assert!(params.is_empty());
    }

    #[test]
    fn test_minimal_schema() {
        let schema = json!({
            "properties": {
                "name": { "type": "string" }
            }
        });
        let params = extract_params_from_schema(&schema);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "name");
        assert_eq!(params[0].param_type, "string");
        assert!(!params[0].required);
    }

    #[test]
    fn test_complete_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum results"
                },
                "enabled": {
                    "type": "boolean"
                }
            },
            "required": ["query"]
        });

        let params = extract_params_from_schema(&schema);
        assert_eq!(params.len(), 3);

        // Required comes first
        assert_eq!(params[0].name, "query");
        assert!(params[0].required);
        assert_eq!(
            params[0].description,
            Some("Search query string".to_string())
        );

        // Optional sorted by name
        assert_eq!(params[1].name, "enabled");
        assert!(!params[1].required);
        assert_eq!(params[1].param_type, "boolean");

        assert_eq!(params[2].name, "limit");
        assert!(!params[2].required);
    }

    #[test]
    fn test_various_types() {
        let schema = json!({
            "properties": {
                "text": { "type": "string" },
                "count": { "type": "number" },
                "active": { "type": "boolean" },
                "data": { "type": "object" },
                "items": { "type": "array" },
                "unknown": {}  // Missing type
            }
        });

        let params = extract_params_from_schema(&schema);
        assert_eq!(params.len(), 6);

        let types: Vec<&str> = params.iter().map(|p| p.param_type.as_str()).collect();
        assert!(types.contains(&"string"));
        assert!(types.contains(&"number"));
        assert!(types.contains(&"boolean"));
        assert!(types.contains(&"object"));
        assert!(types.contains(&"array"));
        assert!(types.contains(&"any")); // Default for missing type
    }

    #[test]
    fn test_mixed_required_optional() {
        let schema = json!({
            "properties": {
                "z_param": { "type": "string" },
                "a_param": { "type": "string" },
                "m_param": { "type": "string" }
            },
            "required": ["m_param"]
        });

        let params = extract_params_from_schema(&schema);
        assert_eq!(params.len(), 3);

        // Required first
        assert_eq!(params[0].name, "m_param");
        assert!(params[0].required);

        // Then sorted alphabetically
        assert_eq!(params[1].name, "a_param");
        assert_eq!(params[2].name, "z_param");
    }
}
