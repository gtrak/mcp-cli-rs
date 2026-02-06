# Domain Pitfalls

**Domain:** MCP CLI Tools (Rust implementation)
**Researched:** 2025-02-06
**Confidence:** HIGH (based on official docs and known issues from Bun implementation)

## Critical Pitfalls

### Pitfall 1: Windows Process Spawning Without kill_on_drop

**What goes wrong:**
Child processes spawned in Windows continue running in the background even after the parent process exits, leaving zombie processes and resource leaks. This was the primary issue in the original Bun implementation that motivated the Rust rewrite.

**Why it happens:**
By default, Rust's `tokio::process::Command` does not automatically kill child processes when the `Child` handle is dropped. On Windows, unlike Unix, orphaned processes don't automatically become child of init. This behavior differs expectations from Unix platforms where SIGTERM handling is more straightforward.

**Consequences:**
- Zombie processes accumulate, consuming system resources
- Named pipe handles remain open, preventing connection reuse
- File descriptors leak, eventually hitting OS limits
- Users cannot cleanly restart the daemon (pipe busy errors)
- Tests fail with "address already in use" or other process conflicts

**Prevention:**
Always set `.kill_on_drop(true)` when spawning child processes for stdio transport. This ensures child processes are terminated when the `Child` handle is dropped.

```rust
use tokio::process::Command;
use std::process::Stdio;

let mut child = Command::new("node")
    .arg("server.js")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)  // CRITICAL on Windows
    .spawn()?;
```

**Warning signs:**
- "Address already in use" errors when starting the daemon
- `ls` or `tasklist` shows multiple instances of same process
- Connection timeouts after daemon restarts
- Tests pass individually but fail in batches
- `lsof` (Unix) or `netstat` (Windows) shows unclosed connections

**Phase to address:**
Phase 1 (Core Transport Implementation) - Process spawning must be tested on Windows first before adding complexity.

---

### Pitfall 2: Named Pipe Security QoS Not Configured

**What goes wrong:**
Opening named pipes on Windows without proper security flags allows privilege escalation vulnerabilities. Malicious programs can act on behalf of privileged processes by tricking them into opening user-specified named pipes.

**Why it happens:**
Default `OpenOptions` in Rust do not set `security_qos_flags`. When opening `\\.\pipe\` paths without these flags, the client process may impersonate with higher privileges than intended. Security-aware code requires explicit configuration.

**Consequences:**
- Privilege escalation vulnerabilities (critical security issue)
- Malicious processes can gain elevated privileges
- Cross-process authorization failures in constrained environments
- Security audits fail on Windows

**Prevention:**
Always call `.security_qos_flags()` when opening named pipes on Windows. Use appropriate security levels:
- `SECURITY_IDENTIFICATION` (0x00010000): Server can identify the client
- `SECURITY_IMPERSONATION` (0x00020000): Server can impersonate the client
- `SECURITY_DELEGATION` (0x00030000): Server can delegate to other services

```rust
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::fs::OpenOptions;

let pipe = OpenOptions::new()
    .write(true)
    .read(true)
    .security_qos_flags(winapi::SECURITY_IDENTIFICATION)  // CRITICAL
    .open(r"\\.\pipe\mcp-cli-daemon")?;
```

**Warning signs:**
- Static analysis tools flag "insecure named pipe opening"
- Security reviewers flag privilege escalation concerns
- Process Explorer shows unexpected impersonation tokens
- Access denied errors when daemon runs elevated

**Phase to address:**
Phase 2 (Daemon Implementation) - Named pipe opening must be security-reviewed and tested.

---

### Pitfall 3: Stdio Transport Missing newline Delimiter

**What goes wrong:**
JSON-RPC messages sent to MCP servers via stdio transport contain embedded newlines, breaking protocol compliance. Servers receive malformed messages or parse incomplete data.

**Why it happens:**
MCP specification requires messages to be delimited by newlines and MUST NOT contain embedded newlines. Developers may write JSON directly to stdout without ensuring newline delimiters or properly formatting multi-line JSON.

**Consequences:**
- Servers fail to parse requests (malformed JSON error)
- Servers hang waiting for message completion
- Protocol mismatch with spec compliance
- Unclear error messages (EOF in middle of message)
- Intermittent failures depending on JSON structure

**Prevention:**
Always delimit JSON messages with newlines and ensure no embedded newlines in JSON strings. Use serde_json's pretty printing carefully:

```rust
use serde_json::to_string;

