//! Unix socket implementation for IPC communication

use async_trait::async_trait;
use std::path::Path;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

use crate::ipc::IpcServer;
use crate::error::{Result, IpcError};

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
    pub fn new(path: &Path) -> Result<Self> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Remove stale socket file if exists
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| IpcError {
                    message: format!("Failed to remove stale socket file: {}", path.display()),
                })?;
        }

        // Bind Unix listener to the socket path
        let listener = UnixListener::bind(path)
            .map_err(|e| IpcError {
                message: format!("Failed to bind Unix socket at {}: {}", path.display(), e),
            })?;

        Ok(Self { listener })
    }
}

#[async_trait]
impl IpcServer for UnixIpcServer {
    /// Accept an incoming connection
    ///
    /// Returns a boxed stream and address string for the connection
    async fn accept(&self) -> Result<(Box<dyn crate::ipc::IpcStream>, String)> {
        match self.listener.accept().await {
            Ok((stream, addr)) => {
                // UnixStream already implements AsyncRead + AsyncWrite
                Ok((Box::new(stream) as Box<dyn crate::ipc::IpcStream>, addr.to_string()))
            }
            Err(e) => Err(IpcError {
                message: format!("Failed to accept connection: {}", e),
            }),
        }
    }
}

/// Unix socket implementation of IPC client
///
/// Connects to IPC servers via Unix domain sockets on Unix-like systems
pub struct UnixIpcClient;

#[async_trait]
impl crate::ipc::IpcClient for UnixIpcClient {
    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, path: &Path) -> Result<Box<dyn crate::ipc::IpcStream>> {
        let stream = UnixStream::connect(path)
            .map_err(|e| IpcError {
                message: format!("Failed to connect to Unix socket {}: {}", path.display(), e),
            })?;

        Ok(Box::new(stream) as Box<dyn crate::ipc::IpcStream>)
    }
}

/// Manual implementation of IpcStream for UnixStream
///
/// UnixStream already implements AsyncRead + AsyncWrite, Send + Sync, Unpin
impl crate::ipc::IpcStream for UnixStream {}
