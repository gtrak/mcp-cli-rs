//! Windows named pipe implementation for IPC communication
//!
//! Uses tokio::net::windows::named_pipe for cross-platform named pipe IPC

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::windows::named_pipe::{NamedPipeClient, ClientOptions};

use crate::ipc::IpcServer;
use crate::error::McpError;
use crate::config::Config;

/// Windows named pipe implementation of IPC server
///
/// Accepts connections via named pipes on Windows systems
pub struct NamedPipeIpcServer {
    pipe_name: String,
}

impl NamedPipeIpcServer {
    /// Create a new NamedPipeIpcServer with the specified pipe name
    ///
    /// Creates a named pipe that can accept multiple client connections
    pub fn new(path: &Path) -> Result<Self, McpError> {
        // Extract pipe name from the path (remove any directory components)
        let pipe_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| McpError::IpcError {
                message: format!("Invalid pipe path: {}", path.display()),
            })?;

        let pipe_name_display = format!(r"\\.\pipe\{}", pipe_name);

        Ok(Self {
            pipe_name: pipe_name_display,
        })
    }

    /// Get the pipe name for logging/debugging purposes
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }
}

#[async_trait]
impl IpcServer for NamedPipeIpcServer {
    /// Accept an incoming connection
    ///
    /// Returns a boxed stream and address string for the connection
    async fn accept(&self) -> Result<(Box<dyn crate::ipc::IpcStream>, String), McpError> {
        // Create server instance for this connection
        let server = tokio::net::windows::named_pipe::ServerOptions::new()
            .reject_remote_clients(true) // Local connections only
            .create(&self.pipe_name)
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to create named pipe {}: {}", self.pipe_name, e),
            })?;

        // Wait for a client connection
        server
            .connect()
            .await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to accept named pipe connection: {}", e),
            })?;

        // The server becomes the connected pipe for communication
        Ok((Box::new(server) as Box<dyn crate::ipc::IpcStream>, self.pipe_name.clone()))
    }
}

/// Windows named pipe implementation of IPC client
///
/// Connects to IPC servers via named pipes on Windows systems
#[derive(Clone)]
pub struct NamedPipeIpcClient {
    config: Arc<Config>,
    custom_path: Option<PathBuf>,
}

impl NamedPipeIpcClient {
    /// Create a new NamedPipeIpcClient with a config reference (convenience method)
    pub fn with_config(config: Arc<Config>) -> Self {
        Self { config, custom_path: None }
    }

    /// Create a new NamedPipeIpcClient that connects to a specific pipe path (for testing)
    pub fn with_path(config: Arc<Config>, path: PathBuf) -> Self {
        Self { config, custom_path: Some(path) }
    }
}

#[async_trait]
impl crate::ipc::IpcClient for NamedPipeIpcClient {
    /// Get the configuration associated with this client
    fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// Send a daemon protocol request and receive response
    async fn send_request(&mut self, request: &crate::daemon::protocol::DaemonRequest) -> Result<crate::daemon::protocol::DaemonResponse, McpError> {
        // Get daemon named pipe path (use custom path if set)
        let pipe_path = match &self.custom_path {
            Some(path) => path.clone(),
            None => crate::ipc::get_socket_path(),
        };
        tracing::debug!("IPC: Connecting to pipe at {:?}", pipe_path);

        // Connect to daemon
        let mut stream = self.connect(&pipe_path).await?;
        tracing::debug!("IPC: Connected to pipe");

        // Split stream for reading and writing
        use tokio::io::{BufReader};
        let (reader, mut writer) = tokio::io::split(stream);
        let mut buf_reader = BufReader::new(reader);

        // Send request using NDJSON protocol
        tracing::debug!("IPC: Sending request: {:?}", request);
        crate::daemon::protocol::send_request(&mut writer, request).await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to send IPC request: {}", e),
            })?;
        tracing::debug!("IPC: Request sent");

        // Receive response using NDJSON protocol
        tracing::debug!("IPC: Waiting for response...");
        let result = crate::daemon::protocol::receive_response(&mut buf_reader).await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to receive IPC response: {}", e),
            });
        tracing::debug!("IPC: Got response: {:?}", result);
        result
    }

    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, path: &Path) -> Result<Box<dyn crate::ipc::IpcStream>, McpError> {
        let pipe_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| McpError::IpcError {
                message: format!("Invalid pipe path: {}", path.display()),
            })?;

        let pipe_name_display = format!(r"\\.\pipe\{}", pipe_name);

        let client = ClientOptions::new()
            .open(&pipe_name_display)
            .map_err(|e| McpError::ConnectionError {
                server: pipe_name_display.clone(),
                source: e,
            })?;

        Ok(Box::new(client) as Box<dyn crate::ipc::IpcStream>)
    }
}


/// Manual implementation of IpcStream for NamedPipeClient
impl crate::ipc::IpcStream for NamedPipeClient {}

/// Manual implementation of IpcStream for NamedPipeServer (after connection)
impl crate::ipc::IpcStream for tokio::net::windows::named_pipe::NamedPipeServer {}
