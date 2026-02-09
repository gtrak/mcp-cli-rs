# Phase 5: Unified Daemon Architecture - Context

**Gathered:** 2026-02-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Refactor the MCP CLI from a two-binary architecture (CLI + daemon) to a unified single binary with three operational modes. This simplifies deployment and provides flexibility in how users run the tool.

**Scope:**
- Remove src/bin/daemon.rs separate binary
- Integrate daemon functionality into main CLI
- Implement 3 operational modes
- Maintain backward compatibility where possible
- Update build configuration (Cargo.toml)

</domain>

<decisions>
## Implementation Decisions

### Architecture
- Single binary: mcp-cli-rs.exe (or mcp)
- Daemon logic becomes a library module called by main CLI
- Three explicit modes instead of implicit behavior

### Operational Modes

**Mode 1: Standalone Daemon**
- Command: `mcp daemon [--ttl <seconds>]`
- Behavior: Run as persistent background daemon
- Use case: Long-running sessions, shared daemon across multiple CLI invocations
- If TTL specified, daemon shuts down after period of inactivity

**Mode 2: Auto-spawn (One-shot with daemon)**
- Command: `mcp --auto-daemon [--ttl <seconds>] <command>`
- Behavior: 
  1. Check if daemon running
  2. If not, spawn daemon with specified TTL
  3. Execute command
  4. Daemon auto-shuts down after TTL of inactivity
- Use case: Best performance without manual daemon management
- Default TTL: 60 seconds (configurable)

**Mode 3: Require Daemon (One-shot, explicit dependency)**
- Command: `mcp --require-daemon <command>`
- Behavior:
  1. Check if daemon running
  2. If not, fail with clear error message
  3. If yes, execute command
- Use case: Scripts that need guaranteed daemon, CI/CD pipelines
- Error message: "Daemon not running. Start with: mcp daemon"

### Default Behavior (no flags)
- Current behavior: Try to use daemon, spawn if not running
- This becomes: Mode 2 with default TTL (auto-spawn)
- No breaking change for existing users

### TTL Configuration
- Environment variable: `MCP_DAEMON_TTL` (seconds)
- Command-line flag: `--daemon-ttl <seconds>`
- Config file: `daemon_ttl` field
- Priority: CLI flag > env var > config file > default (60s)

### Claude's Discretion
- Exact implementation of daemon command parsing
- How to handle concurrent daemon spawns (race condition)
- Signal handling coordination between modes
- Logging/verbosity in different modes

</decisions>

<specifics>
## Specific Ideas

**From user discussion:**
- "Remove the separate daemon binary"
- "3 modes: 1. spawn standalone, 2. one-shot with auto-spawn and TTL, 3. one-shot requiring existing daemon"
- TTL should be configurable

**Error messages:**
- Mode 3 failure: Clear, actionable error telling user how to start daemon
- Mode 2: Silent daemon spawn (unless verbose)

</specifics>

<deferred>
## Deferred Ideas

- Daemon status command (check if running, get PID) — future enhancement
- Daemon restart command — can be done manually (stop + start)
- Multiple daemon instances — out of scope, single daemon per user

</deferred>

---
*Phase: 05-unified-daemon*
*Context gathered: 2026-02-09*
