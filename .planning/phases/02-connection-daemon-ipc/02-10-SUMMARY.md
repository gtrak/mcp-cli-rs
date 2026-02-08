---
phase: 02-connection-daemon-ipc
plan: 10
type: execute
completions: 2
status: complete
gap_closure: true
---

# Plan 02-10 Summary: Implement Config Change Detection and Graceful Shutdown

## Overview

Successfully completed two TODO items that were blocking daemon lifecycle reliability: config fingerprint comparison in ensure_daemon() and graceful shutdown in shutdown_daemon(). These enable the daemon to detect config changes and restart with fresh connections, and to shutdown gracefully on request.

**Key Addition:** Added `send_request()` method to `ProtocolClient` trait to enable low-level daemon protocol communication.

## Tasks Completed

### Task 1: Implement fingerprint comparison in ensure_daemon()

- ✅ Replaced stub implementation (TODO at line 46)
- ✅ Request `DaemonRequest::GetConfigFingerprint` from existing daemon
- ✅ Compare daemon fingerprint with calculated local fingerprint
- ✅ If fingerprints match: reuse existing daemon (return client)
- ✅ If fingerprints differ: shutdown stale daemon, spawn new daemon, connect
- ✅ Handle unexpected responses gracefully (spawn new daemon)
- ✅ Handle connection errors gracefully (spawn new daemon)
- ✅ Fixed return type - return client directly (not Box::new(client)) since client is already Box<dyn ProtocolClient>

**Files modified:**
- `src/cli/daemon.rs` - Implemented fingerprint comparison logic in ensure_daemon()
- `src/ipc/mod.rs` - Added send_request() method to ProtocolClient trait

**Commit:** `feat(02-10): implement config fingerprint comparison and graceful shutdown` (combined with task 2)

---

### Task 2: Implement graceful shutdown in shutdown_daemon()

- ✅ Replaced stub implementation (TODO at line 163)
- ✅ Connect to running daemon
- ✅ Send `DaemonRequest::Shutdown` via IPC
- ✅ Wait for and confirm `DaemonResponse::ShutdownAck`
- ✅ Handle successful shutdown acknowledgment (return Ok(()))
- ✅ Handle unexpected responses (treat as success - daemon may shut down anyway)
- ✅ Handle connection errors (treat as success - daemon may already be gone)
- ✅ Removed TODO comment and placeholder logic

**Safety Considerations:**
- If daemon is already dead or connection fails, we treat as success
- We don't panic or fail - we log warnings and proceed
- The daemon will eventually shut down via idle timeout if graceful shutdown fails

**Files modified:**
- `src/cli/daemon.rs` - Implemented graceful shutdown in shutdown_daemon()

**Commit:** `feat(02-10): implement config fingerprint comparison and graceful shutdown` (combined with task 1)

---

## Key Implementation Details

### ProtocolClient Trait Extension

Added low-level protocol access method:

```rust
#[async_trait]
pub trait ProtocolClient: Send + Sync {
    fn config(&self) -> Arc<Config>;
    async fn send_request(&mut self, request: &DaemonRequest) -> Result<DaemonResponse, McpError>;
    async fn list_servers(&mut self) -> Result<Vec<String>, McpError>;
    // ... other methods
}
```

**Implementation in IpcClientWrapper:**
```rust
async fn send_request(&mut self, request: &DaemonRequest) -> Result<DaemonResponse, McpError> {
    self.client.send_request(request).await
}
```

This enables daemon management operations (fingerprint query, shutdown) without adding specific methods for each to the high-level trait.

### Fingerprint Comparison Flow

```
1. Calculate local fingerprint from current config
2. Try to connect to existing daemon
3. Request GetConfigFingerprint from daemon
4. Compare daemon fingerprint with local fingerprint

Match? → Reuse existing daemon
Mismatch? → Shutdown stale → Spawn new → Connect new
Error/Timeout? → Treat as stale → Spawn new → Connect new
```

**Error Handling Philosophy:** If we can't reliably validate the existing daemon's config, we treat it as stale and spawn a new one. This is safer than potentially using a daemon with stale config.

### Graceful Shutdown Flow

```
1. Connect to daemon
2. Send Shutdown request
3. Wait for ShutdownAck

ShutdownAck? → Success (Ok(()))
Other response? → Success (Ok(())) - daemon may shut down anyway
Connection error? → Success (Ok(())) - daemon already gone
```

**Non-blocking Behavior:** We don't wait for the daemon to actually terminate - we just confirm it received the shutdown request. The daemon will clean up on its own (set shutdown flag in lifecycle).

### Daemon Lifecycle Updates

When config changes:
1. ensure_daemon() detects fingerprint mismatch
2. shutdown_daemon() sends Shutdown request
3. Daemon's handle_request() receives Shutdown, sets state.shutdown()
4. Daemon's main loop detects shutdown flag, breaks out
5. Daemon cleans up (removes socket, PID file, fingerprint file)
6. Daemon exits
7. CLI spawns fresh daemon with new config
8. New daemon establishes fresh connections to MCP servers

This ensures config changes are applied without needing to manually restart the daemon.

## Dependencies

This plan depends on:
- 02-08: IPC NDJSON protocol - enables send_request() communication
- 02-09: Daemon request handlers - enables Shutdown handler in daemon
- Config fingerprint calculation (02-05) - provides fingerprint comparison logic

## Closed Gaps

**Gap 3 from VERIFICATION.md: Config Fingerprint Comparison Not Implemented (Blocker)**

Before:
- ensure_daemon() had TODO at line 46 - "Request fingerprint from daemon and compare"
- Function assumed existing daemon was good without validating
- No config change detection - stale daemon persisted even after config changes
- shutdown_daemon() had TODO at line 163 - "Send DaemonRequest::Shutdown through client"
- Function just disconnected, daemon would idle timeout

After:
- ensure_daemon() actively validates daemon's config fingerprint
- Mismatch fingerprints trigger daemon restart with fresh connections
- shutdown_daemon() sends graceful Shutdown request via IPC
- Waits for ShutdownAck confirmation
- Config changes now trigger automatic daemon restart
- Daemon lifecycle is explicit and managed, not just idle timeout

**Gap 4 from VERIFICATION.md: Missing and Broken IPC Tests**

Indirectly addressed - this plan enables proper daemon lifecycle testing which is a prerequisite for IPC integration tests in plan 02-11.

## Testing Notes

Code compiles successfully. Fingerprint comparison and graceful shutdown logic are in place. These features depend on:
- IPC protocol working (from 02-08)
- Daemon request handlers working (from 02-09)

End-to-end testing of config change detection and graceful shutdown will be part of integration tests in plan 02-11.

## Self-Check

- [x] ensure_daemon() requests GetConfigFingerprint from existing daemon
- [x] ensure_daemon() compares fingerprints with calculated local fingerprint
- [x] ensure_daemon() shuts down stale daemon on mismatch
- [x] ensure_daemon() spawns new daemon with fresh connections
- [x] ensure_daemon() returns existing client if fingerprints match
- [x] shutdown_daemon() sends DaemonRequest::Shutdown
- [x] shutdown_daemon() waits for ShutdownAck
- [x] shutdown_daemon() handles errors gracefully (daemon may be gone)
- [x] No TODO comments remaining in either function
- [x] Code compiles without errors
- [x] send_request() method added to ProtocolClient trait

## Next Steps

Plan 02-11 fixes test compilation errors and creates missing IPC integration tests. This is the final gap closure plan for Phase 2.
