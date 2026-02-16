# MCP CLI

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**MCP CLI client in Rust — Cross-platform MCP server interaction without runtime dependencies**

A CLI for discovering, inspecting, and executing tools from Model Context Protocol (MCP) servers with lazy background daemon. Works on Linux, macOS, and **Windows** (via named pipes).

[Vibe-kit Disclaimer](https://github.com/gtrak/vibe-kit/blob/main/DISCLAIMER.md)

This is just an experiment for its own sake.  I don't intend to rely on it heavily. Use the original https://github.com/philschmid/mcp-cli.

---

## Quick Start

```bash
# Install from source
git clone https://github.com/gtrak/mcp-cli-rs.git
cd mcp-cli-rs
cargo build --release

# Create config at ~/.config/mcp/mcp_servers.toml
cat > ~/.config/mcp/mcp_servers.toml << 'EOF'
[[servers]]
name = "filesystem"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"] }
EOF

# List available tools
./target/release/mcp list
```

---

## Why This Rewrite?

This is a Rust rewrite of the original [Bun-based MCP CLI](https://github.com/f/modelcontextprotocol). Key improvements:

- **Windows Support** — Full Windows compatibility using named pipes (original had Windows process spawning issues)
- **No Runtime Dependencies** — Single compiled binary, no Bun/Node.js required
- **Cross-Platform** — Native Unix sockets on Linux/macOS, named pipes on Windows
- **Better Performance** — Rust's async runtime with connection caching via daemon

---

## Installation

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (1.70+)

### From Source

```bash
# Clone and build
git clone https://github.com/gtrak/mcp-cli-rs.git
cd mcp-cli-rs
cargo build --release

# Binary will be at ./target/release/mcp
# Optional: Copy to your PATH
cp ./target/release/mcp ~/.local/bin/
```

### Platform Notes

- **Linux/macOS**: Uses Unix domain sockets for daemon IPC
- **Windows**: Uses named pipes for daemon IPC (requires daemon mode for server connections)

---

## Usage

### Command Format

Two styles supported for tool calls:

```bash
# Space-separated (clearer for complex arguments)
mcp call filesystem read_file '{"path": "/etc/hosts"}'

# Slash-separated (compact)
mcp call filesystem/read_file '{"path": "/etc/hosts"}'
```

### Basic Examples

```bash
# List all servers and their tools
mcp list
mcp list -d              # With descriptions
mcp list --json          # JSON output for scripting

# Show server information
mcp info filesystem      # Server details
mcp info filesystem read_file   # Tool schema

# Search for tools
mcp search "*file*"      # Pattern matching across all servers

# Call a tool
mcp call filesystem read_file '{"path": "/etc/hosts"}'
mcp call filesystem/read_file < /tmp/args.json   # From stdin

# Daemon control
mcp daemon               # Start daemon
mcp shutdown             # Stop daemon
```

### Global Flags

```
-c, --config <PATH>      Custom config file path
    --json               Output as JSON
    --no-daemon          Run in direct mode (no connection caching)
    --auto-daemon        Auto-spawn daemon if not running (default)
    --require-daemon     Fail if daemon is not running
-h, --help               Show help
-V, --version            Show version
```

---

## Configuration

Config file location (in order of precedence):
1. Path specified with `--config`
2. `~/.config/mcp/mcp_servers.toml`
3. `~/.config/mcp/mcp.toml`

### TOML Format

```toml
[[servers]]
name = "filesystem"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"] }

[[servers]]
name = "fetch"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-fetch"] }

[[servers]]
name = "http-server"
transport = { type = "http", url = "http://localhost:3000/mcp" }

# Optional: Global settings
concurrency_limit = 5    # Max concurrent operations
retry_max = 3           # Max retry attempts
retry_delay_ms = 1000   # Initial retry delay
 timeout_secs = 1800     # Operation timeout
daemon_ttl = 60         # Daemon idle timeout in seconds
```

### Tool Filtering

```toml
[[servers]]
name = "filesystem"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-filesystem", "/"] }

# Allow only specific tools
allowed_tools = ["read_file", "list_directory"]

# Or block specific tools
disabled_tools = ["write_file", "delete_file"]
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `MCP_NO_DAEMON=1` | Disable daemon (direct mode) |
| `MCP_DAEMON_TTL=N` | Set daemon idle timeout in seconds (default: 60) |

---

## Commands

### `list` — List servers and tools

```bash
mcp list                 # List all servers with tool counts
mcp list -d              # Include tool descriptions
mcp list -v              # Verbose with full schemas
mcp list --json          # Machine-readable JSON output
```

### `info` — Show server or tool details

```bash
mcp info <server>              # Server details (transport, tools count)
mcp info <server> <tool>       # Tool schema (parameters, description)

# Examples
mcp info filesystem
mcp info filesystem read_file
mcp info fetch fetch_url
```

### `call` — Execute a tool

```bash
# With JSON argument
mcp call <server> <tool> '<json>'
mcp call <server/tool> '<json>'

# Examples
mcp call filesystem read_file '{"path": "/etc/hosts"}'
mcp call filesystem/read_file '{"path": "/etc/hosts"}'

# With arguments from stdin
echo '{"url": "https://example.com"}' | mcp call fetch fetch_url

# Flag-style arguments (auto-converted to JSON)
mcp call fetch fetch_url --url https://example.com
mcp call filesystem read_file --path /etc/hosts --limit 100
```

### `search` — Find tools by pattern

```bash
mcp search "*file*"      # Find tools with "file" in the name
mcp search "read*"       # Glob pattern matching
```

### `daemon` — Connection caching

```bash
mcp daemon               # Start daemon (runs in foreground)
mcp daemon --ttl 300     # Start with custom 5-minute TTL

mcp shutdown             # Stop running daemon
```

**Daemon modes:**
- `--no-daemon`: Direct mode, no caching (slower but simpler)
- `--auto-daemon`: Spawn daemon if needed (default, recommended)
- `--require-daemon`: Fail if daemon not running

---

## Development

### Setup

```bash
git clone https://github.com/gary/mcp-cli-rs.git
cd mcp-cli-rs
```

### Build

```bash
cargo build              # Debug build
cargo build --release    # Optimized release build
```

### Test

```bash
# Run library tests
cargo test --lib

# Run integration tests
cargo test --test '*'

# Run all tests
cargo test
```

### Project Structure

```
src/
├── cli/              # CLI parsing and command dispatch
├── client/           # MCP client implementations (stdio, HTTP)
├── config/           # Configuration loading and types
├── daemon/           # Daemon lifecycle and management
├── error/            # Error types and handling
├── format/           # Output formatting (text, JSON)
├── ipc/              # Inter-process communication (sockets/pipes)
├── server/           # MCP server protocol handling
├── shutdown/         # Graceful shutdown handling
└── transport/        # Transport abstractions
```

---

## Troubleshooting

### "Daemon not running"

Start the daemon:
```bash
mcp daemon
# Or use direct mode:
mcp --no-daemon list
```

### "Config file not found"

Create the config file:
```bash
mkdir -p ~/.config/mcp
cat > ~/.config/mcp/mcp_servers.toml << 'EOF'
[[servers]]
name = "filesystem"
transport = { type = "stdio", command = "npx", args = ["-y", "@modelcontextprotocol/server-filesystem", "/"] }
EOF
```

### "Tool not found"

- Check server name: `mcp list`
- Check tool name: `mcp info <server>`
- Verify server is running: `mcp info <server>`

### "Connection refused" or "No such file or directory"

- Verify the MCP server package is installed (e.g., `npx @modelcontextprotocol/server-filesystem`)
- Check the command path in your config
- For HTTP servers, verify the URL is accessible: `curl http://localhost:3000/mcp`

### Windows-specific

On Windows, named pipes require the daemon to be running:
```bash
# Start daemon first
mcp daemon

# Then use in another terminal
mcp list
```

### Getting Debug Output

```bash
# Enable debug logging
RUST_LOG=debug mcp list

# Check daemon logs
cat ~/.cache/mcp-cli/daemon.log
```

---

## License

MIT License - See [LICENSE](LICENSE) for details.

---

## See Also

- [Model Context Protocol](https://modelcontextprotocol.io/) — MCP specification
- [Original MCP CLI](https://github.com/f/modelcontextprotocol) — Bun-based implementation that inspired this rewrite
