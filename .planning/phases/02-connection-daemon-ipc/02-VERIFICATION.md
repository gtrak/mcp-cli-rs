---
phase: 02-connection-daemon-ipc
verified: 2026-02-07T15:30:00Z
status: gaps_found
score: 2/6 must-haves verified

gaps:
  - truth: "Daemon automatically spawns on first tool execution and self-terminates after 60 seconds of idle time"
    status: partial
    reason: "Daemon spawning and lifecycle infrastructure exists and compiles, but end-to-end functionality cannot be verified because IPC communication layer (send_request) is not implemented"
    artifacts:
      - path: "src/ipc/unix.rs"
        issue: "send_request() is a stub returning 'NDJSON protocol not implemented yet' error. There are two duplicate implementations (lines 91-102 and 105-114), both stubs."
      - path: "src/ipc/windows.rs"
        issue: "send_request() is a stub returning 'NDJSON protocol not implemented for Windows named pipes yet' error (line 98-107)."
    missing:
      - "NDJSON protocol implementation in UnixIpcClient::send_request() - must serialize request, write to socket, read response, deserialize"
      - "NDJSON protocol implementation in NamedPipeIpcClient::send_request() - must serialize request, write to pipe, read response, deserialize"
      - "Remove duplicate send_request implementation in src/ipc/unix.rs"
      - "Integration test to verify daemon can be spawned and responds to ping request"

  - truth: "First tool execution spawns daemon, subsequent calls reuse cached connections"
    status: failed
    reason: "Even if IPC layer worked, daemon request handlers are stub implementations that return 'not yet implemented' errors"
    artifacts:
      - path: "src/daemon/mod.rs"
        issue: "handle_request() for ExecuteTool returns Error with message 'ExecuteTool not yet implemented' (line 215-217)"
      - path: "src/daemon/mod.rs"
        issue: "handle_request() for ListTools returns Error with message 'ListTools not yet implemented' (line 224-226)"
      - path: "src/daemon/mod.rs"
        issue: "handle_request() for ListServers returns Error with message 'ListServers not yet implemented' (line 233-235)"
    missing:
      - "Implement ExecuteTool handler using connection_pool.get() to get transport, send request via transport, return result"
      - "Implement ListTools handler using connection_pool.get() to get transport, send tools/list request, return tool list"
      - "Implement ListServers handler to return list of configured server names from config"

  - truth: "Daemon detects configuration changes and spawns new daemon with fresh connections"
    status: failed
    reason: "Config fingerprinting infrastructure exists but comparison logic not implemented in ensure_daemon()"
    artifacts:
      - path: "src/cli/daemon.rs"
        issue: "Line 46 has TODO comment: 'TODO: Request fingerprint from daemon and compare'. Function assumes existing daemon is good without validating."
      - path: "src/cli/daemon.rs"
        issue: "shutdown_daemon() function at line 154 has TODO comment at line 163: 'TODO: Send DaemonRequest::Shutdown through client'. Current implementation just disconnects."
    missing:
      - "Implement fingerprint comparison in ensure_daemon(): request fingerprint from existing daemon, compare with calculated fingerprint, shutdown if stale"
      - "Complete shutdown_daemon() implementation to send DaemonRequest::Shutdown instead of just disconnecting"
      - "Integration test to verify config change triggers daemon restart"

  - truth: "Orphaned daemon processes and sockets are cleaned up on startup"
    status: verified
    reason: "Orphan cleanup infrastructure is complete and substantive"
    artifacts:
      - path: "src/daemon/orphan.rs"
        status: " substantive"
        details: "cleanup_orphaned_daemon() fully implemented with PID checking, socket removal, process killing. is_daemon_running() works on both Unix (signal 0) and Windows (GetExitCodeProcess)."
    missing: []

  - artifact: "tests/ipc_tests.rs does not exist"
    status: missing
    reason: "Plan 02-06 specified creating tests/ipc_tests.rs for IPC roundtrip and concurrent connection tests, but this file was never created"
    missing:
      - "Create tests/ipc_tests.rs with test_ipc_roundtrip(), test_concurrent_connections(), test_large_message_transfer()"

  - artifact: "Integration tests fail to compile"
    status: failed
    reason: "cargo test --lib fails with 11 compilation errors in test code"
    artifacts:
      - path: "src/client/stdio.rs"
        issue: "test_write_json() at line 219 uses .await but is not marked async (line 231)"
      - path: "Multiple test files"
        issue: "Various compilation errors: E0405 (use of undeclared type), E0425 (cannot find value), E0428 (wrong number of type arguments), E0728 (await outside async)"
    missing:
      - "Fix test compilation errors in stdio.rs and other test files"
      - "Ensure all integration tests pass"

# Phase 2: Connection Daemon & Cross-Platform IPC Verification Report

