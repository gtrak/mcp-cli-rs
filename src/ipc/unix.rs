//! Unix socket implementation for IPC communication

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

use crate::config::Config;
use crate::error::McpError;
use crate::ipc::IpcServer;

/// Unix socket implementation of IPC server
///
/// Accepts connections via Unix domain sockets on Unix-like systems
pub struct UnixIpcServer {
    listener: UnixListener,
}

impl UnixIpcServer {
    /// Create a new UnixIpcServer at the specified path
    ///
    /// Creates parent directories if needed and removes stale socket files
    pub async fn new(path: &Path) -> Result<Self, McpError> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| McpError::IOError { source: e })?;
        }

        // Remove stale socket file if exists
        if path.exists() {
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| McpError::IpcError {
                    message: format!("Failed to remove stale socket file: {}", path.display()),
                })?;
        }

        // Bind Unix listener to the socket path
        let listener = UnixListener::bind(path).map_err(|e| McpError::SocketBindError {
            path: path.to_string_lossy().to_string(),
            source: e,
        })?;

        Ok(Self { listener })
    }
}

#[async_trait]
impl IpcServer for UnixIpcServer {
    /// Accept an incoming connection
    ///
    /// Returns a boxed stream and address string for the connection
    async fn accept(&self) -> Result<(Box<dyn crate::ipc::IpcStream>, String), McpError> {
        match self.listener.accept().await {
            Ok((stream, addr)) => {
                // UnixStream already implements AsyncRead + AsyncWrite
                let addr_str = addr
                    .as_pathname()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                Ok((
                    Box::new(stream) as Box<dyn crate::ipc::IpcStream>,
                    addr_str,
                ))
            }
            Err(e) => Err(McpError::IpcError {
                message: format!("Failed to accept connection: {}", e),
            }),
        }
    }
}

/// Unix socket implementation of IPC client
///
/// Connects to IPC servers via Unix domain sockets on Unix-like systems
#[derive(Clone)]
pub struct UnixIpcClient {
    config: Arc<Config>,
}

impl UnixIpcClient {
    /// Create a new UnixIpcClient with a config reference
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl crate::ipc::IpcClient for UnixIpcClient {
    /// Get the configuration associated with this client
    fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// Send a daemon protocol request and receive response
    async fn send_request(
        &self,
        request: &crate::daemon::protocol::DaemonRequest,
    ) -> Result<crate::daemon::protocol::DaemonResponse, McpError> {
        // Connect to daemon
        let stream = self.connect(&self.config.socket_path).await?;

        // Split stream for reading and writing
        use tokio::io::BufReader;
        let (reader, mut writer) = tokio::io::split(stream);
        let mut buf_reader = BufReader::new(reader);

        // Send request using NDJSON protocol
        crate::daemon::protocol::send_request(&mut writer, request)
            .await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to send IPC request: {}", e),
            })?;

        // Receive response using NDJSON protocol
        crate::daemon::protocol::receive_response(&mut buf_reader)
            .await
            .map_err(|e| McpError::IpcError {
                message: format!("Failed to receive IPC response: {}", e),
            })
    }

    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, path: &Path) -> Result<Box<dyn crate::ipc::IpcStream>, McpError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(|e| McpError::ConnectionError {
                server: path.to_string_lossy().to_string(),
                source: e,
            })?;

        Ok(Box::new(stream) as Box<dyn crate::ipc::IpcStream>)
    }
}

/// Manual implementation of IpcStream for UnixStream
///
/// UnixStream already implements AsyncRead + AsyncWrite, Send + Sync, Unpin
impl crate::ipc::IpcStream for UnixStream {}
