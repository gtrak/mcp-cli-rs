# Architecture Research

**Domain:** MCP Client CLI (Rust)
**Researched:** 2026-02-06
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Layer                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │  list    │  │ inspect  │  │  call    │  │  search  │   │
│  │ servers  │  │ servers │  │  tools   │  │  tools   │   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘   │
│       │            │            │            │             │
└───────┴────────────┴────────────┴────────────┴─────────────┘
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Command Orchestrator                     │   │
│  │  (Parse args → route → execute → format output)       │   │
│  └──────────────────────┬───────────────────────────────┘   │
├─────────────────────────┼───────────────────────────────────┤
│  ┌─────────────────────▼─────────────────────────────────┐ │
│  │               MCP Client Abstraction                  │ │
│  │         (connection management, protocol)            │ │
│  └──────────────────────┬───────────────────────────────┘ │
├─────────────────────────┼───────────────────────────────────┤
│  ┌──────────────────┬─┴─┬──────────────────┬─────────────┐ │
│  │   Stdio          │   │   HTTP           │   Daemon     │ │
│  │   Transport      │   │   Transport      │   IPC        │ │
│  │   (process)      │   │   (reqwest)      │   (socket)   │ │
│  └──────────────────┘   └──────────────────┘─────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                        Infrastructure                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Config   │  │  Errors  │  │ Env Var  │  │  Retry   │  │
│  │ Parser   │  │  With    │  │  Sub     │  │  Logic   │  │
│  └──────────┘  │  Sug.    │  └──────────┘  └──────────┘  │
│                └──────────┘                                   │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **CLI Layer** | Parse user commands, route to handlers, format output | clap derive macros + subcommand enum |
| **Command Orchestrator** | Connect CLI args to business logic, coordinate async operations | Command pattern, async functions |
| **MCP Client** | Protocol handling, connection lifecycle, request/response | Trait-based abstraction + transport agnostic |
| **Transports** | Low-level communication with MCP servers | tokio::process for stdio, reqwest for HTTP |
| **Daemon Worker** | Connection pooling, persistent connections, IPC server | tokio::net for Unix sockets/named pipes |
| **Config Module** | Parse mcp_servers.json, env var substitution | serde + shellexpand/substitutions |
| **Error Handling** | Structured errors with suggestions, machine-parsable | thiserror + Display + From impls |
| **Retry Logic** | Exponential backoff for transient failures | tokio time + exponential backoff policy |
| **Filtering** | Apply allowedTools/disabledPatterns | glob pattern matching |

## Recommended Project Structure

```
src/
├── lib.rs              # Library exports, re-exports
├── main.rs             # CLI entry point, clap Command setup
├── cli/                # CLI command definitions
│   ├── mod.rs          # CLI module exports
│   ├── commands.rs     # clap derive structs
│   ├── list.rs         # `mcp list servers` command
│   ├── inspect.rs      # `mcp inspect <server>` command
│   ├── call.rs         # `mcp call <tool>` command
│   └── search.rs       # `mcp search <pattern>` command
├── client/             # MCP client abstraction
│   ├── mod.rs          # Client trait + implementation
│   ├── transport.rs    # Transport trait + stdio/HTTP impls
│   ├── protocol.rs     # MCP protocol (JSON-RPC wrapper)
│   └── connection.rs   # Connection management, state
├── daemon/             # Background daemon worker
│   ├── mod.rs          # Daemon entry point, IPC handling
│   ├── ipc.rs          # Unix socket / named pipe abstraction
│   └── pool.rs         # Connection pool management
├── config/             # Configuration management
│   ├── mod.rs          # Config struct, from file/env
│   ├── loader.rs       # mcp_servers.json parsing
│   └── env.rs          # Environment variable substitution
├── error.rs            # Error types enum + Display + suggestions
├── retry.rs            # Retry logic with exponential backoff
└── utils/              # Helper utilities
    ├── glob.rs         # Pattern matching for tool filtering
    └── json.rs         # JSON schema validation helpers

tests/
├── cli_tests.rs        # Integration tests for CLI commands
├── client_tests.rs     # Unit tests for MCP client
└── fixtures/           # Test data (config files, mock responses)
```

