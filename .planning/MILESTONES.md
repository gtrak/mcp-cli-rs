# Milestones: MCP CLI Rust Rewrite

**Started:** 2025-02-06

Milestone tracking for MCP CLI Rust Rewrite project.

---

## Milestone v1.0: Core CLI with Connection Daemon

**Started:** 2025-02-06
**Completed:** 2026-02-09
**Status:** ✅ Complete

### Phase Structure

| Phase | Name | Status |
|-------|------|--------|
| 1 | Core Protocol & Configuration | ✅ Complete |
| 2 | Connection Daemon & Cross-Platform IPC | ✅ Complete |
| 3 | Performance & Reliability | ✅ Complete |
| 4 | Tool Filtering & Cross-Platform Validation | ✅ Complete |

### What Shipped

**Core Features:**
- MCP protocol client (stdio and HTTP transports)
- Configuration parsing from TOML files with environment variable support
- Server connection management with cross-platform process spawning
- Tool discovery, inspection, and execution
- Tool filtering via allowedTools/disabledTools glob patterns
- Connection daemon with cross-platform IPC (Unix sockets, named pipes)
- Parallel server operations with configurable concurrency
- Exponential backoff retry with timeout handling
- Colored terminal output with NO_COLOR support
- Graceful signal handling for resource cleanup
- Cross-platform validation (Windows process spawning, daemon IPC)

**Requirements Delivered:** 42/42 (100%)

### Lessons Learned

1. **Rust async patterns** — tokio provides excellent cross-platform support for process spawning and IPC
2. **Windows named pipes** — requires security_qos_flags to prevent privilege escalation
3. **Connection pooling** — health checks are essential to avoid broken pipe errors
4. **Glob pattern matching** — standard crate provides robust wildcard support
5. **Signal handling** — needs careful coordination between daemon and CLI processes

---
*Last updated: 2026-02-09*

---

## Milestone v1.1: Unified Daemon Architecture

**Started:** 2026-02-09
**Status:** 🚧 In Progress

### Goal

Refactor daemon architecture to unify CLI and daemon into a single binary with three operational modes, removing the need for a separate daemon executable.

### What We're Building

**Architecture Changes:**
- Remove separate daemon.exe binary - integrate daemon logic into main CLI
- Three operational modes for the unified binary:
  1. **Standalone Daemon Mode**: Run as persistent daemon only
  2. **Auto-spawn Mode**: One-shot CLI that spawns daemon with TTL, then daemon auto-shuts down
  3. **Require Daemon Mode**: One-shot CLI that fails if daemon is not running

**Key Features:**
- Configurable TTL (time-to-live) for auto-shutdown daemon
- Clean daemon lifecycle management
- Single binary distribution (no separate daemon.exe)
- Backward compatible with existing config and behavior

### Phase Structure

| Phase | Name | Status |
|-------|------|--------|
| 5 | Unified Daemon Architecture | 🚧 In Progress |

### Requirements

- [ ] Remove daemon binary (src/bin/daemon.rs)
- [ ] Add daemon command to main CLI
- [ ] Implement 3 operational modes
- [ ] Configurable TTL for auto-shutdown
- [ ] Update Cargo.toml (single binary)
- [ ] Test all 3 modes on Windows
- [ ] Test all 3 modes on Unix

### Acceptance Criteria

1. `mcp daemon` starts standalone daemon
2. `mcp --auto-daemon list` spawns daemon, executes command, daemon shuts down after TTL
3. `mcp --require-daemon list` fails with clear error if daemon not running
4. All existing functionality works unchanged
5. No separate daemon.exe binary exists

---
*Last updated: 2026-02-09*
