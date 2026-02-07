//! Platform-agnostic IPC abstraction for daemon communication

use async_trait::async_trait;
use std::path::Path;

/// Stream abstraction for IPC communication
///
/// Wraps platform-specific stream types to provide a unified interface
pub trait IpcStream: async_trait::AsyncRead + async_trait::AsyncWrite + Send + Sync + Unpin {}

/// Server trait for accepting IPC connections
///
/// Implementations provide platform-specific connection handling
#[async_trait]
pub trait IpcServer: Send + Sync {
    /// Accept an incoming connection
    ///
    /// Returns a boxed stream and address string for the connection
    async fn accept(&self) -> Result<(Box<dyn IpcStream>, String), crate::error::IpcError>;
}

/// Client trait for connecting to IPC servers
///
/// Implementations provide platform-specific connection logic
#[async_trait]
pub trait IpcClient: Send + Sync {
    /// Connect to an IPC server at the given path
    ///
    /// Returns a boxed stream for communication
    async fn connect(&self, path: &Path) -> Result<Box<dyn IpcStream>, crate::error::IpcError>;
}

/// Factory function to create platform-specific IPC server
///
/// Returns Box<dyn IpcServer> with platform-specific implementation
#[cfg(unix)]
pub fn create_ipc_server(path: &Path) -> Result<Box<dyn IpcServer>, crate::error::IpcError> {
    Ok(Box::new(crate::ipc::unix::UnixIpcServer::new(path)?))
}

/// Factory function to create platform-specific IPC server
///
/// Returns Box<dyn IpcServer> with platform-specific implementation
#[cfg(unix)]
pub fn create_ipc_server(path: &Path) -> Result<Box<dyn IpcServer>, crate::error::IpcError> {
    Ok(Box::new(crate::ipc::unix::UnixIpcServer::new(path)?))
}

/// Factory function to create platform-specific IPC server
///
/// Returns Box<dyn IpcServer> with platform-specific implementation
#[cfg(windows)]
pub fn create_ipc_server(path: &Path) -> Result<Box<dyn IpcServer>, crate::error::IpcError> {
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
    // Windows implementation to be added in future wave
    std::path::PathBuf::from(format!("\\\\.\\pipe\\mcp-cli-daemon-{}", std::process::id()))
}

/// Re-export platform-specific implementations
#[cfg(unix)]
pub use unix::{UnixIpcServer, UnixIpcClient};

#[cfg(windows)]
pub use windows::{NamedPipeIpcServer, NamedPipeIpcClient};
