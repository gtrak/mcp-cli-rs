---
phase: 13-code-organization
plan: 04
subsystem: cli
tags:
  - refactoring
  - daemon
  - lifecycle
  - modularity

dependency_graph:
  requires:
    - 13-02 (config setup extraction)
  provides:
    - daemon_lifecycle module
  affects:
    - main.rs (reduced size)
    - CLI commands

tech_stack:
  added: []
  patterns:
    - Daemon lifecycle separation
    - Client creation abstraction

key_files:
  created:
    - src/cli/daemon_lifecycle.rs
  modified:
    - src/cli/mod.rs
    - src/main.rs

decisions: []

metrics:
  duration: "plan execution"
  completed: "2026-02-12"
  main_rs_lines_before: 809
  main_rs_lines_after: 403
  daemon_lifecycle_lines: 485
  main_rs_reduction: 406
---

# Phase 13 Plan 04: Extract daemon lifecycle from main.rs to daemon_lifecycle.rs

## Summary

Extracted daemon lifecycle management from main.rs into dedicated daemon_lifecycle.rs module.

## Tasks Completed

| Task | Name | Status |
|------|------|--------|
| 1 | Identify daemon lifecycle code in main.rs | ✓ Complete |
| 2 | Create src/cli/daemon_lifecycle.rs | ✓ Complete |
| 3 | Update src/cli/mod.rs | ✓ Complete |
| 4 | Update main.rs to use daemon_lifecycle | ✓ Complete |
| 5 | Verify compilation and tests | ✓ Complete |

## Functions Extracted

### In daemon_lifecycle.rs (485 lines)

- `create_direct_client` - Create direct client without daemon
- `create_auto_daemon_client` - Connect to daemon or spawn one if needed
- `create_require_daemon_client` - Connect to existing daemon only
- `connect_or_spawn_daemon` - Core auto-daemon connection logic
- `connect_to_daemon` - Connect to existing daemon
- `DirectProtocolClient` - ProtocolClient implementation for direct mode
- `try_connect_to_daemon` - Internal connection attempt
- `spawn_background_daemon` - Spawn daemon as background process

### Remaining in main.rs (thin wrappers)

- `run_direct_mode` - Calls create_direct_client + execute_command
- `run_auto_daemon_mode` - Calls create_auto_daemon_client + execute_command
- `run_require_daemon_mode` - Calls create_require_daemon_client + execute_command

Note: These wrapper functions remain in main.rs because they call `execute_command` which depends on the CLI module (would create circular dependency). They are thin (~8 lines each) and provide CLI-specific orchestration.

## Changes

| File | Before | After | Change |
|------|--------|-------|--------|
| main.rs | 809 lines | 403 lines | -406 lines |
| daemon_lifecycle.rs | N/A | 485 lines | +485 lines |
| cli/mod.rs | 31 lines | 33 lines | +2 lines |

**Net change:** +79 lines (new module) but main.rs reduced by 50%

## Verification

- cargo check --all-targets: ✓ PASS (0 errors)
- cargo test --no-run: ✓ PASS
- Imports in main.rs: ✓ Using daemon_lifecycle module

## Module Structure

```
src/cli/
├── mod.rs              # Declares daemon_lifecycle
├── daemon_lifecycle.rs # 485 lines - daemon client creation
├── commands.rs         # Command dispatch
├── config_setup.rs     # Config loading
├── daemon.rs           # Daemon core
└── ...
```

## Deviations from Plan

None - plan executed as specified.

The core daemon lifecycle logic is extracted. The remaining wrapper functions in main.rs are minimal and handle CLI-specific orchestration (output mode, command execution).
