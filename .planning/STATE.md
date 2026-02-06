# State: MCP CLI Rust Rewrite

**Created:** 2025-02-06
**Mode:** yolo
**Depth:** standard

---

## Project Reference

**Core Value:**
Reliable cross-platform MCP server interaction without dependencies. Developers and AI agents can discover available tools, inspect schemas, and execute operations through a simple CLI that works consistently on Linux, macOS, and Windows.

**Current Focus:**
Roadmap creation complete (4 phases). Ready to begin Phase 1: Core Protocol & Configuration.

---

## Current Position

**Active Phase:** 01-core-protocol-config

**Active Plan:** 04

**Status:** In progress

**Progress:**
```
Phase 1: Core Protocol & Configuration         ██████████████ 100% (4/4 plans)
Phase 2: Connection Daemon & Cross-Platform IPC ░░░░░░░░░░░ 0%
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

### Technical Decisions Made

| Decision | Rationale |
|----------|-----------|
| Use tokio::process::Command with kill_on_drop(true) | Fixes Windows zombie process issue from Bun implementation |
| Layered architecture (CLI → Client → Transport) | Clear separation of concerns, testable, transport-agnostic |
| Trait-based IPC abstraction | Enables Unix sockets (*nix) and named pipes (Windows) without scattering platform conditionals |
| thiserror + anyhow error handling | Domain-specific errors for library, context-aware errors for application |
| shell-words for command parsing | Prevents command injection vulnerabilities in config parsing |
| No global mutable state | Explicit context passing (AppContext) avoids test interference and race conditions |
| Colored output with NO_COLOR support | Better readability, respects terminal preferences |

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

2. **Windows Named Pipe Security:** During Phase 2 planning, research exact pattern for combining tokio async/await with security_qos_flags. May need small test programs to validate pattern.

3. **Daemon Connection Pool Scaling:** Test with 100+ server connections during Phase 2 implementation. Add TTL or eviction if memory usage is excessive (not blocking for MVP).

---

## Session Continuity

**Next Steps:**
- Execute Plan 01-04 (CLI commands for tool discovery and execution)
- After Phase 1 complete, begin Phase 2 (Connection Daemon & Cross-Platform IPC)

**Project Context for New Sessions:**
- Solo developer + Claude workflow (no teams, no stakeholders)
- Roadmap created with 4 phases covering all 42 v1 requirements
- Windows-first approach to catch platform-specific bugs early
- Critical Windows issues: zombie processes (kill_on_drop), named pipe security (security_qos_flags)
- No external MCP SDK dependency - protocol implemented directly
- Architecture: layered, trait-based abstractions, no global mutable state
- Core protocol layer complete: transport abstraction, McpClient with tool discovery/execution, comprehensive error handling

---

**Last updated:** 2026-02-06
**Mode:** yolo
**Depth:** standard
