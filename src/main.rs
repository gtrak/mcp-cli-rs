use clap::{Parser, Subcommand};
use mcp_cli_rs::error::{exit_code, McpError, Result};

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
    match cli.command {
        Some(Commands::List { with_descriptions }) => {
            // TODO: Implement in Plan 04
            eprintln!("List command not yet implemented");
            Ok(())
        }
        Some(Commands::Info { name }) => {
            // TODO: Implement in Plan 04
            eprintln!("Info command not yet implemented");
            Ok(())
        }
        Some(Commands::Tool { tool }) => {
            // TODO: Implement in Plan 04
            eprintln!("Tool command not yet implemented");
            Ok(())
        }
        Some(Commands::Call { tool, args }) => {
            // TODO: Implement in Plan 04
            eprintln!("Call command not yet implemented");
            Ok(())
        }
        Some(Commands::Search { pattern }) => {
            // TODO: Implement in Plan 04
            eprintln!("Search command not yet implemented");
            Ok(())
        }
        None => {
            // DISC-01: List all servers when no subcommand provided
            // TODO: Implement in Plan 04
            eprintln!("No command specified - will list servers");
            Ok(())
        }
    }
}
