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

**Plans:** 11 plans in 8 waves (including gap closure)

Plans:
- [x] 02-01-PLAN.md — IPC abstraction trait and Unix socket implementation
- [x] 02-02-PLAN.md — Windows named pipe implementation with security
- [x] 02-03-PLAN.md — Daemon binary with idle timeout and lifecycle management
- [x] 02-04-PLAN.md — Connection pooling and health checks
- [x] 02-05-PLAN.md — Config change detection and orphan cleanup
- [x] 02-06-PLAN.md — CLI integration and cross-platform tests
- [x] 02-07-PLAN.md — Gap closure: Fix ProtocolClient lifetime issue (Arc<Config>)
- [x] 02-08-PLAN.md — Gap closure: Implement NDJSON protocol for IPC communication
- [x] 02-09-PLAN.md — Gap closure: Implement daemon request handlers (ExecuteTool, ListTools, ListServers)
- [x] 02-10-PLAN.md — Gap closure: Implement config change detection and graceful shutdown
- [x] 02-11-PLAN.md — Gap closure: Fix test compilation and create IPC tests

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

**Plans:** 6 plans in 4 waves

Plans:
- [x] 03-01-PLAN.md — Configuration extensions & colored output infrastructure
- [x] 03-02-PLAN.md — Parallel server discovery infrastructure
- [x] 03-03-PLAN.md — Retry logic with exponential backoff
- [x] 03-06-PLAN.md — Signal handling infrastructure for graceful shutdown
- [x] 03-04-PLAN.md — CLI integration: discovery (parallel execution, colored output)
- [x] 03-05-PLAN.md — CLI integration: execution (retry, timeout, signal handling)

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

**Plans:** 3 plans in 1 wave

Plans:
- [ ] 04-01-PLAN.md — Tool filtering features (FILT-01 through FILT-05)
- [ ] 04-02-PLAN.md — Windows process spawning validation (XP-01)
- [ ] 04-03-PLAN.md — Cross-platform daemon validation (XP-04)

---

## Progress

| Phase | Name | Status | Completion |
|-------|------|--------|------------|
| 1 | Core Protocol & Configuration | Complete | 100% |
| 2 | Connection Daemon & Cross-Platform IPC | Complete | 100% (11/11 plans complete, including 5 gap closure) |
| 3 | Performance & Reliability | Complete | 100% (6/6 plans complete in 4 waves) |
| 4 | Tool Filtering & Cross-Platform Validation | Complete | 100% |
| 5 | Unified Daemon Architecture | Complete | 100% |

---

**Last updated:** 2026-02-10 (Phase 5 complete - Unified daemon architecture implemented)

---

## Phase 6: Output Formatting & Visual Hierarchy

**Goal:** Users can navigate CLI output easily with clear visual hierarchy, prominent tool descriptions, and consistent formatting across all commands.

**Dependencies:**
- Phase 1-5: All core functionality (configuration, connections, discovery, execution, filtering, daemon)

**Requirements (14/18 v1.2):**
- Output Formatting: OUTP-01, OUTP-02, OUTP-03, OUTP-04, OUTP-05, OUTP-06
- Tool Discovery UX: OUTP-11, OUTP-12, OUTP-13, OUTP-14, OUTP-15
- Error & Warning Display: OUTP-16, OUTP-17, OUTP-18

**Success Criteria:**
1. User can see parameter overview (names, types, required/optional status) when listing tools in help-style format
2. Tool descriptions are clearly visible in default view (not truncated or hidden behind -d flag)
3. Multi-server output is visually organized with clear server headers and grouped tools
4. Server status (connected, failed, disabled tools present) is obvious at a glance
5. Tool search results include context showing server name and tool description
6. Empty states display helpful messages (e.g., "No servers configured" with suggestion to create config file)
7. Partial failures clearly indicate which servers succeeded and which failed
8. Warnings are visually distinct but not overwhelming (appropriate use of color/formatting)

