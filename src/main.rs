//! Binary entry point for MCP CLI tool.
//!
//! This is a thin wrapper that delegates to the library entry point
//! in the cli module.

use mcp_cli_rs::cli::entry::main as entry_main;
use mcp_cli_rs::error::exit_code;

#[tokio::main]
async fn main() {
    // Delegate to the library entry point
    if let Err(e) = entry_main().await {
        eprintln!("error: {}", e);
        std::process::exit(exit_code(&e));
    }
}
