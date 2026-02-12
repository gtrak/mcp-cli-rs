# Phase 13: Code Organization - Context

**Gathered:** 2026-02-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Restructure large source files into focused modules with clear separation of concerns. Split commands.rs (1850 lines) and main.rs (809 lines) so all source files are under 600 lines. Maintain existing functionality - all tests must pass after restructuring.

</domain>

<decisions>
## Implementation Decisions

### File Splitting Strategy
- Split logically by functionality/command type
- Group related commands together (list commands, execution commands, discovery commands, server management)
- Each module should have a single, clear responsibility

### Target File Sizes
- commands.rs: 1850 lines → split into modules under 600 lines each
- main.rs: 809 lines → reduce to minimal CLI entry point (~200-300 lines)
- All resulting modules: <600 lines

### Claude's Discretion
- Exact module naming and hierarchy structure
- Re-export patterns at crate root
- Whether to use flat modules or nested sub-modules
- How to organize shared helper functions
- Exact boundary of what stays in main.rs vs moves to modules
- Internal module organization within each split file

</decisions>

<specifics>
## Specific Ideas

- Commands organized by domain: list operations (list, info), execution (call, search), server management (install, remove, validate)
- main.rs becomes minimal: argument parsing, subcommand dispatch, error handling at top level
- Module structure should make dependencies clear - related functionality grouped

</specifics>

<deferred>
## Deferred Ideas

- Further module restructuring beyond Phase 13 scope — Phase 14 will handle duplication elimination
- Refactoring actual command logic — this phase is pure code movement, not behavior changes
- Public API changes — keep same exports, just reorganize internals

</deferred>

---

*Phase: 13-code-organization*
*Context gathered: 2026-02-12*
