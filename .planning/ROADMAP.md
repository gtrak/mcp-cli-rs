# Roadmap: MCP CLI Rust Rewrite

**Created:** 2025-02-06
**Core Value:** Reliable cross-platform MCP server interaction without dependencies
**Depth:** Standard (4 phases)
**Coverage:** 42/42 requirements mapped

## Overview

This roadmap delivers a complete MCP CLI tool in Rust that solves the Windows process spawning issues of the original Bun implementation. The architecture is layered: core transport and protocol → daemon connection pooling → performance optimization → UX refinement. Each phase delivers a verifiable set of user-facing capabilities.

Project follows a solo developer + Claude workflow with no team coordination artifacts. Phases derive from requirements rather than arbitrary templates.

---

## Phase 1: Core Protocol & Configuration

**Goal:** Users can connect to MCP servers, discover tools, execute tools, and handle basic errors with configuration support.

**Dependencies:** 
- Research documents (PITFALLS.md, ARCHITECTURE.md, STACK.md)

**Requirements (25/42):**
- Configuration: CONFIG-01, CONFIG-02, CONFIG-03, CONFIG-04, CONFIG-05
- Server Connections: CONN-01, CONN-02, CONN-03, CONN-04
- Discovery & Search: DISC-01, DISC-02, DISC-03, DISC-04, DISC-06
- Tool Execution: EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-06
- Error Handling: ERR-01, ERR-02, ERR-03, ERR-05, ERR-06
- CLI Support: CLI-01, CLI-02, CLI-03
- Cross-Platform: XP-03

**Success Criteria:**
1. User can configure servers in mcp_servers.toml with stdio (command, args, env, cwd) and HTTP (url, headers) definitions
2. User can discover all configured servers and their available tools by running the CLI with no arguments
3. User can inspect specific tool details including name, description, and input JSON Schema
4. User can execute tools with JSON arguments provided inline or via stdin pipe, receiving formatted results
5. User receives clear, actionable error messages when servers don't exist, tools aren't found, or JSON is invalid

**What This Delivers:**
- Complete configuration parsing with TOML support and environment variable substitutions
- Server connection lifecycle for both stdio (transport-aware) and HTTP transports
- Tool discovery, inspection, and search capabilities with glob pattern matching
- Tool execution with JSON validation and result formatting
- Structured error handling with context-aware suggestions
- CLI foundation with help, version, and config file path support
- MCP protocol compliance (newline-delimited messages, no embedded newlines)

**Avoids Pitfalls:**
- Windows zombie processes (CONN-04 kill_on_drop)
- Command injection (CONFIG parsing with shell-words)
- Stdio transport violations (XP-03 newline delimiters)
- Blocking I/O in async (tokio::fs/process used everywhere)

**Plans:** 4 plans in 3 waves

Plans:
- [x] 01-01-PLAN.md — Project setup, error handling, CLI scaffolding
- [x] 01-02-PLAN.md — Configuration parsing (mcp_servers.toml)
- [x] 01-03-PLAN.md — MCP protocol & transports (stdio + HTTP)
- [x] 01-04-PLAN.md — CLI commands & tool execution

---

## Phase 2: Connection Daemon & Cross-Platform IPC

**Goal:** Users experience significant performance improvement on repeated tool calls through an intelligent connection daemon that manages persistent connections across CLI invocations.

**Dependencies:**
- Phase 1: Core Protocol & Configuration (complete)

**Requirements (4/42):**
- Server Connections: CONN-05, CONN-06, CONN-07, CONN-08

**Success Criteria:**
1. Daemon automatically spawns on first tool execution and self-terminates after 60 seconds of idle time
2. First tool execution spawns daemon, subsequent calls reuse cached connections (50%+ faster)
3. Daemon detects configuration changes and spawns new daemon with fresh connections when config becomes stale
4. Orphaned daemon processes and sockets (from crashed daemon) are cleaned up on startup

**What This Delivers:**
- Cross-platform connection daemon using Unix sockets (*nix) and Windows named pipes
- Lazy daemon spawning on first access with configurable idle timeout (60s default)
- Connection pooling for persistent MCP server connections
- Configuration change detection with daemon restart
- Orphan cleanup process for robust daemon lifecycle management
- Graceful daemon shutdown on CLI signals

**Avoids Pitfalls:**
- Named pipe security vulnerabilities (CONNECTION-02: security_qos_flags)
- Stale connection reuse (CONNECTION-04: health checks)
- Platform conditionals in core logic (IPC abstraction trait)

**Plans:** 7 plans in 4 waves (plus gap closure)

