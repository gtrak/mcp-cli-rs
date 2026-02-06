# Technology Stack

**Project:** MCP CLI Rust Rewrite
**Researched:** 2025-02-06
**Confidence:** HIGH for standard Rust ecosystem, MEDIUM for mcp-sdk (early version)

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **clap** | 4.5.57 | CLI argument parsing | De facto standard for Rust CLIs. Provides derive macro for declarative interface, automatic help generation, subcommands, shell completions. Highly maintained by rust-cli team. |
| **tokio** | 1.49.0 | Async runtime | The standard async runtime for Rust. Use `features = ["full"]` for complete functionality (net, process, fs, signal, time). Provides cross-platform async process spawning that fixes Windows issues from Bun. |

### MCP Protocol

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **mcp-sdk** | 0.0.3 | MCP protocol implementation | Third-party Rust SDK implementing Model Context Protocol. Provides `ClientStdioTransport` (spawns child process and communicates via stdio) and `ClientBuilder`. **LOW maturity (0.0.3)** - evaluate carefully, may need to fork if it doesn't meet requirements. |
| **serde** | 1.0.228 | Serialization framework | Standard serialization framework. Use `features = ["derive"]` for compile-time code generation. Required by mcp-sdk. |
| **serde_json** | 1.0.149 | JSON serialization/deserialization | De facto standard for JSON. Used by mcp-sdk for protocol messages, and for parsing `mcp_servers.json` configuration. |

### Network & Transport

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **reqwest** | 0.13.1 | HTTP client | Higher-level async HTTP client. Use with tokio. Enable `json` feature for MCP HTTP transport. Handles TLS by default, proxies, cookies. Standard choice for async HTTP. |
| **tokio::net** | 1.49.0 (via tokio) | Unix sockets | For daemon IPC on Linux/macOS. Use `tokio::net::UnixStream` for Unix domain socket communication. Built into tokio (requires `net` feature, included in `full`). |
| **named_pipe** | (stdlib) | Windows named pipes | Use `std::os::windows::fs::OpenOptionsExt` for Windows named pipes. No third-party crate needed - Windows APIs are in stdlib. |

### Error Handling

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **anyhow** | 1.0.101 | Application error handling | Flexible, context-aware errors for application code. Use `anyhow::Result<T>` alias instead of `Result<T, anyhow::Error>`. Perfect for `main()` and command handlers where errors need context strings. |
| **thiserror** | 2.0.18 | Library error handling | Derive macro for `std::error::Error`. Use for error types that are part of your public API or that need to be handled differently than other errors. Works well with anyhow - use thiserror for library errors, anyhow for application errors. |

### Logging & Instrumentation

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **tracing** | 0.1.44 | Structured logging/tracing | Modern instrumentation framework (successor to `log`). Used by mcp-sdk. Provides spans, structured data, better async support. Use for all logging. |
| **tracing-subscriber** | 0.3.22 | Log registry/filter | Global registry for collecting and filtering tracing events. Configure with `fmt()` layer for human-readable output, `json()` for machine-parsable logs. Required for any logging output. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| tokio-test | Async testing utilities | Testing futures, mocking I/O. Use `#[tokio::test]` macro for async tests (requires tokio test features). |
| shell-words | Shell argument parsing | Parse command strings according to Unix shell rules. Use for parsing `command` field from mcp_servers.json that may contain quoted arguments. |
| assert_cmd | CLI testing | Integration testing for CLIs. Test parsed args, program output, error messages. |
| tempfile | Test file management | Creates temporary files/directories for testing, auto-cleanup. |

### Environment & Configuration

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| std::env | Environment variable access | Use `std::env::var()` and `std::env::var_os()` for reading environment variables. No crate needed - standard library is sufficient. |
| serde_json | Configuration parsing | Parse `mcp_servers.json` format. The configuration format is JSON with nested structure - serde_json handles this natively. |

## Installation

