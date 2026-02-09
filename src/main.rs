use clap::{Parser, Subcommand};
use mcp_cli_rs::cli::commands::{cmd_list_servers, cmd_server_info, cmd_tool_info, cmd_call_tool, cmd_search_tools};
use mcp_cli_rs::cli::daemon::ensure_daemon;
use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::error::{exit_code, Result};
use mcp_cli_rs::shutdown::{GracefulShutdown, run_with_graceful_shutdown};
use mcp_cli_rs::ipc::ProtocolClient;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "mcp")]
#[command(about = "MCP CLI client for tool discovery and execution", long_about = None)]
struct Cli {
    /// Path to configuration file (mcp_servers.toml)
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Run without daemon (direct mode) - connects to servers directly without caching
    #[arg(long, global = true)]
    no_daemon: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Subcommand)]
enum Commands {
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

    // Run CLI operations with graceful shutdown support
    let result = run_with_graceful_shutdown(
        || async {
            if cli.no_daemon {
                // Direct mode: connect to servers without daemon
                tracing::info!("Running in direct mode (no daemon)");
                run_direct_mode(&cli, Arc::clone(&daemon_config)).await
            } else {
                // Daemon mode: use connection caching
                run_daemon_mode(&cli, Arc::clone(&daemon_config)).await
            }
        },
        shutdown_rx,
    ).await?;

    // Return the result (or ShutdownError if shutdown was requested)
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
async fn execute_command(cli: &Cli, mut client: Box<dyn mcp_cli_rs::ipc::ProtocolClient>) -> Result<()> {
    use mcp_cli_rs::cli::commands::*;
    
    let command = cli.command.clone();
    match command {
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
        
        // Build MCP tools/list JSON-RPC request
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
        
        // Build MCP tools/call JSON-RPC request
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