use clap::{Parser, Subcommand};
use mcp_cli_rs::config::Config;
use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::error::{Result, exit_code};
use mcp_cli_rs::format::OutputMode;
use mcp_cli_rs::ipc::create_ipc_client;
use mcp_cli_rs::shutdown::{GracefulShutdown, run_with_graceful_shutdown};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
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

#[derive(Clone, Subcommand)]
enum Commands {
    /// Start the connection daemon
    Daemon {
        /// Daemon idle timeout in seconds (overrides config and env)
        #[arg(short, long)]
        ttl: Option<u64>,

        /// Socket path for IPC communication (used internally when auto-spawning)
        #[arg(long, hide = true)]
        socket_path: Option<std::path::PathBuf>,
    },

    /// Shutdown the running daemon
    Shutdown,

    /// List all servers and their available tools (CLI-01, DISC-01)
    List {
        /// Show detailed descriptions and parameters
        #[arg(short = 'd', long)]
        describe: bool,

        /// Show verbose output with full schema
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Show details for a specific server (DISC-02)
    Info {
        /// Server name
        name: String,
    },

    /// Show details for a specific tool (DISC-03)
    Tool {
        /// Tool identifier (server/tool or server tool)
        #[arg(value_name = "TOOL")]
        tool: String,

        /// Show detailed descriptions and parameters
        #[arg(short = 'd', long)]
        describe: bool,

        /// Show verbose output with full schema
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Execute a tool (EXEC-01, EXEC-02)
    Call {
        /// Tool identifier (server/tool or server tool)
        #[arg(value_name = "TOOL")]
        tool: String,

        /// JSON arguments for the tool
        #[arg(value_name = "ARGS")]
        args: Option<String>,
    },

    /// Search for tools by name pattern (DISC-04)
    Search {
        /// Glob pattern to match tool names
        #[arg(value_name = "PATTERN")]
        pattern: String,

        /// Show detailed descriptions and parameters
        #[arg(short = 'd', long)]
        describe: bool,

        /// Show verbose output with full schema
        #[arg(short = 'v', long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() {
    // CLI-02: Display version with --version (handled by clap)
    // CLI-01: Display help with --help (handled by clap)

    let cli = Cli::parse();

    // Initialize tracing based on mode (daemon vs CLI)
    // Daemon mode logs to file, CLI mode logs to stderr
    let is_daemon_mode = matches!(cli.command, Some(Commands::Daemon { .. }));
    init_tracing(is_daemon_mode);

    if let Err(e) = run(cli).await {
        eprintln!("error: {}", e);
        std::process::exit(exit_code(&e));
    }
}

/// Initialize tracing subscriber with appropriate output
/// - Daemon mode: logs to file (~/.cache/mcp-cli/daemon.log)
/// - CLI mode: logs to stderr (controlled by RUST_LOG env var)
fn init_tracing(is_daemon: bool) {
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
    let config = if let Some(path) = &cli.config {
        // Use explicitly provided config path
        load_config(path).await?
    } else {
        // Search for config in standard locations
        find_and_load(None).await?
    };

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
async fn run_direct_mode(cli: &Cli, config: Arc<mcp_cli_rs::config::Config>) -> Result<()> {
    // Create a direct client that connects to servers without daemon
    let direct_client =
        Box::new(DirectProtocolClient::new(config)) as Box<dyn mcp_cli_rs::ipc::ProtocolClient>;

    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    // Execute the command
    execute_command(cli, direct_client, output_mode).await
}

/// Execute the CLI command using the provided client
async fn execute_command(
    cli: &Cli,
    client: Box<dyn mcp_cli_rs::ipc::ProtocolClient>,
    output_mode: OutputMode,
) -> mcp_cli_rs::error::Result<()> {
    use mcp_cli_rs::cli::commands::*;

    let command = cli.command.clone();
    match command {
        Some(Commands::Daemon { .. }) => {
            // Daemon subcommand is handled separately in run()
            Ok(())
        }
        Some(Commands::Shutdown) => {
            // Shutdown subcommand is handled separately in run()
            Ok(())
        }
        Some(Commands::List { describe, verbose }) => {
            let detail_level = if verbose {
                mcp_cli_rs::cli::DetailLevel::Verbose
            } else if describe {
                mcp_cli_rs::cli::DetailLevel::WithDescriptions
            } else {
                mcp_cli_rs::cli::DetailLevel::Summary
            };
            cmd_list_servers(client, detail_level, output_mode).await
        }
        Some(Commands::Info { name }) => cmd_server_info(client, &name).await,
        Some(Commands::Tool {
            tool,
            describe,
            verbose,
        }) => {
            let detail_level = if verbose {
                mcp_cli_rs::cli::DetailLevel::Verbose
            } else if describe {
                mcp_cli_rs::cli::DetailLevel::WithDescriptions
            } else {
                mcp_cli_rs::cli::DetailLevel::Summary
            };
            cmd_tool_info(client, &tool, detail_level, output_mode).await
        }
        Some(Commands::Call { tool, args }) => cmd_call_tool(client, &tool, args.as_deref(), output_mode).await,
        Some(Commands::Search {
            pattern,
            describe,
            verbose,
        }) => {
            let detail_level = if verbose {
                mcp_cli_rs::cli::DetailLevel::Verbose
            } else if describe {
                mcp_cli_rs::cli::DetailLevel::WithDescriptions
            } else {
                mcp_cli_rs::cli::DetailLevel::Summary
            };
            cmd_search_tools(client, &pattern, detail_level, output_mode).await
        }
        None => cmd_list_servers(client, mcp_cli_rs::cli::DetailLevel::Summary, output_mode).await,
    }
}

/// Shutdown the running daemon via IPC
async fn shutdown_daemon() -> Result<()> {
    use mcp_cli_rs::config::loader::find_and_load;

    // Load configuration - use default config if no file found
    let config = match find_and_load(None).await {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!("No config file found, using default for IPC: {}", e);
            mcp_cli_rs::config::Config::default()
        }
    };

    // Create IPC client to connect to daemon
    let mut client = create_ipc_client(&config).map_err(|e| {
        mcp_cli_rs::error::McpError::io_error(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            e,
        ))
    })?;

    // Send shutdown request
    client
        .shutdown()
        .await
        .map_err(|e| mcp_cli_rs::error::McpError::io_error(std::io::Error::other(e)))?;

    println!("Daemon shutdown request sent successfully");
    Ok(())
}

/// Run in standalone daemon mode - starts persistent daemon with specified TTL
async fn run_standalone_daemon(
    cli_ttl: Option<u64>,
    cli_socket_path: Option<PathBuf>,
) -> mcp_cli_rs::error::Result<()> {
    use mcp_cli_rs::config::loader::find_and_load;
    use mcp_cli_rs::daemon::run_daemon;

    // Load configuration - allow daemon to start even without config file
    let mut config = match find_and_load(None).await {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!(
                "No config file found, starting daemon with empty config: {}",
                e
            );
            Config::default()
        }
    };

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
    let lifecycle = mcp_cli_rs::daemon::lifecycle::DaemonLifecycle::new(ttl);

    // Run daemon (this blocks until shutdown)
    tracing::info!("Daemon starting...");
    match run_daemon(config, socket_path, lifecycle).await {
        Ok(()) => {
            tracing::info!("Daemon exited normally");
            Ok(())
        }
        Err(e) => {
            tracing::error!("Daemon error: {}", e);
            Err(mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(e),
            })
        }
    }
}

