//! Command routing and dispatch for CLI.
//!
//! Handles the main command dispatch logic, routing each CLI subcommand
//! to its appropriate handler.

use crate::cli::info::{cmd_server_info, cmd_tool_info};
use crate::cli::list::cmd_list_servers;
use crate::cli::search::cmd_search_tools;
use crate::cli::call::cmd_call_tool;
use crate::cli::DetailLevel;
use crate::cli::daemon_lifecycle::{
    create_auto_daemon_client, create_require_daemon_client, create_direct_client,
};
use crate::config::Config;
use crate::error::Result;
use crate::format::OutputMode;
use crate::ipc::ProtocolClient;
use clap::Subcommand;
use std::sync::Arc;

/// CLI commands enum - extracted from main.rs
#[derive(Clone, Subcommand)]
pub enum Commands {
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

/// Run mode for command execution
#[derive(Clone)]
pub enum RunMode {
    /// Direct mode - no daemon
    Direct,
    /// Auto daemon mode - spawn if needed
    AutoDaemon,
    /// Require daemon - fail if not running
    RequireDaemon,
}

/// Execute a CLI command using the specified client and mode.
///
/// # Arguments
/// * `command` - The CLI subcommand to execute
/// * `client` - The protocol client to use
/// * `output_mode` - Output format (human or JSON)
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(McpError)` - Command execution error
pub async fn execute_command(
    command: Option<Commands>,
    client: Box<dyn ProtocolClient>,
    output_mode: OutputMode,
) -> Result<()> {
    let command = match command {
        Some(cmd) => cmd,
        None => {
            // Default: list servers with summary detail
            return cmd_list_servers(client, DetailLevel::Summary, output_mode).await;
        }
    };

    match command {
        Commands::Daemon { .. } => {
            // Daemon subcommand is handled separately in main.rs
            Ok(())
        }
        Commands::Shutdown => {
            // Shutdown subcommand is handled separately in main.rs
            Ok(())
        }
        Commands::List { describe, verbose } => {
            let detail_level = if verbose {
                DetailLevel::Verbose
            } else if describe {
                DetailLevel::WithDescriptions
            } else {
                DetailLevel::Summary
            };
            cmd_list_servers(client, detail_level, output_mode).await
        }
        Commands::Info { name } => cmd_server_info(client, &name, output_mode).await,
        Commands::Tool {
            tool,
            describe,
            verbose,
        } => {
            let detail_level = if verbose {
                DetailLevel::Verbose
            } else if describe {
                DetailLevel::WithDescriptions
            } else {
                DetailLevel::Summary
            };
            cmd_tool_info(client, &tool, detail_level, output_mode).await
        }
        Commands::Call { tool, args } => {
            cmd_call_tool(client, &tool, args.as_deref(), output_mode).await
        }
        Commands::Search {
            pattern,
            describe,
            verbose,
        } => {
            let detail_level = if verbose {
                DetailLevel::Verbose
            } else if describe {
                DetailLevel::WithDescriptions
            } else {
                DetailLevel::Summary
            };
            cmd_search_tools(client, &pattern, detail_level, output_mode).await
        }
    }
}

/// Run command in direct mode (no daemon).
///
/// Creates a direct client and executes the command.
pub async fn run_command_direct(
    command: Option<Commands>,
    config: Arc<Config>,
) -> Result<()> {
    let output_mode = OutputMode::Human;
    
    // Create direct client
    let client = create_direct_client(config).await?;
    
    // Execute command
    execute_command(command, client, output_mode).await
}

/// Run command in auto-daemon mode (spawn if needed).
///
/// Creates or connects to a daemon and executes the command.
pub async fn run_command_auto_daemon(
    command: Option<Commands>,
    config: &Config,
) -> Result<()> {
    let output_mode = OutputMode::Human;
    
    // Get or spawn daemon client
    let client = create_auto_daemon_client(config).await?;
    
    // Execute command
    execute_command(command, client, output_mode).await
}

/// Run command in require-daemon mode (fail if not running).
///
/// Connects to an existing daemon and executes the command.
pub async fn run_command_require_daemon(
    command: Option<Commands>,
    config: &Config,
) -> Result<()> {
    let output_mode = OutputMode::Human;
    
    // Try to connect to daemon
    let client = create_require_daemon_client(config).await?;
    
    // Execute command
    execute_command(command, client, output_mode).await
}

/// Dispatch a CLI command to its handler based on run mode.
///
/// # Arguments
/// * `command` - The CLI subcommand to execute
/// * `config` - Application configuration
/// * `run_mode` - Execution mode (direct, auto-daemon, require-daemon)
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(McpError)` - Command execution error
pub async fn dispatch_command(
    command: Option<Commands>,
    config: Arc<Config>,
    run_mode: RunMode,
) -> Result<()> {
    match run_mode {
        RunMode::Direct => run_command_direct(command, config).await,
        RunMode::AutoDaemon => run_command_auto_daemon(command, &config).await,
        RunMode::RequireDaemon => run_command_require_daemon(command, &config).await,
    }
}

/// Determine run mode from CLI flags.
pub fn get_run_mode(no_daemon: bool, require_daemon: bool) -> RunMode {
    if no_daemon {
        RunMode::Direct
    } else if require_daemon {
        RunMode::RequireDaemon
    } else {
        RunMode::AutoDaemon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_run_mode_direct() {
        let mode = get_run_mode(true, false);
        assert!(matches!(mode, RunMode::Direct));
    }

    #[test]
    fn test_get_run_mode_require_daemon() {
        let mode = get_run_mode(false, true);
        assert!(matches!(mode, RunMode::RequireDaemon));
    }

    #[test]
    fn test_get_run_mode_auto_daemon() {
        let mode = get_run_mode(false, false);
        assert!(matches!(mode, RunMode::AutoDaemon));
    }

    #[test]
    fn test_commands_variants_exist() {
        // Verify Commands enum has expected variants
        let _ = Commands::List {
            describe: false,
            verbose: false,
        };
        let _ = Commands::Info { name: "test".to_string() };
        let _ = Commands::Tool {
            tool: "test".to_string(),
            describe: false,
            verbose: false,
        };
        let _ = Commands::Call {
            tool: "test".to_string(),
            args: None,
        };
        let _ = Commands::Search {
            pattern: "test".to_string(),
            describe: false,
            verbose: false,
        };
        let _ = Commands::Shutdown;
        let _ = Commands::Daemon {
            ttl: None,
            socket_path: None,
        };
    }
}