```toml
[dependencies]
# Core framework
clap = { version = "4.5.57", features = ["derive"] }
tokio = { version = "1.49.0", features = ["full"] }

# MCP protocol
mcp-sdk = "0.0.3"
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

[dev-dependencies]
tokio-test = "0.4.5"
assert_cmd = "2.0"
tempfile = "3.13"
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| **CLI parsing** | clap with derive | structopt | DEPRECATED - structopt is now part of clap. clap 4.x is the official replacement. |
| **CLI parsing** | clap with derive | clap (builder API) | Both work, but derive macros are more maintainable and readable for most CLIs. Use builder only for dynamic CLIs. |
| **HTTP client** | reqwest | hyper | Hyper is lower-level and more complex. reqwest is a wrapper around hyper that's easier to use. Only use hyper if reqwest doesn't support your use case. |
| **HTTP client** | reqwest | surf / isahc | Less mature, smaller ecosystem. reqwest is the de facto standard with better tokio integration. |
| **Async runtime** | tokio | async-std | Tokio has larger ecosystem, better documentation, more widely adopted. async-std is viable but less common. |
| **Logging** | tracing | log crate | `log` is older and less powerful. `tracing` provides spans, better async support, and is compatible with `log` via tracing-log for compatibility layers. |
| **Error handling** | anyhow + thiserror | error-chain | DEPRECATED - error-chain is unmaintained. anyhow is the modern replacement. |
| **Error handling** | anyhow + thiserror | eyre | eyre is an alternative to anyhow, but anyhow is more widely used with better documentation and ecosystem. |
| **Configuration** | serde_json | toml | Config format must be JSON to match Claude Desktop/Gemini/VS Code standard. Use toml only for project-specific config, not mcp_servers.json. |
| **Process spawning** | tokio::process | std::process | Tokio provides async versions which integrate with the async runtime. Use std::process only in contexts where you must block. |
| **Unix sockets** | tokio::net::UnixStream | std::os::unix::net::UnixStream | Same as process spawning - async version integrates with tokio runtime. Use std only in sync contexts. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **mcp-sdk without evaluation** | Version 0.0.3 is pre-alpha, only 827 downloads, minimal documentation (11.36% documented on docs.rs). May not support all protocol features or handle edge cases. | **Evaluate mcp-sdk thoroughly**: Check if it supports your required features, handles Windows correctly, has proper error handling. Consider reimplementing protocol using tokio + serde_json if mcp-sdk falls short. |
| **structopt** | Deprecated since 2023. Maintainers recommend using clap 4.x with derive macros instead. | **clap** with `derive` feature (replacement for structopt). |
| **log crate** | Older framework, less powerful for async code, no span support. | **tracing** - modern instrumentation framework with spans and better async support. |
| **error-chain** | Unmaintained (last updated 2018). DEPRECATED. | **anyhow** for application errors, **thiserror** for library errors. |
| **hyper directly** | Lower-level, requires more boilerplate. reqwest is the recommended abstraction. | **reqwest** - higher-level async HTTP client. |
| **blocking I/O in async code** | Blocking the executor thread will prevent other tasks from running. | Tokio's async APIs (tokio::process, tokio::fs, tokio::net) or `spawn_blocking` for CPU-bound work. |
| **std::process in async context** | std::process is synchronous. Use tokio::process for async child process spawning. | **tokio::process::Command** - async version of Command. |
| **shell commands with exec** | Shell injection risk, cross-platform issues. | Use shell-words crate for parsing, tokio::process::Command for async spawning, std::process::Command for sync. |
| **global mutable state** | Makes testing impossible, breaks async safety, causes race conditions. | Use tokio::sync channels (mpsc, watch, broadcast, oneshot) for task communication. |

## Stack Patterns by Variant

**If building for Windows compatibility:**
- Use **tokio::process** for child process spawning - handles Windows specifics correctly
- mcp-sdk's **ClientStdioTransport** already uses tokio::process internally
- Windows named pipes via `std::os::windows::fs::OpenOptionsExt` - no third-party crate needed
- Test thoroughly on Windows - original Bun implementation had issues here

**If building for Linux/macOS daemon:**
- Use **tokio::net::UnixStream** for Unix domain sockets
- Unix-specific logic via `#[cfg(unix)]` attributes
- Signal handling via `tokio::signal` (SIGTERM, SIGINT, SIGHUP)

**If building cross-platform daemon:**
- **Conditional compilation**: `#[cfg(unix)]` for Unix sockets, `#[cfg(windows)]` for named pipes
- **Abstract transport trait**: Create wrapper around platform-specific IPC
- **Platform-specific tests**: Use `#[cfg(target_os = "linux")]` etc.

**If using mcp-sdk:**
- **Caution**: Version 0.0.3 is early. Test thoroughly:
  - stdio transport reliability (especially on Windows)
  - Error messages quality
  - Protocol compliance
  - JSON schema handling
