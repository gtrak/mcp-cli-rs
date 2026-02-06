# Project Research Summary

**Project:** MCP CLI Rust Rewrite
**Domain:** CLI tools (Model Context Protocol client)
**Researched:** 2025-02-06
**Confidence:** HIGH

## Executive Summary

This is a CLI client tool for the Model Context Protocol (MCP) — a Rust rewrite of an existing Bun-based implementation that suffered from Windows process spawning issues. Experts build async CLI tools in Rust using tokio for the asynchronous runtime, clap for declarative CLI parsing, and trait-based abstractions for transport layer flexibility (stdio vs HTTP). The project requires cross-platform support (Windows/Linux/macOS) with a persistent connection daemon for performance optimization.

The recommended approach is to start with core transport implementation (stdio and HTTP) using tokio::process::Command with explicit `kill_on_drop(true)` handling for Windows process cleanup, then layer on a connection daemon with platform-specific IPC (Unix sockets for *nix, named pipes for Windows). Use the mcp-sdk crate (0.0.3) cautiously — it's pre-alpha with limited documentation and may need to be forked or re-implemented using tokio + serde_json directly if it doesn't meet requirements.

Key risks are concentrated around Windows process spawning (zombie processes without cleanup), named pipe security vulnerabilities when not using `security_qos_flags`, and the maturity of the mcp-sdk crate. Mitigation strategy: test process spawning and named pipes on Windows from Phase 1, use shell-words for secure command parsing, and implement the MCP protocol using tokio + serde_json if mcp-sdk proves insufficient.

## Key Findings

### Recommended Stack

Standard Rust async CLI stack with MCP protocol support. Core libraries are mature and well-documented; the mcp-sdk crate is early-stage (0.0.3, 827 downloads, 11.36% documented) and requires evaluation.

**Core technologies:**
- **clap 4.5.57** — CLI argument parsing with derive macros (`features = ["derive"]`) — De facto standard for Rust CLIs, maintained by rust-cli team
- **tokio 1.49.0** — Async runtime (`features = ["full"]`) — Standard async runtime, provides tokio::process for cross-platform process spawning that fixes Windows issues
- **mcp-sdk 0.0.3** — MCP protocol implementation — Third-party Rust SDK, LOW maturity (evaluate thoroughly, may need to fork or reimplement)
- **reqwest 0.13.1** — HTTP client — Higher-level async HTTP client with tokio integration for MCP HTTP transport
- **anyhow + thiserror** — Error handling — Flexible application errors with context, library errors with derive macros
- **tracing + tracing-subscriber** — Logging — Modern structured logging framework, required by mcp-sdk
- **shell-words 1.1.1** — Shell command parsing — Parse command strings safely, avoiding injection vulnerabilities

### Expected Features

MCP CLI follows standard CLI tool patterns: configure servers, discover tools, inspect schemas, execute tools. Most features are table stakes — missing them makes the tool feel incomplete. The main differentiator from the original Bun implementation is cross-platform reliability and single-binary distribution.

**Must have (table stakes, P1 - MVP):**
- Server connection (stdio & HTTP) — Core capability without which nothing works
- Config parsing (mcp_servers.json) — Standard format used by Claude Desktop, VS Code, Gemini
- Environment variable substitution — `${VAR_NAME}` syntax for credentials and secrets
- Server/tool listing — Users need to see what's available
- Tool inspection (schema display) — Users need to understand tool parameters
- Tool execution with arguments — Primary use case, must support inline JSON and stdin input
- Tool search (glob patterns) — Find tools across many servers quickly
- Basic error messages with suggestions — Users expect actionable error recovery
- Help & version flags — Every CLI tool has these
- Exit code conventions — Scripts need to know success/failure

