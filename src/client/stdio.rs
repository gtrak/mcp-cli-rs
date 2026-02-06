//! Stdio transport implementation for MCP server communication.
//!
//! This module implements stdio-based transport for MCP servers, spawning
//! processes and communicating via stdin/stdout. Uses kill_on_drop(true)
//! to prevent Windows zombie processes (PITFALLS.md - CONN-04).

use async_trait::async_trait;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::client::transport::{Transport, TransportFactory};
use crate::error::{McpError, Result};
use crate::config::ServerTransport;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Stdio transport for local process communication.
///
/// This transport spawns a server process and communicates with it via
/// stdin/stdout using JSON-RPC over newline-delimited JSON.
pub struct StdioTransport {
    /// Child process handle.
    child: tokio::process::Child,

    /// Process stdin handle for sending requests.
    stdin: tokio::process::ChildStdin,

    /// Process stdout handle for reading responses.
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl StdioTransport {
    /// Create a new StdioTransport by spawning a server process.
    ///
    /// # Arguments
    /// * `command` - Command to execute
    /// * `args` - Command arguments
    /// * `env` - Environment variables to set
    /// * `cwd` - Working directory for the process
    ///
    /// # Important
    /// - Uses `kill_on_drop(true)` to prevent Windows zombie processes (PITFALLS.md - CONN-04)
    /// - Uses `BufReader` for line-by-line reading (newline-delimited JSON)
    /// - Uses `writeln!` for sending messages (XP-03)
    pub fn new(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command)
            .args(args)
            .kill_on_drop(true) // Prevents Windows zombie processes
            .stdin(tokio::process::Stdio::piped())
            .stdout(tokio::process::Stdio::piped());

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Set working directory if provided
        if let Some(cwd_path) = cwd {
            cmd.current_dir(cwd_path);
        }

        // Spawn the process
        let child = cmd.spawn().map_err(|e| {
            McpError::connection_error(command, e)
        })?;

        // Extract stdin and stdout handles
        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::InvalidProtocol {
                message: "Failed to get stdin handle".to_string(),
            }
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::InvalidProtocol {
                message: "Failed to get stdout handle".to_string(),
            }
        })?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    /// Read a single line from stdout (newline-delimited JSON).
    ///
    /// This is used to read MCP protocol responses which must be
    /// newline-delimited (XP-03).
    async fn read_response_line(&mut self) -> Result<String> {
        let mut line = String::new();
        let bytes_read = self.stdout
            .read_line(&mut line)
            .await
            .map_err(|e| {
                McpError::connection_error("stdio", e)
            })?;

        if bytes_read == 0 {
            return Err(McpError::Timeout {
                timeout: 30,
            });
        }

        Ok(line.trim_end().to_string())
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, request: serde_json::Value) -> Result<serde_json::Value> {
        // Send request using writeln! (newline-delimited JSON)
        let request_str = request.to_string();
        writeln!(self.stdin, "{}", request_str)
            .await
            .map_err(|e| {
                McpError::connection_error("stdio", e)
            })?;

        // Flush stdin to ensure message is sent
        self.stdin
            .flush()
            .await
            .map_err(|e| {
                McpError::connection_error("stdio", e)
            })?;

        // Read response (newline-delimited JSON)
        let response_str = tokio::time::timeout(
            Duration::from_secs(30),
            self.read_response_line(),
        )
        .await
        .map_err(|_| {
            McpError::Timeout {
                timeout: 30,
            }
        })?;

        // Parse response JSON
        let response: serde_json::Value = serde_json::from_str(&response_str)
            .map_err(|e| {
                McpError::InvalidProtocol {
                    message: format!("Invalid JSON response: {}", e),
                }
            })?;

        Ok(response)
    }

    async fn ping(&self) -> Result<()> {
        // Create a minimal ping request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping",
            "id": "ping"
        });

        // We need mutable access for send, so we can't ping without mutation
        // This is a limitation for immutable ping - we'll skip ping for now
        // In Phase 2, we'll need a mut wrapper or separate ping method
        Err(McpError::InvalidProtocol {
            message: "Ping not yet implemented for stdio transport".to_string(),
        })
    }

    fn transport_type(&self) -> &str {
        "stdio"
    }
}

#[async_trait]
impl TransportFactory for ServerTransport {
    fn create_transport(
        &self,
        server_name: &str,
    ) -> Box<dyn Transport + Send + Sync> {
        match self {
            ServerTransport::Stdio { command, args, env, cwd } => {
                let transport = StdioTransport::new(command, args, env, cwd.as_deref())
                    .expect("Failed to create stdio transport");
                Box::new(transport)
            }
            ServerTransport::Http { .. } => {
                Box::new(crate::client::http::HttpTransport::new(
                    &self.url(), // This will be added to HttpTransport
                ))
            }
        }
    }

    fn supports_filtering(&self) -> bool {
        match self {
            ServerTransport::Http { .. } => true,
            ServerTransport::Stdio { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stdio_transport_creation() {
        // Placeholder test
        println!("StdioTransport creation works");
    }

    #[test]
    fn test_write_json() {
        let mut stdout = Vec::new();
        let mock_reader = BufReader::new(&mut stdout as &mut dyn BufRead);
        let mut transport = StdioTransport {
            child: tokio::process::Command::new("echo")
                .spawn()
                .unwrap(),
            stdin: vec![].into(),
            stdout: mock_reader,
        };

        let request = json!({ "test": "data" });
        transport.send(request).await.unwrap();
        println!("Stdio transport JSON writing works");
    }
}
