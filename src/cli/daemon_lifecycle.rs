//! Daemon lifecycle management for CLI.
//!
//! Provides functions for starting, stopping, and managing
//! the MCP daemon process lifecycle.

use crate::config::Config;
use crate::error::{McpError, Result};
use crate::ipc::{ProtocolClient, create_ipc_client};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Run in direct mode: create a direct client without daemon.
///
/// Returns a connected ProtocolClient for direct server communication.
///
/// # Arguments
/// * `config` - Application configuration (wrapped in Arc)
///
/// # Returns
/// * `Ok(Box<dyn ProtocolClient>)` - Direct client
/// * `Err(McpError)` - Error
pub async fn create_direct_client(config: Arc<Config>) -> Result<Box<dyn ProtocolClient>> {
    let direct_client = Box::new(DirectProtocolClient::new(config)) as Box<dyn ProtocolClient>;
    Ok(direct_client)
}

/// Run in auto-daemon mode: connect to daemon or spawn one if needed.
///
/// Returns a connected ProtocolClient. The daemon auto-shuts down after TTL.
///
/// # Arguments
/// * `config` - Application configuration
///
/// # Returns
/// * `Ok(Box<dyn ProtocolClient>)` - Connected client
/// * `Err(McpError)` - Daemon connection or spawning error
pub async fn create_auto_daemon_client(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    connect_or_spawn_daemon(config).await
}

/// Run in require-daemon mode: connect to existing daemon only.
///
/// Returns a connected ProtocolClient. Fails if daemon not running.
///
/// # Arguments
/// * `config` - Application configuration
///
/// # Returns
/// * `Ok(Box<dyn ProtocolClient>)` - Connected client
/// * `Err(McpError)` - Daemon not running error
pub async fn create_require_daemon_client(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    connect_to_daemon(config).await
}

/// Connect to an existing daemon or spawn one if needed (auto-daemon mode).
///
/// Returns a connected ProtocolClient that can be used to execute commands.
/// The caller is responsible for executing the command and handling the result.
///
/// # Arguments
/// * `config` - Application configuration
///
/// # Returns
/// * `Ok(Box<dyn ProtocolClient>)` - Connected client
/// * `Err(McpError)` - Daemon connection or spawning error
pub async fn connect_or_spawn_daemon(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    tracing::debug!("connect_or_spawn_daemon called");

    // Check if daemon is running
    match try_connect_to_daemon(config).await {
        Ok(client) => {
            // Daemon is running, use it
            tracing::info!("Using existing daemon");
            Ok(client)
        }
        Err(_) => {
            // Daemon not running, spawn it
            tracing::info!("Daemon not running, spawning...");

            // Get TTL from config (includes env var override via config loader)
            // Set minimum TTL of 5 seconds for auto-daemon mode to prevent race conditions
            let mut ttl = config.daemon_ttl;
            if ttl < 5 {
                tracing::warn!(
                    "Auto-daemon TTL too short ({}s), setting minimum of 5s to prevent race conditions",
                    ttl
                );
                ttl = 5;
            }

            // Spawn daemon as background task
            tracing::debug!("Spawning daemon with TTL={}s...", ttl);

            // Clone socket_path for the async block
            let socket_path = config.socket_path.clone();
            tokio::spawn(async move {
                tracing::debug!("Inside tokio::spawn, about to spawn daemon...");
                match spawn_background_daemon(ttl, &socket_path).await {
                    Ok(_) => tracing::debug!("spawn_background_daemon returned Ok"),
                    Err(e) => tracing::debug!("spawn_background_daemon failed: {}", e),
                }
            });

            // Wait for daemon to start with exponential backoff
            let mut retries = 0;
            let max_retries = 20; // More retries
            let mut delay = Duration::from_millis(500); // Start with longer delay

            loop {
                tokio::time::sleep(delay).await;

                match try_connect_to_daemon(config).await {
                    Ok(client) => {
                        tracing::info!("Connected to daemon after {} attempt(s)", retries + 1);
                        return Ok(client);
                    }
                    Err(e) => {
                        retries += 1;
                        if retries >= max_retries {
                            return Err(McpError::IOError {
                                source: std::io::Error::other(format!(
                                    "Failed to start daemon after {} attempts: {}",
                                    max_retries, e
                                )),
                            });
                        }
                        // Linear backoff: add 200ms each time, cap at 2 seconds
                        delay += Duration::from_millis(200);
                        if delay > Duration::from_secs(2) {
                            delay = Duration::from_secs(2);
                        }
                        tracing::debug!(
                            "Daemon not ready, retrying in {:?} (attempt {}/{})",
                            delay,
                            retries,
                            max_retries
                        );
                    }
                }
            }
        }
    }
}

/// Connect to an existing daemon (require-daemon mode).
///
/// Returns a connected ProtocolClient if daemon is running.
/// Fails if daemon is not already running.
///
/// # Arguments
/// * `config` - Application configuration
///
/// # Returns
/// * `Ok(Box<dyn ProtocolClient>)` - Connected client
/// * `Err(McpError)` - Daemon not running error
pub async fn connect_to_daemon(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    match try_connect_to_daemon(config).await {
        Ok(client) => {
            tracing::info!("Using existing daemon");
            Ok(client)
        }
        Err(_) => Err(McpError::daemon_not_running(
            "Daemon is not running. Start it with 'mcp daemon' or use --auto-daemon",
        )),
    }
}

async fn try_connect_to_daemon(config: &Config) -> Result<Box<dyn ProtocolClient>> {
    let client = create_ipc_client(config)?;

    // Actually verify the connection works by sending a ping
    let mut test_client = client;
    match test_client.list_servers().await {
        Ok(_) => Ok(test_client),
        Err(e) => Err(e),
    }
}