**Should have (P2, competitive advantages):**
- Connection daemon (lazy caching) — 50%+ performance improvement for repeated calls, auto-spawns on first use, self-terminates on idle timeout
- Exponential backoff retry — Automatic recovery from transient failures (3x retry with jitter)
- Tool filtering (allowed/disabled) — Security sandboxing for production environments
- Structured error messages — AI agent compatibility with error code, details, suggestion fields
- Concurrent parallel connections — Faster when listing/searching many servers (configurable limit, default 5)
- Cross-platform single binary — No runtime dependencies (solves Bun requirement)
- Graceful signal handling — Proper cleanup on SIGINT/SIGTERM

**Defer (P3, v2+):**
- Server-specific timeout configuration — Advanced performance tuning, MCP protocol already has server-side timeouts
- Config validation command — Pre-flight checking without connecting
- Tool benchmarking — Measure tool execution times
- Batch tool execution — Execute multiple tools in one command

### Architecture Approach

Layered architecture with clear separation between CLI concerns, business logic, and transport layers. Trait-based abstractions enable cross-platform transport switching (stdio vs HTTP) and IPC (Unix sockets vs named pipes) without code changes. No global mutable state — context is explicitly passed, daemon uses Arc<Mutex<State>> only where needed.

**Major components:**
1. **CLI Layer** — Parse user commands with clap, route to handlers, format output (src/cli/)
2. **MCP Client Abstraction** — Protocol handling, connection lifecycle, trait-based transport (src/client/)
3. **Transport Implementations** — StdioTransport (tokio::process), HttpTransport (reqwest), abstracted via Transport trait
4. **Daemon with IPC** — Connection pooling, persistent connections, platform-specific IPC (Unix sockets *nix, named pipes Windows) (src/daemon/)
5. **Config Module** — mcp_servers.json parsing, environment variable substitution (src/config/)
6. **Error Handling** — Domain-specific errors with thiserror, structured messages for AI agents (src/error.rs)

### Critical Pitfalls

Based on known issues from the original Bun implementation and Rust async/Windows specific challenges.

1. **Windows process spawning without kill_on_drop** — Zombie processes accumulate, consuming resources and causing "address already in use" errors. Prevention: ALWAYS set `.kill_on_drop(true)` when using tokio::process::Command.
2. **Named pipe security QoS not configured** — Privilege escalation vulnerability when opening Windows named pipes without security flags. Prevention: Always call `.security_qos_flags()` when opening named pipes, use SECURITY_IDENTIFICATION or SECURITY_IMPERSONATION.
3. **Stdio transport missing newline delimiter** — MCP spec requires newline-delimited JSON messages without embedded newlines. Prevention: Use `writeln!` for messages, never `serde_json::to_string_pretty`.
4. **Connection pool not detecting stale connections** — Reuses closed connections, causing "broken pipe" errors. Prevention: Implement connection health checks (try_wait or ping) before reuse.
5. **Environment variable substitution injection** — Command injection from malicious environment variable values. Prevention: Always parse command strings using `shell-words::split()`, substitute variables atomically, never use shell interpolation.

## Implications for Roadmap

Research suggests a 4-phase roadmap that builds capability incrementally while avoiding Windows-specific pitfalls early. Architecture is layered: core transport → protocol layer → daemon → performance/refinement.

### Phase 1: Core Protocol & Configuration
**Rationale:** Foundation layer — without transport and protocol, nothing else works. Windows process spawning must be tested first to avoid late discovery of zombie process issues.
**Delivers:** Working MCP client that can connect to stdio and HTTP servers, parse config, discover tools, execute tools, display results.
**Addresses:** Features: Server connection, config parsing, environment substitution, tool listing, tool inspection, tool execution, basic error messages, help/version, exit codes
**Avoids:** Pitfalls 1 (Windows process spawning), 3 (stdio transport newlines), 5 (env var injection), 10 (protocol version mismatch)
**Stack elements built:** clap (CLI), tokio (runtime/process), mcp-sdk (or tokio+serde_json), reqwest (HTTP), shell-words, serde, serde_json, anyhow, thiserror, tracing
**Architecture components:** CLI Layer, MCP Client Abstraction, Stdio/HTTP Transports, Config/Env Substitution, Error Handling