**Phase Goal:** Users experience significant performance improvement on repeated tool calls through an intelligent connection daemon that manages persistent connections across CLI invocations.
**Verified:** 2026-02-07T15:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth                                                                 | Status     | Evidence |
| --- | --------------------------------------------------------------------- | ---------- | -------- |
| 1   | Daemon automatically spawns on first tool execution and self-terminates after 60 seconds | ⚠️ PARTIAL | Spawning infrastructure exists, lifecycle works, IPC daemon compiles, but end-to-end cannot be verified - IPC send_request not implemented |
| 2   | First tool execution spawns daemon, subsequent calls reuse cached connections | ✗ FAILED   | Daemon request handlers (ExecuteTool, ListTools, ListServers) are stubs returning "not yet implemented" |
| 3   | Daemon detects configuration changes and spawns new daemon with fresh connections | ✗ FAILED   | Fingerprinting exists, but ensure_daemon() has TODO comment at line 46 - fingerprint comparison not implemented, shutdown_daemon() incomplete |
| 4   | Orphaned daemon processes and sockets are cleaned up on startup                | ✓ VERIFIED | cleanup_orphaned_daemon() fully implemented with PID tracking, process killing on both platforms, socket/file removal |

**Score:** 2/6 must-haves verified (1 partial, 3 failed)

### Required Artifacts

| Artifact                     | Expected                             | Status                   | Details |
| ---------------------------- | ------------------------------------ | ------------------------ | ------- |
| `src/ipc/unix.rs`           | Unix socket IPC implementation      | ✗ STUB                  | Connection logic exists, but `send_request()` returns "NDJSON protocol not implemented yet" - two duplicate implementations (bug) |
| `src/ipc/windows.rs`        | Windows named pipe IPC implementation | ✗ STUB                  | Connection logic exists, but `send_request()` returns "NDJSON protocol not implemented for Windows named pipes yet" |
| `src/cli/daemon.rs`         | ensure_daemon with lifecycle management | ⚠️ PARTIAL            | Spawning, orphan cleanup exist, but fingerprint comparison TODO at line 46, shutdown incomplete TODO at line 163 |
| `src/daemon/orphan.rs`      | cleanup_orphaned_daemon function    | ✓ VERIFIED              | Full implementation with PID tracking, cross-platform process detection, file cleanup |
| `src/daemon/fingerprint.rs` | calculate_fingerprint function      | ✓ VERIFIED              | SHA256-based fingerprinting with ConfigFingerprint struct and comparison methods |
| `src/daemon/mod.rs`         | Daemon main loop and request handlers | ✗ STUB                  | Main loop and lifecycle work, but ExecuteTool, ListTools, ListServers handlers return "not yet implemented" |
| `src/daemon/lifecycle.rs`   | Idle timeout management              | ✓ VERIFIED              | DaemonLifecycle with 60s timeout, activity tracking, run_idle_timer() background task |
| `src/daemon/pool.rs`        | Connection pool with health checks   | ✓ VERIFIED              | ConnectionPool with health checks, failure counting, ConnectionPoolInterface for mocking |
| `src/daemon/protocol.rs`    | CLI-daemon communication protocol   | ✓ VERIFIED              | DaemonRequest/DaemonResponse enums, NDJSON send/receive helpers |
| `src/bin/daemon.rs`         | Daemon binary entry point            | ✓ VERIFIED              | main() with config loading, socket path, run_daemon() call |
| `Cargo.toml`                | Binary entries and dependencies      | ✓ VERIFIED              | `[[bin]]` entry for mcp-daemon, interprocess dependency present |
| `tests/daemon_tests.rs`     | Daemon lifecycle tests              | ✗ PARTIAL              | Created but fails to compile - multiple test compilation errors |
| `tests/ipc_tests.rs`        | IPC communication tests             | ✗ MISSING               | Specified in plan 02-06 but never created |

### Key Link Verification