**What This Delivers:**
- Help-style parameter display showing required vs optional parameters
- Progressive detail levels: default (summary) → -d (with descriptions) → -v (verbose with full schema)
- Default list output showing tool count and brief descriptions per server
- Visual hierarchy in multi-server listings (headers, indentation, separators)
- Consistent formatting across list, info, grep, and call commands
- Standard CLI parameter conventions (e.g., `name <type>` for required, `name [type]` for optional)
- Prominent tool descriptions in default view
- Usage hints in tool listings (e.g., "Use `mcp info server tool` for full schema")
- Clear server status indicators (connected ✓, failed ✗, with disabled tools)
- Context-rich search results with server + tool + description
- Helpful empty state messages with actionable suggestions
- Consistent error format with context and recovery suggestions
- Partial failure reporting showing success/failure per server

**Avoids Pitfalls:**
- Information overload (progressive disclosure keeps default view clean)
- Truncated descriptions (descriptions are prominent by default)
- Inconsistent formatting between commands (shared formatting utilities)
- Unclear server state (visual indicators make status obvious)
- Missing context in search results (server + tool shown together)

**Plans:** 4 plans in 3 waves

Plans:
- [x] 06-01-PLAN.md — Formatting infrastructure (schema parsing, parameter formatting)
- [x] 06-02-PLAN.md — Enhanced list command with visual hierarchy
- [x] 06-03-PLAN.md — Info and grep commands with consistent formatting
- [x] 06-04-PLAN.md — Error/warning display enhancement

---

## Phase 7: JSON Output & Machine-Readable Modes

**Goal:** Scripts and automation tools can reliably parse CLI output through a consistent JSON mode with complete metadata.

**Dependencies:**
- Phase 6: Output Formatting & Visual Hierarchy (complete)

**Requirements (4/18 v1.2):**
- Output Modes: OUTP-07, OUTP-08, OUTP-09, OUTP-10

**Success Criteria:**
1. User can get JSON output by adding `--json` flag to any command
2. JSON output includes complete tool metadata (name, description, parameters, full schema)
3. JSON schema is consistent across all commands (list, info, grep, call)
4. Plain text mode works correctly when piped or when `--no-color` is set
5. Scripts can parse tool listings programmatically without fragile text parsing

**What This Delivers:**
- `--json` flag supported across all CLI commands
- Complete tool metadata in JSON format (name, description, parameters, JSON schema)
- Consistent JSON schema structure regardless of command
- Proper plain text output when stdout is not a TTY or NO_COLOR is set
- Machine-readable output suitable for scripting and automation
- JSON output that includes both tool metadata and execution results

**Avoids Pitfalls:**
- Inconsistent JSON schema between commands (shared serialization logic)
- Missing metadata in JSON mode (all fields from human-readable output included)
- Breaking changes to JSON schema (versioned or documented structure)
- Escape sequence pollution in piped output (proper TTY detection)

**Plans:** 4 plans in 3 waves

Plans:
- [x] 07-01-PLAN.md — Add --json flag infrastructure and OutputMode enum
- [x] 07-02-PLAN.md — Implement JSON output for discovery commands (list, info, search)
- [x] 07-03-PLAN.md — Implement JSON output for tool execution (call command)
- [x] 07-04-PLAN.md — Integration tests and schema documentation

---

## Progress

| Phase | Name | Status | Completion |
|-------|------|--------|------------|
| 1 | Core Protocol & Configuration | Complete | 100% |
| 2 | Connection Daemon & Cross-Platform IPC | Complete | 100% |
| 3 | Performance & Reliability | Complete | 100% |
| 4 | Tool Filtering & Cross-Platform Validation | Complete | 100% |
| 5 | Unified Daemon Architecture | Complete | 100% |
| 6 | Output Formatting & Visual Hierarchy | Complete | 100% |
| 7 | JSON Output & Machine-Readable Modes | Complete | 100% (4/4 plans) |

**v1.2 Coverage:** 18/18 requirements mapped ✓

---

**Last updated:** 2026-02-11 (Phase 7 complete - JSON Output & Machine-Readable Modes)

---

## Phase 8: Fix Phase 4 Windows Tests (XP-01)

