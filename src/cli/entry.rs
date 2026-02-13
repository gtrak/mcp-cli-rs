//! CLI entry point module.
//!
//! This module contains the main CLI entry point logic including
//! the Cli struct definition, main function, and initialization.

use crate::cli::command_router::{Commands, execute_command};
use crate::cli::config_setup::{setup_config, setup_config_for_daemon, setup_config_optional};
use crate::cli::daemon_lifecycle::{
    create_auto_daemon_client, create_direct_client, create_require_daemon_client,
};
use crate::config::Config;
use crate::error::{McpError, Result};
use crate::format::OutputMode;
use crate::ipc::create_ipc_client;
use crate::shutdown::{GracefulShutdown, run_with_graceful_shutdown};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// CLI argument structure for the MCP CLI client.
#[derive(Parser, Clone)]
#[command(name = "mcp")]
#[command(about = "MCP CLI client for tool discovery and execution", long_about = None)]
pub struct Cli {
    /// Path to configuration file (mcp_servers.toml)
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Run without daemon (direct mode) - **currently recommended for daemon-mode issues**
    #[arg(long, global = true)]
    no_daemon: bool,

    /// Auto-spawn daemon if not running (default behavior - **has known issues on Windows**)
    #[arg(long, global = true, conflicts_with = "no_daemon")]
    auto_daemon: bool,

    /// Require daemon to be already running (fail if not running)
    #[arg(long, global = true, conflicts_with_all = ["no_daemon", "auto_daemon"])]
    require_daemon: bool,

    /// Output results as JSON for programmatic use
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Initialize tracing subscriber with appropriate output
/// - Daemon mode: logs to file (~/.cache/mcp-cli/daemon.log)
/// - CLI mode: logs to stderr (controlled by RUST_LOG env var)
pub fn init_tracing(is_daemon: bool) {
    if is_daemon {
        // Daemon mode: log to file
        let log_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("mcp-cli");

        let _ = std::fs::create_dir_all(&log_dir);
        let log_file = log_dir.join("daemon.log");

        let file_appender = tracing_subscriber::fmt::layer()
            .with_writer(move || {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file)
                    .unwrap_or_else(|_| {
                        std::fs::File::create(&log_file).expect("Failed to create log file")
                    })
            })
            .with_ansi(false);

        tracing_subscriber::registry()
            .with(file_appender)
            .with(tracing_subscriber::EnvFilter::new("debug"))
            .init();
    } else {
        // CLI mode: log to stderr
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }
}

/// Main entry point for the CLI application.
/// This function is called from the binary wrapper in main.rs.
pub async fn main() -> Result<()> {
    // CLI-02: Display version with --version (handled by clap)
    // CLI-01: Display help with --help (handled by clap)

    let cli = Cli::parse();

    // Initialize tracing based on mode (daemon vs CLI)
    // Daemon mode logs to file, CLI mode logs to stderr
    let is_daemon_mode = matches!(cli.command, Some(Commands::Daemon { .. }));
    init_tracing(is_daemon_mode);

    run(cli).await
}

/// Internal run function that handles command dispatching.
async fn run(cli: Cli) -> Result<()> {
    // Handle daemon subcommand first (standalone mode)
    if let Some(Commands::Daemon { ttl, socket_path }) = &cli.command {
        return run_standalone_daemon(*ttl, socket_path.clone()).await;
    }

    // Handle shutdown command
    if let Some(Commands::Shutdown) = &cli.command {
        return shutdown_daemon().await;
    }

    // Load configuration using the loader
    let config = setup_config(cli.config.clone()).await?;

    // Wrap config in Arc for shared ownership
    let daemon_config = Arc::new(config);

    // Initialize GracefulShutdown for clean shutdown on signals
    let shutdown = GracefulShutdown::new();
    shutdown.spawn_signal_listener();

    // Subscribe to shutdown notifications
    let shutdown_rx = shutdown.subscribe();

    // Determine operational mode
    if cli.no_daemon {
        // Direct mode (existing behavior)
        run_with_graceful_shutdown(
            || run_direct_mode(&cli, Arc::clone(&daemon_config)),
            shutdown_rx,
        )
        .await?
    } else if cli.require_daemon {
        // Require-daemon mode: fail if daemon not running
        run_with_graceful_shutdown(
            || run_require_daemon_mode(&cli, &daemon_config),
            shutdown_rx,
        )
        .await?
    } else {
        // Auto-daemon mode (default): spawn if needed, use TTL
        run_with_graceful_shutdown(|| run_auto_daemon_mode(&cli, &daemon_config), shutdown_rx)
            .await?
    };

    Ok(())
}