### Structure Rationale

- **`cli/`**: Isolates command-line concerns (parsing, formatting) from business logic. Makes testing easier and keeps business logic library-usable.
- **`client/`**: Core MCP protocol handling. Transport-agnostic via trait, enabling stdio/HTTP without business logic changes.
- **`daemon/`**: Separate concern for long-running processes. IPC abstraction allows Unix sockets (*nix) and named pipes (Windows) via trait.
- **`config/`**: Parsing and substitution logic is reusable across CLI commands and daemon. Separate from app logic for easy testing.
- **`error.rs`**: Centralized error type with `thiserror` provides consistent user-facing and machine-parsable error messages.
- **`utils/`**: Glob matching and JSON helpers extracted when logic gets complex enough to warrant reuse.

## Architectural Patterns

### Pattern 1: Transport Abstraction (Trait-based)

**What:** Define a `Transport` trait that abstracts stdio process spawning and HTTP requests. MCP client depends on trait, not concrete implementations.

**When to use:** Multiple communication modes (stdio, HTTP) need to be switched at runtime without code changes.

**Trade-offs:**
- **Pros:** Testable (mock transports), flexible (add SSE later), cleaner separation
- **Cons:** Boxing/dyn overhead (minimal for this use case), trait bounds complexity

**Example:**
```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, message: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn ping(&self) -> Result<()>;
}

// Stdio transport
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

// HTTP transport
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: Url,
}

// Client uses trait
impl McpClient {
    pub async fn connect(&mut self, transport: Box<dyn Transport>) -> Result<()> {
        // Implementation works regardless of transport
    }
}
```

### Pattern 2: Async Command Pattern

**What:** Each CLI subcommand is an async function that returns `Result<Output>`. Main function routes to command and formats output.

**When to use:** All commands perform async operations (transport I/O, daemon communication).

**Trade-offs:**
- **Pros:** Clear control flow, easy to test commands, consistent error handling
- **Cons:** Requires runtime everywhere (but tokio is needed anyway)

**Example:**
```rust
// Define command trait or use async functions
pub async fn list_servers(ctx: &AppContext, args: &ListArgs) -> Result<Output> {
    let servers = ctx.config.load_servers()?;
    let mut results = Vec::new();

    for server in servers {
        let client = ctx.client_pool.get(&server.name).await?;
        let tools = client.list_tools().await?;
        results.push(ServerInfo { name: server.name, tool_count: tools.len() });
    }

    Ok(Output::Servers(results))
}

// Main routing
#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let ctx = AppContext::new(&args.config_path).await?;

    match args.command {
        Commands::List(list_args) => {
            let output = commands::list_servers(&ctx, &list_args).await?;
            println!("{}", output);
        }
        Commands::Call(call_args) => {
            let output = commands::call_tool(&ctx, &call_args).await?;
            println!("{}", output);
        }
        // ...
    }

    Ok(())
}
```

### Pattern 3: Daemon IPC Abstraction

**What:** Define `IpcServer` and `IpcClient` traits for platform-specific communication. Unix uses `tokio::net::UnixListener`, Windows uses named pipes.

**When to use:** Need persistent connection pooling across CLI invocations with cross-platform support.

**Trade-offs:**
- **Pros:** Cross-platform unified API, testable connections, easy to extend
- **Cons:** Platform conditionals (`#[cfg(unix)]`, `#[cfg(windows)]`), testing complexity

**Example:**
```rust
#[async_trait]
pub trait IpcServer {
    async fn accept(&mut self) -> Result<IpcStream>;
    async fn close(self) -> Result<()>;
}

#[async_trait]
pub trait IpcClient {
    async fn connect(&self) -> Result<IpcStream>;
}

// Unix implementation
#[cfg(unix)]
pub struct UnixIpcServer {
    listener: UnixListener,
}

// Windows implementation
#[cfg(windows)]
pub struct NamedPipeServer {
    // Windows-specific named pipe implementation
}
```

