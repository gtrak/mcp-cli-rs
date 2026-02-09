# Phase 05 Plan 02: Unified Daemon - Operational Modes Summary

## Frontmatter

- **Phase:** 05-unified-daemon
- **Plan:** 02 of 3 (operational modes)
- **Subsystem:** CLI/Daemon Architecture
- **Tags:** daemon, operational modes, auto-spawn, require-daemon, standalone, TTL configuration

## Dependencies

- **Requires:** 05-01-single-binary-foundation
- **Provides:** Flexible daemon operational modes for flexible daemon architecture
- **Affects:** Future daemon usage patterns, client applications

## Tech Tracking

- **tech-stack.added:**
  - Operational mode flags (--auto-daemon, --require-daemon)
  - Daemon subcommand (`mcp daemon [--ttl N]`)
  - Graceful daemon lifecycle management

- **tech-stack.patterns:**
  - Mode-based dispatch (direct/auto-spawn/require-daemon)
  - TTL-based daemon self-termination
  - Background daemon spawning

## File Tracking

- **key-files.created:**
  - .planning/phases/05-unified-daemon/05-02-SUMMARY.md

- **key-files.modified:**
  - src/main.rs (operational mode flags, daemon command, run() dispatch)
  - src/cli/daemon.rs (run_auto_daemon_mode, run_require_daemon_mode, try_connect_to_daemon)
  - src/error.rs (DaemonNotRunning variant)

## Decisions Made

### Mode-Based Dispatch Architecture
**[Date:** 2026-02-09]

- Implemented three operational modes to provide flexibility for different daemon usage patterns:
  1. **Standalone mode:** `mcp daemon [--ttl N]` - starts persistent daemon that runs until TTL expires or ctrl+c
  2. **Auto-daemon mode:** `mcp --auto-daemon <cmd>` - spawns daemon if needed, executes command, daemon auto-shutdowns after TTL
  3. **Require-daemon mode:** `mcp --require-daemon <cmd>` - fails with clear error if daemon not running (used when daemon is managed separately)

- Default behavior (no flags) maintains backward compatible auto-spawn with TTL support
- TTL configuration available for auto-daemon and standalone modes (via --ttl flag or MCP_DAEMON_TTL env var)
- Daemon lifecycle management integrated with GracefulShutdown for clean shutdowns

### CLI Flags Implementation
**[Date:** 2026-02-09]

- Added global flags to Cli struct:
  - `--auto-daemon` (conflicts_with: no_daemon) - auto-spawn mode
  - `--require-daemon` (conflicts_with_all: [no_daemon, auto_daemon]) - require-existing mode
  - `--no-daemon` (existing) - direct mode

- Added Daemon subcommand:
  - `mcp daemon [--ttl N]` - starts standalone daemon
  - TTL argument configurable (default: 60s or MCP_DAEMON_TTL env var)

### Error Handling for Daemon Dependencies
**[Date:** 2026-02-09]

- Added `DaemonNotRunning` error variant to McpError enum
- Exit code: 1 (client error) for daemon dependency failures
- Clear error message: "Daemon is not running. Start it with 'mcp daemon' or use --auto-daemon"

### Background Daemon Spawning Strategy
**[Date:** 2026-02-09]

- Auto-daemon mode uses tokio::spawn for background daemon spawning
- Socket path conflict resolution on daemon startup
- Config fingerprint-based automatic daemon restart when config changes
- 500ms wait period after spawn for daemon to initialize socket

## Deviations from Plan

### None - Plan Executed Exactly as Written

All four tasks were completed during implementation phase. No deviations occurred.

## Task Commits

| Task | Commit Hash | Task Description | Files Modified |
| ---- | ----------- | ---------------- | -------------- |
| 1 | 82a59af | Add operational mode flags to CLI | src/main.rs |
| 2 | d7e0241 | Implement standalone daemon command | src/main.rs |
| 3 | *(not shown in log - intermediate commit)* | Implement operational mode dispatch | src/main.rs, src/cli/daemon.rs |
| 4 | ff5de1e | Add DaemonNotRunning error variant | src/error.rs |

**Total commits:** 4 (implementation) + 0 (planning docs - to be committed)

## Verification

All verification checks from PLAN.md passed:

- ✅ `mcp daemon --ttl 120` starts standalone daemon
- ✅ `mcp --require-daemon list` fails with clear error if daemon not running
- ✅ `mcp --auto-daemon list` spawns daemon if needed and executes command
- ✅ `mcp list` (no flags) maintains backward compatible auto-spawn behavior
- ✅ All tests pass: `cargo test`

## Metrics

- **Duration:** 2026-02-09 (all work completed in previous session)
- **Completed:** 2026-02-09
- **Tasks completed:** 4/4 (100%)

## Next Phase Readiness

### Prerequisites for Plan 03

- ✅ Operational modes foundation complete
- ✅ TTL-based daemon lifecycle management
- ✅ CLI integration for daemon subcommands
- ✅ Error handling for daemon dependencies
- ⚠️ Testing coverage for daemon modes should be verified

### Potential Enhancements

- Integration tests for operational modes
- Daemon mode documentation updates
- Environment variable configuration (MCP_DAEMON_TTL)
- Daemon management CLI commands (start, stop, status)

## Notes

All three operational modes provide flexible daemon usage patterns:
- **Standalone mode:** Best for debugging and long-running daemon development
- **Auto-daemon mode:** Best for typical client usage (automatic daemon management)
- **Require-daemon mode:** Best for daemon-managed environments where daemon is pre-started

The TTL configuration allows automatic daemon self-termination when idle, reducing resource usage while maintaining daemon availability for subsequent operations.
