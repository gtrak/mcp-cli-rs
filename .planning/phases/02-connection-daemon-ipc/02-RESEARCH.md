# Phase 2: Connection Daemon & Cross-Platform IPC - Research

**Researched:** 2025-02-06
**Domain:** Cross-platform IPC, process daemon management, connection pooling
**Confidence:** HIGH

## Summary

This research covers the implementation of a connection daemon with cross-platform IPC for the MCP CLI tool. The daemon will provide persistent connection pooling across CLI invocations, significantly improving performance (50%+ faster on repeated calls) by avoiding repeated process spawning and HTTP handshakes.

**Key architectural decisions:**
1. **Use `interprocess` crate** for cross-platform IPC abstraction (Unix sockets on *nix, Windows named pipes on Windows)
2. **Implement trait-based IPC abstraction** to avoid scattered `#[cfg]` conditionals in core logic
3. **Default Windows named pipe security** is `SECURITY_IDENTIFICATION` - sufficient for preventing privilege escalation
4. **Idle timeout via `tokio::time::timeout`** with activity tracking for daemon self-termination
5. **File-based coordination** (PID files, mtime checks) for config change detection and orphan cleanup

**Primary recommendation:** Use `interprocess` crate's local socket abstraction with tokio async support, implement daemon as a separate binary spawned lazily by CLI, use JSON-RPC over IPC for CLI-daemon communication.

## Standard Stack

### Core Libraries
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `interprocess` | 2.3+ | Cross-platform IPC (Unix sockets / named pipes) | Provides unified API over platform-specific IPC mechanisms, tokio integration |
| `tokio` | 1.49+ | Async runtime, time/signal handling | Already in use, supports Windows named pipes natively |
| `serde_json` | 1.0+ | JSON-RPC message serialization | Already in use, standard for JSON handling |
| `async-trait` | 0.1+ | Async trait methods | Already in use for Transport trait |

### Platform-Specific
| Platform | Primitive | Implementation |
|----------|-----------|----------------|
| Unix | Unix domain sockets | `tokio::net::UnixListener` / `UnixStream` via `interprocess` |
| Windows | Named pipes | `tokio::net::windows::named_pipe` via `interprocess` |

**Installation:**
```toml
[dependencies]
interprocess = { version = "2.3", features = ["tokio"] }
tokio = { version = "1.49", features = ["full"] }
```

## Architecture Patterns

### Pattern 1: IPC Abstraction Trait

**What:** Define platform-agnostic traits for IPC server/client, with platform-specific implementations behind `#[cfg]` gates in isolated modules.

**When to use:** When you need cross-platform IPC without scattering platform conditionals throughout business logic.

**Example:**
```rust
// ipc/mod.rs - Platform-agnostic interface
#[async_trait]
pub trait IpcServer: Send + Sync {
    async fn accept(&mut self) -> Result<Box<dyn IpcStream>>;
    fn local_addr(&self) -> Result<String>;
}

#[async_trait]
pub trait IpcClient: Send + Sync {
    async fn connect(&self) -> Result<Box<dyn IpcStream>>;
}

#[async_trait]
pub trait IpcStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    async fn shutdown(&mut self) -> Result<()>;
}

// Platform-specific implementations in submodules
#[cfg(unix)]
pub use unix::{UnixIpcServer, UnixIpcClient};

#[cfg(windows)]
pub use windows::{NamedPipeServer, NamedPipeClient};
```

### Pattern 2: Daemon Process Lifecycle

**What:** CLI spawns daemon as separate process on first access, daemon self-terminates after idle timeout.

**Key components:**
1. **Lazy spawning:** CLI checks for daemon via IPC connect, spawns if not running
2. **Idle timeout:** Daemon tracks last activity, exits if idle for 60s
3. **Config fingerprint:** Daemon stores config hash/mtime, CLI checks before reusing
4. **Orphan cleanup:** CLI checks for stale PID files/sockets on startup

