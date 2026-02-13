# Phase 14: Duplication Elimination - Context

**Gathered:** 2026-02-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Consolidate duplicate code across JSON command functions and connection interfaces. Remove the 16→8 JSON/human command duplication by introducing model + formatter architecture. Unify connection interfaces (pool, client, IPC) under a single McpClient trait.

**Scope:**
- Model + formatter architecture for commands
- Unified McpClient trait (async only)
- Gradual module migration (pool → client → IPC)
- Breaking changes to internal APIs (no deprecation)

**Out of scope:**
- Public API changes beyond internal refactoring
- New functionality
- Performance optimization (separate phase if needed)

</domain>

<decisions>
## Implementation Decisions

### Architecture: Model + Formatter
- Internal model types capture daemon responses (not formatted strings)
- Separate formatters convert models to human or JSON output
- Commands return models, not formatted output
- Enables single source of truth for testing

### Consolidation Strategy
- Start with: Connection interfaces (unified McpClient trait) - gives stable foundation
- Trait scope: Async-only with async_trait, minimal operations (list_tools, call_tool, server_info, subscribe, unsubscribe)
- Error handling: Generic `Result<T, McpError>` with thiserror for nested errors
- Rollout: Gradual migration module-by-module (pool → client → IPC)

### Trait Design (McpClient)
- Core operations: Complete protocol (list_tools, call_tool, server_info, subscribe, unsubscribe)
- Ownership: `&mut self` (mutable borrow, matches current pattern)
- Trait bounds: Use async_trait (handles Send/Sync automatically)
- Return types: Concrete types (Vec<ToolInfo>, etc.)

### Backward Compatibility
- Breaking change: Clean break, no deprecation warnings
- Scope: Internal only - we control all call sites
- Migration: Atomic commits - trait changes + caller updates together
- Testing: Trust existing test suite + add model/formatter tests

### Test Coverage (New)
- Model tests: `tests/cli/command_models_test.rs` - verify models capture all daemon response fields
- Formatter tests: `tests/cli/formatters_test.rs` - verify human/JSON formatters produce correct output
- Full coverage via model + formatter tests (no separate round-trip tests needed)
- Each command pair (list, info, tool_info, call, search) gets model + formatter coverage

### Model Location
- Models live near commands: `src/cli/models.rs`
- Keeps related code together, easy to find

</decisions>

<specifics>
## Specific Ideas

- "If we had invertible conversion between formats, round-trip tests would cover it all" - led to model + formatter architecture
- Commands should format themselves (not the trait)
- Use async_trait for clean async trait handling
- thiserror for nested errors (unless too verbose)

</specifics>

<deferred>
## Deferred Ideas

- None - discussion stayed within phase scope

</deferred>

---

*Phase: 14-duplication-elimination*
*Context gathered: 2026-02-12*