let message = to_string(&json_request)?;  // No newlines
writeln!(stdin, "{}", message)?;  // Add newline delimiter

// WRONG - pretty_print adds embedded newlines
writeln!(stdin, "{}", to_string_pretty(&json_request)?)?;
```

**Warning signs:**
- MCP servers report "malformed JSON" errors
- Protocol violations in server logs
- Messages hang on simple commands
- Works on some tools but fails on others (depends on JSON content)

**Phase to address:**
Phase 1 (Core Transport Implementation) - Test against multiple MCP servers to validate stdio transport.

---

### Pitfall 4: Connection Pool Not Detecting Stale Connections

**What goes wrong:**
Daemon connection pool reuses closed or terminated server connections, leading to broken pipe errors. Client requests fail silently or with confusing errors.

**Why it happens:**
Daemon caches connections but doesn't check if they're still alive. MCP servers (especially stdio-based) can exit unexpectedly (timeout, crash, user interrupt). Pool reuses without verifying connection state.

**Consequences:**
- "Broken pipe" errors in middle of operations
- Requests hang indefinitely (timeout)
- Silent failures when daemon thinks it has active connection
- Users must restart daemon manually to recover
- Connection pools fill with dead connections

**Prevention:**
Implement connection liveness checks before reuse. For stdio transport, send a lightweight ping or check child process status:

```rust
// Before reusing connection
child.try_wait()?.is_some().then(|| {
    // Process already exited, connection is dead
    return Err(McpError::ConnectionClosed);
});

// Or send ping protocol message
let ping_result = send_ping(&mut transport).await;
if ping_result.is_err() {
    pool.remove(&server_name);
    // Reconnect...
}
```

**Warning signs:**
- "Broken pipe" or "EOF" errors
- Works initially, fails after inactivity
- Must restart daemon to fix issues
- Connection errors don't resolve by retrying

**Phase to address:**
Phase 2 (Daemon Implementation) - Connection pooling must include health checks.

---

### Pitfall 5: Environment Variable Substitution Injection

**What goes wrong:**
Configuration shell command parsing with unescaped variable values leads to command injection. Malicious environment variable values execute arbitrary commands.

**Why it happens:**
Using string concatenation or shell string interpolation to build command lines from config. Malicious environment variables containing spaces, quotes, or shell metacharacters change command meaning.

**Consequences:**
- Command injection (security vulnerability - CRITICAL)
- Unauthorized code execution
- Environment variables breaking command parsing
- Configuration appears valid but executes wrong commands

**Prevention:**
Always parse command strings using `shell-words::split()` and substitute variables atomically. Never use shell string interpolation:

```rust
use shell_words::split;
use std::env;

// WRONG - vulnerable to injection
let command = format!("{} {}", program, args_with_var);

// RIGHT - parse then substitute
let parts = split(&config.command)?;
let mut command = Command::new(&parts[0]);
command.args(&parts[1..]);