### Pattern 4: Error Chain with Suggestions

**What:** Use `thiserror` to derive error types from external errors, add context, and provide actionable user suggestions via custom Display.

**When to use:** CLI needs human-readable errors and machine-parsable output for LLM skills.

**Trade-offs:**
- **Pros:** Consistent error handling, good UX, easy debugging
- **Cons:** Boilerplate, manual suggestion maintenance

**Example:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("Failed to connect to server '{name}': {source}")]
    ConnectionError {
        name: String,
        #[source]
        source: std::io::Error,
        #[suggestion]
        help: &'static str,
    },

    #[error("Invalid JSON-RPC response: {message}")]
    InvalidProtocol { message: String },

    #[error("Tool '{tool}' not found in server '{server}'")]
    ToolNotFound { tool: String, server: String },
}

// Custom Display with suggestions
impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionError { name, source, help } => {
                write!(f, "Failed to connect to '{}': {}\n\n{}", name, source, help)
            }
            _ => write!(f, "{self:?}")
        }
    }
}
```

## Data Flow

### Request Flow (Direct Mode - No Daemon)

```
[User Input]
    ↓
[clap::Parser] → Parsed args (struct)
    ↓
[Command Handler] → Business logic + async operations
    ↓
[McpClient] → Transport trait
    ↓
[StdioTransport] → tokio::process::Command.spawn()
    ↓
[Server Process] → JSON-RPC over stdin/stdout
    ↓
[JsonRpcResponse] → Parse & validate
    ↓
[Result Formatter] → Human-friendly output
    ↓
[stdout] → Display to user
```

### Request Flow (Daemon Mode - Connection Pooling)

```
[CLI] → [IpcClient] → [Unix Socket / Named Pipe]
    ↓                                      ↓
[Request] <───────────────────────────── [IpcServer]
    ↓                                      ↓
    ↓                                 [Daemon Loop]
    ↓                                      ↓
    ↓                              [Connection Pool]
    ↓                                      ↓
    <──────────────────────────────── [Cached Client]
    ↓
[Response] → Format → stdout
```

### State Management

**No global mutable state.** Each command execution:

1. Loads config from file/env
2. Creates context (`AppContext`) from config
3. Initializes connections (daemon or direct)
4. Executes async command
5. Returns result
6. Drops context (clean up)

**Daemon state:**
```rust
struct DaemonState {
    connections: HashMap<String, Box<dyn Transport>>,
    config: Arc<Config>,
}

