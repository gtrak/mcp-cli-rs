//! Windows named pipe implementation for IPC communication
//!
//! Uses tokio::net::windows::named_pipe for cross-platform named pipe IPC

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::net::windows::named_pipe::{NamedPipeClient, ClientOptions};

use crate::ipc::IpcServer;
use crate::error::McpError;
use crate::config::Config;

/// Windows named pipe name
const PIPE_NAME: &str = r"\\.\pipe\mcp-cli-daemon-socket";

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
    pub fn new(_path: &Path) -> Result<Self, McpError> {
        Ok(Self {
            pipe_name: PIPE_NAME.to_string(),
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
            .first_pipe_instance(false)  // Allow multiple instances
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
}

impl NamedPipeIpcClient {
    /// Create a new NamedPipeIpcClient with a config reference (convenience method)
    pub fn with_config(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Connect to the named pipe with retry logic for ERROR_PIPE_BUSY (231)
    async fn connect_with_retry(&self) -> Result<NamedPipeClient, McpError> {
        loop {
            match ClientOptions::new().open(PIPE_NAME) {
                Ok(client) => return Ok(client),
                Err(e) => {
                    // ERROR_PIPE_BUSY = 231
                    if e.raw_os_error() == Some(231) {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        continue;
                    }
                    return Err(McpError::ConnectionError {
                        server: PIPE_NAME.to_string(),
                        source: e,
                    });
                }
            }
        }
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
        // Connect to daemon with retry logic
        let stream = self.connect_with_retry().await?;

        // Split stream for reading and writing
        use tokio::io::{BufReader};
        let (reader, mut writer) = tokio::io::split(stream);
        let mut buf_reader = BufReader::new(reader);

        // Send request using NDJSON protocol
        crate::daemon::protocol::send_request(&mut writer, request).await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to send IPC request: {}", e),
            })?;

        // Receive response using NDJSON protocol
        crate::daemon::protocol::receive_response(&mut buf_reader).await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to receive IPC response: {}", e),
            })
    }

    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, _path: &Path) -> Result<Box<dyn crate::ipc::IpcStream>, McpError> {
        let client = self.connect_with_retry().await?;
        Ok(Box::new(client) as Box<dyn crate::ipc::IpcStream>)
    }
}

/// Manual implementation of IpcStream for NamedPipeClient
impl crate::ipc::IpcStream for NamedPipeClient {}

/// Manual implementation of IpcStream for NamedPipeServer (after connection)
impl crate::ipc::IpcStream for tokio::net::windows::named_pipe::NamedPipeServer {}
