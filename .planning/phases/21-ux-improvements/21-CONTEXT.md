# Phase 21: UX Improvements - Context

**Gathered:** 2026-02-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix CLI UX issues from Phase 20 audit: help text improvements, error message enhancements, and command usability. Tool calling and all subcommands already work correctly. This phase is purely UX polish.

**Deferred to later phases:**
- --version flag implementation (not critical)
- stdin support for tool call args (not needed)

</domain>

<decisions>
## Implementation Decisions

### Error Message Format
- **FIX-05: "Did you mean?" suggestions** — When user types unknown subcommand that's close to a valid one (edit distance ≤ 2), show "Did you mean 'X'?"
- **FIX-06: Server list in errors** — When server not found, show "Available servers: a, b, c" after the error
- **FIX-07: JSON error suggestions** — On invalid JSON, append "Valid JSON example: {}" to the error message

### Help Text Improvements
- **FIX-02: Add examples** — Add 1-2 key examples per subcommand in help text (not comprehensive, just most common use cases)
- **FIX-03: Remove warning text** — Remove "currently recommended for daemon-mode issues" and "has known issues on Windows" from help text
- **FIX-04: Document environment variables** — Add ENV VARS section to main help showing MCP_DAEMON_TTL, MCP_NO_DAEMON
- **FIX-10: Format explanation** — Add brief note explaining "server/tool" vs "server tool" formats in relevant help text

### Command Behavior
- **FIX-08: grep alias** — Add `grep` as a supported subcommand that aliases to `search` (both work)

</decisions>

<specifics>
## Specific Ideas

- Keep help examples concise — 1-2 per command, not exhaustive
- Error messages should be helpful but not verbose
- "Did you mean?" only for close matches (avoid false positives)

</specifics>

<deferred>
## Deferred Ideas

- --version flag — not critical, can add later
- stdin support for tool call args — decided not needed

</deferred>

---

*Phase: 21-ux-improvements*
*Context gathered: 2026-02-13*