| From                     | To                      | Via                      | Status | Details |
| ------------------------ | ----------------------- | ------------------------ | ------ | ------- |
| `src/cli/daemon.rs::ensure_daemon` | `src/daemon/orphan.rs::cleanup_orphaned_daemon` | Function call | ✓ WIRED | ensure_daemon calls cleanup_orphaned_daemon at line 33 |
| `src/cli/daemon.rs::ensure_daemon` | `src/daemon/fingerprint.rs::calculate_fingerprint` | Function call | ✓ WIRED | ensure_daemon calls calculate_fingerprint at line 39 |
| `src/cli/daemon.rs::ensure_daemon` | `src/ipc/mod.rs::create_ipc_client` | Function call | ✓ WIRED | connect_to_daemon calls create_ipc_client at line 119 |
| `src/ipc/mod.rs::create_ipc_client` | `src/ipc/unix.rs::UnixIpcClient` | Factory function | ✓ WIRED | Unix factory creates UnixIpcClient at line 180 |
| `src/ipc/mod.rs::create_ipc_client` | `src/ipc/windows.rs::NamedPipeIpcClient` | Factory function | ✓ WIRED | Windows factory creates NamedPipeIpcClient at line 189 |
| `src/main.rs`            | `src/cli/daemon.rs::ensure_daemon` | Function call | ✓ WIRED | main.rs calls ensure_daemon at line 88 with Arc<Config> |
| `src/cli/commands.rs`    | `src/ipc/mod.rs::ProtocolClient` | Trait usage | ✓ WIRED | Commands use Box<dyn ProtocolClient> for daemon communication |
| `src/daemon/mod.rs::handle_request` | `src/daemon/pool.rs` | Connection pool | ⚠️ PARTIAL | DaemonState has connection_pool field, but handlers don't use it (stubs) |
| `src/ipc/unix.rs::send_request` | Actual NDJSON protocol | Implementation | ✗ NOT_WIRED | send_request is stub - no socket write/read logic |
| `src/ipc/windows.rs::send_request` | Actual NDJSON protocol | Implementation | ✗ NOT_WIRED | send_request is stub - no pipe write/read logic |

### Requirements Coverage

**REQUIREMENTS.md mapped to this phase:**
(checked ROADMAP.md - requirements are listed but not mapped to phase with phase numbers, unable to do comprehensive requirements coverage check)

### Anti-Patterns Found

| File                    | Line | Pattern                     | Severity | Impact |
| ----------------------- | ---- | -------------------------- | -------- | ------ |
| `src/ipc/unix.rs`       | 91   | TODO comment indicating stub | 🛑 Blocker | IPC protocol not implemented - prevents all CLI-daemon communication |
| `src/ipc/unix.rs`       | 105  | Duplicate function definition | 🛑 Blocker | Two send_request implementations, both stubs - indicates copy-paste error |
| `src/ipc/unix.rs`       | 100  | NotImplemented error       | 🛑 Blocker | Returns NotImplemented instead of doing work |
| `src/ipc/windows.rs`    | 98   | TODO comment indicating stub | 🛑 Blocker | IPC protocol not implemented on Windows |
| `src/ipc/windows.rs`    | 105  | NotImplemented error       | 🛑 Blocker | Returns NotImplemented instead of doing work |
| `src/daemon/mod.rs`     | 214  | "not yet implemented" error | 🛑 Blocker | ExecuteTool handler returns error - prevents tool execution via daemon |
| `src/daemon/mod.rs`     | 223  | "not yet implemented" error | 🛑 Blocker | ListTools handler returns error - prevents listing tools via daemon |
| `src/daemon/mod.rs`     | 232  | "not yet implemented" error | 🛑 Blocker | ListServers handler returns error - prevents listing servers via daemon |
| `src/cli/daemon.rs`     | 46   | TODO comment               | ⚠️ Warning | Fingerprint comparison not implemented - stale daemon not detected |
| `src/cli/daemon.rs`     | 163  | TODO comment               | ⚠️ Warning | Shutdown request not sent - daemon not gracefully shut down |
| `src/client/stdio.rs`    | 231  | await outside async function | ⚠️ Warning | Test compilation error - blocks test execution |

### Human Verification Required

### 1. End-to-End Daemon Lifecycle Test

**Test:** Run CLI command to execute a tool and verify daemon is spawned
```bash
cargo build --release --bin mcp-cli-rs --bin mcp-daemon
./target/release/mcp-cli-rs tool ls
```
**Expected:**
- Daemon process appears in process list (e.g., `ps aux | grep mcp-daemon` on Unix, `tasklist | findstr mcp-daemon` on Windows)
- Tools are listed successfully
- Socket file created on Unix: `ls -la /run/user/$UID/mcp-cli/` or `/tmp/mcp-cli-$UID/`

**Why human:** Cannot verify programmatically - requires observing actual process state and file system. Also, system will fail due to stub implementations - need human to observe failure mode.

### 2. Idle Timeout Verification

**Test:** Start daemon, wait 65+ seconds without activity, verify it terminates
```bash
# Start daemon manually
./target/release/mcp-daemon &
DAEMON_PID=$!

# Wait 65 seconds
sleep 65

# Check if process still exists
ps -p $DAEMON_PID
```
**Expected:**
- Process no longer exists (ps command returns error)
- Socket file cleaned up

**Why human:** Requires waiting and checking process state over time. Also, cannot currently work due to IPC stubs blocking.

### 3. Config Change Detection

