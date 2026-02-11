//! Stdio transport implementation for MCP server communication.
//!
//! This module implements stdio-based transport for MCP servers, spawning
//! processes and communicating via stdin/stdout. Uses kill_on_drop(true)
//! to prevent Windows zombie processes (PITFALLS.md - CONN-04).

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

use crate::client::http::HttpTransport;
use crate::config::ServerTransport;
use crate::error::{McpError, Result};
use crate::transport::{Transport, TransportFactory};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Stdio transport for local process communication.
///
/// This transport spawns a server process and communicates with it via
/// stdin/stdout using JSON-RPC over newline-delimited JSON.
pub struct StdioTransport {
    /// Child process handle.
    _child: tokio::process::Child,

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
        // Set stdin/stdout before environment variables
        let mut cmd = Command::new(command);
        cmd.args(args).kill_on_drop(true); // Prevents Windows zombie processes

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());

        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Set working directory if provided
        if let Some(cwd_path) = cwd {
            cmd.current_dir(cwd_path);
        }

        // Spawn the process
        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::connection_error(command, e))?;

        // Extract stdin and stdout handles
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Failed to get stdin handle".to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::InvalidProtocol {
                message: "Failed to get stdout handle".to_string(),
            })?;

        // Return StdioTransport
        Ok(StdioTransport {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn receive_notification(&mut self) -> Result<serde_json::Value> {
        use std::time::Duration;
        use tokio::time::timeout;

        let mut line = String::new();
        let notification = timeout(Duration::from_secs(10), async move {
            let _ = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| McpError::connection_error("stdio", e))?;
            if line.trim().is_empty() {
                return Err(McpError::InvalidProtocol {
                    message: "Empty notification line".to_string(),
                });
            }
            let notification: serde_json::Value =
                serde_json::from_str(&line).map_err(|e| McpError::InvalidProtocol {
                    message: format!("Invalid JSON notification: {}", e),
                })?;
            Ok(notification)
        })
        .await;

        match notification {
            Ok(Ok(n)) => Ok(n),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(McpError::Timeout { timeout: 10 }),
        }
    }

    async fn send(&mut self, request: serde_json::Value) -> Result<serde_json::Value> {
        // Send request using write! + newline (newline-delimited JSON)
        let request_str = request.to_string();
        use tokio::io::AsyncWriteExt;
        self.stdin
            .write_all(request_str.as_bytes())
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;

        // Flush stdin to ensure message is sent
        self.stdin
            .flush()
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;

        // Read response (newline-delimited JSON)
        let response_str = tokio::time::timeout(Duration::from_secs(30), async move {
            let mut line = String::new();
            let _ = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| McpError::connection_error("stdio", e))?;
            if line.trim().is_empty() {
                return Err(McpError::InvalidProtocol {
                    message: "Empty response line".to_string(),
                });
            }
            Ok(line)
        })
        .await
        .map_err(|_| McpError::Timeout { timeout: 30 })?;

        // Parse response JSON
        let response_str = response_str?;
        let response: serde_json::Value =
            serde_json::from_str(&response_str).map_err(|e| McpError::InvalidProtocol {
                message: format!("Invalid JSON response: {}", e),
            })?;

        Ok(response)
    }

    async fn send_notification(&mut self, notification: serde_json::Value) -> Result<()> {
        // Send notification using write! + newline (newline-delimited JSON)
        let notification_str = notification.to_string();
        use tokio::io::AsyncWriteExt;
        self.stdin
            .write_all(notification_str.as_bytes())
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;

        // Flush stdin to ensure message is sent
        self.stdin
            .flush()
            .await
            .map_err(|e| McpError::connection_error("stdio", e))?;

        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        // Create a minimal ping request
        let _request = serde_json::json!({
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

impl TransportFactory for ServerTransport {
    fn create_transport(&self, _server_name: &str) -> Box<dyn Transport + Send + Sync> {
        match self {
            ServerTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                let transport = StdioTransport::new(command, args, env, cwd.as_deref())
                    .expect("Failed to create stdio transport");
                Box::new(transport)
            }
            ServerTransport::Http { url, headers } => {
                let transport = HttpTransport::new(url, headers.clone());
                Box::new(transport)
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