**Daemon main loop:**
```rust
pub async fn run_daemon(config: Config, socket_path: PathBuf) -> Result<()> {
    let listener = create_ipc_listener(&socket_path).await?;
    let last_activity = Arc::new(Mutex::new(Instant::now()));
    let config_fingerprint = calculate_fingerprint(&config);
    
    loop {
        let accept_future = listener.accept();
        let timeout_future = tokio::time::sleep(Duration::from_secs(60));
        
        tokio::select! {
            Ok((stream, _)) = accept_future => {
                *last_activity.lock().await = Instant::now();
                let last_activity = last_activity.clone();
                tokio::spawn(handle_client(stream, last_activity));
            }
            _ = timeout_future => {
                let elapsed = last_activity.lock().await.elapsed();
                if elapsed >= Duration::from_secs(60) {
                    tracing::info!("Idle timeout reached, shutting down");
                    break;
                }
            }
        }
    }
    
    cleanup_socket(&socket_path).await?;
    Ok(())
}
```

### Pattern 3: Connection Pool with Health Checks

**What:** Daemon maintains map of server_name → Transport connections, validates before reuse.

**Implementation:**
```rust
pub struct ConnectionPool {
    connections: Arc<Mutex<HashMap<String, PooledConnection>>>,
}

pub struct PooledConnection {
    transport: Box<dyn Transport>,
    created_at: Instant,
    last_used: Instant,
}

impl ConnectionPool {
    pub async fn get(&self, server_name: &str, config: &ServerConfig) -> Result<Box<dyn Transport>> {
        let mut connections = self.connections.lock().await;
        
        // Check if we have a cached connection
        if let Some(conn) = connections.get_mut(server_name) {
            // Health check before reuse
            if conn.health_check().await.is_ok() {
                conn.last_used = Instant::now();
                return Ok(conn.transport);
            }
            // Remove stale connection
            connections.remove(server_name);
        }
        
        // Create new connection
        let transport = create_transport(config).await?;
        connections.insert(server_name.to_string(), PooledConnection {
            transport: transport,
            created_at: Instant::now(),
            last_used: Instant::now(),
        });
        
        Ok(transport)
    }
}
```

### Pattern 4: Config Change Detection

**What:** Compare config file mtime/hash between CLI and daemon to detect stale config.

**Implementation:**
```rust
pub async fn ensure_fresh_daemon(config_path: &Path) -> Result<IpcClient> {
    let current_fingerprint = calculate_fingerprint(&fs::read_to_string(config_path).await?).await;
    
    // Try to connect to existing daemon
    match try_connect_to_daemon().await {
        Ok(client) => {
            // Check if daemon's config is still fresh
            let daemon_fingerprint = client.get_config_fingerprint().await?;
            if daemon_fingerprint == current_fingerprint {
                return Ok(client);
            }
            // Config changed, shut down old daemon
            client.shutdown().await.ok();
        }
        Err(_) => {
            // No daemon running, continue to spawn
        }
    }
    
    // Spawn new daemon with current config
    spawn_daemon(config_path, current_fingerprint).await?;
    try_connect_to_daemon().await
}
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-platform IPC | Custom abstractions over raw sockets/pipes | `interprocess` crate | Handles all platform quirks, tokio integration, well-tested |
| JSON message framing | Manual length-prefix or delimiter handling | Newline-delimited JSON (NDJSON) | MCP spec uses NDJSON, simple and robust |
| Process daemonization | Double-fork, setsid, signal handling manually | `tokio::process::Command` + simple spawn | Modern approach: spawn child process, let parent exit naturally |
| Connection health checks | Custom heartbeat protocol | MCP `ping` method | MCP spec defines standard ping, use it |
| Idle timeout | Manual timer threads | `tokio::time::timeout` or `tokio::select!` with sleep | Integrates with async runtime, cancel-safe |
| PID file management | Custom file locking | Atomic file operations + `std::process::id()` | Simple and sufficient for single-user CLI tool |

## Common Pitfalls

### Pitfall 1: Named Pipe Security Vulnerabilities (XP-02)

**What goes wrong:** Opening named pipes without `security_qos_flags` allows privilege escalation attacks where malicious processes impersonate the client.

**Why it happens:** Default `OpenOptions` doesn't set security QoS flags. Windows named pipes support impersonation levels that must be explicitly configured.

**How to avoid:** 
- **Using `interprocess` crate:** It sets appropriate defaults (`SECURITY_IDENTIFICATION`)
- **Using tokio directly:** Call `.security_qos_flags(SECURITY_IDENTIFICATION)` on `ClientOptions`

**Example with tokio directly:**
```rust
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
use windows_sys::Win32::Security::SECURITY_IDENTIFICATION;

