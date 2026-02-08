# State: MCP CLI Rust Rewrite

**Created:** 2025-02-06
**Last updated:** 2026-02-08 - Completed gap closure plan 02-11 (test compilation fixes), Phase 2 at 100%
**Mode:** yolo
**Depth:** standard

---

## Project Reference

**Core Value:**
Reliable cross-platform MCP server interaction without dependencies. Developers and AI agents can discover available tools, inspect schemas, and execute operations through a simple CLI that works consistently on Linux, macOS, and Windows.

**Current Focus:**
Executing Phase 3: Performance & Reliability (Wave 5)

---

## Current Position

**Active Phase:** 03-connection-health-checks

**Active Plan:** 03-01 (connection health checks & reliability metrics)

**Status:** Ready to start Phase 3 execution

**Progress:**
```
Phase 1: Core Protocol & Configuration         ██████████████ 100% (4/4 plans complete)
Phase 2: Connection Daemon & Cross-Platform IPC ██████████████ 100% (11/11 plans complete, 5 gap closure)
Phase 3: Performance & Reliability             ░░░░░░░░░░░ 0%
Phase 4: Tool Filtering & Cross-Platform Validation ░░░░░░░░░░░ 0%
```

---

## Performance Metrics

**Requirements Coverage:**
- Total v1 requirements: 42
- Covered in roadmap: 42 (100%)
- Unmapped: 0

**Phase Distribution:**
- Phase 1: 25 requirements
- Phase 2: 4 requirements
- Phase 3: 6 requirements
- Phase 4: 7 requirements

---

## Accumulated Context

### Key Decisions