// Substitute environment variables in individual arguments
for arg in command_config.args.iter_mut() {
    if let Some(var_name) = arg.strip_prefix("${").and_then(|s| s.strip_suffix("}")) {
        *arg = env::var(var_name).unwrap_or_else(|_| arg.clone());
    }
}
```

**Warning signs:**
- Commands execute incorrectly when environment variables contain spaces
- Variable substitution behaves unexpectedly
- Shell metacharacters break commands
- Security review flags string interpolation in command building

**Phase to address:**
Phase 1 (Configuration Module) - All command parsing must use shell-words.

---

## Moderate Pitfalls

### Pitfall 6: Blocking I/O in Async Context

**What goes wrong:**
Using `std::fs` or `std::process` inside async functions blocks the tokio executor, preventing concurrent operations.

**Why it happens:**
Developers copy code from sync Rust to async code. `std::fs::read_to_file` and `std::process::Command` are synchronous operations that block the thread.

**Consequences:**
- All async operations slow down when any one blocks
- Concurrent tool calls execute serially instead of in parallel
- Timeout handling doesn't work (executor can't schedule tasks)
- User experience degrades unexpectedly

**Prevention:**
Use tokio's async equivalents everywhere in async code:
- `tokio::fs::read_to_file` instead of `std::fs::read_to_file`
- `tokio::process::Command` instead of `std::process::Command`
- `spawn_blocking` for CPU-bound work that can't be async

```rust
// WRONG - blocks executor
let config = std::fs::read_to_string("config.json")?;

// RIGHT - async
let config = tokio::fs::read_to_string("config.json").await?;

// For blocking CPU work
let result = tokio::task::spawn_blocking(move || {
    heavy_computation(data)
}).await??;
```

**Warning signs:**
- Operations are slower than expected
- Logging shows gaps between async operations
- Timeouts don't trigger (executor stuck)

**Phase to address:**
Phase 1 (Architecture) - Enforce async patterns via code review and lints.

---

### Pitfall 7: Error Context Not Propagated

**What goes wrong:**
Errors bubble up without context (which server? which tool? which operation?), making debugging difficult for users.

**Why it happens:**
Using `?` operator directly on `std::io::Error` or `serde_json::Error` without mapping to domain-specific errors. Generic error messages don't help users diagnose issues.

**Consequences:**
- Users can't tell what went wrong
- Bug reports lack information
- AI agents can't provide helpful suggestions
- Support burden increases

**Prevention:**
Use `thiserror` for domain errors with context. Wrap external errors with location-specific information:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("Failed to connect to server '{name}': {source}")]
    ConnectionError {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Config file '{path}' is invalid: {source}")]
    InvalidConfig {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Tool '{tool}' not found in server '{server}'")]
    ToolNotFound { tool: String, server: String },
}

// Use in code
let conn = connect(&server_name).map_err(|e| McpError::ConnectionError {
    name: server_name,
    source: e
})?;
```

**Warning signs:**
- Error messages are generic ("I/O error")
- Users report "it just broke"
- Logs show `std::io::Error` without context
- Requires source code dive to understand failure

**Phase to address:**
Phase 1 (Error Module) - Define error types early, enforce in code review.

---

### Pitfall 8: Platform-Specific Code in Core Logic

**What goes wrong:**
Platform conditionals (`#[cfg(windows)]`, `#[cfg(unix)]`) scattered throughout codebase make testing and maintenance difficult.

**Why it happens:**
Developers handle platform differences inline when they encounter them, instead of abstracting behind traits.

**Consequences:**
- Code is tested on only one platform
- Bugs appear on different platforms
- Code review requires platform expertise
- Abstractions leak platform details

**Prevention:**
Abstract platform differences behind traits, keep platform-specific code in isolated modules:

```rust
// Cross-platform trait
#[async_trait]
pub trait IpcTransport: Send + Sync {
    async fn connect(&self) -> Result<Box<dyn IpcStream>>;
    async fn listen(&self) -> Result<Box<dyn IpcListener>>;
}

// Unix implementation (unix/mod.rs)
#[cfg(unix)]
pub struct UnixIpcTransport {
    path: PathBuf,
}

// Windows implementation (windows/mod.rs)
#[cfg(windows)]
pub struct NamedPipeTransport {
    pipe_name: String,
}

// Core code uses trait only
let ipc: Box<dyn IpcTransport> = #[cfg(unix)] {
    Box::new(UnixIpcTransport::new(&path))
} else {
    Box::new(NamedPipeTransport::new(&pipe_name))
};
```

**Warning signs:**
- Multiple `#[cfg]` attributes in single function
- Business mixed with platform details
- Some tests compile only on specific platforms
- Can't run full test suite cross-platform