### Phase 2: Connection Daemon & Cross-Platform IPC
**Rationale:** Performance optimization layer — adds connection caching for 50%+ speed improvement on repeated calls. Platform-specific IPC must be abstracted behind traits to avoid platform conditionals in core logic.
**Delivers:** Background daemon worker, connection pooling, cross-platform IPC (Unix sockets + named pipes), graceful shutdown.
**Addresses:** Features: Connection daemon (performance), retry logic (reliability), concurrent connections (speed), graceful signal handling (cleanup)
**Uses:** Stack: tokio::net (Unix sockets), std::os::windows (named pipes)
**Implements:** Architecture: Daemon worker, IPC trait abstraction (UnixIpcTransport, NamedPipeTransport)
**Avoids:** Pitfalls 2 (named pipe security), 4 (stale connections in pool), 8 (platform conditionals)

### Phase 3: Performance & Reliability Features
**Rationale:** Enhances robustness of Phase 1 and 2. Retry logic and structured errors improve production reliability; concurrent connections improve speed for many-server scenarios.
**Delivers:** Exponential backoff retry with jitter, tool filtering (allowed/disabled patterns), structured error formatting, concurrent parallel connection limits.
**Addresses:** Features: Exponential backoff retry, tool filtering, structured error messages, concurrent parallel connections
**Stack elements:** tokio-time (retry scheduling), glob pattern matching (tool filtering)
**Implements:** Retry logic module, tool filtering middleware, structured error display

### Phase 4: UX Polish & Advanced Features
**Rationale:** Refinements that don't block core functionality. Colored output improves readability, better stdin handling improves pipeline compatibility, config validation helps users debug before running.
**Delivers:** Colored output with NO_COLOR support, improved stdin auto-detect, config validation command, server-specific timeout configuration.
**Addresses:** Features: Colored output, stdin auto-detect, server-specific timeouts, config validation

### Phase Ordering Rationale

- **Layered architecture dependency:** Phase 1 builds foundation (transport + protocol), Phase 2 adds daemon on top, Phase 3 enhances reliability, Phase 4 polishes UX. Each phase depends on previous phases' infrastructure.
- **Windows-first for critical issues:** Phase 1 tests Windows process spawning and stdio transport immediately, preventing late discovery of zombie processes or protocol violations. This was the Bun implementation's primary failure.
- **Feature dependency alignment:** Configuration (Phase 1) is prerequisite for daemon (Phase 2). Connection pooling (Phase 2) is prerequisite for concurrent operations (Phase 3). Error handling improvements span all phases.
- **Pitfall avoidance scheduling:** Critical Windows issues (process spawning, named pipe security) tested early (Phases 1-2) when codebase is simpler, not later when complexity makes debugging harder.

### Research Flags

**Phases likely needing deeper research during planning:**
- **Phase 2 (Connection Daemon):** Cross-platform IPC implementation has sparse documentation for Windows named pipes in Rust. The exact pattern for secure named pipe creation with `security_qos_flags` may need investigation. Platform abstraction design needs careful planning to avoid scattering `#[cfg(windows)]` and `#[cfg(unix)]` throughout code.

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Core Protocol):** Well-documented Rust async/CLI patterns. tokio::process, clap, reqwest, serde have extensive examples. MCP protocol is documented (JSON-RPC over stdio/HTTP). Standard patterns apply.
- **Phase 3 (Performance):** Exponential backoff with tokio::time, concurrent execution with futures::join_all, glob pattern matching — all standard Rust patterns with good ecosystem support.
- **Phase 4 (UX Polish):** Colored output with colored/atty crates, stdin detection — solved problems with known solutions in similar CLIs.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core technologies (clap, tokio, reqwest, serde) are mature, well-documented, and widely used. mcp-sdk is LOW confidence (0.0.3) but can be abandoned in favor of direct tokio + serde_json implementation. |
| Features | HIGH | Feature requirements based on original mcp-cli implementation (validated by real use) and MCP standard specification. Dependencies and MVP definition are well-reasoned. |
| Architecture | MEDIUM | Pattern recommendations are standard (transport abstraction, trait-based design), but exact shape of daemon IPC abstraction needs validation. Platform-specific details for named pipes may require iteration. |
| Pitfalls | HIGH | Critical Windows issues sourced from original Bun implementation failure mode and official tokio documentation. Prevention strategies are concrete and actionable. |

