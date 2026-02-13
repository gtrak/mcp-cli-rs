---
phase: 17
type: gap_closure
plan: 04
subsystem: test_infrastructure
tags: [testing, http, mock-server, parallel-execution, race-conditions]
requires: [17-03]
provides: [http-test-stability]
affects: [17-05, future-test-suites]
tech-stack:
  added: []
  patterns: [parameterized-configuration, test-isolation]
key-files:
  created: []
  modified:
    - tests/fixtures/mock_http_server.rs
    - tests/tool_call_http_tests.rs
    - tests/fixtures/mod.rs
decisions:
  - Remove env var dependency from HTTP mock server to eliminate race conditions
  - Use MockServerConfig struct passed directly to server start()
  - Maintain separate config types for HTTP (parameterized) vs stdio (env vars)
metrics:
  duration: 30 minutes
  completed: 2026-02-13
---

# Phase 17 Plan 04: Fix HTTP Test Flakiness Summary

## Objective
Fix intermittent HTTP test failures caused by environment variable race conditions during parallel test execution.

## Root Cause
MockHttpServer read MOCK_TOOLS and MOCK_RESPONSES from environment variables at startup. When tests ran in parallel:
1. Test A sets env vars for its configuration
2. Test B overwrites env vars with its configuration  
3. Test A's server starts and reads Test B's configuration
4. Test A fails with "Tool not found" or "expected 3 tools but got X"

## Solution Implemented
Refactored MockHttpServer to accept configuration via parameters instead of environment variables:

### Changes to tests/fixtures/mock_http_server.rs
- Added `MockServerConfig` struct with `tools`, `responses`, `errors` fields
- Changed `MockHttpServer::start()` signature to accept `config: MockServerConfig`
- Added `MockServerState::from_config()` constructor
- Removed environment variable reading from server initialization
- Added `from_parts()` helper for easy config construction

### Changes to tests/tool_call_http_tests.rs  
- Updated `with_mock_server()` helper to accept config parameter
- Updated all 7 HTTP tests to use `MockServerConfig::from_parts(tools, responses, errors)`
- Removed all `unsafe { std::env::set_var(...) }` calls
- Removed `clear_mock_config()` function and cleanup calls
- Added import: `use fixtures::mock_http_server;`

### Changes to tests/fixtures/mod.rs
- Removed re-export of `MockServerConfig` (now distinct types for HTTP vs stdio)
- Removed deprecated `start_mock_http()` function
- Updated module documentation

## Test Results

### Before Fix (Flaky)
- 11/13 tests passing intermittently
- Failures: `test_http_basic_tool_call`, `test_http_complex_nested_arguments`, `test_http_tools_list`
- Error: "Tool not found" or configuration mismatch

### After Fix (Consistent)
All 5 consecutive runs with 8 parallel threads:
```
=== Run 1 ===
test result: ok. 13 passed; 0 failed; 0 ignored

=== Run 2 ===
test result: ok. 13 passed; 0 failed; 0 ignored

=== Run 3 ===
test result: ok. 13 passed; 0 failed; 0 ignored

=== Run 4 ===
test result: ok. 13 passed; 0 failed; 0 ignored

=== Run 5 ===
test result: ok. 13 passed; 0 failed; 0 ignored
```

### Full Tool Call Test Suite
- HTTP tests: 13/13 passing
- Stdio tests: 4/4 passing  
- Error tests: 7/7 passing
- **Total: 18/18 tests passing**

## Verification Criteria
- [x] MockHttpServer::start() accepts config parameter
- [x] All 13 HTTP tests pass consistently when run in parallel (--test-threads=8)
- [x] No unsafe env var operations in HTTP test file
- [x] Code compiles with zero warnings in test files

## Key Learnings

### Race Condition Pattern
Environment variables are global mutable state. When multiple tests run in parallel:
- Test isolation requires either:
  1. Process-level isolation (stdio tests spawn separate processes)
  2. Explicit parameter passing (HTTP tests now use this)
  3. Thread-local state (complex, error-prone)

### Design Decision: Two Config Types
Maintained separate `MockServerConfig` types:
- `fixtures::MockServerConfig` - for stdio tests (needs `to_env()`, `apply()`)
- `fixtures::mock_http_server::MockServerConfig` - for HTTP tests (direct parameter)

This allows stdio tests to continue using env vars (appropriate for subprocess communication) while HTTP tests use parameterized config (appropriate for in-process servers).

## Commits
1. `9967b36` - feat(17-04): refactor MockHttpServer to accept parameterized configuration
2. `70471c3` - feat(17-04): update HTTP tests to use parameterized configuration

## Files Modified
- tests/fixtures/mock_http_server.rs (+76/-36 lines)
- tests/tool_call_http_tests.rs (+39/-94 lines)
- tests/fixtures/mod.rs (cleanup, removed deprecated function)

## Deviations from Plan
None - plan executed exactly as written.

## Self-Check
- [x] All modified files exist and contain expected changes
- [x] All commits recorded in git history
- [x] Tests pass consistently in parallel execution
