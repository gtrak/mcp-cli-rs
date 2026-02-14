# Phase 23: Help Text Improvements - Context

**Gathered:** 2026-02-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix JSON error message to show valid format and document both JSON and `--args` flag usage in help text. Help adapts to tool schema for tool-specific help.

</domain>

<decisions>
## Implementation Decisions

### Error message format
- Error message is specific about what went wrong (parse error, schema mismatch, etc.)
- Includes hint pointing to tool-specific help: `Run 'mcp-cli call <tool-name> --help' for valid parameters`
- If tool name is known at error time, hint includes the specific tool name

### Help text structure
- Main help (`--help`) documents both JSON format and `--args` flag format
- Tool-specific help via `mcp-cli call <tool-name> --help` adapts to that tool's schema
- Tool-specific help displays: parameter names, types, required vs optional, descriptions from schema
- No usage examples needed in help

### Error differentiation
- Named args (`--key value`) get translated to JSON internally before validation
- Errors are unified after translation - both direct JSON input and `--args` input produce the same error if resulting JSON doesn't match tool schema
- Error focuses on schema validation, not input format differences

</decisions>

<specifics>
## Specific Ideas

- "help should adapt to the tool schema" — tool-specific help shows relevant parameters
- Error hint dynamically includes tool name when available
- No examples needed in help text

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 23-help-text-improvements*
*Context gathered: 2026-02-14*
