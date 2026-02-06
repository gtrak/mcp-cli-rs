# Research: Phase 1 - Core Protocol & Configuration

**Phase:** 1 - Core Protocol & Configuration
**Source:** .planning/research/*-SUMMARY.md and detailed research docs

## Standard Stack (from SUMMARY.md)

**Core technologies:**
- **clap 4.5.57** — CLI argument parsing with derive macros
- **tokio 1.49.0** — Async runtime with `features = ["full"]`
- **mcp-sdk 0.0.3** — MCP protocol implementation (LOW maturity - evaluate)
- **reqwest 0.13.1** — HTTP client for MCP HTTP transport
- **anyhow + thiserror** — Error handling
- **tracing + tracing-subscriber** — Logging
- **shell-words 1.1.1** — Shell command parsing (prevent injection)

## mcp-sdk Evaluation Gap

**Status:** Requires evaluation during Phase 1 planning

**Concerns:**
- Version 0.0.3 has minimal documentation (11.36% on docs.rs)
- Low adoption (827 downloads)
- May not handle initialization handshake, message delimiters, or error responses correctly

**Decision needed:** Use mcp-sdk or reimplement with tokio + serde_json?

## Architecture (from ARCHITECTURE.md)

**Recommended structure:**
```
src/
├── lib.rs              # Library exports
├── main.rs             # CLI entry point, clap Command setup
├── cli/                # CLI command definitions (list, inspect, call, search)
├── client/             # MCP client abstraction (transport, protocol, connection)
├── config/             # Configuration management (loader, env substitution)
├── error.rs            # Error types enum + Display + suggestions
└── utils/              # Helper utilities (glob, json)
```

**Key patterns:**
- Transport abstraction trait (stdio vs HTTP)
- Async command pattern
- Error chain with suggestions (thiserror)
- No global mutable state (explicit AppContext)

## Critical Pitfalls for Phase 1 (from PITFALLS.md)

1. **Windows process spawning without kill_on_drop** — Zombie processes accumulate
   - Prevention: ALWAYS set `.kill_on_drop(true)` with `tokio::process::Command`

2. **Stdio transport missing newline delimiter** — MCP spec requires newline-delimited JSON
   - Prevention: Use `writeln!` for messages, never `serde_json::to_string_pretty`

3. **Environment variable substitution injection** — Command injection vulnerability
   - Prevention: Always parse using `shell-words::split()`, substitute atomically

4. **Blocking I/O in async code** — Blocks executor thread
   - Prevention: Use `tokio::fs` and `tokio::process` everywhere

## Phase 1 Focus

**What to build (25 requirements):**
- Configuration parsing (CONFIG-01 through CONFIG-05)
- Server connections (stdio + HTTP) (CONN-01, CONN-02, CONN-03, CONN-04)
- Tool discovery & listing (DISC-01, DISC-02, DISC-03, DISC-04, DISC-06)
- Tool execution (EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-06)
- Error handling (ERR-01, ERR-02, ERR-03, ERR-05, ERR-06)
- CLI support (CLI-01, CLI-02, CLI-03)
- Cross-platform stdio (XP-03)

**Success criteria (from ROADMAP):**
1. User can configure servers in mcp_servers.toml
2. User can discover all configured servers and tools
3. User can inspect tool details including JSON Schema
4. User can execute tools with JSON arguments
5. User receives clear, actionable error messages

## Installation

```toml
[dependencies]
# Core framework
clap = { version = "4.5.57", features = ["derive"] }
tokio = { version = "1.49.0", features = ["full"] }

# MCP protocol
mcp-sdk = "0.0.3"  # Evaluate - may replace with tokio + serde_json
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"

# Transport/Network
reqwest = { version = "0.13.1", features = ["json"] }

# Error handling
anyhow = "1.0.101"
thiserror = "2.0.18"

# Logging
tracing = "0.1.44"
tracing-subscriber = "0.3.22"

# Utilities
shell-words = "1.1.1"
```

