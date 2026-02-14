# Phase 22: Dynamic Flag Parsing - Context

**Gathered:** 2026-02-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Parse `--key value` style arguments as JSON fields for tool calls. Supports auto-coercion of types, multiple flags, and falls back to JSON argument for complex values.

**Phase delivers:**
1. `--key value` becomes `{"key": "value"}`
2. `--key {"a":1}` parses JSON value directly
3. Backward compatible with JSON argument

</domain>

<decisions>
## Implementation Decisions

### Value type handling
- Auto-detect and coerce types: numbers (`--count 42` → `{"count": 42}`), booleans (`--verbose true` → `{"verbose": true}`)
- Fail with clear, informative error message if value can't be coerced to the tool's schema type

### Multiple flags
- Multiple `--key value` pairs combine: `--key1 val1 --key2 val2` → `{"key1": "val1", "key2": "val2"}`

### Edge case: value looks like a flag
- If value starts with `--`, user must fall back to JSON format: `--meta --other` requires JSON input

### Error handling
- Clear, informative error messages that help users understand what went wrong
- Suggest JSON fallback when flag parsing fails

### Claude's Discretion
- Exact coercion logic (how to detect "true"/"false" strings, number formats)
- Error message wording and suggestions
- Implementation approach for parsing

</decisions>

<specifics>
## Specific Ideas

- Reference behavior from common CLI tools: `curl`, `git` style `--key value` conventions
- Error messages should guide users toward the JSON fallback when needed

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 22-dynamic-flag-parsing*
*Context gathered: 2026-02-14*
