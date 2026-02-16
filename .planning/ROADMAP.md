# Roadmap: MCP CLI Rust Rewrite

**Created:** 2025-02-06
**Core Value:** Reliable cross-platform MCP server interaction without dependencies
**Depth:** Standard (4 phases)
**Coverage:** 159/159 requirements mapped

## Overview

This roadmap delivers a complete MCP CLI tool in Rust that solves the Windows process spawning issues of the original Bun implementation. The architecture is layered: core transport and protocol → daemon connection pooling → performance optimization → UX refinement → ergonomic output. Each phase delivers a verifiable set of user-facing capabilities.

Project follows a solo developer + Claude workflow with no team coordination artifacts. Phases derive from requirements rather than arbitrary templates.

---

<details>
<summary>✅ v1 Core Implementation (Phases 1-5) — SHIPPED 2026-02-09</summary>

**Milestone v1: MVP with daemon connection pooling**

- [x] Phase 1: Core Protocol & Configuration (4/4 plans) — Completed 2026-02-10
- [x] Phase 2: Connection Daemon & Cross-Platform IPC (11/11 plans) — Completed with gap closure
- [x] Phase 3: Performance & Reliability (6/6 plans) — Completed 2026-02-08
- [x] Phase 4: Tool Filtering & Cross-Platform Validation (3/3 plans) — Completed (XP-01 deferred to Phase 8)
- [x] Phase 5: Unified Daemon Architecture (1/1 plans) — Completed 2026-02-09

</details>

---

<details>
<summary>✅ v1.2 Ergonomic CLI Output (Phases 6-11) — SHIPPED 2026-02-12</summary>

**Milestone v1.2: Output formatting, JSON mode, cross-platform validation**

- [x] Phase 6: Output Formatting & Visual Hierarchy (4/4 plans) — Completed 2026-02-10
- [x] Phase 7: JSON Output & Machine-Readable Modes (4/4 plans) — Completed 2026-02-11
- [x] Phase 8: Fix Phase 4 Windows Tests (1/1 plans) — Completed 2026-02-11 (XP-01 validated)
- [x] Phase 9: Cross-Platform Verification (1/1 plans) — Completed 2026-02-11 (XP-02/XP-04 verified)
- [x] Phase 10: Phase 6 Verification Documentation (1/1 plans) — Completed 2026-02-12
- [x] Phase 11: Code Quality Cleanup (1/1 plans) — Completed 2026-02-12

**Archived to:** `.planning/milestones/v1.2-ROADMAP.md`
**Requirements archived to:** `.planning/milestones/v1.2-REQUIREMENTS.md`

</details>

---

<details>
<summary>✅ v1.3-v1.6 Previous Milestones — SHIPPED</summary>

- [x] **v1.3**: Tech Debt Cleanup & Code Quality (Phases 12-16) — Shipped 2026-02-13
- [x] **v1.4**: Test Coverage (Phases 17-19) — Shipped 2026-02-13
- [x] **v1.5**: UX Audit & Improvements (Phases 20-21) — Shipped 2026-02-13
- [x] **v1.6**: CLI Calling Conventions (Phases 22-23) — Shipped 2026-02-14

**Total:** 139/139 requirements satisfied across v1.0-v1.6

</details>

---

## 🚧 v1.7 Linux Compatibility & Documentation

**Status:** In Progress

### Phase 24: Linux Compatibility Fixes

**Goal:** Fix compilation errors and platform-specific issues on Linux

**Requirements:** LINUX-01 through LINUX-09

**Success Criteria:**
1. `cargo build` succeeds on Linux without errors
2. `cargo test --lib` passes all library tests
3. All platform-specific code properly gated with `#[cfg()]` attributes
4. Cross-platform IPC exports are consistent

**Plans:** 4 plans

**Key Fixes Needed:**
- Add `nix` crate dependency for Unix signal handling
- Gate Windows-only exports (`create_ipc_server`) properly
- Fix `send_request` method signature mismatch
- Fix Unix socket address `to_string_lossy` compatibility
- Add missing `DaemonNotRunning` error pattern
- Make `windows-sys` dependency Windows-only

**Plan List:**
- [x] 24-01-PLAN.md — Fix Cargo.toml dependencies (platform-gated windows-sys, add nix crate)
- [x] 24-02-PLAN.md — Add Unix implementation of create_ipc_server
- [x] 24-03-PLAN.md — Fix Unix socket address and error handling
- [x] 24-04-PLAN.md — Verify library tests pass on Linux

**Status:** ✅ COMPLETE — All Linux compatibility issues resolved, all 109 tests pass

---

### Phase 25: Cross-Platform Test Validation