// Server side - create pipe
let server = ServerOptions::new()
    .reject_remote_clients(true)  // Only local connections
    .create(r"\\.\pipe\mcp-cli-daemon")?;

// Client side - connect with security flags
let client = ClientOptions::new()
    .security_qos_flags(SECURITY_IDENTIFICATION)  // Prevent privilege escalation
    .open(r"\\.\pipe\mcp-cli-daemon")?;
```

**Key insight from tokio docs:**
> By default `security_qos_flags` is set to `SECURITY_IDENTIFICATION`... When `security_qos_flags` is not set, a malicious program can gain the elevated privileges of a privileged Rust process when it allows opening user-specified paths, by tricking it into opening a named pipe.

**Confidence:** HIGH (from tokio official documentation)

### Pitfall 2: Orphaned Daemon Processes (CONN-08)

**What goes wrong:** Daemon processes or sockets left behind after crashes prevent new daemon startup with "address already in use" errors.

**Why it happens:** Daemon crashes don't clean up PID files or sockets. On Unix, sockets remain in filesystem. On Windows, named pipe instances may linger.

**How to avoid:**
1. **Cleanup on startup:** Before spawning new daemon, check for and clean up stale resources
2. **PID file tracking:** Store daemon PID in file, check if process still running on startup
3. **Socket path strategy:** Include user ID and timestamp in socket path to avoid collisions

**Implementation:**
```rust
pub async fn cleanup_orphaned_daemon(socket_path: &Path) -> Result<()> {
    // Try to connect - if succeeds, daemon is still running
    if try_connect_ipc(socket_path).await.is_ok() {
        return Ok(());  // Daemon still active
    }
    
    // Clean up stale socket file (Unix)
    if socket_path.exists() {
        fs::remove_file(socket_path).await.ok();
    }
    
    // Check PID file (both platforms)
    let pid_path = socket_path.with_extension("pid");
    if let Ok(pid_str) = fs::read_to_string(&pid_path).await {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // Try to kill orphaned process (platform-specific)
            kill_process_if_exists(pid).await.ok();
        }
    }
    fs::remove_file(&pid_path).await.ok();
    
    Ok(())
}
```

### Pitfall 3: Blocking the Async Runtime

**What goes wrong:** Using synchronous I/O or blocking operations in daemon's async context stalls all concurrent connections.

**Why it happens:** Developers accidentally use `std::fs` or blocking syscalls inside async functions.

**How to avoid:**
- Always use `tokio::fs` for file operations
- Use `tokio::process` for spawning
- For unavoidable blocking operations, use `tokio::task::spawn_blocking`

### Pitfall 4: Stale Connection Reuse (CONN-04 / CONNECTION-04)

**What goes wrong:** Daemon reuses connections that have been closed by the server, resulting in "broken pipe" errors.

**Why it happens:** MCP servers (especially stdio-based) can exit unexpectedly. Daemon must verify connection health before reuse.

**How to avoid:** Implement health checks using MCP `ping` method before returning cached connection:

```rust
async fn health_check(transport: &mut Box<dyn Transport>) -> Result<()> {
    // Send MCP ping request
    let ping = json!({
        "jsonrpc": "2.0",
        "method": "ping",
        "id": "health-check"
    });
    
    match tokio::time::timeout(Duration::from_secs(5), transport.send(ping)).await {
        Ok(Ok(_)) => Ok(()),
        _ => Err(McpError::ConnectionClosed),
    }
}
```

## Code Examples

### IPC Server Setup (Cross-Platform)

```rust
// ipc/mod.rs
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait IpcServer: Send + Sync {
    async fn accept(&mut self) -> Result<(Box<dyn IpcStream>, String)>;
}

#[async_trait]
pub trait IpcStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    async fn shutdown(&mut self) -> Result<()>;
}

// Factory function - only place with platform conditionals
pub async fn create_ipc_server(socket_path: &Path) -> Result<Box<dyn IpcServer>> {
    #[cfg(unix)]
    {
        use unix::UnixIpcServer;
        Ok(Box::new(UnixIpcServer::bind(socket_path).await?))
    }
    
    #[cfg(windows)]
    {
        use windows::NamedPipeIpcServer;
        Ok(Box::new(NamedPipeIpcServer::create(socket_path).await?))
    }
}
```

### Unix Implementation

```rust
// ipc/unix.rs
use tokio::net::{UnixListener, UnixStream};
use std::path::Path;

