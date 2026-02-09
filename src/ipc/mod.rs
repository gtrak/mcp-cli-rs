//! Platform-agnostic IPC abstraction for daemon communication

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use std::path::Path;
use std::sync::Arc;
use crate::error::McpError;
use crate::config::Config;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// Stream abstraction for IPC communication
///
/// Wraps platform-specific stream types to provide a unified interface
/// Note: For AsyncBufRead operations, wrap in tokio::io::BufReader
pub trait IpcStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

// Implement IpcStream for Box<dyn IpcStream> so boxed streams can be used directly
impl IpcStream for Box<dyn IpcStream> {}

/// Server trait for accepting IPC connections
///
/// Implementations provide platform-specific connection handling
#[async_trait]
pub trait IpcServer: Send + Sync {
    /// Accept an incoming connection
    ///
    /// Returns a boxed stream and address string for the connection
    async fn accept(&self) -> Result<(Box<dyn IpcStream>, String), McpError>;
}

/// Client trait for connecting to IPC servers
///
/// Implementations provide platform-specific connection logic
/// Note: This trait is not object-safe due to generic protocol communication requirements
#[async_trait]
pub trait IpcClient: Send + Sync {
    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, path: &Path) -> Result<Box<dyn IpcStream>, McpError>;

    /// Get the configuration associated with this client
    ///
    /// Returns the Config for validation and server listing
    fn config(&self) -> Arc<Config>;

    /// Send a daemon protocol request and receive response
    ///
    /// Generic method for NDJSON communication
    async fn send_request(&mut self, request: &crate::daemon::protocol::DaemonRequest) -> Result<crate::daemon::protocol::DaemonResponse, McpError>;
}

/// Wrapper struct that implements concrete protocol methods using a generic IpcClient
///
/// This allows Box<dyn IpcClient> to have protocol-specific methods added to it
#[derive(Clone)]
pub struct IpcClientWrapper<T: Clone> {
    client: T,
    config: Arc<Config>,
}

impl<T: Clone + IpcClient> IpcClientWrapper<T> {
    pub fn new(client: T, config: Arc<Config>) -> Self {
        Self { client, config }
    }

    /// Create wrapper with config (convenience method for wrapped clients)
    pub fn with_config(client: T, config: Arc<Config>) -> Self {
        Self { client, config }
    }

    /// Get the configuration associated with this client
    fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// List all configured servers
    pub async fn list_servers(&mut self) -> Result<Vec<String>, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ListServers).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ServerList(servers) => Ok(servers),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ServerList response, got {:?}", response),
            }),
        }
    }

    /// List tools for a specific server
    pub async fn list_tools(&mut self, server_name: &str) -> Result<Vec<crate::daemon::protocol::ToolInfo>, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ListTools {
            server_name: server_name.to_string(),
        }).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ToolList(tools) => Ok(tools),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ToolList response for '{}', got {:?}", server_name, response),
            }),
        }
    }

    /// Execute a tool on a server
    pub async fn execute_tool(&mut self, server_name: &str, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ExecuteTool {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
        }).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ToolResult(result) => Ok(result),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ToolResult for '{}.{}', got {:?}", server_name, tool_name, response),
            }),
        }
    }
}

/// Trait for protocol-specific client methods
///
/// This trait wraps the IpcClient trait to provide protocol methods as a trait object
#[async_trait]
pub trait ProtocolClient: Send + Sync {
    fn config(&self) -> Arc<Config>;
    async fn send_request(&mut self, request: &crate::daemon::protocol::DaemonRequest) -> Result<crate::daemon::protocol::DaemonResponse, McpError>;
    async fn list_servers(&mut self) -> Result<Vec<String>, McpError>;
    async fn list_tools(&mut self, server_name: &str) -> Result<Vec<crate::daemon::protocol::ToolInfo>, McpError>;
    async fn execute_tool(&mut self, server_name: &str, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value, McpError>;
}

#[async_trait]
impl<T: IpcClient + Send + Sync + Clone> ProtocolClient for IpcClientWrapper<T> {
    fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    async fn send_request(&mut self, request: &crate::daemon::protocol::DaemonRequest) -> Result<crate::daemon::protocol::DaemonResponse, McpError> {
        self.client.send_request(request).await
    }

