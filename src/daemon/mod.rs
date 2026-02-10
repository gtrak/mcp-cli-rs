use anyhow::Result;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::config::Config;
use crate::daemon::protocol::{DaemonRequest, DaemonResponse};
use crate::ipc::{create_ipc_server, IpcServer};
use crate::daemon::lifecycle::DaemonLifecycle;
use crate::daemon::pool::ConnectionPool;

pub mod protocol;
pub mod lifecycle;
pub mod pool;
pub mod fingerprint;
pub mod orphan;

/// Configuration fingerprint hash
pub type ConfigFingerprint = String;

/// Daemon state managed in run_daemon()
#[derive(Clone)]
pub struct DaemonState {
    /// Loaded configuration
    pub config: Arc<Config>,
    /// Config file content fingerprint for validation
    pub config_fingerprint: ConfigFingerprint,
    /// Lifecycle manager for idle timeout
    pub lifecycle: DaemonLifecycle,
    /// Connection pool for persistent MCP server connections
    pub connection_pool: Arc<crate::daemon::pool::ConnectionPool>,
}

impl DaemonState {
    /// Update activity timestamp
    pub fn update_activity(&self) {
        self.lifecycle.update_activity();
    }

    /// Check if daemon should shutdown
    pub fn should_shutdown(&self) -> bool {
        self.lifecycle.should_shutdown()
    }

    /// Signal that daemon should shut down
    pub fn shutdown(&self) {
        self.lifecycle.shutdown();
    }

    /// Check if daemon is running
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }
}