**Overall confidence:** HIGH

**Key uncertainties:**
- **mcp-sdk maturity:** Version 0.0.3 has minimal documentation (11.36% on docs.rs) and low adoption (827 downloads). May need to fork or re-implement protocol using tokio + serde_json directly.
- **Windows named pipe security:** Exact pattern for `security_qos_flags` usage with tokio/async context may need iteration. Security review recommended before Phase 2 completion.
- **Daemon IPC performance:** Connection pooling scaling for 100+ servers not validated. May need TTL or sharding (not blockng for MVP).

### Gaps to Address

**Areas where research was inconclusive or needs validation during implementation:**

- **Exact mcp-sdk JSON-RPC handling:** Documentation is sparse. During Phase 1 planning, verify if mcp-sdk handles MCP protocol initialization handshake, message delimiters, and error responses correctly. If not, plan to reimplement protocol using tokio + serde_json directly (straightforward: JSON-RPC messages over stream).

- **Windows named pipe testing pattern:** No example code in research for combining `tokio`, async/await, and `security_qos_flags`. During Phase 2 planning, create small test programs to verify Windows named pipe server/client patterns work correctly before committing to abstraction design.

- **Daemon connection pool scaling:** Architecture proposes connection pooling with health checks, but scale limits unknown. During Phase 2 implementation, test with 100+ server connections, monitor memory usage, and add TTL/eviction if needed. Not blocking for MVP.

## Sources

### Primary (HIGH confidence)
- [tokio 1.49.0 documentation](https://docs.rs/tokio/1.49.0/tokio/) — Async runtime, process spawning, Windows-specific behavior, kill_on_drop
- [clap 4.5.57 documentation](https://docs.rs/clap/4.5.57/clap/) — CLI parsing with derive macros, subcommands
- [reqwest 0.13.1 documentation](https://docs.rs/reqwest/0.13.1/reqwest/) — HTTP client with tokio integration
- [std::os::windows::fs::OpenOptionsExt](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html) — Named pipe security_qos_flags
- [MCP Protocol - Transports](https://modelcontextprotocol.io/docs/concepts/transports/) — stdio/HTTP requirements, message delimiters
- [Model Context Protocol TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) — Official reference implementation for protocol understanding
- [shell-words 1.1.1 documentation](https://docs.rs/shell-words/1.1.1/shell_words/) — Shell command parsing for injection prevention
- [thiserror 2.0.18 documentation](https://docs.rs/thiserror/2.0.18/thiserror/) — Error with context and suggestions
- [tracing 0.1.44 documentation](https://docs.rs/tracing/0.1.44/tracing/) — Structured logging framework

### Secondary (MEDIUM confidence)
- [mcp-sdk 0.0.3 documentation](https://docs.rs/mcp-sdk/0.0.3/mcp_sdk/) — Third-party Rust SDK, LOW maturity, 827 downloads, will likely need fork or reimplementation
- [mcp-sdk on crates.io](https://crates.io/crates/mcp-sdk) — Active contributor (last updated 2025-01-20), evaluate during Phase 1
- [Original mcp-cli Bun repository](../mcp-cli) — Source of Windows process spawning issues that motivated Rust rewrite, validated feature set
- [MCP Inspector CLI](https://github.com/modelcontextprotocol/inspector) — Official development tool, reference for expected CLI behavior

### Tertiary (LOW confidence)
- [Rust CLI Book](https://rust-cli.github.io/book/) — Community resource for CLI patterns
- [crates.io API](https://crates.io/) — Latest version verification (checked 2025-02-06)

---
*Research completed: 2025-02-06*
*Ready for roadmap: yes*