    async fn list_servers(&mut self) -> Result<Vec<String>, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ListServers).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ServerList(servers) => Ok(servers),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ServerList response, got {:?}", response),
            }),
        }
    }

    async fn list_tools(&mut self, server_name: &str) -> Result<Vec<crate::daemon::protocol::ToolInfo>, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ListTools {
            server_name: server_name.to_string(),
        }).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ToolList(tools) => Ok(tools),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ToolList response for '{}', got {:?}", server_name, response),
            }),
        }
    }

    async fn execute_tool(&mut self, server_name: &str, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value, McpError> {
        let response = self.client.send_request(&crate::daemon::protocol::DaemonRequest::ExecuteTool {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
        }).await?;
        match response {
            crate::daemon::protocol::DaemonResponse::ToolResult(result) => Ok(result),
            _ => Err(crate::error::McpError::InvalidProtocol {
                message: format!("Expected ToolResult for '{}.{}', got {:?}", server_name, tool_name, response),
            }),
        }
    }
}

/// Factory function to create platform-specific IPC client wrapper
///
/// Returns Box<dyn ProtocolClient> with platform-specific implementation
#[cfg(unix)]
pub fn create_ipc_client(config: Arc<Config>) -> Result<Box<dyn ProtocolClient>, McpError> {
    let client = crate::ipc::UnixIpcClient::new(config.clone());
    Ok(Box::new(crate::ipc::IpcClientWrapper::with_config(client, config)))
}

/// Factory function to create platform-specific IPC client wrapper
///
/// Returns Box<dyn ProtocolClient> with platform-specific implementation
#[cfg(windows)]
pub fn create_ipc_client(config: Arc<Config>) -> Result<Box<dyn ProtocolClient>, McpError> {
    let client = crate::ipc::NamedPipeIpcClient::with_config(config.clone());
    Ok(Box::new(crate::ipc::IpcClientWrapper::with_config(client, config)))
}

/// Factory function to create platform-specific IPC server
///
/// Returns Box<dyn IpcServer> with platform-specific implementation
#[cfg(windows)]
pub fn create_ipc_server(path: &Path) -> Result<Box<dyn IpcServer>, McpError> {
    Ok(Box::new(crate::ipc::windows::NamedPipeIpcServer::new(path)?))
}

/// Get platform-specific socket path for IPC communication
///
/// Returns a PathBuf for the socket file on Unix systems
#[cfg(unix)]
pub fn get_socket_path() -> std::path::PathBuf {
    // Use /run/user/{uid}/mcp-cli/daemon.sock or /tmp/mcp-cli-{uid}/daemon.sock
    // Prefer /run for Linux systems with XDG_RUNTIME_DIR support
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        runtime_dir.join("mcp-cli").join("daemon.sock")
    } else {
        // Fallback to tmpdir based on UID
        let uid = nix::unistd::Uid::effective().as_raw();
        std::path::PathBuf::from(format!("/tmp/mcp-cli-{}", uid))
            .join("daemon.sock")
    }
}

/// Get platform-specific socket path for IPC communication
///
/// Returns a PathBuf for the socket file on Windows systems
#[cfg(windows)]
pub fn get_socket_path() -> std::path::PathBuf {
    // Use a consistent named pipe name across all CLI and daemon processes
    // Store just the pipe name without the UNC prefix
    std::path::PathBuf::from("mcp-cli-daemon")
}

/// Re-export platform-specific implementations
#[cfg(unix)]
pub use unix::{UnixIpcServer, UnixIpcClient};

#[cfg(windows)]
pub use windows::{NamedPipeIpcServer, NamedPipeIpcClient};
