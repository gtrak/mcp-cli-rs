# Requirements: MCP CLI Rust - v1.6 CLI Calling Conventions

**Defined:** 2026-02-14
**Core Value:** Reliable cross-platform MCP server interaction without dependencies.

## v1.6 Requirements: CLI Calling Conventions

Add bash-style calling conventions and fix JSON help text.

### Dynamic Flag Parsing

- [ ] **ARGS-01**: Parse `--key value` as JSON field `{"key": "value"}`
- [ ] **ARGS-02**: Parse `--key=value` as JSON field `{"key": "value"}`
- [ ] **ARGS-03**: Parse JSON values directly: `--key {"a":1}` → `{"key": {"a": 1}}`
- [ ] **ARGS-04**: Fall back to JSON argument if no flags provided (backward compatibility)

### Help Text Improvements

- [ ] **HELP-01**: Fix error message to show valid JSON format `{"key": "value"}`
- [ ] **HELP-02**: Document both JSON and `--key value` formats in call help
- [ ] **HELP-03**: Show example for flag usage in call command help
- [ ] **HELP-04**: Update list command to show calling hint for tools

---

## Out of Scope

| Feature | Reason |
|---------|--------|
| Multi-level nesting | Only one level deep |
| Auto-detection of types | Keep simple - always string |
| Positional arguments | Only flags |

---

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| ARGS-01 | Phase 22 | Complete |
| ARGS-02 | Phase 22 | Complete |
| ARGS-03 | Phase 22 | Complete |
| ARGS-04 | Phase 22 | Complete |
| ARGS-05 | Phase 22 | Pending |
| HELP-01 | Phase 23 | Pending |
| HELP-02 | Phase 23 | Pending |
| HELP-03 | Phase 23 | Pending |
| HELP-04 | Phase 23 | Pending |

**Coverage:**
- v1.6 requirements: 9 total
- Mapped to phases: 4
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-14*