pub struct UnixIpcServer {
    listener: UnixListener,
}

impl UnixIpcServer {
    pub async fn bind(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Remove stale socket file if exists
        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }
        
        let listener = UnixListener::bind(path)?;
        Ok(Self { listener })
    }
}

#[async_trait]
impl IpcServer for UnixIpcServer {
    async fn accept(&mut self) -> Result<(Box<dyn IpcStream>, String)> {
        let (stream, addr) = self.listener.accept().await?;
        let addr_str = addr.as_pathname()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Ok((Box::new(stream), addr_str))
    }
}
```

### Windows Implementation

```rust
// ipc/windows.rs
use tokio::net::windows::named_pipe::{ServerOptions, NamedPipeServer, NamedPipeClient};
use std::path::Path;

pub struct NamedPipeIpcServer {
    pipe_name: String,
    current_server: Option<NamedPipeServer>,
}

impl NamedPipeIpcServer {
    pub async fn create(socket_path: &Path) -> Result<Self> {
        let pipe_name = format!(r"\\.\pipe\{}", 
            socket_path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| McpError::InvalidSocketPath)?
        );
        
        let server = ServerOptions::new()
            .reject_remote_clients(true)  // Security: local only
            .first_pipe_instance(true)    // Ensure single daemon
            .create(&pipe_name)?;
        
        Ok(Self {
            pipe_name,
            current_server: Some(server),
        })
    }
}

#[async_trait]
impl IpcServer for NamedPipeIpcServer {
    async fn accept(&mut self) -> Result<(Box<dyn IpcStream>, String)> {
        let server = self.current_server.take()
            .ok_or_else(|| McpError::ServerNotListening)?;
        
        // Wait for connection
        server.connect().await?;
        
        // Create next server instance for subsequent connections
        let next_server = ServerOptions::new()
            .reject_remote_clients(true)
            .create(&self.pipe_name)?;
        
        let client_addr = self.pipe_name.clone();
        self.current_server = Some(next_server);
        
        Ok((Box::new(server), client_addr))
    }
}
```

### Daemon-CLI Communication Protocol

```rust
// daemon/protocol.rs
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    Ping,
    GetConfigFingerprint,
    ExecuteTool {
        server_name: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    ListTools {
        server_name: String,
    },
    Shutdown,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    Pong,
    ConfigFingerprint(String),
    ToolResult(serde_json::Value),
    ToolList(Vec<ToolInfo>),
    ShutdownAck,
    Error { code: u32, message: String },
}

// Send/receive helpers
pub async fn send_request<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    request: &DaemonRequest,
) -> Result<()> {
    let json = serde_json::to_string(request)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;  // NDJSON delimiter
    writer.flush().await?;
    Ok(())
}