/// Run in auto-daemon mode: spawn if needed, execute command, daemon auto-shutdowns after TTL
pub async fn run_auto_daemon_mode(cli: &Cli, config: &Config) -> mcp_cli_rs::error::Result<()> {
    tracing::debug!("run_auto_daemon_mode called");

    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    // Check if daemon is running
    match try_connect_to_daemon(config).await {
        Ok(client) => {
            // Daemon is running, use it
            tracing::info!("Using existing daemon");
            execute_command(cli, client, output_mode).await
        }
        Err(_) => {
            // Daemon not running, spawn it
            tracing::info!("Daemon not running, spawning...");

            // Get TTL from config (includes env var override via config loader)
            // Set minimum TTL of 5 seconds for auto-daemon mode to prevent race conditions
            let mut ttl = config.daemon_ttl;
            if ttl < 5 {
                tracing::warn!(
                    "Auto-daemon TTL too short ({}s), setting minimum of 5s to prevent race conditions",
                    ttl
                );
                ttl = 5;
            }

            // Spawn daemon as background task
            tracing::debug!("Spawning daemon with TTL={}s...", ttl);

            // Clone socket_path for the async block
            let socket_path = config.socket_path.clone();
            tokio::spawn(async move {
                tracing::debug!("Inside tokio::spawn, about to spawn daemon...");
                match spawn_background_daemon(ttl, &socket_path).await {
                    Ok(_) => tracing::debug!("spawn_background_daemon returned Ok"),
                    Err(e) => tracing::debug!("spawn_background_daemon failed: {}", e),
                }
            });

            // Wait for daemon to start with exponential backoff
            let mut retries = 0;
            let max_retries = 20; // More retries
            let mut delay = Duration::from_millis(500); // Start with longer delay

            loop {
                tokio::time::sleep(delay).await;

                match try_connect_to_daemon(config).await {
                    Ok(client) => {
                        tracing::info!("Connected to daemon after {} attempt(s)", retries + 1);
                        return execute_command(cli, client, output_mode).await;
                    }
                    Err(e) => {
                        retries += 1;
                        if retries >= max_retries {
                            return Err(mcp_cli_rs::error::McpError::IOError {
                                source: std::io::Error::other(format!(
                                    "Failed to start daemon after {} attempts: {}",
                                    max_retries, e
                                )),
                            });
                        }
                        // Linear backoff: add 200ms each time, cap at 2 seconds
                        delay += Duration::from_millis(200);
                        if delay > Duration::from_secs(2) {
                            delay = Duration::from_secs(2);
                        }
                        tracing::debug!(
                            "Daemon not ready, retrying in {:?} (attempt {}/{})",
                            delay,
                            retries,
                            max_retries
                        );
                    }
                }
            }
        }
    }
}