**Goal:** Ensure all tests pass on Linux, Windows, and macOS

**Requirements:** LINUX-02, LINUX-03

**Success Criteria:**
1. All 109 library tests pass on Linux
2. All integration tests pass on Linux
3. Test suite runs successfully in CI environment
4. Platform-specific test variations documented

**Plans:** 4 plans (2 original + 2 gap closure)

**Plan List:**
- [x] 25-01-PLAN.md — Fix integration test compilation errors
- [x] 25-02-PLAN.md — Run and verify all tests pass
- [x] 25-03-PLAN.md — Fix create_ipc_server runtime nesting bug (gap closure)
- [x] 25-04-PLAN.md — Update docs and re-verify tests (gap closure)

**Gap Closure:**
✅ **COMPLETE** - All verification gaps addressed:
1. ✅ **Code bug fixed:** create_ipc_server() made async, removed Handle::block_on() (25-03)
2. ✅ **Status corrected:** LINUX-03 verified complete with test results (25-04)
3. ✅ **Docs corrected:** 25-02-SUMMARY.md accurately describes code bug (25-04)
4. ✅ **Socket conflicts fixed:** Unique socket paths with AtomicU64 counters (25-05)
5. ✅ **Socket cleanup added:** Robust cleanup helpers and stale file handling (25-06)
6. ✅ **Daemon tests fixed:** All daemon_ipc_tests pass (was 1/4) (25-07)

**Status:** ✅ COMPLETE — All cross-platform test validation done, gaps closed

---

### Phase 26: README and Documentation

**Goal:** Create comprehensive README with installation, usage, and examples

**Requirements:** DOC-01 through DOC-07

**Success Criteria:**
1. README.md exists at project root
2. Installation instructions cover all platforms
3. Quick start guide with examples
4. Configuration documentation
5. All commands documented with examples
6. Development setup guide
7. Troubleshooting section

---

### Phase 27: CI/CD Setup

**Goal:** Automated testing across all platforms

**Requirements:** CI-01 through CI-04

**Success Criteria:**
1. GitHub Actions workflow for Linux testing
2. GitHub Actions workflow for Windows testing
3. GitHub Actions workflow for macOS testing
4. All workflows trigger on PR and push to main

---

## Progress

| Phase | Name | Status | Completion |
|-------|------|--------|------------|
| 1 | Core Protocol & Configuration | Complete | 100% |
| 2 | Connection Daemon & Cross-Platform IPC | Complete | 100% |
| 3 | Performance & Reliability | Complete | 100% |
| 4 | Tool Filtering & Cross-Platform Validation | Complete | 100% |
| 5 | Unified Daemon Architecture | Complete | 100% |
| 6 | Output Formatting & Visual Hierarchy | Complete | 100% |
| 7 | JSON Output & Machine-Readable Modes | Complete | 100% |
| 8 | Fix Phase 4 Windows Tests (XP-01) | Complete | 100% |
| 9 | Cross-Platform Verification (XP-02, XP-04) | Complete | 100% |
| 10 | Phase 6 Verification Documentation | Complete | 100% |
| 11 | Code Quality Cleanup | Complete | 100% |
| 12 | Test Infrastructure | Complete | 100% |
| 13 | Code Organization | Complete | 100% |
| 14 | Duplication Elimination | Complete | 100% |
| 15 | Documentation & API | Complete | 100% |
| 16 | Code Quality Sweep | Complete | 100% |
| 17 | Tool Call Integration Tests | Complete | 100% |
| 18 | Retry and IPC Tests | Complete | 100% |
| 19 | Error Paths and Regression Tests | Complete | 100% |
| 20 | UX Audit | Complete | 100% |
| 21 | UX Improvements | Complete | 100% |
| 22 | Dynamic Flag Parsing | Complete | 100% |
| 23 | Help Text Improvements | Complete | 100% |
| 24 | Linux Compatibility Fixes | Complete | 100% |
| 25 | Cross-Platform Test Validation | Gap Closure | 50% |
| 26 | README and Documentation | Not Started | 0% |
| 27 | CI/CD Setup | Not Started | 0% |

**Progress Summary:**
- **Phases completed:** 23/27
- **Total plans:** 75 plans executed (v1.0-v1.6)
- **v1.0-v1.6 Coverage:** 139/139 requirements satisfied ✅
- **v1.7 Coverage:** 9/20 requirements satisfied (LINUX-01 through LINUX-09)

---

**Last updated:** 2026-02-16 (v1.7 started: Linux Compatibility & Documentation)

---

*For archived roadmap details, see `.planning/milestones/v1-ROADMAP.md` through `.planning/milestones/v1.6-ROADMAP.md`*