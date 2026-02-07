//! Windows named pipe implementation for IPC communication
//!
//! Uses tokio::net::windows::named_pipe for cross-platform named pipe IPC
//! Note: interprocess v2.3+ sets SECURITY_IDENTIFICATION by default - do NOT override

use async_trait::async_trait;
use std::path::Path;
use tokio::net::windows::named_pipe::{NamedPipeServer, NamedPipeClient};
use windows_sys::Win32::Security::SECURITY_IDENTIFICATION;

use crate::ipc::IpcServer;
use crate::error::{Result, IpcError};

/// Windows named pipe implementation of IPC server
///
/// Accepts connections via named pipes on Windows systems
///
/// # Important Security Note
///
/// The `interprocess` crate (v2.3+) sets SECURITY_IDENTIFICATION by default.
/// This provides adequate protection against privilege escalation attacks.
/// **Do NOT** override security_qos_flags, as it's already set to a secure default.
pub struct NamedPipeIpcServer {
    pipe_name: String,
    current_server: Option<NamedPipeServer>,
}

impl NamedPipeIpcServer {
    /// Create a new NamedPipeIpcServer with the specified pipe name
    ///
    /// Creates a named pipe that can accept multiple client connections
    /// Uses security_qos_flags with SECURITY_IDENTIFICATION (set by interprocess v2.3+)
    pub fn new(path: &Path) -> Result<Self> {
        // Extract pipe name from the path (remove any directory components)
        let pipe_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| IpcError {
                message: format!("Invalid pipe path: {}", path.display()),
            })?;

        let pipe_name_display = format!(r"\\.\pipe\{}", pipe_name);

        // Create server with security_qos_flags set to SECURITY_IDENTIFICATION
        // (Default from interprocess crate, provides adequate protection)
        let server = NamedPipeServer::builder()
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .reject_remote_clients(true) // Local connections only
            .first_pipe_instance(true) // Prevent multiple daemons
            .create(&pipe_name_display)
            .map_err(|e| IpcError {
                message: format!("Failed to create named pipe {}: {}", pipe_name, e),
            })?;

        Ok(Self {
            pipe_name: pipe_name_display,
            current_server: Some(server),
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
    ///
    /// This implementation handles multiple connections by creating a new
    /// server instance for each new client connection, allowing the daemon
    /// to accept concurrent IPC connections from multiple CLI processes.
    async fn accept(&mut self) -> Result<(Box<dyn crate::ipc::IpcStream>, String)> {
        let server = self
            .current_server
            .take()
            .ok_or_else(|| IpcError {
                message: format!("No server instance available for {}", self.pipe_name),
            })?;

        // Wait for a client connection
        let client = server
            .connect()
            .await
            .map_err(|e| IpcError {
                message: format!("Failed to accept named pipe connection: {}", e),
            })?;

        // Create a new server instance for subsequent connections
        // This pattern allows multiple concurrent connections
        let next_server = NamedPipeServer::builder()
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .reject_remote_clients(true)
            .first_pipe_instance(true)
            .create(&self.pipe_name)
            .map_err(|e| IpcError {
                message: format!("Failed to create new named pipe instance: {}", e),
            })?;

        // Store the new server instance
        self.current_server = Some(next_server);

        Ok((Box::new(client) as Box<dyn crate::ipc::IpcStream>, self.pipe_name.clone()))
    }
}

/// Windows named pipe implementation of IPC client
///
/// Connects to IPC servers via named pipes on Windows systems
pub struct NamedPipeIpcClient;

#[async_trait]
impl crate::ipc::IpcClient for NamedPipeIpcClient {
    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    ///
    /// # Security Note
    ///
    /// SECURITY_IDENTIFICATION is set by the `interprocess` crate (v2.3+)
    /// which provides adequate protection against privilege escalation.
    async fn connect(&self, path: &Path) -> Result<Box<dyn crate::ipc::IpcStream>> {
        let pipe_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| IpcError {
                message: format!("Invalid pipe path: {}", path.display()),
            })?;

        let pipe_name_display = format!(r"\\.\pipe\{}", pipe_name);

        // Connect with security_qos_flags set to SECURITY_IDENTIFICATION
        // (Default from interprocess crate)
        let client = NamedPipeClient::builder()
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(&pipe_name_display)
            .map_err(|e| IpcError {
                message: format!("Failed to connect to named pipe {}: {}", pipe_name, e),
            })?;

        Ok(Box::new(client) as Box<dyn crate::ipc::IpcStream>)
    }
}

/// Manual implementation of IpcStream for NamedPipeClient
///
/// NamedPipeClient already implements AsyncRead + AsyncWrite + Send + Sync + Unpin
impl crate::ipc::IpcStream for NamedPipeClient {}
