# Comprehensive Test Coverage Plan: Restore Deleted Features

**Created:** 2026-02-11
**Priority:** High
**Status:** Planning Complete ✅

## Executive Summary

During recent refactoring, extensive test coverage and functionality was removed (1574 lines deleted in daemon/orphan/lifecycle/fingerprint code). Current tests show 61 passing but IPC communication tests are failing (3/3 failures). Goal: Restore comprehensive test coverage and functionality for Phase 2 daemon architecture before proceeding to Phase 7.

**Status:** Planning Complete ✅ - Ready for execution

---

## Deletion Analysis

### Files Deleted/Heavily Modified

| File | Lines Deleted | Impact |
|------|--------------|--------|
| src/daemon/fingerprint.rs | 140 lines | SHA256 config fingerprinting removed |
| src/daemon/orphan.rs | 296 lines | Orphaned daemon cleanup removed |
| src/daemon/lifecycle.rs | 160 lines | Lifecycle management simplified |
| src/daemon/mod.rs | 90 lines | Core daemon logic removed |
| src/config/mod.rs | 63 lines | Config structure changed |
| tests/daemon_lifecycle_tests.rs | 568→34 lines | Tests reduced 94% |
| tests/daemon_mode_test.rs | 33→0 lines | Tests removed |

### Current Test Status
- **Passing:** 61 tests (unit tests, protocol tests, basic IPC)
- **Failing:** 3 IPC tests (roundtrip, large transfer, concurrent)
- **Ignored:** 1 integration test (requires running daemon)
- **Missing:** No tests for fingerprinting, orphan cleanup, lifecycle restart

---

## Critical Missing Functionality

### 1. Config Fingerprinting (Phase 2 Requirement)
**Requirement:** CONN-06 - Daemon detects configuration changes and spawns new daemon with fresh connections when config becomes stale.

**Current State:** Completely removed (140 lines deleted)

**Required Tests:**
- `test_config_fingerprint_basic()` - SHA256 hash calculation
- `test_config_fingerprint_changes()` - Hash changes when config changes
- `test_config_fingerprint_files()` - Hash includes config file content
- `test_daemon_fingerprint_detection()` - Client detects stale daemon
- `test_auto_daemon_restart_on_config_change()` - Daemon restarts when hash mismatch

**Implementation Needed:**
```rust
pub fn config_fingerprint(config: &Config) -> String {
    // SHA256 hash of current config content
    // Checksum includes: servers list, timeouts, retry settings
}

pub fn config_hash_changed(old_fingerprint: &str, new_config: &Config) -> bool {
    // Compare fingerprints to detect changes
}
```

### 2. Orphan Cleanup (Phase 2 Requirement)
**Requirement:** CONN-07 - Orphaned daemon processes and sockets cleaned up on startup.

**Current State:** Completely removed (296 lines deleted)

**Required Tests:**
- `test_orphan_socket_cleanup_unix()` - Removes stale Unix sockets
- `test_orphan_socket_cleanup_windows()` - Removes stale named pipes
- `test_orphan_pid_file_cleanup()` - Removes stale PID files
- `test_orphan_fingerprint_file_cleanup()` - Removes stale fingerprint files
- `test_no_false_positives()` - Doesn't remove active daemons
- `test_pid_file_validation()` - Validates PID file content

**Implementation Needed:**
```rust
pub async fn cleanup_orphaned_daemon(socket_path: &Path) -> Result<()> {
    // Detect orphaned daemon by PID existence
    // Verify daemon is stale via fingerprint
    // Kill process and cleanup resources
}
```

### 3. Daemon Lifecycle Management
**Requirement:** CONN-05 - Idle timeout (60s) and self-termination after inactivity.

**Current State:** Simplified but incomplete

**Required Tests:**
- `test_idle_timeout_triggers()` - Daemon terminates after 60s idle
- `test_idle_timeout_with_activity()` - Timeout resets on activity
- `test_daemon_lifecycle_graceful_shutdown()` - SIGTERM handled properly
- `test_lifecycle_manager_concurrency()` - Multiple concurrent connections
- `test_lifecycle_state_transition()` - Proper state machine flow

**Implementation Needed:**
```rust
pub async fn run_idle_timer(lifecycle: &DaemonLifecycle) -> Result<()> {
    // Monitor activity
    // Trigger shutdown after TTL
    // Handle SIGTERM gracefully
}
```

### 4. IPC Communication (Broken)
**Current State:** 3 tests failing

**Failing Tests:**
1. `ipc_tests::test_ipc_roundtrip` - ConnectionError: NotFound
2. `ipc_tests::test_large_message_transfer` - ConnectionError: NotFound
3. `ipc_tests::test_concurrent_connections` - ConnectionError: NotFound

**Root Cause:**
- Test creates server at custom path `mcp-test-{pid}.sock`
- `create_ipc_client` expects daemon at default path `.mcp-cli/daemon.sock`
- Paths don't match → connection fails

