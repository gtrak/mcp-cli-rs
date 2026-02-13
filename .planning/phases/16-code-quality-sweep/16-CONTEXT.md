# Phase 16: Code Quality Sweep - Context

**Gathered:** 2026-02-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace unsafe unwrap() calls with proper error handling, remove unnecessary #[allow(dead_code)] attributes, enforce consistent Result<> patterns, and reduce overall codebase to 10,800-11,500 lines. This is the final cleanup phase for v1.3.

</domain>

<decisions>
## Implementation Decisions

### Error handling strategy
- Use thiserror for library code (src/lib/ modules) — rich, specific error types with context
- Use anyhow for CLI code (src/cli/ modules) — just bubble up errors that can't be recovered
- Keep the separation clean: library = thiserror, CLI = anyhow

### Dead code handling
- Remove ALL #[allow(dead_code)] attributes
- Clean slate approach — if something is truly unused, remove it entirely

### Result consistency
- Per-module error types using thiserror
- Each library module defines its own error enum
- No global error type — keeps thiserror benefits (context, no crate coupling)

### Size target approach
- Aim for aggressive reduction: 10,800 lines (13% reduction from 12,408)
- Quality first, but want both quality improvement AND code removal
- If quality and removal conflict, prioritize quality

</decisions>

<specifics>
## Specific Ideas

- "I like thiserror for the 'library' parts of the application"
- "anyhow for just bubbling up errors that can't be recovered"
- "I prefer improving quality if I had to pick between that and removing code, but I really want both"

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 16-code-quality-sweep*
*Context gathered: 2026-02-12*
