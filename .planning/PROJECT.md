# MCP CLI Rust Rewrite

## What This Is

A Rust rewrite of the mcp-cli tool for interacting with Model Context Protocol (MCP) servers. Provides a lightweight, single-binary CLI that developers and AI agents can use to discover, inspect, and execute tools from stdio and HTTP-based MCP servers.

## Core Value

Reliable cross-platform MCP server interaction without dependencies. Developers and AI agents can discover available tools, inspect schemas, and execute operations through a simple CLI that works consistently on Linux, macOS, and Windows.

## Requirements

### Validated (v1.0 - v1.1)

- ✅ Connect to MCP servers via stdio and HTTP transports
- ✅ List all configured servers and their available tools
- ✅ Display server details (transport, connection info, tool count, instructions)
- ✅ Display tool schemas (name, description, input JSON Schema)
- ✅ Search tools by glob pattern across all servers
- ✅ Execute tools with JSON arguments (inline or stdin)
- ✅ Parse and substitute environment variables from configuration
- ✅ Filter tools based on allowedTools/disabledTools patterns
- ✅ Implement retry logic with exponential backoff for transient errors
- ✅ Use connection daemon for caching (configurable, optional)
- ✅ Provide structured, actionable error messages
- ✅ Support environment variables for configuration (timeout, concurrency, retry, etc.)
- ✅ Handle concurrent parallel connections with configurable limits
- ✅ Gracefully handle signals and cleanup resources
- ✅ Format tool call results for CLI-friendly display
- ✅ Validate JSON arguments with clear error messages
- ✅ Auto-detect stdin input for tool arguments
- ✅ Colored terminal output with NO_COLOR support
- ✅ Cross-platform support (Windows, Linux, macOS)
- ✅ Unified daemon architecture (single binary, three operational modes)
- ✅ Configurable TTL for auto-shutdown daemon

## Current Milestone: v1.2 Ergonomic CLI Output

**Goal:** Improve CLI output format to be more ergonomic, self-describing, and aligned with standard CLI conventions, making it easier for both humans and LLMs to navigate.

**Target features:**
- Redesigned tool listing with parameter overview (like --help format)
- Progressive detail levels (summary → parameters → full schema)
- Consistent command structure across all subcommands
- Better default output for tool discovery (descriptions + usage hints)
- Machine-readable JSON output option for scripting
- Grouped tool display by server with clear visual hierarchy

### Active (v1.2)

- [ ] Redesigned tool listing format with parameter overview
- [ ] Progressive detail: summary view → parameter details → full schema
- [ ] Improved default `list` output with descriptions and usage hints
- [ ] Consistent help-style formatting across all commands
- [ ] JSON output mode for programmatic use
- [ ] Better visual hierarchy in multi-server listings

### Out of Scope

- Bug-for-bug compatibility with Bun implementation — this is an opportunity to improve (where reasonable)
- Public distribution/crates.io publishing — local compilation only
- MCP server implementation — this tool is a client only
- SSE and Streamable HTTP transports — deferred to post-MVP
- Tool aliasing/shortcuts — config complexity without clear benefit
- Multi-server transactions — MCP doesn't support transactions

## Context

Original mcp-cli implementation exists at `../mcp-cli` and is Bun-based with the official MCP SDK. Reimplementation motivated by Windows process spawning issues and desire to remove Bun dependency for a standalone Rust binary.

The tool will be wrapped in a skill for LLM use, so error messages and output should be both human-readable and machine-parsable. Compatible with standard mcp_servers.json configuration format used by Claude Desktop, Gemini, and VS Code.

## Constraints

- **Tech Stack**: Rust — chosen for cross-platform binaries, no runtime dependencies, and reliable process spawning
- **MCP Client**: Use existing Rust SDK if available (evaluate during planning)
- **Compatibility**: Must work on Windows, Linux, and macOS without platform-specific bugs
- **Config**: Must be compatible with existing mcp_servers.json format and environment variable substitution
- **Daemons**: Must handle both Unix sockets (*nix) and named pipes (Windows) for connection caching
- **Testing**: Comprehensive test coverage required (unit and integration)
- **Distribution**: Single binary, local compilation only (no public package distribution)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust language | Fix Windows process spawning, remove Bun dependency, single binary | ✅ Verified - 42/42 requirements met |
| Daemon architecture | Maintain performance benefit of connection caching | ✅ Verified - 50%+ performance improvement |
| Use Rust MCP SDK | Avoid reimplementing MCP protocol from scratch | ✅ Working well, protocol compliance verified |
| Skill-compatible output | Tool will be wrapped for LLM use | ✅ Machine-parsable errors implemented |

## Next Milestone Goals (v1.2)

**Focus:** Ergonomic CLI Output

**Goals:**
1. Improve tool listing to show parameter overview (like --help)
2. Progressive disclosure: summary → parameters → full schema
3. Consistent formatting across all commands
4. JSON output mode for scripting
5. Better visual hierarchy

**Target:** Complete v1.2 output improvements

---

*Last updated: 2026-02-10 after v1.1 milestone completion*
