# MCP CLI Rust Rewrite

## What This Is

A Rust rewrite of the mcp-cli tool for interacting with Model Context Protocol (MCP) servers. Provides a lightweight, single-binary CLI that developers and AI agents can use to discover, inspect, and execute tools from stdio and HTTP-based MCP servers.

## Core Value

Reliable cross-platform MCP server interaction without dependencies. Developers and AI agents can discover available tools, inspect schemas, and execute operations through a simple CLI that works consistently on Linux, macOS, and Windows.

## Current Milestone

**Milestone:** v1.1 Unified Daemon Architecture
**Status:** In Progress
**Started:** 2026-02-09

**Goal:** Refactor to a single binary with 3 operational modes - standalone daemon, auto-spawn with TTL, and require-existing-daemon.

**Target features:**
- Remove separate daemon binary
- Unified CLI with `mcp daemon` command
- Auto-spawn mode with configurable TTL
- Require-daemon mode for explicit dependency

---

## Requirements

### Validated (v1.0 — Shipped 2026-02-09)

- ✓ Connect to MCP servers via stdio and HTTP transports
- ✓ List all configured servers and their available tools
- ✓ Display server details (transport, connection info, tool count, instructions)
- ✓ Display tool schemas (name, description, input JSON Schema)
- ✓ Search tools by glob pattern across all servers
- ✓ Execute tools with JSON arguments (inline or stdin)
- ✓ Parse and substitute environment variables from configuration
- ✓ Filter tools based on allowedTools/disabledPatterns
- ✓ Implement retry logic with exponential backoff for transient errors
- ✓ Use connection daemon for caching (configurable, optional)
- ✓ Provide structured, actionable error messages
- ✓ Support environment variables for configuration (timeout, concurrency, retry, etc.)
- ✓ Handle concurrent parallel connections with configurable limits
- ✓ Gracefully handle signals and cleanup resources
- ✓ Format tool call results for CLI-friendly display
- ✓ Validate JSON arguments with clear error messages
- ✓ Auto-detect stdin input for tool arguments
- ✓ Windows named pipe IPC support
- ✓ Direct mode (no daemon) for one-shot operations

### Active (v1.1)

- [ ] Unified single binary (no separate daemon.exe)
- [ ] Standalone daemon mode (`mcp daemon`)
- [ ] Auto-spawn mode with TTL (`mcp --auto-daemon`)
- [ ] Require-daemon mode (`mcp --require-daemon`)
- [ ] Configurable TTL for auto-shutdown
- [ ] Clean daemon lifecycle management

### Out of Scope

- Bug-for-bug compatibility with Bun implementation — this is an opportunity to improve (where reasonable)
- Public distribution/crates.io publishing — local compilation only
- MCP server implementation — this tool is a client only

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
| Rust language | Fix Windows process spawning, remove Bun dependency, single binary | ✓ Working |
| Unified binary (v1.1) | Simplify deployment, remove daemon.exe management | — In Progress |
| Three operational modes | Flexibility: persistent daemon, auto-spawn, or explicit dependency | — In Progress |
| Skill-compatible output | Tool will be wrapped for LLM use, need machine-parsable errors | ✓ Working |

---
*Last updated: 2026-02-09 — Milestone v1.1 started*
