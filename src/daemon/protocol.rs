use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Request types sent from CLI to daemon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonRequest {
    /// Keep daemon alive
    Ping,
    /// Get config file fingerprint for cache validation
    GetConfigFingerprint,
    /// Execute a tool on a specific server
    ExecuteTool {
        server_name: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    /// List available tools on a specific server
    ListTools {
        server_name: String,
    },
    /// List all configured servers
    ListServers,
    /// Request daemon shutdown
    Shutdown,
}

/// Response types sent from daemon to CLI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Acknowledge ping
    Pong,
    /// Return config file fingerprint
    ConfigFingerprint(String),
    /// Tool execution result
    ToolResult(serde_json::Value),
    /// List of available tools
    ToolList(Vec<ToolInfo>),
    /// List of configured servers
    ServerList(Vec<String>),
    /// Acknowledge shutdown request
    ShutdownAck,
    /// Error response
    Error {
        code: u32,
        message: String,
    },
}

/// Tool information returned by daemon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolInfo {
    pub fn new(name: impl Into<String>, description: impl Into<String>, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Server information for JSON output
#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub status: String,
    pub tool_count: usize,
    pub tools: Vec<ToolInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Top-level structure for list command JSON output
#[derive(Debug, Serialize)]
pub struct ListOutput {
    pub servers: Vec<ServerInfo>,
    pub total_servers: usize,
    pub connected_servers: usize,
    pub failed_servers: usize,
    pub total_tools: usize,
}

/// Parameter detail for tool info JSON output
#[derive(Debug, Serialize)]
pub struct ParameterDetail {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Top-level structure for tool info JSON output
#[derive(Debug, Serialize)]
pub struct ToolDetailOutput {
    pub name: String,
    pub description: String,
    pub server: String,
    pub transport: String,
    pub parameters: Vec<ParameterDetail>,
    pub input_schema: serde_json::Value,
}

/// Match entry for search JSON output
#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub server: String,
    pub name: String,
    pub description: String,
}

/// Top-level structure for search JSON output
#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub pattern: String,
    pub total_matches: usize,
    pub match_count: usize,
    pub matches: Vec<SearchMatch>,
    pub failed_servers: Vec<String>,
}

/// Send a NDJSON-encoded request to daemon
pub async fn send_request<W>(writer: &mut W, request: &DaemonRequest) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let json = serde_json::to_string(request).context("Failed to serialize request")?;
    writer.write_all(json.as_bytes()).await.context("Failed to write request")?;
    writer.write_all(b"\n").await.context("Failed to write newline")?;
    writer.flush().await.context("Failed to flush request")?;
    Ok(())
}

/// Receive a NDJSON-encoded request from daemon
pub async fn receive_request<R>(reader: &mut R) -> Result<DaemonRequest>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    reader.read_line(&mut line).await.context("Failed to read request")?;
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow::anyhow!("Received empty NDJSON line"));
    }
    serde_json::from_str(line).context("Failed to deserialize request")
}

/// Send a NDJSON-encoded response to daemon
pub async fn send_response<W>(writer: &mut W, response: &DaemonResponse) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let json = serde_json::to_string(response).context("Failed to serialize response")?;
    writer.write_all(json.as_bytes()).await.context("Failed to write response")?;
    writer.write_all(b"\n").await.context("Failed to write newline")?;
    writer.flush().await.context("Failed to flush response")?;
    Ok(())
}

/// Receive a NDJSON-encoded response from daemon
pub async fn receive_response<R>(reader: &mut R) -> Result<DaemonResponse>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    reader.read_line(&mut line).await.context("Failed to read response")?;
    let line = line.trim();
    if line.is_empty() {
        return Err(anyhow::anyhow!("Received empty NDJSON line"));
    }
    serde_json::from_str(line).context("Failed to deserialize response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = DaemonRequest::Ping;
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "\"ping\"");
    }

    #[test]
    fn test_response_serialization() {
        let resp = DaemonResponse::Pong;
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "\"pong\"");
    }

    #[test]
    fn test_tool_info() {
        let info = ToolInfo::new("test_tool", "Test description", serde_json::json!({}));
        assert_eq!(info.name, "test_tool");
    }
}
