//! Graceful signal handling and shutdown utilities.
//!
//! Provides cross-platform signal handling for SIGINT/SIGTERM (Unix)
//! and Ctrl+C (Windows). Implements CLI-04.

use tokio::sync::broadcast;
use tokio::signal;

/// Graceful shutdown handler for signals.
///
/// Allows async operations to respond to termination requests
/// and clean up resources properly.
pub struct GracefulShutdown {
    /// Shutdown sender.
    shutdown_tx: broadcast::Sender<bool>,

    /// Shutdown receiver.
    shutdown_rx: broadcast::Receiver<bool>,
}

impl GracefulShutdown {
    /// Create a new GracefulShutdown handler.
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        Self {
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Spawn the signal listener task.
    ///
    /// Listens for SIGINT/SIGTERM on Unix and Ctrl+C on Windows.
    /// Sends shutdown signal when received.
    pub fn spawn_signal_listener(&self) {
        let mut shutdown_tx = self.shutdown_tx.clone();

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{self, SignalKind};

                // Setup signal handlers for POSIX systems
                let mut sigint = unix::signal(SignalKind::interrupt())
                    .expect("Failed to setup SIGINT handler");
                let mut sigterm = unix::signal(SignalKind::terminate())
                    .expect("Failed to setup SIGTERM handler");

                tokio::select! {
                    _ = sigint.recv() => {
                        println!("\nReceived SIGINT (Ctrl+C), shutting down...");
                    }
                    _ = sigterm.recv() => {
                        println!("\nReceived SIGTERM, shutting down...");
                    }
                }
            }

            #[cfg(windows)]
            {
                // Setup Ctrl+C handler for Windows
                if signal::ctrl_c().await.is_ok() {
                    println!("\nReceived shutdown signal, shutting down...");
                }
            }

            // Send shutdown signal to all listeners
            let _ = shutdown_tx.send(true);
        });
    }

    /// Subscribe to shutdown notifications.
    ///
    /// Operations holding a receiver can check for shutdown requests.
    pub fn subscribe(&self) -> broadcast::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    /// Check if shutdown was requested.
    ///
    /// Returns true if shutdown signal was sent.
    pub fn is_shutdown_requested(&mut self) -> bool {
        match self.shutdown_rx.try_recv() {
            Ok(value) => value,
            Err(_) => false,
        }
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// Run an async operation with graceful shutdown support.
///
/// Automatically spawns signal listener and cancels operation on shutdown.
///
/// # Arguments
/// * `op` - Async operation to run
///
/// # Returns
/// Result<T, Error> from the operation or Err(Error) if shutdown occurred
///
/// # Example
/// ```rust,ignore
/// let shutdown = GracefulShutdown::new();
/// shutdown.spawn_signal_listener();
///
/// let result = run_with_graceful_shutdown(
///     || async {
///         // Your async operation here
///         Ok::<_, McpError>("success")
///     },
///     shutdown.subscribe(),
/// ).await?;
/// ```
pub async fn run_with_graceful_shutdown<F, T, Fut>(op: F, mut shutdown_rx: broadcast::Receiver<bool>) -> crate::error::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = crate::error::Result<T>>,
{
    tokio::select! {
        result = op() => result,
        _ = shutdown_rx.recv() => {
            println!("Shutting down gracefully...");
            // Return shutdown error on termination
            Err(crate::error::McpError::io_error(
                std::io::Error::new(std::io::ErrorKind::Interrupted, "Shutdown requested")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graceful_shutdown_default() {
        let shutdown = GracefulShutdown::default();
        // Create a receiver to ensure the system is initialized
        let _rx = shutdown.subscribe();
    }

    #[test]
    fn test_is_shutdown_requested() {
        let mut shutdown = GracefulShutdown::new();
        assert!(!shutdown.is_shutdown_requested());

        // Send shutdown signal
        let _ = shutdown.shutdown_tx.send(true);
        assert!(shutdown.is_shutdown_requested());
    }
}