async fn spawn_background_daemon(ttl: u64, socket_path: &Path) -> Result<()> {
    // Spawn the daemon as a separate process using the binary itself
    // This is necessary because the daemon runs an IPC server that needs
    // to be independent of the client process

    // Get the current executable path
    let current_exe = std::env::current_exe().map_err(|e| McpError::IOError {
        source: std::io::Error::other(format!("Failed to get executable path: {}", e)),
    })?;

    // Build arguments for daemon subcommand - pass socket path explicitly
    // to ensure daemon uses the same IPC endpoint as the client expects
    let socket_path_str = socket_path.to_string_lossy().to_string();
    let args = vec![
        "daemon".to_string(),
        "--socket-path".to_string(),
        socket_path_str,
    ];

    // Spawn the daemon process
    tracing::info!(
        "Spawning daemon process: {:?} daemon (TTL: {}s, socket: {:?})",
        current_exe,
        ttl,
        socket_path
    );

    tracing::debug!("Spawning daemon: {:?} with args: {:?}", current_exe, args);

    // Get current working directory so daemon can find config
    let current_dir = std::env::current_dir().map_err(|e| McpError::IOError {
        source: std::io::Error::other(format!("Failed to get current directory: {}", e)),
    })?;

    // On Windows, we need to use a different approach to spawn a truly independent process
    // Using CREATE_NEW_PROCESS_GROUP and CREATE_NO_WINDOW flags
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut cmd = std::process::Command::new(&current_exe);
        cmd.args(&args)
            .env("MCP_DAEMON_TTL", ttl.to_string())
            .current_dir(&current_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);

        let _child = cmd.spawn().map_err(|e| McpError::IOError {
            source: std::io::Error::other(format!("Failed to spawn daemon: {}", e)),
        })?;

        tracing::debug!("Daemon spawned with PID: {:?}", _child.id());
    }

    #[cfg(not(windows))]
    {
        let _child = tokio::process::Command::new(&current_exe)
            .args(&args)
            .env("MCP_DAEMON_TTL", ttl.to_string())
            .current_dir(&current_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(false)
            .spawn()
            .map_err(|e| McpError::IOError {
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to spawn daemon: {}", e),
                ),
            })?;

        tracing::debug!("Daemon spawned with PID: {:?}", _child.id());
    }

    // Give the daemon time to create the named pipe
    // This is critical - the daemon needs time to start the IPC server
    tracing::debug!("Waiting for daemon to initialize...");
    tokio::time::sleep(Duration::from_millis(1000)).await;

    Ok(())
}

/// Direct protocol client that connects to servers without daemon
pub struct DirectProtocolClient {
    config: Arc<Config>,
}

impl DirectProtocolClient {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl ProtocolClient for DirectProtocolClient {
    fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    async fn send_request(
        &mut self,
        _request: &crate::daemon::protocol::DaemonRequest,
    ) -> Result<crate::daemon::protocol::DaemonResponse> {
        // Direct mode doesn't use daemon protocol - commands handle connections directly
        Err(McpError::InvalidProtocol {
            message: "Direct mode doesn't support daemon protocol requests".to_string(),
        })
    }

    async fn list_servers(&mut self) -> Result<Vec<String>> {
        let servers: Vec<String> = self.config.servers.iter().map(|s| s.name.clone()).collect();
        Ok(servers)
    }

    async fn list_tools(
        &mut self,
        server_name: &str,
    ) -> Result<Vec<crate::daemon::protocol::ToolInfo>> {
        // Get server config and create transport directly
        let server_config =
            self.config
                .get_server(server_name)
                .ok_or_else(|| McpError::ServerNotFound {
                    server: server_name.to_string(),
                })?;

        let mut transport = server_config.create_transport(server_name)?;

        // MCP Protocol: Send initialize request first
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        // Send initialize and get response
        transport
            .send(init_request)
            .await
            .map_err(|e| McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Server automatically sends notifications/initialized - we don't need to send it
        // Now send tools/list request
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        // Send request and get response
        let response = transport
            .send(mcp_request)
            .await
            .map_err(|e| McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Parse response
        if let Some(result) = response.get("result") {
            let tools = if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array
                    .iter()
                    .filter_map(|tool| {
                        Some(crate::daemon::protocol::ToolInfo {
                            name: tool.get("name")?.as_str()?.to_string(),
                            description: tool
                                .get("description")?
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            input_schema: tool.get("inputSchema").cloned().unwrap_or_else(|| {
                                serde_json::Value::Object(serde_json::Map::new())
                            }),
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Ok(tools)
        } else {
            Err(McpError::InvalidProtocol {
                message: "Invalid MCP response format".to_string(),
            })
        }
    }

    async fn execute_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Get server config and create transport directly
        let server_config =
            self.config
                .get_server(server_name)
                .ok_or_else(|| McpError::ServerNotFound {
                    server: server_name.to_string(),
                })?;

        let mut transport = server_config.create_transport(server_name)?;

        // MCP Protocol: Send initialize request first
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        // Send initialize and get response
        transport
            .send(init_request)
            .await
            .map_err(|e| McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Server automatically sends notifications/initialized - we don't need to send it
        // Now send tools/call request
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        // Send request and get response
        let response = transport
            .send(mcp_request)
            .await
            .map_err(|e| McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Parse response
        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            Err(McpError::InvalidProtocol {
                message: format!("Tool execution failed: {}", message),
            })
        } else {
            Err(McpError::InvalidProtocol {
                message: "Invalid MCP response format".to_string(),
            })
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Direct mode doesn't support daemon shutdown
        Err(McpError::InvalidProtocol {
            message: "Direct mode doesn't support daemon shutdown".to_string(),
        })
    }
}
