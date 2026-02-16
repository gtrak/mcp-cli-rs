---
phase: 24-linux-compatibility
plan: 01
subsystem: build
status: complete
duration: 5 min
started: 2026-02-16T06:41:07Z
completed: 2026-02-16T06:46:00Z

must_haves:
  truths_verified:
    - "cargo build compiles on Linux without dependency errors"
    - "windows-sys is only compiled on Windows targets"
    - "nix crate is available for Unix signal handling"
  artifacts_verified:
    - path: "Cargo.toml"
      verified: true
      contains: "[target.'cfg(windows)'.dependencies]"
    - path: "Cargo.toml"
      verified: true
      contains: "nix = { version = \"0.29\", features = [\"signal\", \"process\"] }"
    - path: "Cargo.lock"
      verified: true
      contains: "name = \"nix\""

dependencies:
  requires: []
  provides: ["Platform-specific dependency gating"]
  affects: ["24-02", "24-03"]

tech-stack:
  added:
    - nix 0.29 (Unix signal handling)
  modified:
    - windows-sys (moved to platform-gated section)

key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock

deviations: []
---

# Phase 24 Plan 01: Linux Compatibility - Dependency Gating

## Summary

Fixed Cargo.toml dependencies for Linux compatibility by adding platform-specific gating. Moved `windows-sys` to a Windows-only dependency section and added `nix` crate for Unix signal handling.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Move windows-sys to Windows-only dependencies | 2f54d78 | Cargo.toml |
| 2 | Add nix crate for Unix signal handling | e86aaee | Cargo.toml |
| 3 | Update Cargo.lock | 8e826d5 | Cargo.lock |

## Changes Made

### Cargo.toml

**Before:**
```toml
[dependencies]
# ... other deps
windows-sys = { version = "0.61", features = ["Win32_System_Threading"] }
# ... other deps
```

**After:**
```toml
[dependencies]
# ... other deps (no windows-sys)

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = ["Win32_System_Threading"] }

[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal", "process"] }
```

## Requirements Satisfied

- ✅ **LINUX-04**: Add nix crate dependency for Unix signal handling
- ✅ **LINUX-09**: Make windows-sys dependency Windows-only

## Verification

1. ✅ `windows-sys` is now under `[target.'cfg(windows)'.dependencies]`
2. ✅ `nix` crate is declared under `[target.'cfg(unix)'.dependencies]`
3. ✅ `cargo update` successfully added nix v0.29.0 to Cargo.lock
4. ✅ Dependencies compile without errors on Linux

## Notes

The dependency changes enable proper cross-platform compilation:
- Windows builds will include `windows-sys` for native API access
- Unix/Linux builds will include `nix` for signal handling
- Each platform only compiles its relevant dependencies

There are pre-existing code-level compilation errors in the codebase (method signature mismatches, missing match arms) that are outside the scope of this dependency configuration task and will be addressed in subsequent plans.

## Next Steps

Ready for **24-02-PLAN.md**: Fix Unix IPC implementation issues (method signatures, error handling)
