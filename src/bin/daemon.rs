use anyhow::Context;
use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::daemon::run_daemon;
use mcp_cli_rs::ipc::get_socket_path;
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;

/// Get daemon socket path
///
/// Uses the same logic as CLI client to determine socket location:
/// - Unix: ~/.mcp-daemon/socket
/// - Windows: C:\Users\[username]\AppData\Local\mcp-daemon\socket
pub fn get_daemon_socket_path() -> std::path::PathBuf {
    get_socket_path()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with colored output
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .init();

    tracing::info!("MCP Daemon starting...");

    // Load configuration using find_and_load (finds config and loads it)
    let config = find_and_load(None)
        .await
        .context("Failed to load configuration")?;

    // Get socket path
    let socket_path = get_daemon_socket_path();
    tracing::info!("Using socket path: {:?}", socket_path);

    // Remove existing socket file if present (allowing another daemon to start)
    let result = std::fs::remove_file(&socket_path);
    if let Err(e) = result {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Could not remove existing socket file: {}", e);
        }
    }

    // Run daemon
    match run_daemon(config, socket_path).await {
        Ok(()) => {
            tracing::info!("Daemon exited normally");
            std::process::exit(0);
        }
        Err(e) => {
            tracing::error!("Daemon error: {}", e);
            std::process::exit(1);
        }
    }
}
