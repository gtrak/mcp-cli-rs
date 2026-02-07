use clap::{Parser, Subcommand};
use mcp_cli_rs::cli::commands::{cmd_list_servers, cmd_server_info, cmd_tool_info, cmd_call_tool, cmd_search_tools};
use mcp_cli_rs::cli::daemon::ensure_daemon;
use mcp_cli_rs::config::Config;
use mcp_cli_rs::config::loader::{find_and_load, load_config};
use mcp_cli_rs::error::{exit_code, Result};

#[derive(Parser)]
#[command(name = "mcp")]
#[command(about = "MCP CLI client for tool discovery and execution", long_about = None)]
struct Cli {
    /// Path to configuration file (mcp_servers.toml)
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
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
    let daemon_config = if let Some(path) = &cli.config {
        // Use explicitly provided config path
        load_config(path).await?
    } else {
        // Search for config in standard locations
        find_and_load(None).await?
    };

    // Ensure daemon is running with fresh config
    let daemon_client = ensure_daemon(&daemon_config).await
        .map_err(|e| mcp_cli_rs::error::McpError::io_error(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    // Use daemon client for all operations
    match cli.command {
        Some(Commands::List { with_descriptions }) => {
            cmd_list_servers(daemon_client, with_descriptions).await
        }
        Some(Commands::Info { name }) => {
            cmd_server_info(daemon_client, &name).await
        }
        Some(Commands::Tool { tool }) => {
            cmd_tool_info(daemon_client, &tool).await
        }
        Some(Commands::Call { tool, args }) => {
            cmd_call_tool(daemon_client, &tool, args.as_deref()).await
        }
        Some(Commands::Search { pattern }) => {
            cmd_search_tools(daemon_client, &pattern).await
        }
        None => {
            cmd_list_servers(daemon_client, false).await
        }
    }
}