/// Run in direct mode without daemon
async fn run_direct_mode(cli: &Cli, config: Arc<Config>) -> Result<()> {
    // Create a direct client that connects to servers without daemon
    let client = create_direct_client(config).await?;

    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    // Execute the command (dispatch to command_router)
    execute_command(cli.command.clone(), client, output_mode).await
}

/// Run in auto-daemon mode: spawn if needed, execute command, daemon auto-shutdowns after TTL
async fn run_auto_daemon_mode(cli: &Cli, config: &Config) -> Result<()> {
    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    // Get or spawn daemon client
    let client = create_auto_daemon_client(config).await?;

    // Execute the command (dispatch to command_router)
    execute_command(cli.command.clone(), client, output_mode).await
}

/// Run in require-daemon mode: fail if daemon not running
async fn run_require_daemon_mode(cli: &Cli, config: &Config) -> Result<()> {
    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    // Try to connect to daemon
    let client = create_require_daemon_client(config).await?;

    // Execute the command (dispatch to command_router)
    execute_command(cli.command.clone(), client, output_mode).await
}

/// Shutdown the running daemon via IPC
async fn shutdown_daemon() -> Result<()> {
    // Load configuration - use default config if no file found
    let config = setup_config_optional(None).await?;

    // Create IPC client to connect to daemon
    let mut client = create_ipc_client(&config).map_err(|e| {
        McpError::io_error(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            e,
        ))
    })?;

    // Send shutdown request
    client
        .shutdown()
        .await
        .map_err(|e| McpError::io_error(std::io::Error::other(e)))?;

    println!("Daemon shutdown request sent successfully");
    Ok(())
}

/// Run in standalone daemon mode - starts persistent daemon with specified TTL
async fn run_standalone_daemon(
    cli_ttl: Option<u64>,
    cli_socket_path: Option<PathBuf>,
) -> crate::error::Result<()> {
    use crate::daemon::run_daemon;

    // Load configuration - allow daemon to start even without config file
    let mut config = setup_config_for_daemon(None).await?;

    // Determine TTL: CLI flag > env var > config > default (60s)
    let ttl = cli_ttl
        .or_else(|| {
            std::env::var("MCP_DAEMON_TTL")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(config.daemon_ttl);

    tracing::info!("Starting standalone daemon with TTL: {}s", ttl);

    // Use provided socket path or fall back to config's default
    let socket_path = cli_socket_path.unwrap_or_else(|| config.socket_path.clone());
    tracing::info!("Using socket path: {:?}", socket_path);

    // Update config with the socket path we're actually using
    config.socket_path = socket_path.clone();

    // Remove existing socket file if present (only on Unix, Windows named pipes clean up automatically)
    #[cfg(unix)]
    {
        if let Err(e) = std::fs::remove_file(&socket_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("Could not remove existing socket file: {}", e);
        }
    }

    // Create daemon lifecycle with specified TTL
    let lifecycle = crate::daemon::lifecycle::DaemonLifecycle::new(ttl);

    // Run daemon (this blocks until shutdown)
    tracing::info!("Daemon starting...");
    match run_daemon(config, socket_path, lifecycle).await {
        Ok(()) => {
            tracing::info!("Daemon exited normally");
            Ok(())
        }
        Err(e) => {
            tracing::error!("Daemon error: {}", e);
            Err(McpError::IOError {
                source: std::io::Error::other(e),
            })
        }
    }
}