**Fix Needed:**
- Update tests to use `get_socket_path()` instead of custom path
- Or ensure daemon path is configurable
- Add path validation in client connection logic

---

## Comprehensive Test Coverage Plan

### Phase A: Restore Core Daemon Tests (Priority: High)

**Files to Restore/Create:**
1. **tests/config_fingerprint_tests.rs**
   - Tests: 6 new tests
   - Lines: ~200
   - Status: Not existent → Need to create

2. **tests/orphan_cleanup_tests.rs**
   - Tests: 6 new tests
   - Lines: ~250
   - Status: Not existent → Need to create

3. **tests/lifecycle_tests.rs**
   - Tests: 5 new tests
   - Lines: ~200
   - Status: Reduced from 568 to 34 lines → Need to restore

**Target:** 17 comprehensive daemon tests passing

---

### Phase B: Fix Broken IPC Tests (Priority: High)

**File: tests/ipc_tests.rs**
- Fix custom socket path to use `get_socket_path()`
- Add connection timeout handling
- Add cleanup verification
- Add error recovery

**Expected Result:** All 3 failing tests pass

---

### Phase C: Integration Tests (Priority: Medium)

**File: tests/integration_daemon_test.rs**
- End-to-end daemon lifecycle
- Config change detection
- Orphan cleanup in real scenario
- Idle timeout behavior

**Target:** 5 integration tests

---

### Phase D: Cross-Platform Validation (Priority: Medium)

**File: tests/cross_platform_daemon_tests.rs**
- Windows named pipe operations (already has 10 tests)
- Unix socket operations (currently 0 tests)
- Concurrent connections (already has 1 test, need 3 more)
- Message size limits (already has 1 test, need 2 more)

**Target:** 20 total platform-specific tests

---

## Test Coverage Matrix

| Feature | Unit Tests | Integration Tests | Platform Tests | Total | Status |
|---------|-----------|-------------------|----------------|-------|--------|
| Config Fingerprinting | 6 | 0 | 0 | 6 | ❌ Missing |
| Orphan Cleanup | 6 | 1 | 0 | 7 | ❌ Missing |
| Lifecycle Management | 5 | 2 | 0 | 7 | ⚠️ Partial |
| IPC Communication | 4 | 0 | 16 | 20 | ✅ Partial |
| Cross-Platform | 0 | 0 | 20 | 20 | ⚠️ Partial |
| Tool Discovery/Filtering | 5 | 3 | 0 | 8 | ✅ Complete |
| Config Parsing | 6 | 2 | 0 | 8 | ✅ Complete |

**Current Total:** 61 passing tests
**Target Total:** 80+ comprehensive tests
**Gap:** 19 tests needed to reach target

---

## Implementation Strategy

### Week 1: Core Daemon Tests
**Monday:** Create config_fingerprint_tests.rs with 6 tests
**Tuesday:** Create orphan_cleanup_tests.rs with 6 tests
**Wednesday:** Restore lifecycle_tests.rs from git history (568→full)
**Thursday:** Run tests, fix any failures
**Friday:** Verify all 17 daemon tests pass

### Week 2: Fix IPC Tests & Integration
**Monday:** Fix 3 failing IPC tests (path resolution, connection errors)
**Tuesday:** Create integration_daemon_test.rs with 5 E2E scenarios
**Wednesday:** Add retry logic, timeout handling tests
**Thursday:** Full test suite run, document results
**Friday:** Code review, finalize Phase A completion

### Week 3: Cross-Platform & Coverage
**Monday:** Add Unix socket tests to cross_platform_daemon_tests.rs
**Tuesday:** Add concurrent connection tests
**Wednesday:** Add message size limit tests
**Thursday:** Run platform-specific test suite
**Friday:** Final test coverage report

---

## Success Criteria

✅ **All 80+ tests passing**
✅ **Config fingerprinting functional**
✅ **Orphan cleanup tested**
✅ **Daemon lifecycle verified**
✅ **IPC communication stable**
✅ **Cross-platform tested**
✅ **No zombie processes on Windows/Linux**
✅ **Test coverage >85% of core functionality**

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Async/await complexity | Medium | High | Use tokio::time test utilities, add logging |
| Cross-platform incompatibility | Medium | Medium | Test on both Windows and Linux CI |
| Memory leaks in tests | Low | High | Use tokio::test for proper cleanup |
| Race conditions in concurrent tests | Low | Medium | Use timeouts, mutexes, proper async handling |

---

## Next Steps

1. **Create detailed PLAN document for Phase 2 Restore**
2. **Restore config_fingerprint_tests.rs**
3. **Restore orphan_cleanup_tests.rs**
4. **Restore lifecycle_tests.rs**
5. **Fix 3 failing IPC tests**
6. **Run full test suite**
7. **Document coverage metrics**

---

**Estimated Effort:** 3-5 days of focused development
**Estimated Lines of Code:** ~800 lines of new tests
**Target Completion:** Before proceeding to Phase 7