/// Run the daemon with IPC server and idle timeout
///
/// This function:
/// 1. Creates an IPC server (Unix socket on Unix, named pipe on Windows)
/// 2. Calculates config fingerprint
/// 3. Spawns idle timeout monitor
/// 4. Main loop accepts connections and handles requests
/// 5. Removes socket file on exit
pub async fn run_daemon(config: Config, socket_path: PathBuf, lifecycle: DaemonLifecycle) -> Result<()> {
    tracing::info!("Starting daemon with socket: {:?}", socket_path);

    // Create IPC server
    let ipc_server: Box<dyn IpcServer> = create_ipc_server(&socket_path)?;
    tracing::info!("IPC server started on: {:?}", socket_path);

    // Calculate config fingerprint
    let config_fingerprint = config_fingerprint(&config);
    tracing::info!("Config fingerprint: {}", config_fingerprint);

    // Get current process PID
    let pid = std::process::id();
    tracing::info!("Daemon PID: {}", pid);

    // Write PID to file for orphan detection
    let _ = crate::daemon::orphan::write_daemon_pid(&socket_path, pid);
    tracing::info!("PID file written");

    // Spawn idle timeout monitor
    let lifecycle_clone = lifecycle.clone();
    let lifecycle_task = tokio::spawn(async move {
        crate::daemon::lifecycle::run_idle_timer(&lifecycle_clone).await;
    });

    // Initialize connection pool
    let connection_pool = Arc::new(ConnectionPool::new(Arc::new(config.clone())));

    let state = DaemonState {
        config: Arc::new(config),
        config_fingerprint: config_fingerprint.clone(),
        lifecycle,
        connection_pool,
    };

    tracing::info!("Daemon main loop starting");

    // Main loop: accept connections or wait for shutdown signal
    loop {
        tokio::select! {
            // Accept new connection
            result = ipc_server.accept() => {
                match result {
                    Ok((stream, client_addr)) => {
                        tracing::debug!("Accepted connection from: {}", client_addr);
                        let state_clone = state.clone();
                        let stream_clone = stream;
                        tokio::spawn(async move {
                            handle_client(stream_clone, state_clone).await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Error accepting connection: {}", e);
                        // Check if we should shutdown
                        if state.should_shutdown() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            // Wait for shutdown timeout
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                // Check if we should shutdown
                if state.should_shutdown() {
                    break;
                }
            }
        }
    }

    tracing::info!("Daemon shutting down, removing resource files");
    let socket_path_clone = socket_path.clone();
    cleanup_socket(socket_path_clone).await?;

    // Remove PID file
    let _ = crate::daemon::orphan::remove_pid_file(&socket_path);
    tracing::info!("PID file removed");

    // Remove fingerprint file
    let _ = crate::daemon::orphan::remove_fingerprint_file(&socket_path);
    tracing::info!("Fingerprint file removed");

    tracing::info!("Daemon shutdown complete");
    Ok(())
}

/// Handle client requests
pub async fn handle_client(
    stream: impl crate::ipc::IpcStream + Unpin,
    state: DaemonState,
) {
    use tokio::io::BufReader;
    
    // Update activity timestamp
    state.update_activity();

    // Wrap stream for buffered reading
    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = BufReader::new(reader);

    // Read request from stream
    let request = match crate::daemon::protocol::receive_request(&mut buf_reader).await {
        Ok(req) => req,
        Err(e) => {
            tracing::warn!("Error reading request: {}", e);
            return;
        }
    };

    // Handle request
    let response = handle_request(request, &state).await;

    // Send response
    if let Err(e) = crate::daemon::protocol::send_response(&mut writer, &response).await {
        tracing::warn!("Error sending response: {}", e);
        return;
    }

    // Update activity timestamp
    state.update_activity();
}

/// Handle daemon request and return response
pub async fn handle_request(request: crate::daemon::protocol::DaemonRequest, state: &DaemonState)
    -> crate::daemon::protocol::DaemonResponse
{
    match request {
        crate::daemon::protocol::DaemonRequest::Ping => {
            crate::daemon::protocol::DaemonResponse::Pong
        }

        crate::daemon::protocol::DaemonRequest::GetConfigFingerprint => {
            crate::daemon::protocol::DaemonResponse::ConfigFingerprint(state.config_fingerprint.clone())
        }

        crate::daemon::protocol::DaemonRequest::Shutdown => {
            tracing::info!("Shutdown requested by client");
            // Shutdown the lifecycle
            state.shutdown();
            crate::daemon::protocol::DaemonResponse::ShutdownAck
        }

        crate::daemon::protocol::DaemonRequest::ExecuteTool { server_name, tool_name, arguments } => {
            tracing::info!("ExecuteTool: server={}, tool={}", server_name, tool_name);

            // Get transport from connection pool
            let mut transport: Box<dyn crate::transport::Transport + Send + Sync> = match state.connection_pool.get(&server_name).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get transport for {}: {}", server_name, e);
                    return crate::daemon::protocol::DaemonResponse::Error {
                        code: 2,
                        message: format!("Server not found or connection failed: {}", e),
                    };
                }
            };

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
                        "name": "mcp-cli-rs-daemon",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            });

            // Send initialize and get response
            if let Err(e) = transport.send(init_request).await {
                tracing::error!("Failed to initialize MCP connection for {}: {}", server_name, e);
                return crate::daemon::protocol::DaemonResponse::Error {
                    code: 3,
                    message: format!("Failed to initialize MCP connection: {}", e),
                };
            }

            // Build MCP tools/call JSON-RPC request
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
            match transport.send(mcp_request).await {
                Ok(response) => {
                    // Parse JSON-RPC response
                    if let Some(result) = response.get("result") {
                        // Success - return tool result
                        crate::daemon::protocol::DaemonResponse::ToolResult(result.clone())
                    } else if let Some(error) = response.get("error") {
                        // MCP server returned error
                        let message = error.get("message")
                            .and_then(|m: &serde_json::Value| m.as_str())
                            .unwrap_or("Unknown error");
                        tracing::error!("Tool execution failed: {}", message);
                        crate::daemon::protocol::DaemonResponse::Error {
                            code: error.get("code").and_then(|c: &serde_json::Value| c.as_u64()).unwrap_or(3) as u32,
                            message: message.to_string(),
                        }
                    } else {
                        // Invalid response format
                        tracing::error!("Invalid MCP response: missing result and error fields");
                        crate::daemon::protocol::DaemonResponse::Error {
                            code: 3,
                            message: "Invalid MCP response format".to_string(),
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Tool execution failed: {}", e);
                    crate::daemon::protocol::DaemonResponse::Error {
                        code: 3,
                        message: format!("Tool execution failed: {}", e),
                    }
                }
            }
        }

        crate::daemon::protocol::DaemonRequest::ListTools { server_name } => {
            tracing::info!("ListTools: server={}", server_name);

            // Get transport from connection pool
            let mut transport: Box<dyn crate::transport::Transport + Send + Sync> = match state.connection_pool.get(&server_name).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to get transport for {}: {}", server_name, e);
                    return crate::daemon::protocol::DaemonResponse::Error {
                        code: 2,
                        message: format!("Server not found or connection failed: {}", e),
                    };
                }
            };

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
                        "name": "mcp-cli-rs-daemon",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            });

            // Send initialize and get response
            if let Err(e) = transport.send(init_request).await {
                tracing::error!("Failed to initialize MCP connection for {}: {}", server_name, e);
                return crate::daemon::protocol::DaemonResponse::Error {
                    code: 3,
                    message: format!("Failed to initialize MCP connection: {}", e),
                };
            }

            // Build MCP tools/list JSON-RPC request
            let mcp_request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list"
            });

            // Send request and get response
            match transport.send(mcp_request).await {
                Ok(response) => {
                    // Parse JSON-RPC response
                    if let Some(result) = response.get("result") {
                        // Extract tools array from result
                        let tools = if let Some(tools_array) = result.get("tools").and_then(|t: &serde_json::Value| t.as_array()) {
                            tools_array.iter().filter_map(|tool: &serde_json::Value| {
                                Some(crate::daemon::protocol::ToolInfo {
                                    name: tool.get("name")
                                        .and_then(|n: &serde_json::Value| n.as_str())
                                        .unwrap_or("unknown")
                                        .to_string(),
                                    description: tool.get("description")
                                        .and_then(|d: &serde_json::Value| d.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    input_schema: tool.get("inputSchema")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                                })
                            }).collect()
                        } else {
                            Vec::new()
                        };

                        crate::daemon::protocol::DaemonResponse::ToolList(tools)
                    } else if let Some(error) = response.get("error") {
                        // MCP server returned error
                        let message = error.get("message")
                            .and_then(|m: &serde_json::Value| m.as_str())
                            .unwrap_or("Unknown error");
                        tracing::error!("List tools failed: {}", message);
                        crate::daemon::protocol::DaemonResponse::Error {
                            code: error.get("code").and_then(|c: &serde_json::Value| c.as_u64()).unwrap_or(3) as u32,
                            message: message.to_string(),
                        }
                    } else {
                        // Invalid response format
                        tracing::error!("Invalid MCP response: missing result and error fields");
                        crate::daemon::protocol::DaemonResponse::Error {
                            code: 3,
                            message: "Invalid MCP response format".to_string(),
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("List tools failed: {}", e);
                    crate::daemon::protocol::DaemonResponse::Error {
                        code: 3,
                        message: format!("List tools failed: {}", e),
                    }
                }
            }
        }

        crate::daemon::protocol::DaemonRequest::ListServers => {
            tracing::info!("ListServers requested");

            // Get list of configured server names from config
            let servers: Vec<String> = state.config.servers.iter()
                .map(|s| s.name.clone())
                .collect();

            crate::daemon::protocol::DaemonResponse::ServerList(servers)
        }
    }
}