1. **Stack Selection:** Rust with tokio async runtime, clap CLI parser, reqwest HTTP client, tokio process spawning. Critical: tokio::process::Command with kill_on_drop(true) to prevent Windows zombie processes (the Bun implementation's main failure).

2. **Phase Structure:** 4 phases following natural delivery boundaries, not arbitrary templates. Phase 1 builds foundation (transport + protocol), Phase 2 adds daemon (performance), Phase 3 enhances reliability, Phase 4 polishes UX and validates cross-platform behavior.

3. **Windows-First Approach:** Critical Windows issues (process spawning, named pipe security) tested in early phases (1-2) when codebase is simpler, avoiding late discovery of platform-specific bugs.

4. **No mcp-sdk dependency:** Implemented MCP protocol directly using tokio + serde_json. Version 0.0.3 has only 11.36% documented coverage and 827 weekly downloads, insufficient for production use. Transport abstraction trait enables stdio/HTTP switching without dependencies.

5. **Architecture Approach:** Layered architecture with trait-based abstractions. Transport abstraction (stdio vs HTTP), IPC abstraction (Unix sockets vs named pipes), no global mutable state (explicit AppContext passing).

6. **Transport abstraction pattern:** ServerTransport trait with send(), ping(), and transport_type() methods. TransportFactory trait converts ServerTransport to actual transport instances.

7. **Error handling with thiserror:** Domain-specific error types (ConnectionError, InvalidProtocol, Timeout, InvalidRequest, NoResult, ParseError) with context-aware error messages.

8. **IPC abstraction implementation:** Created IpcServer, IpcClient, IpcStream traits with factory functions (create_ipc_server, get_socket_path). Unix socket implementation provided with create_dir_all for socket directory and StaleSocket detection.

9. **Windows named pipe IPC:** Windows implementation using tokio::net::windows::named_pipe with SECURITY_IDENTIFICATION (set by interprocess crate). NamedPipeIpcServer/Client support multiple connections with first_pipe_instance(true) to prevent multiple daemons.

10. **NDJSON Protocol for CLI-Daemon Communication:** Newline-delimited JSON format for CLI-daemon requests/responses, one JSON object per line with newline terminator. Simple and robust, matching MCP protocol conventions.

11. **Idle Timeout Default 60 Seconds:** Daemon self-terminates after 60 seconds of inactivity, tracked via Arc<Mutex<Instant>> for thread-safe activity updates.

12. **Config Fingerprinting:** SHA256-based fingerprint calculation for config validation, enabling CLI to detect configuration changes and restart daemon when needed.

13. **Stub Implementation Strategy:** Implemented stub handlers for ExecuteTool, ListTools, ListServers returning Error::NotImplemented. Full implementation deferred to plan 02-05 (connection pool CLI integration).

14. **Connection Pool with Health Checks:** Implemented connection pool caching transport connections with MCP ping health checks. Connections are validated before reuse, and automatically recreated after 3 consecutive health check failures. Thread-safe pool enables concurrent access from multiple client handlers.

15. **Test Compilation Gap Closure (Plan 02-11):** Fixed all compilation errors in daemon lifecycle tests and IPC integration tests. Created comprehensive IPC integration tests with 3 test cases covering roundtrip, concurrent connections, and large message transfer. All IPC tests compile successfully with proper timeout handling, JoinError imports, and BufReader wrapping for AsyncBufRead compliance.

16. **ProtocolClient Lifetime Issue (RESOLVED):** ProtocolClient trait lifetime issue prevented CLI compilation. Gap closure plan 02-07 created to resolve this by converting from &Config to Arc<Config>, eliminating lifetime parameter and enabling shared ownership. Requires trait and wrapper refactoring (completed in gap closure).

### Technical Decisions Made

| Decision | Rationale |
|----------|-----------|
| Use tokio::process::Command with kill_on_drop(true) | Fixes Windows zombie process issue from Bun implementation |
| Layered architecture (CLI → Client → Transport) | Clear separation of concerns, testable, transport-agnostic |
| Trait-based IPC abstraction | Enables Unix sockets (*nix) and named pipes (Windows) without scattering #[cfg] conditionals |
| thiserror + anyhow error handling | Domain-specific errors for library, context-aware errors for application |
| shell-words for command parsing | Prevents command injection vulnerabilities in config parsing |
| No global mutable state | Explicit context passing (AppContext) avoids test interference and race conditions |
| Colored output with NO_COLOR support | Better readability, respects terminal preferences |
| IPC abstraction using interprocess crate | Provides unified IPC support across platforms with tokio async features |
| IPC error categorization (IpcError variants) | Enables precise error handling and better error messages for IPC failures |
| tokio::named_pipe instead of interprocess | Better tokio integration, same SECURITY_IDENTIFICATION protection, cleaner async patterns |
| Connection pool with health checks | Thread-safe caching of transport connections, MCP ping validation, automatic recreation on failures |
| Arc<Config> for ProtocolClient lifetime fix | Eliminates lifetime parameter, enables shared ownership across CLI operations |
| Comprehensive test compilation fixes (plan 02-11) | Created IPC integration tests, fixed all async/await and timeout compilation errors, enabled complete codebase compilation |

### Known Pitfalls to Avoid

From research/PITFALLS.md:

**Critical (address in Phase 1):**
- ✅ Set `.kill_on_drop(true)` on all tokio::process::Command spawns (prevent Windows zombie processes)
- ✅ Use `shell-words::split()` for command parsing (prevent command injection)
- ✅ Use `writeln!` for stdio transport messages (prevent embedded newlines breaking MCP protocol)
- ✅ Use `tokio::fs` and `tokio::process` in async code (prevent blocking executor)
- ✅ Wrap errors with context using thiserror (prevent generic error messages)
- ✅ Implement exit code conventions (0=success, 1=client error, 2=server error, 3=network error)

**Critical (address in Phase 2):**
- ✅ Use `security_qos_flags` when opening Windows named pipes (prevent privilege escalation)
- ✅ Implement connection health checks before reuse (prevent stale connection reuse)
- ✅ Abstract platform differences behind traits (prevent scattered #[cfg] conditionals)
- ✅ Fix all test compilation errors (plan 02-11 gap closure completed)

### Requirements by Phase

**Phase 1 (25):**
- Configuration (5): CONFIG-01 through CONFIG-05
- Server Connections (4): CONN-01, CONN-02, CONN-03, CONN-04
- Discovery & Search (5): DISC-01, DISC-02, DISC-03, DISC-04, DISC-06
- Tool Execution (5): EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-06
- Error Handling (5): ERR-01, ERR-02, ERR-03, ERR-05, ERR-06
- CLI Support (3): CLI-01, CLI-02, CLI-03
- Cross-Platform (1): XP-03

**Phase 2 (4):**
- Server Connections (4): CONN-05, CONN-06, CONN-07, CONN-08

**Phase 3 (6):**
- Discovery & Search (1): DISC-05
- Tool Execution (2): EXEC-05, EXEC-07
- Error Handling (2): ERR-04, ERR-07
- CLI Support (1): CLI-04

**Phase 4 (7):**
- Tool Filtering (5): FILT-01, FILT-02, FILT-03, FILT-04, FILT-05
- CLI Support (1): CLI-05
- Cross-Platform (3): XP-01, XP-02, XP-04

### Outstanding Questions

1. **mcp-sdk Evaluation:** During Phase 1 planning, verify if mcp-sdk 0.0.3 handles MCP protocol correctly (initialization handshake, message delimiters, error responses). If not, re-implement using tokio + serde_json.

2. **Daemon Connection Pool Scaling:** Test with 100+ server connections during Phase 2 implementation. Add TTL or eviction if memory usage is excessive (not blocking for MVP).

3. ~~Connection Health Checks~~ ✅ **RESOLVED in plan 02-04:** Implemented connection pool with MCP ping health checks; connections validated before reuse, recreated after 3 failures.

4. ~~Test Compilation Gap Closure~~ ✅ **RESOLVED in plan 02-11:** Fixed all compilation errors in daemon lifecycle tests and IPC integration tests. Created comprehensive IPC integration tests with 3 test cases covering roundtrip, concurrent connections, and large message transfer. All IPC tests compile successfully with proper timeout handling, JoinError imports, and BufReader wrapping for AsyncBufRead compliance. Phase 2 gap closure complete.

---

## Session Continuity

**Next Steps:**
- Execute Phase 3 plan 03-01 via `/gsd-execute-phase 3` to implement connection health checks and reliability metrics
- Phase 2 gap closure completed: all test compilation errors fixed, IPC integration tests created and working
- After Phase 3: proceed to Phase 4 (tool filtering, cross-platform validation)

**Project Context for New Sessions:**
- Solo developer + Claude workflow (no teams, no stakeholders)
- Roadmap created with 4 phases covering all 42 v1 requirements
- Windows-first approach to catch platform-specific bugs early
- Critical Windows issues: zombie processes (kill_on_drop), named pipe security (security_qos_flags)
- No external MCP SDK dependency - protocol implemented directly
- Architecture: layered, trait-based abstractions, no global mutable state
- Core protocol layer complete: transport abstraction, McpClient with tool discovery/execution, comprehensive error handling
- IPC abstraction layer complete: Unix socket implementation + Windows named pipe backend
- Daemon layer complete: binary with IPC communication, idle timeout, lifecycle management
- Phase 2 complete: daemon lifecycle, cross-platform IPC, connection pools, health checks, all tests compile successfully

---

**Last updated:** 2026-02-08 - Completed gap closure plan 02-11 (test compilation fixes), Phase 2 at 100%
**Mode:** yolo
**Depth:** standard
**Plans completed:** 02-01, 02-02, 02-03, 02-04, 02-05, 02-06, 02-11 (all 7 plans complete, including gap closure)