**Phase to address:**
Phase 2 (Daemon/IPC) - Platform abstraction must be designed before implementation.

---

### Pitfall 9: Global Mutable State

**What goes wrong:**
Using `lazy_static!` or `once_cell` to share state across invocations leads to test failures and surprising behavior.

**Why it happens:**
Developers want to share cache or configuration globally for convenience, not realizing async safety implications.

**Consequences:**
- Tests interfere with each other
- Daemon state leaks between tests
- Race conditions (data races in async context)
- Hard to reproduce bugs

**Prevention:**
Pass state explicitly via `AppContext` struct, use `Arc<Mutex<T>>` only where absolutely necessary:

```rust
// WRONG - global state
lazy_static! {
    static ref CONFIG: Mutex<Option<Config>> = Mutex::new(None);
}

// RIGHT - explicit context
pub struct AppContext {
    pub config: Arc<Config>,
    pub pool: Arc<ConnectionPool>,
}

async fn execute_command(ctx: &AppContext, args: &Args) -> Result<Output> {
    // Use ctx.config, ctx.pool
}
```

**Warning signs:**
- Tests pass in isolation but fail when run together
- Test order matters
- Data race warnings from Rust compiler or tests
- Behavior differs on first run vs subsequent runs

**Phase to address:**
Phase 1 (Architecture) - Define context struct pattern, ban global mutable state.

---

### Pitfall 10: MCP Protocol Version Mismatch

**What goes wrong:**
Client assumes server protocol version without negotiation, leading to incompatible communication.

**Why it happens:**
Developers implement against spec version they read, neglecting version negotiation and backwards compatibility requirements.

**Consequences:**
- Messages rejected by servers
- Feature failures on older/newer servers
- Protocol errors without clear explanations
- Hard to diagnose compatibility issues

**Prevention:**
Always negotiate protocol version during initialization, respect negotiated version, include version header for HTTP:

```rust
// Initialization handshake
let init_response = send_initialize(&mut transport, "2025-06-18").await?;

// Use negotiated version for subsequent messages
let server_version = init_response.protocol_version;

// For HTTP transport, include version header
reqwest::Client::new()
    .post(url)
    .header("MCP-Protocol-Version", negotiated_version)
    .json(&request)
    .send()
    .await?;
```

**Warning signs:**
- Server rejects messages with version errors
- Works with official inspector but not custom servers
- Different behavior across MCP server deployments
- Protocol negotiation section skipped in flow

**Phase to address:**
Phase 1 (MCP Client) - Version negotiation is part of core protocol implementation.

---

## Minor Pitfalls

### Pitfall 11: Shell Autodetect for Stdin Without Fallback

**What goes wrong:**
Auto-detecting stdin input fails when not connected to TTY (e.g., in CI environments or file redirects), breaking pipeline compatibility.

**Why it happens:**
Relying solely on `atty::isnt(atty::Stream::Stdin)` for stdin detection. CI environments often aren't TTYs but also don't have pipe input.

**Consequences:**
- Commands hangs waiting for input in CI
- Pipeline data ignored unexpectedly
- Requires explicit `-f-` flag unnecessarily

**Prevention:**
Check for actual data on stdin, not just TTY presence:

```rust
use std::io::{self, Read};

fn has_stdin_input() -> bool {
    // Check if stdin has data available
    match io::stdin().bytes().next() {
        Some(Ok(_)) => true,
        _ => false,
    }
}

// In CLI
let json_arg = if has_stdin_input() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    buf
} else {
    args.json.unwrap_or_default()
};
```

**Warning signs:**
- Command appears to hang
- CI tests time out on input detection
- Workarounds in shell scripts (`yes | cmd`)

**Phase to address:**
Phase 3 (CLI Refinement) - Improves UX, not blocking for MVP.

---

### Pitfall 12: Exit Codes Not Differentiated

**What goes wrong:**
All failures return exit code 1, preventing scripts from distinguishing error types (config error vs server error vs invalid args).