pub async fn receive_response<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> Result<DaemonResponse> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let response = serde_json::from_str(&line)?;
    Ok(response)
}
```

## State of the Art

| Approach | Status | Recommendation |
|----------|--------|----------------|
| Double-fork daemon | Outdated | Don't use - modern approach is simple child process spawn |
| Unix domain sockets | Standard | Use via `interprocess` or `tokio::net` |
| Windows named pipes | Standard | Use via `interprocess` or `tokio::net::windows::named_pipe` |
| `security_qos_flags` | Required | Always set to `SECURITY_IDENTIFICATION` minimum |
| Signal-based shutdown | Platform-specific | Use `tokio::signal` for cross-platform signal handling |
| PID files | Standard | Simple file-based coordination sufficient for CLI tool |

**Deprecated/outdated:**
- **Systemd socket activation:** Overkill for user-level CLI tool
- **D-Bus:** Too heavy dependency for simple IPC
- **TCP localhost:** Unnecessary network stack overhead, security concerns

## Cross-Platform Compatibility

### Socket/Path Naming

| Platform | Naming Convention | Permissions |
|----------|-------------------|-------------|
| Linux | `/run/user/{uid}/mcp-cli/daemon.sock` or `/tmp/mcp-cli-{uid}/daemon.sock` | User-only (0o600) |
| macOS | `$TMPDIR/mcp-cli/daemon.sock` | User-only |
| Windows | `\\.\pipe\mcp-cli-{username}-daemon` | Default (user-isolated) |

### Signal Handling

| Signal | Unix | Windows | Handling |
|--------|------|---------|----------|
| SIGINT/SIGTERM | Yes | No (Ctrl+C) | Graceful shutdown, close connections |
| SIGHUP | Yes | No | Config reload (optional) |
| Ctrl+C | Yes | Yes | Same as SIGINT |
| Ctrl+Break | No | Yes | Force immediate exit |

### Process Management

| Aspect | Unix | Windows |
|--------|------|---------|
| Orphan detection | PID file + `kill(pid, 0)` | PID file + OpenProcess check |
| Process spawning | `tokio::process::Command` | Same, but MUST use `kill_on_drop(true)` |
| Zombie prevention | Automatic (init reaps) | `kill_on_drop(true)` required |

## Open Questions

1. **Configuration Reload Strategy**
   - What we know: Need to detect config changes (mtime/hash comparison)
   - What's unclear: Should daemon reload config in-place or require restart?
   - Recommendation: Conservative approach - spawn new daemon with fresh config, migrate connections gradually

2. **Multiple Concurrent CLI Instances**
   - What we know: Daemon must handle concurrent connections from multiple CLI processes
   - What's unclear: Should we limit concurrent connections? Implement request queuing?
   - Recommendation: Use `tokio::spawn` per connection, rely on OS backpressure for queuing

3. **Connection Pool Size Limits**
   - What we know: Need to cache connections to MCP servers
   - What's unclear: Should we implement max pool size? LRU eviction?
   - Recommendation: Start with simple HashMap, add LRU if memory becomes concern ( Phase 3+)

## Implementation Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Named pipe security misconfiguration | HIGH (privilege escalation) | Use `interprocess` crate which sets safe defaults; security code review |
| Orphaned daemon processes | MEDIUM (resource leak) | Implement PID file cleanup on startup; idle timeout (60s) |
| Stale connection reuse | MEDIUM (broken pipe errors) | Implement ping health check before connection reuse |
| Platform-specific bugs | MEDIUM (Windows-only issues) | Test on Windows from day one; abstract behind traits |
| Config change not detected | LOW (stale data) | Include config hash in daemon handshake; validate on each CLI invocation |
| Daemon fails to start | MEDIUM (fallback to direct) | Implement fallback: if daemon spawn fails, use direct connections |

## Sources

### Primary (HIGH confidence)
- [tokio named_pipe documentation](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/index.html) - Windows named pipe API, security_qos_flags behavior
- [interprocess crate docs](https://docs.rs/interprocess/latest/interprocess/) - Cross-platform IPC abstraction
- [interprocess local_socket](https://docs.rs/interprocess/latest/interprocess/local_socket/index.html) - Unified IPC API
- [MCP Protocol Specification](https://modelcontextprotocol.io/docs/concepts/transports/) - IPC requirements, JSON-RPC format

### Secondary (MEDIUM confidence)
- [tokio UnixListener](https://docs.rs/tokio/latest/tokio/net/struct.UnixListener.html) - Unix socket implementation
- [tokio::time::timeout](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html) - Idle timeout implementation
- [tokio::signal](https://docs.rs/tokio/latest/tokio/signal/index.html) - Cross-platform signal handling
- Project PITFALLS.md - Documented Windows process spawning and security issues

### Tertiary (LOW confidence)
- General Rust async patterns for daemon implementation
- Community best practices for connection pooling

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - interprocess is widely used, tokio is standard
- Architecture patterns: HIGH - standard async Rust patterns
- Pitfalls: HIGH - documented in tokio docs and project PITFALLS.md
- Windows security: HIGH - official tokio documentation clear on security_qos_flags

**Research date:** 2025-02-06
**Valid until:** 2025-05-06 (90 days for stable APIs)

---

**Next Steps for Planner:**
1. Create IPC abstraction module with trait definitions
2. Implement Unix socket backend using tokio::net::UnixListener
3. Implement Windows named pipe backend using tokio::net::windows::named_pipe
4. Create daemon binary entry point with idle timeout logic
5. Implement CLI-to-daemon communication protocol (JSON-RPC over IPC)
6. Add connection pooling with health checks in daemon
7. Implement config fingerprinting and change detection
8. Add orphan cleanup logic on CLI startup
9. Write integration tests for both platforms