**Goal:** Create missing Windows process integration tests to complete XP-01 validation

**Dependencies:**
- Phase 4: Tool Filtering & Cross-Platform Validation (complete)

**Tech Debt Addressed:**
- XP-01: Windows integration tests file (windows_process_spawn_tests.rs) was promised but never created in Phase 4

**Success Criteria:**
1. Missing integration test file created (tests/windows_process_spawn_tests.rs)
2. All 9 integration test scenarios implemented (CLI, concurrency, timeouts, daemon, batch, errors)
3. Tests compile without errors (correct tokio::io traits and types)
4. Both Windows test files (unit + integration) exist and work together
5. XP-01 validated through comprehensive test coverage

**Plans:** 1 plan in 1 wave

**Plans:** 1 plan in 1 wave

Plans:
- [x] 08-01-PLAN.md — Create Windows process spawning integration tests

---

## Phase 9: Cross-Platform Verification (XP-02, XP-04)

**Goal:** Document XP-02 security implementation and verify daemon works across all platforms

**Dependencies:**
- Phase 8: Fix Phase 4 Windows Tests (complete)

**Tech Debt Addressed:**
- XP-02: Security flags implementation unclear
- XP-04: Cross-platform daemon needs runtime verification

**Success Criteria:**
1. XP-02 security approach documented in code
2. Daemon tested on Linux, macOS, and Windows
3. No platform-specific behavior differences
4. Verification results documented

**Tasks:**
- Document XP-02 security approach (reject_remote_clients vs security_qos_flags)
- Run daemon tests on Linux
- Run daemon tests on macOS
- Run daemon tests on Windows
- Document verification results

---

## Phase 10: Phase 6 Verification Documentation

**Goal:** Create VERIFICATION.md documenting Phase 6 output formatting completion

**Dependencies:**
- Phase 9: Cross-Platform Verification (complete)

**Tech Debt Addressed:**
- Documentation: No VERIFICATION.md file for Phase 6

**Success Criteria:**
1. VERIFICATION.md created for Phase 6
2. Goal-backward analysis documented
3. All 14 v1.2 requirements verified
4. Evidence from plan summaries included

**Tasks:**
- Create VERIFICATION.md with goal-backward analysis
- Document all 14 v1.2 requirements satisfied
- Add evidence from plan summaries
- Document any deviations

---

## Phase 11: Code Quality Cleanup

**Goal:** Clean up minor code quality issues

**Dependencies:**
- Phase 10: Phase 6 Verification Documentation (complete)

**Tech Debt Addressed:**
- Minor: Unused imports in commands.rs
- Minor: Commented-out code in src/cli/daemon.rs

**Success Criteria:**
1. No unused imports in commands.rs
2. No commented-out code in src/cli/daemon.rs
3. Code passes clippy/format checks

**Tasks:**
- Remove unused imports in commands.rs
- Remove commented-out code in src/cli/daemon.rs
- Run clippy and fmt checks

---

## Progress

| Phase | Name | Status | Completion |
|-------|------|--------|------------|
| 1 | Core Protocol & Configuration | Complete | 100% |
| 2 | Connection Daemon & Cross-Platform IPC | Complete | 100% |
| 3 | Performance & Reliability | Complete | 100% |
| 4 | Tool Filtering & Cross-Platform Validation | Complete | 100% |
| 5 | Unified Daemon Architecture | Complete | 100% |
| 6 | Output Formatting & Visual Hierarchy | Complete | 100% |
| 7 | JSON Output & Machine-Readable Modes | Complete | 100% |
| 8 | Fix Phase 4 Windows Tests (XP-01) | Complete | 100% |
| 9 | Cross-Platform Verification (XP-02, XP-04) | Pending | 0% |
| 10 | Phase 6 Verification Documentation | Pending | 0% |
| 11 | Code Quality Cleanup | Pending | 0% |

**v1.2 Tech Debt Coverage:** 3/4 items pending

---

**Last updated:** 2026-02-11 (Phase 8 complete - Windows integration tests created)