/// Run in require-daemon mode: fail if daemon not running
pub async fn run_require_daemon_mode(cli: &Cli, config: &Config) -> mcp_cli_rs::error::Result<()> {
    // Determine output mode from CLI flags
    let output_mode = OutputMode::from_flags(cli.json);

    match try_connect_to_daemon(config).await {
        Ok(client) => {
            tracing::info!("Using existing daemon");
            execute_command(cli, client, output_mode).await
        }
        Err(_) => Err(mcp_cli_rs::error::McpError::daemon_not_running(
            "Daemon is not running. Start it with 'mcp daemon' or use --auto-daemon",
        )),
    }
}

async fn try_connect_to_daemon(
    config: &Config,
) -> mcp_cli_rs::error::Result<Box<dyn mcp_cli_rs::ipc::ProtocolClient>> {
    let client = create_ipc_client(config)?;

    // Actually verify the connection works by sending a ping
    let mut test_client = client;
    match test_client.list_servers().await {
        Ok(_) => Ok(test_client),
        Err(e) => Err(e),
    }
}

async fn spawn_background_daemon(
    ttl: u64,
    socket_path: &std::path::Path,
) -> mcp_cli_rs::error::Result<()> {
    // Spawn the daemon as a separate process using the binary itself
    // This is necessary because the daemon runs an IPC server that needs
    // to be independent of the client process

    // Get the current executable path
    let current_exe =
        std::env::current_exe().map_err(|e| mcp_cli_rs::error::McpError::IOError {
            source: std::io::Error::other(format!("Failed to get executable path: {}", e)),
        })?;

    // Build arguments for daemon subcommand - pass socket path explicitly
    // to ensure daemon uses the same IPC endpoint as the client expects
    let socket_path_str = socket_path.to_string_lossy().to_string();
    let args = vec![
        "daemon".to_string(),
        "--socket-path".to_string(),
        socket_path_str,
    ];

    // Spawn the daemon process
    tracing::info!(
        "Spawning daemon process: {:?} daemon (TTL: {}s, socket: {:?})",
        current_exe,
        ttl,
        socket_path
    );

    tracing::debug!("Spawning daemon: {:?} with args: {:?}", current_exe, args);

    // Get current working directory so daemon can find config
    let current_dir =
        std::env::current_dir().map_err(|e| mcp_cli_rs::error::McpError::IOError {
            source: std::io::Error::other(format!("Failed to get current directory: {}", e)),
        })?;

    // On Windows, we need to use a different approach to spawn a truly independent process
    // Using CREATE_NEW_PROCESS_GROUP and CREATE_NO_WINDOW flags
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut cmd = std::process::Command::new(&current_exe);
        cmd.args(&args)
            .env("MCP_DAEMON_TTL", ttl.to_string())
            .current_dir(&current_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);

        let _child = cmd
            .spawn()
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(format!("Failed to spawn daemon: {}", e)),
            })?;

        tracing::debug!("Daemon spawned with PID: {:?}", _child.id());
    }

    #[cfg(not(windows))]
    {
        let _child = tokio::process::Command::new(&current_exe)
            .args(&args)
            .env("MCP_DAEMON_TTL", ttl.to_string())
            .current_dir(&current_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(false)
            .spawn()
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to spawn daemon: {}", e),
                ),
            })?;

        tracing::debug!("Daemon spawned with PID: {:?}", _child.id());
    }

    // Give the daemon time to create the named pipe
    // This is critical - the daemon needs time to start the IPC server
    tracing::debug!("Waiting for daemon to initialize...");
    tokio::time::sleep(Duration::from_millis(1000)).await;

    Ok(())
}

