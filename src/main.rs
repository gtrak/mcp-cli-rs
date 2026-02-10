use clap::{Parser, Subcommand};
use mcp_cli_rs::cli::commands::{cmd_list_servers, cmd_server_info, cmd_tool_info, cmd_call_tool, cmd_search_tools};
use mcp_cli_rs::cli::daemon::ensure_daemon;
use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::error::{exit_code, Result};
use mcp_cli_rs::shutdown::{GracefulShutdown, run_with_graceful_shutdown};
use mcp_cli_rs::ipc::{ProtocolClient, get_socket_path, create_ipc_client};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "mcp")]
#[command(about = "MCP CLI client for tool discovery and execution", long_about = None)]
struct Cli {
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
    },

    /// List all servers and their available tools (CLI-01, DISC-01)
    List {
        /// Include tool descriptions
        #[arg(short = 'd', long)]
        with_descriptions: bool,
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
    },
}

#[tokio::main]
async fn main() {
    // CLI-02: Display version with --version (handled by clap)
    // CLI-01: Display help with --help (handled by clap)

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("error: {}", e);
        std::process::exit(exit_code(&e));
    }
}

async fn run(cli: Cli) -> Result<()> {
    // Handle daemon subcommand first (standalone mode)
    if let Some(Commands::Daemon { ttl }) = &cli.command {
        return run_standalone_daemon(*ttl).await;
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
    let result = if cli.no_daemon {
        // Direct mode (existing behavior)
        run_with_graceful_shutdown(
            || run_direct_mode(&cli, Arc::clone(&daemon_config)),
            shutdown_rx,
        ).await?
    } else if cli.require_daemon {
        // Require-daemon mode: fail if daemon not running
        run_with_graceful_shutdown(
            || run_require_daemon_mode(&cli, Arc::clone(&daemon_config)),
            shutdown_rx,
        ).await?
    } else {
        // Auto-daemon mode (default): spawn if needed, use TTL
        run_with_graceful_shutdown(
            || run_auto_daemon_mode(&cli, Arc::clone(&daemon_config)),
            shutdown_rx,
        ).await?
    };

    Ok(result)
}

/// Run in daemon mode with connection caching
async fn run_daemon_mode(cli: &Cli, daemon_config: Arc<mcp_cli_rs::config::Config>) -> Result<()> {
    // Ensure daemon is running with fresh config
    let daemon_client = match ensure_daemon(daemon_config).await {
        Ok(client) => client,
        Err(e) => {
            return Err(mcp_cli_rs::error::McpError::io_error(
                std::io::Error::new(std::io::ErrorKind::Other, e)
            ));
        }
    };

    // Use daemon client for all operations
    execute_command(cli, daemon_client).await
}

/// Run in direct mode without daemon
async fn run_direct_mode(cli: &Cli, config: Arc<mcp_cli_rs::config::Config>) -> Result<()> {
    use mcp_cli_rs::cli::commands::*;
    
    // Create a direct client that connects to servers without daemon
    let direct_client = Box::new(DirectProtocolClient::new(config)) as Box<dyn mcp_cli_rs::ipc::ProtocolClient>;
    
    // Execute the command
    execute_command(cli, direct_client).await
}

/// Execute the CLI command using the provided client
async fn execute_command(cli: &Cli, mut client: Box<dyn mcp_cli_rs::ipc::ProtocolClient>) -> mcp_cli_rs::error::Result<()> {
    use mcp_cli_rs::cli::commands::*;

    let command = cli.command.clone();
    match command {
        Some(Commands::Daemon { .. }) => {
            // Daemon subcommand is handled separately in run()
            Ok(())
        }
        Some(Commands::List { with_descriptions }) => {
            cmd_list_servers(client, with_descriptions).await
        }
        Some(Commands::Info { name }) => {
            cmd_server_info(client, &name).await
        }
        Some(Commands::Tool { tool }) => {
            cmd_tool_info(client, &tool).await
        }
        Some(Commands::Call { tool, args }) => {
            cmd_call_tool(client, &tool, args.as_deref()).await
        }
        Some(Commands::Search { pattern }) => {
            cmd_search_tools(client, &pattern).await
        }
        None => {
            cmd_list_servers(client, false).await
        }
    }
}

/// Run in standalone daemon mode - starts persistent daemon with specified TTL
async fn run_standalone_daemon(cli_ttl: Option<u64>) -> mcp_cli_rs::error::Result<()> {
    use mcp_cli_rs::config::loader::find_and_load;
    use mcp_cli_rs::daemon::run_daemon;

    // Load configuration
    let config = find_and_load(None)
        .await
        .map_err(|e| mcp_cli_rs::error::McpError::usage_error(
            format!("Failed to load configuration: {}", e)
        ))?;

    // Determine TTL: CLI flag > env var > config > default (60s)
    let ttl = cli_ttl
        .or_else(|| std::env::var("MCP_DAEMON_TTL").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(config.daemon_ttl);

    tracing::info!("Starting standalone daemon with TTL: {}s", ttl);

    // Get socket path
    let socket_path = get_socket_path();
    tracing::info!("Using socket path: {:?}", socket_path);

    // Remove existing socket file if present
    if let Err(e) = std::fs::remove_file(&socket_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
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
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })
        }
    }
}