Plans:
- [x] 02-01-PLAN.md — IPC abstraction trait and Unix socket implementation
- [x] 02-02-PLAN.md — Windows named pipe implementation with security
- [x] 02-03-PLAN.md — Daemon binary with idle timeout and lifecycle management
- [x] 02-04-PLAN.md — Connection pooling and health checks
- [x] 02-05-PLAN.md — Config change detection and orphan cleanup
- [x] 02-06-PLAN.md — CLI integration and cross-platform tests
- [ ] 02-07-PLAN.md — Gap closure: Fix ProtocolClient lifetime issue (Arc<Config>)

---

## Phase 3: Performance & Reliability

**Goal:** Users experience faster discovery across multiple servers and reliable tool execution that automatically recovers from transient failures.

**Dependencies:**
- Phase 1: Core Protocol & Configuration (complete)
- Phase 2: Connection Daemon & Cross-Platform IPC (complete)

**Requirements (6/42):**
- Discovery & Search: DISC-05
- Tool Execution: EXEC-05, EXEC-07
- Error Handling: ERR-04, ERR-07
- CLI Support: CLI-04

**Success Criteria:**
1. Server tool discovery processes multiple servers in parallel (default 5 concurrent) instead of sequentially
2. Tool execution automatically retries (up to 3 attempts) with exponential backoff for transient errors (network timeouts, HTTP 502/503/504/429)
3. Operation timeout (default 1800s) stops retries when time budget is exhausted
4. Terminal output uses colors for better readability when stdout is a TTY and NO_COLOR is not set
5. CLI gracefully handles SIGINT and SIGTERM with proper cleanup of connections and daemon
6. When some servers fail during parallel operations, user receives warning message but operation continues

**What This Delivers:**
- Concurrent parallel connections with configurable limits (default 5)
- Exponential backoff retry logic for transient errors
- Configurable retry limits (max 3 attempts, base 1000ms delay)
- Overall operation timeout enforcement (default 1800s)
- Colored terminal output with NO_COLOR support
- Graceful signal handling for resource cleanup
- Partial failure warnings for parallel operations

**Avoids Pitfalls:**
- Race conditions in concurrent execution (proper mutex/arc usage)
- Blocking the async executor with retry delays (tokio::time::sleep)
- Orphaned resources on signal (proper cleanup in signal handler)

---

## Phase 4: Tool Filtering & Cross-Platform Validation

**Goal:** Production environments can securely limit available tools, and the tool behaves consistently across Windows, Linux, and macOS without platform-specific bugs.

**Dependencies:**
- Phase 1: Core Protocol & Configuration (complete)
- Phase 2: Connection Daemon & Cross-Platform IPC (complete)
- Phase 3: Performance & Reliability (complete)

**Requirements (7/42):**
- Tool Filtering: FILT-01, FILT-02, FILT-03, FILT-04, FILT-05
- Error Handling: ERR-06
- CLI Support: CLI-05
- Cross-Platform: XP-01, XP-02, XP-04

**Success Criteria:**
1. Server configuration can specify glob patterns for allowedTools to restrict available tools
2. Server configuration can specify glob patterns for disabledTools to block specific tools
3. When both allowedTools and disabledTools are defined, disabledTools patterns take precedence
4. User receives clear error message when attempting to call a disabled tool
5. Tool filtering supports glob patterns with wildcards (*, ?) for flexible tool matching
6. Windows process spawning is tested and confirmed to have no zombie processes after execution
7. Connection daemon functions correctly on Linux, macOS, and Windows with proper IPC

**What This Delivers:**
- Tool filtering based on allowedTools glob patterns
- Tool blocking based on disabledTools glob patterns
- Precedence rules (disabledTools > allowedTools when both present)
- Error messages for disabled tool attempts
- Glob pattern matching with wildcards (*, ?)
- Windows process spawning validation (no zombie processes)
- Cross-platform daemon IPC validation (Unix sockets and named pipes)
- Support for both space-separated (server tool) and slash-separated (server/tool) argument formats

**Avoids Pitfalls:**
- Privilege escalation on Windows (XP-02: named pipe security_qos_flags)
- Platform-specific behavior differences (comprehensive cross-platform testing)
- Incomplete glob pattern matching (standard glob crate usage)

---

## Progress

| Phase | Name | Status | Completion |
|-------|------|--------|------------|
| 1 | Core Protocol & Configuration | Complete | 100% |
| 2 | Connection Daemon & Cross-Platform IPC | Gap Closure | 86% (6/7 plans + 1 gap closure) |
| 3 | Performance & Reliability | Pending | 0% |
| 4 | Tool Filtering & Cross-Platform Validation | Pending | 0% |

---

**Last updated:** 2026-02-07 (added gap closure plan 02-07)
