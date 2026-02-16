# Phase 26: README and Documentation - Context

**Gathered:** 2026-02-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Create comprehensive README.md with installation instructions, usage examples, and key differentiators. Documentation should be tight, reference the original Bun implementation, and highlight Windows compatibility via named pipes.

</domain>

<decisions>
## Implementation Decisions

### Documentation style
- **Tight and focused** — no bloat, get to the point quickly
- Reference the original Bun-based MCP CLI implementation for context
- Emphasize this is a Rust rewrite solving Windows process spawning issues

### Key differentiators to highlight
- **Windows support via named pipes** — primary selling point vs original
- Cross-platform: Linux (Unix sockets), Windows (named pipes), macOS (Unix sockets)
- No runtime dependencies (unlike Bun-based original)

### Examples
- **Must show usage examples** — syntax differs from original implementation
- Include common commands: list tools, call tools, configuration
- Show both simple and slightly more complex examples
- Examples should demonstrate the different CLI syntax clearly

### Structure
- Quick start at the top (installation + first command)
- Key differences from original highlighted early
- Platform-specific notes (Windows named pipes called out)
- Configuration section
- Command reference with examples

### Claude's Discretion
- Exact wording and phrasing
- Specific example commands to showcase
- Order of sections after quick start
- Whether to include troubleshooting section
- Badge/shield choices for top of README

</decisions>

<specifics>
## Specific Ideas

- "Make it tight" — concise documentation without fluff
- Reference original: github.com/f/modelcontextprotocol
- Key message: "Works on Windows using named pipes"
- Show syntax differences prominently since users may be migrating from original

</specifics>

<deferred>
## Deferred Ideas

- Full website/documentation site — out of scope, README only
- Man pages or generated docs — future phase if needed
- Video tutorials or GIFs — nice to have, not essential

</deferred>

---

*Phase: 26-readme-and-documentation*
*Context gathered: 2026-02-16*