/// Run in auto-daemon mode: spawn if needed, execute command, daemon auto-shutdowns after TTL
pub async fn run_auto_daemon_mode(
    cli: &Cli,
    config: Arc<mcp_cli_rs::config::Config>,
) -> mcp_cli_rs::error::Result<()> {
    // Check if daemon is running
    let socket_path = get_socket_path();

    match try_connect_to_daemon(config.clone(), &socket_path).await {
        Ok(client) => {
            // Daemon is running, use it
            tracing::info!("Using existing daemon");
            execute_command(cli, client).await
        }
        Err(_) => {
            // Daemon not running, spawn it
            tracing::info!("Daemon not running, spawning...");

            // Get TTL from config (includes env var override via config loader)
            let ttl = config.daemon_ttl;

            // Spawn daemon as background task
            let config_clone = Arc::clone(&config);
            tokio::spawn(async move {
                if let Err(e) = spawn_background_daemon(config_clone, ttl).await {
                    tracing::error!("Failed to spawn daemon: {}", e);
                }
            });

            // Wait for daemon to start with exponential backoff
            let mut retries = 0;
            let max_retries = 20;  // More retries
            let mut delay = Duration::from_millis(500);  // Start with longer delay

            loop {
                tokio::time::sleep(delay).await;

                match try_connect_to_daemon(config.clone(), &socket_path).await {
                    Ok(client) => {
                        tracing::info!("Connected to daemon after {} attempt(s)", retries + 1);
                        return execute_command(cli, client).await;
                    }
                    Err(e) => {
                        retries += 1;
                        if retries >= max_retries {
                            return Err(mcp_cli_rs::error::McpError::IOError {
                                source: std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("Failed to start daemon after {} attempts: {}", max_retries, e)
                                ),
                            });
                        }
                        // Linear backoff: add 200ms each time, cap at 2 seconds
                        delay += Duration::from_millis(200);
                        if delay > Duration::from_secs(2) {
                            delay = Duration::from_secs(2);
                        }
                        tracing::debug!("Daemon not ready, retrying in {:?} (attempt {}/{})", delay, retries, max_retries);
                    }
                }
            }
        }
    }
}

/// Run in require-daemon mode: fail if daemon not running
pub async fn run_require_daemon_mode(
    cli: &Cli,
    config: Arc<mcp_cli_rs::config::Config>,
) -> mcp_cli_rs::error::Result<()> {
    let socket_path = get_socket_path();

    match try_connect_to_daemon(config.clone(), &socket_path).await {
        Ok(client) => {
            tracing::info!("Using existing daemon");
            execute_command(cli, client).await
        }
        Err(_) => {
            Err(mcp_cli_rs::error::McpError::daemon_not_running(
                "Daemon is not running. Start it with 'mcp daemon' or use --auto-daemon"
            ))
        }
    }
}

async fn try_connect_to_daemon(config: Arc<mcp_cli_rs::config::Config>, _socket_path: &std::path::Path) -> mcp_cli_rs::error::Result<Box<dyn mcp_cli_rs::ipc::ProtocolClient>> {
    let client = create_ipc_client(config.clone())?;
    
    // Actually verify the connection works by sending a ping
    let mut test_client = client;
    match test_client.list_servers().await {
        Ok(_) => Ok(test_client),
        Err(e) => Err(e),
    }
}

async fn spawn_background_daemon(_config: Arc<mcp_cli_rs::config::Config>, ttl: u64) -> mcp_cli_rs::error::Result<()> {
    // Spawn the daemon as a separate process using the binary itself
    // This is necessary because the daemon runs an IPC server that needs
    // to be independent of the client process
    
    // Get the current executable path
    let current_exe = std::env::current_exe()
        .map_err(|e| mcp_cli_rs::error::McpError::IOError {
            source: std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get executable path: {}", e)),
        })?;
    
    // Spawn the daemon process
    tracing::info!("Spawning daemon process: {:?} daemon (TTL: {}s)", current_exe, ttl);
    
    // Get current working directory so daemon can find config
    let current_dir = std::env::current_dir()
        .map_err(|e| mcp_cli_rs::error::McpError::IOError {
            source: std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get current directory: {}", e)),
        })?;
    
    // Spawn daemon as truly independent process
    #[cfg(windows)]
    {
        spawn_windows_daemon(&current_exe, ttl, &current_dir)?;
    }
    
    #[cfg(not(windows))]
    {
        let args = vec!["daemon".to_string()];
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
                source: std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to spawn daemon: {}", e)),
            })?;
        
        tracing::info!("Daemon spawned with PID: {:?}", _child.id());
    }
    
    // Give the daemon time to start up before returning
    tokio::time::sleep(Duration::from_millis(1500)).await;
    
    Ok(())
}