/// Direct protocol client that connects to servers without daemon
pub struct DirectProtocolClient {
    config: Arc<mcp_cli_rs::config::Config>,
}

impl DirectProtocolClient {
    pub fn new(config: Arc<mcp_cli_rs::config::Config>) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl mcp_cli_rs::ipc::ProtocolClient for DirectProtocolClient {
    fn config(&self) -> Arc<mcp_cli_rs::config::Config> {
        Arc::clone(&self.config)
    }

    async fn send_request(
        &mut self,
        _request: &mcp_cli_rs::daemon::protocol::DaemonRequest,
    ) -> mcp_cli_rs::error::Result<mcp_cli_rs::daemon::protocol::DaemonResponse> {
        // Direct mode doesn't use daemon protocol - commands handle connections directly
        Err(mcp_cli_rs::error::McpError::InvalidProtocol {
            message: "Direct mode doesn't support daemon protocol requests".to_string(),
        })
    }

    async fn list_servers(&mut self) -> mcp_cli_rs::error::Result<Vec<String>> {
        let servers: Vec<String> = self.config.servers.iter().map(|s| s.name.clone()).collect();
        Ok(servers)
    }

    async fn list_tools(
        &mut self,
        server_name: &str,
    ) -> mcp_cli_rs::error::Result<Vec<mcp_cli_rs::daemon::protocol::ToolInfo>> {
        // Get server config and create transport directly
        let server_config = self.config.get_server(server_name).ok_or_else(|| {
            mcp_cli_rs::error::McpError::ServerNotFound {
                server: server_name.to_string(),
            }
        })?;

        let mut transport = server_config.create_transport(server_name)?;

        // MCP Protocol: Send initialize request first
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        // Send initialize and get response
        transport
            .send(init_request)
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Server automatically sends notifications/initialized - we don't need to send it
        // Now send tools/list request
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        // Send request and get response
        let response = transport.send(mcp_request).await.map_err(|e| {
            mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(e),
            }
        })?;

        // Parse response
        if let Some(result) = response.get("result") {
            let tools = if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array
                    .iter()
                    .filter_map(|tool| {
                        Some(mcp_cli_rs::daemon::protocol::ToolInfo {
                            name: tool.get("name")?.as_str()?.to_string(),
                            description: tool
                                .get("description")?
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            input_schema: tool.get("inputSchema").cloned().unwrap_or_else(|| {
                                serde_json::Value::Object(serde_json::Map::new())
                            }),
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Ok(tools)
        } else {
            Err(mcp_cli_rs::error::McpError::InvalidProtocol {
                message: "Invalid MCP response format".to_string(),
            })
        }
    }

    async fn execute_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> mcp_cli_rs::error::Result<serde_json::Value> {
        // Get server config and create transport directly
        let server_config = self.config.get_server(server_name).ok_or_else(|| {
            mcp_cli_rs::error::McpError::ServerNotFound {
                server: server_name.to_string(),
            }
        })?;

        let mut transport = server_config.create_transport(server_name)?;

        // MCP Protocol: Send initialize request first
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {},
                    "sampling": {},
                    "tools": {}
                },
                "clientInfo": {
                    "name": "mcp-cli-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        // Send initialize and get response
        transport
            .send(init_request)
            .await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(e),
            })?;

        // Server automatically sends notifications/initialized - we don't need to send it
        // Now send tools/call request
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        // Send request and get response
        let response = transport.send(mcp_request).await.map_err(|e| {
            mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::other(e),
            }
        })?;

        // Parse response
        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            Err(mcp_cli_rs::error::McpError::InvalidProtocol {
                message: format!("Tool execution failed: {}", message),
            })
        } else {
            Err(mcp_cli_rs::error::McpError::InvalidProtocol {
                message: "Invalid MCP response format".to_string(),
            })
        }
    }

    async fn shutdown(&mut self) -> mcp_cli_rs::error::Result<()> {
        // Direct mode doesn't support daemon shutdown
        Err(mcp_cli_rs::error::McpError::InvalidProtocol {
            message: "Direct mode doesn't support daemon shutdown".to_string(),
        })
    }
}