**Test:** Start daemon, modify config file, run CLI command, verify new daemon spawned
```bash
# First CLI call (spawns daemon A)
./target/release/mcp-cli-rs tool ls
DAEMON_PID_1=$(pgrep mcp-daemon | head -1)

# Modify config file
echo "# Modified" >> ~/.mcp-cli/config.toml

# Second CLI call
./target/release/mcp-cli-rs tool ls
DAEMON_PID_2=$(pgrep mcp-daemon | head -1)

# Verify different PID
echo "PID 1: $DAEMON_PID_1, PID 2: $DAEMON_PID_2"
```
**Expected:**
- Different PIDs (new daemon spawned)
- No "daemon already running" warning

**Why human:** Requires modifying config and observing process behavior. Also, fingerprint comparison not implemented - expect this to fail.

### 4. Performance Improvement Measurement

**Test:** Measure time difference between first and repeated tool calls
```bash
# First call (spawns daemon, creates connections)
time ./target/release/mcp-cli-rs tool exec server tool '{}'

# Second call (reuses daemon and cached connections)
time ./target/release/mcp-cli-rs tool exec server tool '{}'

# Third call
time ./target/release/mcp-cli-rs tool exec server tool '{}'
```
**Expected:**
- First call: 2-5 seconds (daemon spawn + process creation + HTTP handshake)
- Subsequent calls: <1 second (cached connection reuse)
- 50%+ improvement on second/third calls vs first

**Why human:** Requires measuring actual execution time and observing performance improvement. Also, cannot work currently due to stub implementations.

### 5. Cross-Platform IPC Verification

**Test:** Verify platform-specific IPC artifacts
**Unix:**
```bash
# Check socket file exists and has correct permissions
ls -la /run/user/$UID/mcp-cli/daemon.sock
stat /run/user/$UID/mcp-cli/daemon.sock

# Check pipe name format (Linux)
echo "Pipe: $(~/.mcp-cli/daemon.sock)"
```

**Windows:**
```powershell
# Check named pipe exists
Get-ChildItem \\.\pipe\ | Where-Object { $_.Name -like "*mcp-cli*" }
```

**Expected:**
- Unix: Socket file exists with correct path format (XDG_RUNTIME_DIR or /tmp/mcp-cli-$UID/)
- Windows: Named pipe exists with format \\\.\pipe\\\mcp-cli-daemon-pid

**Why human:** Platform-specific verification requires manual inspection of system resources.

### Gaps Summary

**Critical Gaps Blocking Goal Achievement:**

1. **IPC NDJSON Protocol Not Implemented (Blocker)**
   - Both Unix and Windows client `send_request()` methods are stubs
   - No actual serialization, socket_write/read, deserialization logic
   - This prevents ANY communication between CLI and daemon
   - Impact: Entire daemon system non-functional despite complete infrastructure

2. **Daemon Request Handlers Are Stubs (Blocker)**
   - ExecuteTool, ListTools, ListServers all return "not yet implemented"
   - Connection pool exists but not used by request handlers
   - Even if IPC worked, daemon would return errors for all useful operations
   - Impact: Daemon cannot do anything useful - just runs and waits

3. **Config Fingerprint Comparison Not Implemented (Blocker)**
   - ensure_daemon() has TODO comment at line 46 - doesn't actually compare fingerprints
   - shutdown_daemon() incomplete - doesn't send graceful shutdown request
   - Impact: Stale daemon not detected/removed, config changes don't trigger restart

**Secondary Gaps:**

4. **Missing IPC Tests**
   - tests/ipc_tests.rs specified in plan 02-06 but never created
   - No integration tests for IPC roundtrip, concurrent connections, large messages

5. **Test Compilation Failures**
   - cargo test --lib fails with 11 compilation errors
   - stdio.rs test has async/await mismatch, various type errors

**Root Cause Analysis:**

The phase has:
- ✅ Excellent infrastructure (IPC traits, lifecycle, pool, fingerprinting, orphan cleanup)
- ✅ All artifacts exist and build (both binaries compile successfully)
- ✅ Most code is substantive (not just placeholder files)
- ✅ Code is well-structured and follows the planned architecture

But it's missing:
- ❌ The actual protocol implementation that makes the infrastructure useful
- ❌ The request handlers that make the daemon functional
- ❌ The config comparison logic needed for change detection

This appears to be the classic "infrastructure complete, implementation pending" situation. The design is sound, all the pieces are wired together correctly, but the critical paths that do the actual work have TODO/comments/stub implementations instead of real code.

**Why These Gaps Matter for the Phase Goal:**

The phase goal states: "Users experience significant performance improvement on repeated tool calls through an intelligent connection daemon that manages persistent connections across CLI invocations."

With the current gaps:
- Users cannot experience ANY tool calls through the daemon (IPC not implemented)
- No performance improvement possible (nothing works to measure)
- "Intelligent" daemon not possible (config change detection not implemented)

The infrastructure enables the goal, but the gaps prevent achieving it.

---

_Verified: 2026-02-07T15:30:00Z_
_Verifier: Claude (gsd-verifier)_