// Daemon spawns tokio tasks for each request
// State is shared via Arc<Mutex<DaemonState>> if needed (usually read-only)
```

### Key Data Flows

1. **Configuration Loading:**
   - Read `mcp_servers.json` from config location
   - Substitute `{{ENV_VAR}}` with environment variables
   - Validate server definitions (URL, command, env)
   - Cache in memory for command duration

2. **Stdio Transport (Direct):**
   - `tokio::process::Command::new(program).args(args).spawn()`
   - Write JSON-RPC request to stdin
   - Read JSON response from stdout (async, with timeout)
   - Handle process exit codes

3. **HTTP Transport:**
   - `reqwest::Client::post(url).json(request).send().await`
   - Parse JSON response
   - Handle HTTP errors (4xx, 5xx) with retry logic

4. **Tool Execution:**
   - Parse tool schema (input JSON Schema)
   - Validate user arguments against schema
   - Send `callTool` request to server
   - Parse response, format output

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-10 servers | Direct connections, no daemon (simpler) |
| 10-100 servers | Daemon with connection pooling (default target) |
| 100+ servers | Consider sharding, lazy loading, TTL for connections |

### Scaling Priorities

1. **First bottleneck:** Connection setup overhead (process spawning, HTTP handshake).
   - **Fix:** Daemon with persistent connections (already in design)

2. **Second bottleneck:** Concurrent tool calls blocking each other.
   - **Fix:** Semaphores for concurrent limits (`tokio::sync::Semaphore`), async parallel execution (`futures::future::join_all`)

3. **Memory usage:** Tool schema caching for 1000+ tools.
   - **Fix:** Lazy loading (fetch schema on demand), cache with TTL

## Anti-Patterns

### Anti-Pattern 1: Blocking in Async Code

**What people do:** Use `std::fs::read_to_file` or `std::process::Command` inside `.await` code paths.

**Why it's wrong:** Blocks the entire tokio executor, making async concurrent operations effectively serial.

**Do this instead:** Use `tokio::fs::read_to_file` and `tokio::process::Command` everywhere.

### Anti-Pattern 2: Cloning Large Data in Hot Path

**What people do:** Clone entire config or server list for each tool call.

**Why it's wrong:** Unnecessary allocations, slows down concurrent operations.

**Do this instead:** Use `Arc<Config>` and pass references (`&Config`) instead of cloning.

### Anti-Pattern 3: Ignoring Error Context

**What people do:** Return `std::io::Error` directly without context (which program? which server?).

**Why it's wrong:** Users can't debug failure ("failed to read" vs "failed to read config from $HOME/.config/mcp/servers.json").

**Do this instead:** Wrap errors with `map_err` or use `thiserror` to add context.

### Anti-Pattern 4: Tight Coupling to Specific Transport

**What people do:** `tokio::process::Command::new()` called directly in command handlers.

**Why it's wrong:** Can't switch to HTTP or other transports without rewriting commands.

**Do this instead:** Depend on `dyn Transport` trait, concrete implementations are interchangeable.

### Anti-Pattern 5: Global Mutable State

**What people do:** `lazy_static!` or `once_cell` mutable caches across invocations.

**Why it's wrong:** Hard to test, thread safety issues, surprising behavior.

**Do this instead:** Pass context (`AppContext`) to commands. Daemon uses `Arc<Mutex<State>>` explicitly only where needed.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| MCP Server (stdio) | `tokio::process` + JSON-RPC over stdin/stdout | Spawns process, communicates via pipes |
| MCP Server (HTTP) | `reqwest` + JSON-RPC over HTTP POST | Supports both Streamable HTTP and SSE fallback |
| Daemon (Unix) | `tokio::net::UnixListener` + Unix domain socket | /tmp/mcp-cli-<user>/daemon.sock |
| Daemon (Windows) | Named pipes (see `interprocess` crate) | \\\\.\\pipe\\mcp-cli-daemon |
| Config File | `tokio::fs` + `serde_json` + shellexpand | $HOME/.config/mcp/servers.json |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| CLI → Client | Trait (`McpClient`) | Commands depend on trait, not implementation |
| Client → Transport | Trait (`Transport`) | Client can use stdio or HTTP without code changes |
| CLI → Daemon | IPC (Unix socket / named pipe) | Structured messages (JSON or bincode) |
| Commands → Config | Shared struct (`Arc<Config>`) | Read-only after load, thread-safe |

## Sources

- [MCP TypeScript SDK - Client Documentation](https://raw.githubusercontent.com/modelcontextprotocol/typescript-sdk/main/docs/client.md) (HIGH confidence - official)
- [clap Parser Tutorial](https://docs.rs/clap/latest/clap/) (HIGH confidence - official docs)
- [tokio::process::Command](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) (HIGH confidence - official docs)
- [tokio::net for IPC](https://docs.rs/tokio/latest/tokio/net/) (HIGH confidence - official docs)
- [thiserror crate](https://docs.rs/thiserror/) (HIGH confidence - community standard)
- [Rust CLI Book](https://rust-cli.github.io/book/) (MEDIUM confidence - community resource)
- [MCP GitHub Topic](https://github.com/topics/model-context-protocol) (LOW confidence - ecosystem survey)

---
*Architecture research for: MCP Client CLI (Rust)*
*Researched: 2026-02-06*
