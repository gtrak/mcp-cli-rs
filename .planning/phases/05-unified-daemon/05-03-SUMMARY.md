# Plan 05-03 Summary: TTL Configuration

**Status:** Complete
**Date:** 2026-02-09
**Duration:** ~1 hour

## Overview
Added configurable TTL (time-to-live) for daemon idle timeout via CLI flag, environment variable, and config file. This completes DAEMON-03, DAEMON-10, and DAEMON-11 requirements.

## Work Completed

### Tasks Completed

**Task 1: Add daemon_ttl to Config struct (Already Done)**
- `daemon_ttl: u64` field added to Config struct (line 165)
- `default_daemon_ttl()` function returns 60 seconds (line 213-215)
- Applied `#[serde(default = "default_daemon_ttl")]` attribute

**Task 2: Update DaemonLifecycle to use configurable TTL (Already Done)**
- `DaemonLifecycle::new(idle_timeout_secs: u64)` accepts timeout parameter
- Uses configurable timeout for idle timer instead of hardcoded value
- Tests verify TTL configuration

**Task 3: Wire TTL from config to daemon initialization (Completed)**
- Changed `Commands::Daemon { ttl }` from required `u64` to `Option<u64>`
- Updated `run_standalone_daemon()` to accept `Option<u64>` parameter
- Implemented TTL resolution: CLI flag > env var > config > default (60s)
- TTL resolution logic: `cli_ttl.or_else(|| env var).or_else(|| config.daemon_ttl).unwrap_or(60)`

**Task 4: Update auto-daemon mode to use config TTL (Already Done)**
- `run_auto_daemon_mode()` uses `config.daemon_ttl` for spawned daemon
- Config loader already respects MCP_DAEMON_TTL env var

**Task 5: Add TTL configuration tests (Already Done)**
- `test_daemon_ttl_default()` - verifies default 60s value
- `test_daemon_ttl_custom()` - verifies custom TTL parsing from TOML
- `test_config_fingerprint_includes_daemon_ttl()` - verifies fingerprinting
- Fixed `Config::default()` implementation to use correct defaults instead of Rust zeros

### Key Changes

**src/main.rs:**
```rust
// Changed from required u64 to Option<u64>
Daemon {
    #[arg(short, long)]  // removed default_value
    ttl: Option<u64>,
}

// TTL resolution in run_standalone_daemon
let ttl = cli_ttl
    .or_else(|| std::env::var("MCP_DAEMON_TTL").ok().and_then(|v| v.parse().ok()))
    .unwrap_or(config.daemon_ttl);

// Changed function signature
async fn run_standalone_daemon(cli_ttl: Option<u64>) -> Result<()> {
    // ... TTL resolution then create lifecycle with ttl
}
```

**src/config/mod.rs:**
```rust
// Manual Default implementation instead of derive
impl Default for Config {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            concurrency_limit: 5,
            retry_max: 3,
            retry_delay_ms: 1000,
            timeout_secs: 1800,
            daemon_ttl: 60,  // Uses correct default
        }
    }
}
```

**src/daemon/mod.rs:**
```rust
// Updated to accept lifecycle as parameter
pub async fn run_daemon(config: Config, socket_path: PathBuf, lifecycle: DaemonLifecycle) -> Result<()>
```

## Verification

### Must-Haves Met

✅ **TTL configurable via --ttl flag (DAEMON-03)**
- `mcp daemon --ttl 120` uses 120 second TTL
- TTL is optional - if not specified, uses env var or config

✅ **TTL configurable via MCP_DAEMON_TTL env var (DAEMON-10)**
- `MCP_DAEMON_TTL=90 mcp daemon` uses 90 second TTL
- Config loader respects env var override

✅ **TTL configurable via config file daemon_ttl field (DAEMON-11)**
- Config file with `daemon_ttl = 180` is respected
- Default is 60 seconds if not specified

✅ **Default TTL is 60 seconds**
- All three default functions return 60
- Config::default() now correctly uses 60 instead of 0

### Priority Order Implemented

**TTL Resolution Priority (highest to lowest):**
1. CLI flag (`--ttl N`) - explicit override
2. Environment variable (`MCP_DAEMON_TTL=N`)
3. Config file (`daemon_ttl = N`)
4. Default (60 seconds)

### Tests Passed

✅ `test_daemon_ttl_default` - Config::default() returns 60
✅ `test_daemon_ttl_custom` - Config parses TTL from TOML
✅ `test_config_fingerprint_includes_daemon_ttl` - Fingerprinting works
✅ Build succeeds - single binary mcp-cli-rs.exe

### Binary Verification

✅ Single binary: `target/release/mcp-cli-rs.exe` (46.2 MB)
✅ No separate daemon binary
✅ `src/bin/daemon.rs` deleted

## Deviations and Notes

1. **src/bin/daemon.rs deletion:** This file still existed and caused build errors. Deleted it as part of this work (should have been done in 05-01).

2. **CLI/daemon.rs duplicates:** The functions in `src/cli/daemon.rs` (run_auto_daemon_mode, run_require_daemon_mode, etc.) were commented out as they're duplicates of the implementations in `src/main.rs`.

3. **Import cleanup:** Added missing imports to main.rs: `use std::time::Duration;` and `get_socket_path`, `create_ipc_client` from mcp_cli_rs::ipc.

4. **Config::default() fix:** Removed `#[derive(Default)]` and implemented manually to ensure daemon_ttl defaults to 60 instead of 0.

## Integration

This plan integrates with:
- **05-01:** Single binary foundation - uses daemon lifecycle from lib exports
- **05-02:** Three operational modes - TTL applies to standalone, auto-spawn, and require-daemon modes

## Commit

```
commit a7715bc
feat(05-03): add TTL configuration support
```

## Next Steps

All three plans in Phase 5 (05-01, 05-02, 05-03) are now complete. Phase verification and next steps will be determined by the orchestrator.
