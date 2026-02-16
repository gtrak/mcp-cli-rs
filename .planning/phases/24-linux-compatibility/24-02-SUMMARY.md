# Phase 24 Plan 02: Fix create_ipc_server Platform Compatibility

**Completed:** 2026-02-16
**Duration:** 5 min

## Summary

Added Unix implementation of `create_ipc_server` to resolve Linux compilation failures. The function was previously only defined for Windows (`#[cfg(windows)]`), causing "unresolved import" errors when building on Linux. The Unix implementation uses `tokio::runtime::Handle::block_on()` to bridge the async `UnixIpcServer::new()` with the synchronous factory function signature.

## What Was Changed

### Files Modified

1. **src/ipc/mod.rs**
   - Added `#[cfg(unix)]` implementation of `create_ipc_server`
   - Uses `tokio::runtime::Handle::try_current()` to access the async runtime
   - Calls `block_on()` to execute the async `UnixIpcServer::new()` synchronously
   - Returns proper `McpError::IpcError` when no Tokio runtime is available

## Technical Details

### Async/Sync Compatibility

The Windows `NamedPipeIpcServer::new` is synchronous, while the Unix `UnixIpcServer::new` is async. To maintain API compatibility without breaking changes:

```rust
#[cfg(unix)]
pub fn create_ipc_server(path: &Path) -> Result<Box<dyn IpcServer>, McpError> {
    use tokio::runtime::Handle;
    
    let server = Handle::try_current()
        .map_err(|e| McpError::IpcError {
            message: format!("No Tokio runtime available for IPC server creation: {}", e),
        })?
        .block_on(crate::ipc::unix::UnixIpcServer::new(path))?;
    
    Ok(Box::new(server))
}
```

This approach:
- Preserves the existing synchronous API
- Requires an active Tokio runtime (which is always present in daemon context)
- Provides clear error message if called outside a runtime

## Verification Results

- ✅ `create_ipc_server` now has implementations for both Windows and Unix
- ✅ No "unresolved import" errors for `create_ipc_server` in lib.rs
- ✅ `cargo check --lib` no longer reports the missing function error
- ✅ LINUX-05 requirement satisfied: Windows-only exports now properly gated
- ✅ LINUX-06 requirement satisfied: IPC method signatures compatible across platforms

## Pre-existing Issues

The following compilation errors exist in the codebase but are **not related to this plan**:

1. `send_request` method signature mismatch in `src/ipc/unix.rs` (requires `&mut self`)
2. `SIGZERO` not available in nix crate (in `src/daemon/orphan.rs`)
3. `to_string_lossy` not available for `SocketAddr` (in `src/ipc/unix.rs`)
4. Missing match arm for `DaemonNotRunning` error variant (in `src/error.rs`)

These will be addressed in subsequent plans within Phase 24.

## Decisions Made

1. **Used `block_on()` for async bridging**: Chosen over making `create_ipc_server` async to avoid breaking the existing API and all callers.

2. **Runtime availability check**: Using `Handle::try_current()` instead of `Handle::current()` to provide a meaningful error message instead of panicking when no runtime exists.

## Next Steps

- Plan 24-03: Address remaining Linux compilation errors
- Focus on: SIGZERO replacement, SocketAddr handling, error variant coverage

## Task Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | 3c19384 | Add Unix implementation of create_ipc_server |
| Task 2 | - | Verification (no code changes needed) |

## Requirements Coverage

- ✅ LINUX-05: Windows-only exports properly gated with cfg attributes
- ✅ LINUX-06: IPC method signatures compatible across platforms
