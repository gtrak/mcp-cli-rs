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

<details>
<summary>✅ v1.7 Linux Compatibility & Documentation (Phases 24-27) — SHIPPED 2026-02-16</summary>

**Milestone v1.7: Linux compatibility, comprehensive README, CI/CD automation**

- [x] Phase 24: Linux Compatibility Fixes (4/4 plans) — Fixed compilation, platform-gated dependencies, all 109 tests pass
- [x] Phase 25: Cross-Platform Test Validation (7/7 plans) — Fixed runtime nesting bug, socket conflicts, daemon tests pass
- [x] Phase 26: README and Documentation (1/1 plan) — 354-line README with installation, usage, troubleshooting
- [x] Phase 27: CI/CD Setup (1/1 plan) — GitHub Actions with matrix builds for Linux/Windows/macOS

**Key Fixes:**
- Platform-gated windows-sys dependency, added nix crate for Unix
- Fixed critical `create_ipc_server()` runtime nesting bug (removed Handle::block_on())
- Fixed socket path conflicts with AtomicU64 unique identifiers
- Fixed TempDir lifetime bug in daemon tests

**Requirements:** 17/20 satisfied (85% — LINUX-03 gap closure complete, test isolation is tech debt)

**Archived to:** `.planning/milestones/v1.7-ROADMAP.md`
**Requirements archived to:** `.planning/milestones/v1.7-REQUIREMENTS.md`

</details>

---

## 🚧 Next Milestone (Planning)

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
| 20 | UX Audit | Complete | 100% |
| 21 | UX Improvements | Complete | 100% |
| 22 | Dynamic Flag Parsing | Complete | 100% |
| 23 | Help Text Improvements | Complete | 100% |
| 24 | Linux Compatibility Fixes | Complete | 100% |
| 25 | Cross-Platform Test Validation | Complete | 100% |
| 26 | README and Documentation | Complete | 100% |
| 27 | CI/CD Setup | Complete | 100% |

**Progress Summary:**
- **Phases completed:** 27/27 (ALL COMPLETE) ✅
- **Total plans:** 76 plans executed across v1.0-v1.7
- **Total requirements:** 156/159 satisfied (98.1%) ✅
- **Milestones shipped:** v1.0, v1.2, v1.3, v1.4, v1.5, v1.6, v1.7 ✅

---

**Last updated:** 2026-02-16 (Milestone v1.7 shipped)

---

*For archived roadmap details, see `.planning/milestones/v1-ROADMAP.md` through `.planning/milestones/v1.7-ROADMAP.md`*