/// Spawn daemon process on Windows using windows-rs APIs
#[cfg(windows)]
fn spawn_windows_daemon(
    current_exe: &std::path::Path,
    ttl: u64,
    current_dir: &std::path::Path,
) -> mcp_cli_rs::error::Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{CreateProcessW, CREATE_UNICODE_ENVIRONMENT, CREATE_NO_WINDOW, STARTUPINFOW, PROCESS_INFORMATION, STARTF_USESHOWWINDOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    
    // Check if debug mode is enabled
    let debug_mode = std::env::var("MCP_DAEMON_DEBUG").is_ok();
    
    // Build command line: executable daemon --ttl {ttl}
    let cmd_line = format!(r#""{}" daemon --ttl {}"#, current_exe.display(), ttl);
    let cmd_wide: Vec<u16> = OsStr::new(&cmd_line)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    // Current directory as wide string
    let dir_wide: Vec<u16> = current_dir
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    // Startup info - hide window properly
    let mut startup_info = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    
    // If not in debug mode, hide the window completely
    if !debug_mode {
        startup_info.dwFlags = STARTF_USESHOWWINDOW;
        startup_info.wShowWindow = SW_HIDE.0 as u16;
    }
    
    let mut process_info = PROCESS_INFORMATION::default();
    
    // Creation flags: hide window unless debugging
    let creation_flags = if debug_mode {
        CREATE_UNICODE_ENVIRONMENT
    } else {
        CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT
    };
    
    unsafe {
        CreateProcessW(
            None,  // Application name (use command line)
            windows::core::PWSTR(cmd_wide.as_ptr() as *mut u16),
            None,  // Process security attributes
            None,  // Thread security attributes
            false, // Inherit handles
            creation_flags,
            None,  // Environment (inherit)
            windows::core::PCWSTR(dir_wide.as_ptr()),
            &startup_info,
            &mut process_info,
        )
        .map_err(|e| mcp_cli_rs::error::McpError::IOError {
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to spawn daemon process: {}", e)
            ),
        })?;
        
        // Close handles immediately - we don't need them and the process will continue running
        let _ = CloseHandle(process_info.hProcess);
        let _ = CloseHandle(process_info.hThread);
        
        if debug_mode {
            tracing::info!("Daemon spawned with visible console (debug mode), PID: {}", process_info.dwProcessId);
        } else {
            tracing::info!("Daemon spawned with hidden console, PID: {}", process_info.dwProcessId);
        }
    }
    
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

    async fn send_request(&mut self, _request: &mcp_cli_rs::daemon::protocol::DaemonRequest) -> mcp_cli_rs::error::Result<mcp_cli_rs::daemon::protocol::DaemonResponse> {
        // Direct mode doesn't use daemon protocol - commands handle connections directly
        Err(mcp_cli_rs::error::McpError::InvalidProtocol {
            message: "Direct mode doesn't support daemon protocol requests".to_string(),
        })
    }

    async fn list_servers(&mut self) -> mcp_cli_rs::error::Result<Vec<String>> {
        let servers: Vec<String> = self.config.servers.iter().map(|s| s.name.clone()).collect();
        Ok(servers)
    }

    async fn list_tools(&mut self, server_name: &str) -> mcp_cli_rs::error::Result<Vec<mcp_cli_rs::daemon::protocol::ToolInfo>> {
        // Get server config and create transport directly
        let server_config = self.config.get_server(server_name)
            .ok_or_else(|| mcp_cli_rs::error::McpError::ServerNotFound {
                server: server_name.to_string(),
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
        transport.send(init_request).await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?;

        // Server automatically sends notifications/initialized - we don't need to send it
        // Now send tools/list request
        let mcp_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        // Send request and get response
        let response = transport.send(mcp_request).await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?;

        // Parse response
        if let Some(result) = response.get("result") {
            let tools = if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array.iter().filter_map(|tool| {
                    Some(mcp_cli_rs::daemon::protocol::ToolInfo {
                        name: tool.get("name")?.as_str()?.to_string(),
                        description: tool.get("description")?.as_str().map(|s| s.to_string()).unwrap_or_default(),
                        input_schema: tool.get("inputSchema").map(|v| v.clone())
                            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
                    })
                }).collect()
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

    async fn execute_tool(&mut self, server_name: &str, tool_name: &str, arguments: serde_json::Value) -> mcp_cli_rs::error::Result<serde_json::Value> {
        // Get server config and create transport directly
        let server_config = self.config.get_server(server_name)
            .ok_or_else(|| mcp_cli_rs::error::McpError::ServerNotFound {
                server: server_name.to_string(),
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
        transport.send(init_request).await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
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
        let response = transport.send(mcp_request).await
            .map_err(|e| mcp_cli_rs::error::McpError::IOError {
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?;

        // Parse response
        if let Some(result) = response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = response.get("error") {
            let message = error.get("message")
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
}