/// Calculate config file fingerprint using SHA256
fn config_fingerprint(config: &Config) -> String {
    use sha2::{Digest, Sha256};

    // Serialize config to JSON
    let json = serde_json::to_string(config).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();

    // Convert to hex string
    hex::encode(result)
}

/// Clean up socket file on daemon exit
pub async fn cleanup_socket(socket_path: PathBuf) -> Result<()> {
    // Try to remove socket file
    let result = std::fs::remove_file(&socket_path);
    match result {
        Ok(()) => {
            tracing::debug!("Socket file removed: {:?}", socket_path);
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            tracing::debug!("Socket file not found, skipping cleanup: {:?}", socket_path);
        }
        Err(e) => {
            tracing::warn!("Failed to remove socket file: {}", e);
            // Don't return error - daemon shutdown is normal even if socket cleanup fails
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_fingerprint() {
        let config = Config::default();
        let fp = config_fingerprint(&config);
        assert!(!fp.is_empty());
    }

    #[tokio::test]
    async fn test_handle_request_ping() {
        let lifecycle = DaemonLifecycle::new(30);
        let config = Config::default();
        let state = DaemonState {
            config: Arc::new(config),
            config_fingerprint: String::new(),
            lifecycle,
            connection_pool: Arc::new(crate::daemon::pool::ConnectionPool::new(Arc::new(Config::default()))),
        };

        let response = handle_request(DaemonRequest::Ping, &state).await;
        assert!(matches!(response, DaemonResponse::Pong));
    }

    #[tokio::test]
    async fn test_handle_request_shutdown() {
        let lifecycle = DaemonLifecycle::new(30);
        let config = Config::default();
        let state = DaemonState {
            config: Arc::new(config),
            config_fingerprint: String::new(),
            lifecycle: lifecycle.clone(),
            connection_pool: Arc::new(crate::daemon::pool::ConnectionPool::new(Arc::new(Config::default()))),
        };

        let response = handle_request(DaemonRequest::Shutdown, &state).await;
        assert!(matches!(response, DaemonResponse::ShutdownAck));
        assert!(!lifecycle.is_running());
    }
}

/// Remove PID file
pub fn remove_pid_file(socket_path: &PathBuf) -> Result<()> {
    let pid_file = crate::daemon::orphan::get_pid_file_path(socket_path);
    if pid_file.exists() {
        if let Err(e) = std::fs::remove_file(&pid_file) {
            tracing::warn!("Failed to remove PID file: {}", e);
        }
    }
    Ok(())
}

/// Remove fingerprint file
pub fn remove_fingerprint_file(socket_path: &PathBuf) -> Result<()> {
    let fp_file = crate::daemon::orphan::get_fingerprint_file_path(socket_path);
    if fp_file.exists() {
        if let Err(e) = std::fs::remove_file(&fp_file) {
            tracing::warn!("Failed to remove fingerprint file: {}", e);
        }
    }
    Ok(())
}