**Why it happens:**
Using `std::process::exit(1)` for all errors or using `?` operator that maps all errors to nonzero exit.

**Consequences:**
- Shell scripts can't react intelligently
- Can't implement retry logic based on error type
- Monitoring/alerting can't distinguish severity

**Prevention:**
Define meaningful exit codes using `ExitCode` enum, match error types:

```rust
#[derive(Debug, Copy, Clone)]
pub enum ExitCode {
    Success = 0,
    InvalidArgs = 2,
    ConfigError = 3,
    ServerNotFound = 4,
    ToolError = 5,
    NetworkError = 6,
    InternalError = 1,
}

// In main
let result = run(args);
match result {
    Ok(_) => Ok(ExitCode::Success),
    Err(McpError::InvalidArgs(_)) => Ok(ExitCode::InvalidArgs),
    Err(McpError::ConfigError { .. }) => Ok(ExitCode::ConfigError),
    Err(McpError::ConnectionError { .. }) => Ok(ExitCode::NetworkError),
    Err(_) => Ok(ExitCode::InternalError),
}.map(|code| code as i32)?;
```

**Warning signs:**
- Shell scripts always report same error code
- Error scripts `while ! cmd; do :; done` retry wrong failures
- Can't implement conditional behavior

**Phase to address:**
Phase 1 (CLI Foundation) - Exit code scheme should be established early.

---

## Testing Gaps

### Gap 1: Windows Integration Testing Missing

**Problem:** Most tests run only on Linux/macOS, Windows-specific issues caught late.

**Impact:** Critical bugs (process spawning, named pipes) discovered after release.

**Prevention:**
- Test on Windows from day one (GitHub Actions or local Windows)
- Mock Windows APIs for Linux tests
- Add Windows-specific test cases for named pipes
- Use `#[cfg(windows)]` test modules

**Phase:** Phase 1 (Core Transport) - Mandatory Windows testing before merging.

---

### Gap 2: Concurrent Access Not Tested

**Problem:** Tests run sequentially, race conditions in concurrent tool execution go unnoticed.

**Impact:** Production failures when users call tools in parallel.

**Prevention:**
- Add tests that call multiple tools concurrently
- Use `futures::join_all` to test parallel execution
- Test daemon connection pool under load
- Test signal handling during active requests

**Phase:** Phase 2 (Daemon/Performance) - Concurrent tests reveal race conditions.

---

### Gap 3: Protocol Compliance Not Validated

**Problem:** Tests only check that code runs, not that MCP protocol messages are correct.

**Impact:** Non-compliant messages work with custom servers but fail with spec-compliant ones.

**Prevention:**
- Capture and validate JSON-RPC message format in tests
- Test against official MCP TypeScript SDK servers
- Validate message delimiters and encoding
- Test initialization handshake sequence

**Phase:** Phase 1 (Core Protocol) - Protocol validation prevents rework.

---

## "Looks Done But Isn't" Checklist

