use anyhow::Result;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::daemon::lifecycle::DaemonLifecycle;
use crate::daemon::pool::ConnectionPool;
use crate::ipc::{IpcServer, create_ipc_server};

pub mod lifecycle;

pub mod pool;
pub mod protocol;

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
    pub lifecycle: Arc<Mutex<DaemonLifecycle>>,
    /// Connection pool for persistent MCP server connections
    pub connection_pool: Arc<crate::daemon::pool::ConnectionPool>,
}

impl DaemonState {
    /// Update activity timestamp
    pub async fn update_activity(&self) {
        self.lifecycle.lock().await.update_activity().await;
    }

    /// Signal that daemon should shut down
    pub async fn shutdown(&self) {
        self.lifecycle.lock().await.shutdown();
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
pub async fn run_daemon(
    config: Config,
    socket_path: PathBuf,
    lifecycle: DaemonLifecycle,
) -> Result<()> {
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

    // Initialize connection pool
    let connection_pool = Arc::new(ConnectionPool::new(Arc::new(config.clone())));

    let state = DaemonState {
        config: Arc::new(config),
        config_fingerprint: config_fingerprint.clone(),
        lifecycle: Arc::new(Mutex::new(lifecycle)),
        connection_pool,
    };

    let state2 = state.clone();
    // Spawn idle timeout monitor
    let _lifecycle_task = tokio::spawn(async move {
        crate::daemon::lifecycle::run_idle_timer(state2.lifecycle).await;
    });

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
                        if state.lifecycle.lock().await.should_shutdown().await {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            // Wait for shutdown timeout
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                // Check if we should shutdown
                if state.lifecycle.lock().await.should_shutdown().await {
                    break;
                }
            }
        }
    }

    tracing::info!("Daemon shutting down, removing resource files");
    let socket_path_clone = socket_path.clone();
    cleanup_socket(socket_path_clone).await?;

    tracing::info!("Daemon shutdown complete");
    Ok(())
}

/// Handle client requests
pub async fn handle_client(stream: impl crate::ipc::IpcStream, state: DaemonState) {
    use tokio::io::BufReader;
    tracing::debug!("Daemon: New client connected");

    // Update activity timestamp
    state.update_activity().await;

    // Wrap stream for buffered reading
    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = BufReader::new(reader);

    // Read request from stream
    tracing::debug!("Daemon: Reading request...");
    let request = match crate::daemon::protocol::receive_request(&mut buf_reader).await {
        Ok(req) => {
            tracing::debug!("Daemon: Got request: {:?}", req);
            req
        }
        Err(e) => {
            tracing::debug!("Daemon: Error reading request: {}", e);
            return;
        }
    };

    // Handle request
    tracing::debug!("Daemon: Handling request...");
    let response = handle_request(request, &state).await;
    tracing::debug!("Daemon: Generated response: {:?}", response);

    // Send response
    tracing::debug!("Daemon: Sending response...");
    if let Err(e) = crate::daemon::protocol::send_response(&mut writer, &response).await {
        tracing::debug!("Daemon: Error sending response: {}", e);
        return;
    }
    tracing::debug!("Daemon: Response sent");

    // Update activity timestamp
    state.update_activity().await;
}

/// Handle daemon request and return response
pub async fn handle_request(
    request: crate::daemon::protocol::DaemonRequest,
    state: &DaemonState,
) -> crate::daemon::protocol::DaemonResponse {
    match request {
        crate::daemon::protocol::DaemonRequest::Ping => {
            crate::daemon::protocol::DaemonResponse::Pong
        }

        crate::daemon::protocol::DaemonRequest::GetConfigFingerprint => {
            crate::daemon::protocol::DaemonResponse::ConfigFingerprint(
                state.config_fingerprint.clone(),
            )
        }

        crate::daemon::protocol::DaemonRequest::Shutdown => {
            tracing::info!("Shutdown requested by client");
            // Shutdown the lifecycle
            state.shutdown().await;
            crate::daemon::protocol::DaemonResponse::ShutdownAck
        }

        crate::daemon::protocol::DaemonRequest::ExecuteTool {
            server_name,
            tool_name,
            arguments,
        } => {
            tracing::info!("ExecuteTool: server={}, tool={}", server_name, tool_name);

            // Execute tool using connection pool's execute method
            match state
                .connection_pool
                .execute(&server_name, &tool_name, arguments)
                .await
            {
                Ok(result) => crate::daemon::protocol::DaemonResponse::ToolResult(result),
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

            // Get list of tools using connection pool's list_tools method
            match state.connection_pool.list_tools(&server_name).await {
                Ok(tools) => crate::daemon::protocol::DaemonResponse::ToolList(tools),
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
            let servers: Vec<String> = state
                .config
                .servers
                .iter()
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
    use crate::daemon::protocol::{DaemonRequest, DaemonResponse};

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
            lifecycle: Arc::new(Mutex::new(lifecycle)),
            connection_pool: Arc::new(crate::daemon::pool::ConnectionPool::new(Arc::new(
                Config::default(),
            ))),
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
            lifecycle: Arc::new(Mutex::new(lifecycle)),
            connection_pool: Arc::new(crate::daemon::pool::ConnectionPool::new(Arc::new(
                Config::default(),
            ))),
        };

        let response = handle_request(DaemonRequest::Shutdown, &state).await;
        assert!(matches!(response, DaemonResponse::ShutdownAck));
    }
}