- **Consider forking**: If mcp-sdk is close but needs fixes, fork and improve
- **Consider reimplementing**: mcp-sdk is just JSON-RPC over transport. Using tokio + serde_json directly may be more maintainable if mcp-sdk doesn't fit your needs

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| tokio 1.49.0 | reqwest 0.13.1 | reqwest depends on tokio ^1.0 |
| tokio 1.49.0 | tokio-test 0.4.5 | tokio-test depends on tokio ^1.2.0 |
| tokio 1.49.0 | mcp-sdk 0.0.3 | mcp-sdk depends on tokio ^1.0 with `full` features |
| serde 1.0.228 | serde_json 1.0.149 | Compatible - serde_json follows semver |
| anyhow 1.0.101 | thiserror 2.0.18 | Compatible - they serve different purposes |
| tracing 0.1.44 | tracing-subscriber 0.3.22 | tracing-subscriber 0.3.x works with tracing 0.1.x |

## Windows Process Spawning - Critical Guidance

**Problem from original Bun implementation:**
Windows process spawning and stdio handling were problematic, causing the Rust rewrite.

**Solution (via stack choices):**

1. **Use tokio::process::Command** - Async child process spawning
   - Handles Windows-specific process creation correctly
   - Provides async I/O with child stdin/stdout/stderr
   - Cross-platform API (same on Unix and Windows)

2. **Or use mcp-sdk's ClientStdioTransport**
   - Wrapper around tokio::process
   - Handles JSON-RPC over stdio automatically
   - **Evaluate on Windows first** - this is where the issues were

3. **Testing on Windows is mandatory**
   - Test spawn/wait/kill lifecycle
   - Test stdin/stdout/stderr piping
   - Test process cleanup (no zombie processes)
   - Test signal handling (SIGTERM equivalent)

4. **Key tokio::process methods:**
   ```rust
   use tokio::process::Command;

   // Spawn child with piped stdio
   let mut child = Command::new("node")
       .arg("server.js")
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped())
       .kill_on_drop(true)  // Important: kill on Windows
       .spawn()?;

   // Read/write async streams
   let mut stdin = child.stdin.take().unwrap();
   let mut stdout = child.stdout.take().unwrap();

   // Spawn on separate thread or use tokio::io::copy
   ```

5. **Windows-specific concerns:**
   - Process handle management: Use `kill_on_drop(true)` to prevent zombies
   - Console creation: Some options spawn new console windows - test various configurations
   - Signal handling: Windows doesn't have Unix signals, use `SetConsoleCtrlHandler` or `job objects` (complex)
   - Working directory: Windows paths with spaces, backslashes as separators

## Sources

### HIGH Confidence (official documentation)

- [clap 4.5.57 documentation](https://docs.rs/clap/4.5.57/clap/) - Official docs.rs
- [tokio 1.49.0 documentation](https://docs.rs/tokio/1.49.0/tokio/) - Official docs.rs
- [reqwest 0.13.1 documentation](https://docs.rs/reqwest/0.13.1/reqwest/) - Official docs.rs
- [serde 1.0.228 documentation](https://docs.rs/serde/1.0.228/serde/) - Official docs.rs
- [thiserror 2.0.18 documentation](https://docs.rs/thiserror/2.0.18/thiserror/) - Official docs.rs
- [anyhow 1.0.101 documentation](https://docs.rs/anyhow/1.0.101/anyhow/) - Official docs.rs
- [tracing 0.1.44 documentation](https://docs.rs/tracing/0.1.44/tracing/) - Official docs.rs
- [tokio-test 0.4.5 documentation](https://docs.rs/tokio-test/0.4.5/tokio_test/) - Official docs.rs
- [Model Context Protocol TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) - Official reference implementation (for protocol understanding)
- [crates.io API](https://crates.io/) - For verifying latest versions

### MEDIUM Confidence (third-party SDK with evaluation needed)

- [mcp-sdk 0.0.3 documentation](https://docs.rs/mcp-sdk/0.0.3/mcp_sdk/) - Low version (0.0.3), limited documentation (11.36%), but provides needed functionality
- [mcp-sdk on crates.io](https://crates.io/crates/mcp-sdk) - Active contributor (last updated 2025-01-20), 827 downloads

### HIGH Confidence (standard library)

- [std::env documentation](https://doc.rust-lang.org/std/env/) - Official Rust stdlib
- [shell-words 1.1.1 documentation](https://docs.rs/shell-words/1.1.1/shell_words/) - Official docs.rs for shell parsing

### Version Verification

All version numbers verified against:
- crates.io API (2025-02-06)
- docs.rs latest documentation (2025-02-06)

---

*Stack research for: MCP CLI Rust Rewrite*
*Researched: 2025-02-06*
