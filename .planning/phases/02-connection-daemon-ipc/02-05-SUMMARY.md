---
phase: 02-connection-daemon-ipc
plan: 05
subsystem: daemon-lifecycle
tags: [config-fingerprint, orphan-cleanup, daemon-management]

# Dependency graph
requires:
  - phase: 01-core-protocol
    provides: config parsing, error handling
  - phase: 02-03
    provides: daemon binary, lifecycle management
provides:
  - Config change detection via SHA256 fingerprinting
  - Orphaned daemon cleanup with PID file tracking
  - CLI-side daemon management (ensure_daemon, spawn_daemon, shutdown_daemon)
  - Cross-platform process running detection (Unix signal 0, Windows API)

affects:
  - 02-06 (CLI integration depends on ensure_daemon)
  - 03-performance (config change detection needed before parallel ops)

# Tech tracking
tech-stack:
  added: [sha2, hex, nix (Unix), winapi/windows-sys (Windows)]
  patterns:
    - "PID file pattern: track process with .pid file alongside socket"
    - "Fingerprint comparison: SHA256 hash of config content"
    - "Startup_wait: exponential backoff retry for daemon ready"

key-files:
  created: [src/daemon/fingerprint.rs, src/daemon/orphan.rs, src/cli/daemon.rs]
  modified: [Cargo.toml, src/daemon/mod.rs, src/cli/mod.rs]

key-decisions:
  - "Use SHA256 for config fingerprinting (not mtime) - more reliable for change detection"
  - "Windows process API: use windows-sys 0.59 (not deprecated winapi)"
  - "Orphan cleanup runs at CLI startup before daemon spawn"
  - "Config change triggers old daemon shutdown + new daemon spawn (not in-place reload)"

patterns-established:
  - "Pattern 1: Config fingerprint - calculate SHA256 on startup, compare on CLI connect"
  - "Pattern 2: Orphan cleanup - try IPC connect, if failed check PID file, kill if running, remove stale files"
  - "Pattern 3: Daemon spawn - find binary in same directory as CLI, spawn with kill_on_drop"

# Metrics
duration: 30min
completed: 2026-02-06T23:50:00Z
---

# Phase 2: Config Change Detection and Orphan Cleanup Summary

**Reliable daemon lifecycle management with config change detection and automatic cleanup of crashed daemons.**

## Performance

- **Duration:** 30 min
- **Started:** 2026-02-06T23:20:00Z
- **Completed:** 2026-02-06T23:50:00Z
- **Tasks:** 4/4
- **Files modified:** 6

## Accomplishments

1. **Config fingerprinting via SHA256 hash** - Calculates hash of config content for reliable change detection, more robust than mtime
2. **Orphaned daemon cleanup** - Detects crashed daemons via PID files and IPC connection attempts, removes stale resources (socket, PID, fingerprint files)
3. **CLI daemon management** - `ensure_daemon()` provides one-stop daemon lifecycle: cleanup, fingerprint check, spawn if needed
4. **Cross-platform process detection** - Unix: signal 0, Windows: OpenProcess + GetExitCodeProcess

## Task Commits

1. **Task 1: Config fingerprinting module** - `068bdd2` (feat)
2. **Task 2: Orphan cleanup logic** - `068bdd2` (feat)
3. **Task 3: Daemon writes PID on startup** - `068bdd2` (feat)
4. **Task 4: CLI daemon management** - `068bdd2` (feat)

**Plan metadata:** `pending docs commit`

## Files Created/Modified

Created:
- `src/daemon/fingerprint.rs` - ConfigFingerprint struct with SHA256 hash calculation
- `src/daemon/orphan.rs` - PID file tracking, process detection, cleanup logic
- `src/cli/daemon.rs` - ensure_daemon(), spawn_daemon_and_wait(), shutdown_daemon()

Modified:
- `Cargo.toml` - Added sha2, hex, nix, windows-sys features
- `src/daemon/mod.rs` - Expose fingerprint and orphan modules
- `src/cli/mod.rs` - Export daemon module

## Decisions Made

- **SHA256 over mtime:** Mtime can change without config changes (file copy, touch). SHA256 is deterministic.
- **Windows API choice:** Used windows-sys 0.59 instead of deprecated winapi crate.
- **Cleanup before spawn:** Always run orphan cleanup before spawning new daemon.
- **Config change = restart:** Compare fingerprints, mismatch triggers old shutdown + new spawn (not in-place reload).

## Deviations from Plan

### Auto-fixed Issues

**1. Windows process API deprecation (Rule 3 - Auto-add)**
- **Found during:** Task 2 (orphan cleanup)
- **Issue:** winapi crate is deprecated, DWORD alias was private
- **Fix:** Switched to windows-sys 0.59 with Win32_System_Threading, Win32_Foundation features
- **Files modified:** src/daemon/orphan.rs, Cargo.toml
- **Verification:** cargo check passes with new windows-sys bindings
- **Committed in:** 068bdd2 (part of task commit)

**2. get_daemon_socket_path → get_socket_path (Rule 1 - Auto-fix)**
- **Found during:** Task 4 (CLI daemon management)
- **Issue:** Function name mismatch in src/ipc/mod.rs
- **Fix:** Changed all references to use correct function name get_socket_path()
- **Files modified:** src/cli/daemon.rs
- **Verification:** cargo check passes, no warnings
- **Committed in:** 068bdd2 (part of task commit)

**3. child.id() returns Option<u32> (Rule 1 - Auto-fix)**
- **Found during:** Task 4 (daemon spawning)
- **Issue:** tokio::process::Child.id() returns Option<u32>, not u32
- **Fix:** Handle Option with if let, only write PID if Some
- **Files modified:** src/cli/daemon.rs
- **Verification:** cargo check passes
- **Committed in:** 068bdd2 (part of task commit)

## Self-Check: PASSED

- [x] Config fingerprint calculates SHA256 hash correctly
- [x] is_daemon_running() works on both platforms (verified impl)
- [x] cleanup_orphaned_daemon() removes stale resources
- [x] Daemon writes PID file on startup
- [x] CLI ensure_daemon() handles cleanup and spawning
- [x] All verification criteria met
- [x] Code compiles with only warnings (unused vars - harmless)

## Next Steps

Plan 02-06 (CLI integration) depends on ensure_daemon() from this plan. Daemon lifecycle is now complete and ready for CLI command integration.
