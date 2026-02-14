# Requirements: MCP CLI Rust - v1.6 CLI Calling Conventions

**Defined:** 2026-02-14
**Core Value:** Reliable cross-platform MCP server interaction without dependencies.

## v1.6 Requirements: CLI Calling Conventions

Add bash-style calling conventions and fix JSON help text.

### Bash-Style Arguments

- [ ] **ARGS-01**: Add `--args` flag to `call` command for key=value syntax
- [ ] **ARGS-02**: Support multiple `--args` flags for multiple parameters
- [ ] **ARGS-03**: Support `--args key=value` where value is unquoted for simple values
- [ ] **ARGS-04**: Support `--args key="quoted value"` for values with spaces
- [ ] **ARGS-05**: Fall back to JSON if `--args` not provided (backward compatibility)

### Help Text Improvements

- [ ] **HELP-01**: Fix error message to show valid JSON format `{"key": "value"}`
- [ ] **HELP-02**: Document both JSON and `--args` formats in call help
- [ ] **HELP-03**: Show example for `--args` usage in call command help
- [ ] **HELP-04**: Update list command to show calling hint for tools

---

## Out of Scope

| Feature | Reason |
|---------|--------|
| Positional arguments | Only --args flag this milestone |
| Auto-detection of argument types | Keep simple - always string unless quoted |
| JSON5 or relaxed JSON | Stick to standard JSON for --args |

---

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| ARGS-01 | Phase 22 | Pending |
| ARGS-02 | Phase 22 | Pending |
| ARGS-03 | Phase 22 | Pending |
| ARGS-04 | Phase 22 | Pending |
| ARGS-05 | Phase 22 | Pending |
| HELP-01 | Phase 23 | Pending |
| HELP-02 | Phase 23 | Pending |
| HELP-03 | Phase 23 | Pending |
| HELP-04 | Phase 23 | Pending |

**Coverage:**
- v1.6 requirements: 9 total
- Mapped to phases: 0
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-14*
