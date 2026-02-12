# Requirements: MCP CLI Rust Cleanup

**Defined:** 2026-02-12
**Core Value:** Reliable cross-platform MCP server interaction without dependencies.

## v1.3 Requirements Tech Debt Cleanup & Code Quality

Requirements for code quality, maintainability, and duplication elimination.

### Test Infrastructure

- [ ] **TEST-01**: Create test setup helpers module (`tests/helpers.rs`) with TestEnvironment struct for temp directory management
- [ ] **TEST-02**: Create platform-specific socket/pipe path generators with unified interface in helpers module
- [ ] **TEST-03**: Create IPC test helpers (server/client roundtrip patterns) in helpers module
- [ ] **TEST-04**: Create test config factories for common server/tool configurations
- [ ] **TEST-05**: Refactor ipc_tests.rs, cross_platform_daemon_tests.rs, lifecycle_tests.rs, windows_process_spawn_tests.rs, orphan_cleanup_tests.rs to use helpers
- [ ] **TEST-06**: Split cross_platform_daemon_tests.rs (785 lines) into tests/unix/*.rs, tests/windows/*.rs, tests/common/*.rs
- [ ] **TEST-07**: Organize test files by platform and common patterns, maintain test coverage
- [ ] **TEST-08**: All tests use helpers instead of inline setup (eliminate ~200-300 lines of duplication)

### Code Organization

- [ ] **ORG-01**: Split src/cli/commands.rs (1850 lines) into src/cli/list_commands.rs (list, grep), src/cli/info_commands.rs (info), src/cli/call_commands.rs (call), src/cli/commands.rs (orchestration)
- [ ] **ORG-02**: Extract daemon lifecycle from src/main.rs (809 lines) to src/cli/daemon_lifecycle.rs
- [ ] **ORG-03**: Extract command routing from src/main.rs to src/cli/command_routing.rs
- [ ] **ORG-04**: Extract config loading/merging from src/main.rs to src/cli/config_setup.rs
- [ ] **ORG-05**: Extract CLI entry point from src/main.rs to src/cli/entry.rs, leaving main.rs as thin entry wrapper
- [ ] **ORG-06**: Split src/config/mod.rs (432 lines) into src/config/types.rs (Config structs), src/config/parser.rs (TOML parsing), src/config/validator.rs (validation)
- [ ] **ORG-07**: All module re-exports updated to reflect new structure
- [ ] **ORG-08**: Module structure is clear with separation of concerns, no file >600 lines

### Duplication Elimination

- [ ] **DUP-01**: Consolidate 16 JSON command functions (cmd_xxx, cmd_xxx_json pairs) into 8 multi-mode commands with OutputMode parameter
- [ ] **DUP-02**: Create format_for_json() and format_for_human() helper methods to eliminate duplicate formatting logic (~200-300 lines)
- [ ] **DUP-03**: Unify duplicate connection interfaces (daemon/pool.rs, client/mod.rs, ipc/mod.rs) into single McpClient trait
- [ ] **DUP-04**: Remove duplicate list_tools() and call_tool() implementations across pool, client, ipc
- [ ] **DUP-05**: Merge src/transport.rs (81 lines) and src/client/transport.rs (68 lines) into single transport abstraction or eliminate duplicate
- [ ] **DUP-06**: All duplicate interfaces eliminated, single source of truth for MCP client operations

### Documentation & API

- [ ] **DOC-01**: Fix all 9 cargo doc warnings (unclosed HTML tags, URL formatting)
- [ ] **DOC-02**: Audit 106 public functions and 34 public structs, mark unnecessary exports as private
- [ ] **DOC-03**: Reduce public API surface by removing 50-100 lines of unnecessary exports
- [ ] **DOC-04**: Improve module-level documentation with clear scope and usage examples
- [ ] **DOC-05**: All public APIs have rustdoc comments with examples
- [ ] **DOC-06**: cargo doc generates zero warnings

### Code Quality

- [ ] **QUAL-01**: Review 72 unwrap() usages for safety, replace with ok_or_else() or expect() with context where appropriate
- [ ] **QUAL-02**: Review and remove unnecessary #[allow(dead_code)] attributes
- [ ] **QUAL-03**: Ensure consistent error handling patterns across codebase
- [ ] **QUAL-04**: Consistent use of Result<> return types, no bare unwrap() in production code
- [ ] **QUAL-05**: All clippy warnings addressed (currently zero, maintain)

### Codebase Size Target

- [ ] **SIZE-01**: Overall codebase reduced to 10,800-11,500 lines (from 12,408 lines = 8-13% reduction)
- [ ] **SIZE-02**: No single file >600 lines (commands.rs was 1850)
- [ ] **SIZE-03**: Test duplication reduced by ~200-300 lines
- [ ] **SIZE-04**: Command duplication reduced by ~200-300 lines
- [ ] **SIZE-05**: API surface reduced by ~50-100 lines

## Out of Scope

| Feature | Reason |
|---------|--------|
| Test fixtures and assertion helpers | Low priority, test helpers sufficient for now |
| Comprehensive rewrite of modules | Refactor only, no behavior changes |
| New features or functionality | This is cleanup only |
| Performance optimization | Code organization focus, not performance |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TEST-01 through TEST-08 | Phase 12 | Pending |
| ORG-01 through ORG-08 | Phase 13 | Pending |
| DUP-01 through DUP-06 | Phase 14 | Pending |
| DOC-01 through DOC-06 | Phase 15 | Pending |
| QUAL-01 through QUAL-05 | Phase 16 | Pending |
| SIZE-01 through SIZE-05 | Phases 12-16 | Pending |

**Coverage:**
- v1.3 requirements: 37 total
- Mapped to phases: 37
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-12*
*Last updated: 2026-02-12 after requirements definition*
