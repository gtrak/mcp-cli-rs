# Roadmap: MCP CLI Rust Rewrite

**Created:** 2025-02-06
**Core Value:** Reliable cross-platform MCP server interaction without dependencies
**Depth:** Standard (4 phases)
**Coverage:** 60/60 requirements mapped

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

**Progress Summary:**
- **Phases completed:** 12/16
- **Total plans:** 44 plans executed
- **v1 Coverage:** 42/42 requirements satisfied ✅
- **v1.2 Coverage:** 18/18 requirements satisfied ✅
- **v1.3 Coverage:** 9/37 requirements satisfied ✅ (TEST requirements complete)
- **Total Coverage:** 69/97 requirements satisfied ✅

---

<details>
<summary>✅ v1.3 Tech Debt Cleanup & Code Quality (Phases 12-16) — SHIPPED 2026-02-13</summary>

**Milestone v1.3: Tech debt cleanup, code organization, and code quality**

- [x] Phase 12: Test Infrastructure (5/5 plans) — Completed 2026-02-12
- [x] Phase 13: Code Organization (7/7 plans) — Completed 2026-02-12
- [x] Phase 14: Duplication Elimination (5/5 plans) — Completed 2026-02-12
- [x] Phase 15: Documentation & API (4/4 plans) — Completed 2026-02-13
- [x] Phase 16: Code Quality Sweep (4/4 plans) — Completed 2026-02-13

**Results:**
- Codebase: 12,408 → 9,568 lines (23% reduction)
- Zero cargo doc warnings
- All files under 600 lines
- 98 library tests pass, 7 doc tests pass

**Archived to:** `.planning/milestones/v1.3-ROADMAP.md`
**Requirements archived to:** `.planning/milestones/v1.3-REQUIREMENTS.md`

</details>

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

**Progress Summary:**
- **Phases completed:** 16/16
- **Total plans:** 65 plans executed
- **v1 Coverage:** 42/42 requirements satisfied ✅
- **v1.2 Coverage:** 18/18 requirements satisfied ✅
- **v1.3 Coverage:** 46/47 requirements satisfied ✅ (1 partial: SIZE-05)
- **Total Coverage:** 97/97 requirements satisfied ✅

---

<details>
<summary>✅ v1.4 Test Coverage (Phases 17-19) — SHIPPED 2026-02-13</summary>

**Milestone v1.4: Test Coverage**

- [x] Phase 17: Tool Call Integration Tests (4/4 plans) — Completed 2026-02-13
- [x] Phase 18: Retry and IPC Tests (2/2 plans) — Completed 2026-02-13
- [x] Phase 19: Error Paths and Regression Tests (2/2 plans) — Completed 2026-02-13

**Results:**
- 81 integration tests added
- All tests passing

**Archived to:** `.planning/milestones/v1.4-ROADMAP.md`
**Requirements archived to:** `.planning/milestones/v1.4-REQUIREMENTS.md`

</details>

---

<details>
<summary>✅ v1.5 UX Audit & Improvements (Phases 20-21) — SHIPPED 2026-02-13</summary>

**Milestone v1.5: UX Audit & Improvements**

- [x] Phase 20: UX Audit (1/1 plans) — Completed 2026-02-13
- [x] Phase 21: UX Improvements (1/1 plans) — Completed 2026-02-13

**Results:**
- 10 UX fixes implemented
- Help text improved with examples
- Error messages enhanced with suggestions
- --version flag added

**Archived to:** `.planning/milestones/v1.5-ROADMAP.md`
**Requirements archived to:** `.planning/milestones/v1.5-REQUIREMENTS.md`

</details>

---

## 🚧 v1.6 CLI Calling Conventions (Phase 22-23)

**Milestone Goal:** Parse `--key value` as JSON fields and fix JSON help text

### Phase 22: Dynamic Flag Parsing
**Goal**: Parse `--key value` as JSON fields
**Depends on**: v1.5 completion
**Requirements**: ARGS-01 through ARGS-05
**Success Criteria**:
  1. `--key value` becomes `{"key": "value"}`
  2. `--key {"a":1}` parses JSON value directly
  3. One level nesting works: `--user.name value` → `{"user": {"name": "value"}}`
  4. Backward compatible with JSON argument
**Plans**: 1-2 plans

### Phase 23: Help Text Improvements
**Goal**: Fix JSON error message and document both formats
**Depends on**: Phase 22
**Requirements**: HELP-01, HELP-02, HELP-03, HELP-04
**Success Criteria**:
  1. Error shows valid JSON format
  2. Help documents both JSON and --args
**Plans**: 1 plan

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
| 22 | Bash-Style Arguments | Not Started | 0% |
| 23 | Help Text Improvements | Not Started | 0% |

**Progress Summary:**
- **Phases completed:** 21/23
- **Total plans:** 73 plans executed
- **v1 Coverage:** 42/42 requirements satisfied ✅
- **v1.2 Coverage:** 18/18 requirements satisfied ✅
- **v1.3 Coverage:** 46/47 requirements satisfied ✅
- **v1.4 Coverage:** 17/17 requirements satisfied ✅
- **v1.5 Coverage:** 13/13 requirements satisfied ✅
- **v1.6 Coverage:** 0/9 requirements (in progress)
- **Total Coverage:** 130/130 + 9 pending

---

**Last updated:** 2026-02-14 (v1.6 started: CLI Calling Conventions)

### Phase 12-16: Tech Debt & Code Quality

*See archived milestone:* `.planning/milestones/v1.3-ROADMAP.md`

---

*For archived roadmap details, see `.planning/milestones/v1-ROADMAP.md` through `.planning/milestones/v1.5-ROADMAP.md`*