- [ ] **Windows process spawning:** Often missing kill_on_drop configuration — verify zombie process cleanup
- [ ] **Named pipe security:** Often missing security_qos_flags — verify security review
- [ ] **Stdio transport:** Often missing newline delimiters — verify with MCP servers
- [ ] **Connection reuse:** Often missing health checks — verify stale connection detection
- [ ] **Error context:** Often missing domain-specific error types — verify user-friendly messages
- [ ] **Environment substitution:** Often using shell interpolation — verify shell-words usage
- [ ] **Global state:** Often using lazy_static — verify explicit context passing
- [ ] **Blocking I/O in async:** Often using std::fs — verify tokio::fs usage
- [ ] **Platform conditionals:** Often scattered in core logic — verify trait abstraction
- [ ] **Protocol version:** Often hardcoded — verify version negotiation

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| **Windows zombie processes** | MEDIUM | 1. Kill all child processes manually (`taskkill /F /IM node.exe`)<br>2. Add `kill_on_drop(true)` to all spawns<br>3. Add process cleanup tests |
| **Named pipe security** | HIGH | 1. Audit all named pipe openings<br>2. Add `security_qos_flags` to all OpenOptions<br>3. Run security scanner<br>4. May need privilege audit if exploited |
| **Stdio newline delimiters** | LOW | 1. Verify message formatting with several MCP servers<br>2. Add writeln! for all messages<br>3. Remove pretty printing from JSON output |
| **Stale connections** | MEDIUM | 1. Clear daemon connections (restart daemon)<br>2. Add connection health checks<br>3. Implement connection timeout/reconnection logic |
| **Blocking I/O in async** | LOW | 1. Add tokio-taskblocking linter<br>2. Replace std::fs with tokio::fs<br>3. Replace std::process with tokio::process<br>4. Add async task monitoring to detect blocking |
| **Error context missing** | MEDIUM | 1. Add thiserror-derived error types<br>2. Map all external errors to domain errors<br>3. Add error context tests |
| **Global state** | MEDIUM | 1. Refactor to explicit AppContext<br>2. Wrap state in Arc<Mutex<T>> where needed<br>3. Add tests for concurrent access |
| **Platform conditionals** | HIGH | 1. Extract platform code to separate modules<br>2. Define trait interfaces<br>3. Implement per-platform<br>4. Test on all target platforms |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Windows process spawning (kill_on_drop) | Phase 1 (Core Transport) | Integration tests on Windows: spawn multiple processes, verify cleanup |
| Named pipe security (QoS) | Phase 2 (Daemon/IPC) | Security review test: verify OpenOptionsExt usage |
| Stdio transport (newlines) | Phase 1 (Transport) | Protocol compliance test: validate message format with real MCP servers |
| Stale connections | Phase 2 (Daemon Pooling) | Connection lifecycle test: kill server, verify error on reuse |
| Environment variable injection | Phase 1 (Config/Env) | Security test: craft malicious env var, verify safe parsing |
| Blocking I/O in async | Phase 1 (Architecture) | Tokio-console or linter: check for async blocking calls |
| Error context | Phase 1 (Error Module) | Error message test: trigger errors, verify helpful messages |
| Platform conditionals | Phase 2 (IPC Abstraction) | Cross-platform tests: verify tests compile and pass on all targets |
| Global mutable state | Phase 1 (Architecture) | Concurrency test: run in parallel, verify no interference |
| MCP version negotiation | Phase 1 (Protocol) | Protocol test: connect to multiple MCP versions, verify handshake |
| Stdin autodetect issues | Phase 3 (CLI Refinement) | Pipeline test: pipe data, verify auto-detection works |
| Exit code differentiation | Phase 1 (CLI Foundation) | Exit code test: trigger different errors, verify different codes |
| Windows integration testing | Phase 1 (All phases) | CI test: verify all tests pass on Windows runner |
| Concurrent access testing | Phase 2 (Daemon/Performance) | Load test: spawn 10 concurrent tool calls, verify no races |
| Protocol compliance | Phase 1 (Core Protocol) | Compliance test: validate JSON-RPC message structure in tests |

---

## Sources

### HIGH Confidence (official documentation)

- [tokio::process::Command documentation](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) - Windows process spawning, kill_on_drop behavior
- [std::os::windows::fs::OpenOptionsExt](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html) - Named pipe security flags
- [MCP Protocol - Transports](https://modelcontextprotocol.io/docs/concepts/transports/) - stdio/HTTP transport requirements, message delimiters
- [shell-words crate documentation](https://docs.rs/shell-words/latest/shell_words/) - Shell command parsing, avoiding injection

### MEDIUM Confidence (known issues from Bun implementation)

- Original mcp-cli Bun repository - Documented Windows process spawning issues (motivated Rust rewrite)
- Common Rust async pitfalls - Blocking I/O in async executor, global mutable state issues

### LOW Confidence (general community knowledge)

- Rust CLI best practices - Error context patterns, exit code conventions
- Cross-platform development challenges - Platform conditionals, testing gaps

---

*Pitfalls research for: MCP CLI Rust Rewrite*
*Researched: 2025-02-06*
