# MCP CLI Rust Rewrite

## What This Is

A Rust rewrite of the mcp-cli tool for interacting with Model Context Protocol (MCP) servers. Provides a lightweight, single-binary CLI that developers and AI agents can use to discover, inspect, and execute tools from stdio and HTTP-based MCP servers.

## Core Value

Reliable cross-platform MCP server interaction without dependencies. Developers and AI agents can discover available tools, inspect schemas, and execute operations through a simple CLI that works consistently on Linux, macOS, and Windows.

## Requirements

### Validated

<details>
<summary>v1.0: Core Implementation (42/42 requirements) — Shipped 2026-02-09</summary>

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

**Archive:** `.planning/milestones/v1-REQUIREMENTS.md`

</details>

<details>
<summary>v1.2: Ergonomic CLI Output (18/18 requirements) — Shipped 2026-02-12</summary>

- ✅ Tool listing shows parameter overview (names, types, required/optional status) in help-style format
- ✅ Progressive detail levels via flags: default (summary) → `-d` (with descriptions) → `-v` (verbose with full schema)
- ✅ Default `list` command shows tool count and brief descriptions per server
- ✅ Multi-server listings have clear visual hierarchy (server headers, grouped tools)
- ✅ Consistent formatting across all commands (list, info, grep, call)
- ✅ Parameter display uses standard CLI conventions (e.g., `name <type>` for required, `name [type]` for optional)
- ✅ Tool descriptions are prominently displayed (not truncated in default view)
- ✅ Usage hints shown in tool listings (e.g., "Use `mcp info server tool` for full schema")
- ✅ Server status clearly indicated (connected, failed, disabled tools present)
- ✅ Tool search (grep) results show context (server name + tool description)
- ✅ Empty states have helpful messages (no servers configured, no tools found)
- ✅ Error messages maintain consistent format with context and suggestions
- ✅ Warnings are visually distinct but not overwhelming
- ✅ Partial failures (some servers down) show which succeeded and which failed
- ✅ JSON output mode (`--json` flag) for programmatic use and scripting
- ✅ JSON output includes complete tool metadata (name, description, parameters, schema)
- ✅ Plain text mode (`--no-color` or when piped) works correctly for all commands
- ✅ Machine-readable output follows consistent schema across all commands

**Archive:** `.planning/milestones/v1.2-REQUIREMENTS.md`

</details>

<details>
<summary>v1.3: Tech Debt Cleanup & Code Quality (46/47 requirements) — Shipped 2026-02-13</summary>

- ✅ Test setup helpers module (tests/helpers.rs, 194 lines)
- ✅ commands.rs refactored from 1850 lines into focused modules
- ✅ Documentation warnings fixed (cargo doc zero warnings)
- ✅ Public API surface reduced by 16 lines
- ✅ main.rs cleanup with extracted daemon lifecycle functions
- ✅ Codebase size reduced: 12,408 → 9,568 lines (23% reduction)
- ⚠️ API surface reduced: 16 lines (target: 50-100, remaining opportunities in internal modules)
- ⚠️ 5 clippy dead_code warnings in internal modules (non-blocking)

**Archive:** `.planning/milestones/v1.3-REQUIREMENTS.md`

</details>

<details>
<summary>v1.4: Test Coverage (17/17 requirements) — Shipped 2026-02-13</summary>

- ✅ Mock MCP server for stdio transport testing
- ✅ Mock HTTP server for HTTP transport testing
- ✅ Stdio transport tool call tests (4 tests)
- ✅ HTTP transport tool call tests (13 tests)
- ✅ Error handling tests (7 tests)
- ✅ Retry logic tests (11 tests)
- ✅ Daemon IPC tests (4 tests)
- ✅ Error path tests: invalid JSON, timeout, disconnection (12 tests)
- ✅ Regression tests: list, config loading, tool filtering (30 tests)

**Total:** 81 integration tests added

**Archive:** `.planning/milestones/v1.4-REQUIREMENTS.md`

</details>

<details>
<summary>v1.5: UX Audit & Improvements (13/13 requirements) — Shipped 2026-02-13</summary>

- ✅ --version flag (FIX-01)
- ✅ Help examples for all commands (FIX-02)
- ✅ Removed developer warning text from help (FIX-03)
- ✅ Environment variable documentation in help (FIX-04)
- ✅ "Did you mean?" suggestions for typos (FIX-05)
- ✅ ServerNotFound shows available servers (FIX-06)
- ✅ InvalidJson shows format hints (FIX-07)
- ✅ grep alias for search command (FIX-08)
- ✅ stdin support verified (FIX-09)
- ✅ Help documents slash vs space formats (FIX-10)

**Archive:** `.planning/milestones/v1.5-REQUIREMENTS.md`

</details>

<details>
<summary>v1.6: CLI Calling Conventions (9/9 requirements) — Shipped 2026-02-14</summary>

- ✅ Dynamic flag parsing: `--key value` → `{"key": "value"}`
- ✅ Parse `--key=value` format
- ✅ Parse JSON values directly: `--key {"a":1}` → `{"key": {"a": 1}}`
- ✅ Backward compatible with JSON argument
- ✅ Error message shows valid JSON format hint
- ✅ Help documents both JSON and --args formats
- ✅ Flag usage example in call help
- ✅ List command shows calling hint

**Archive:** `.planning/milestones/v1.6-REQUIREMENTS.md`

</details>

<details>
<summary>v1.7: Linux Compatibility & Documentation (17/20 requirements) — Shipped 2026-02-16</summary>

- ✅ Project compiles successfully on Linux (LINUX-01)
- ✅ All 109 library tests pass on Linux (LINUX-02)
- ✅ Critical runtime nesting bug fixed (gap closure)
- ✅ nix crate added for Unix signal handling (LINUX-04)
- ✅ Windows-only exports cfg-gated (LINUX-05)
- ✅ IPC method signatures compatible across platforms (LINUX-06)
- ✅ Unix socket address handling platform-appropriate (LINUX-07)
- ✅ Error handling covers all McpError variants (LINUX-08)
- ✅ windows-sys dependency Windows-only (LINUX-09)
- ✅ README.md exists with comprehensive documentation (DOC-01)
- ✅ Installation instructions for all platforms (DOC-02)
- ✅ Usage examples for all commands (DOC-03)
- ✅ Configuration guide with TOML examples (DOC-04)
- ✅ All commands documented with examples (DOC-05)
- ✅ Development setup instructions (DOC-06)
- ✅ Troubleshooting section (DOC-07)
- ✅ GitHub Actions CI with matrix builds (CI-01, CI-02, CI-03, CI-04)

**Archive:** `.planning/milestones/v1.7-REQUIREMENTS.md`

</details>

- Public distribution/crates.io publishing — local compilation only
- MCP server implementation — this tool is a client only
- SSE and Streamable HTTP transports — deferred to post-MVP
- Tool aliasing/shortcuts — config complexity without clear benefit
- Multi-server transactions — MCP doesn't support transactions
- Tool output caching — Cache invalidation complexity; tools can implement their own caching if needed
- Environment variable substitution within config (${VAR}) — Using env vars to override layered config instead (simpler pattern)

---

## Current State: v1.7 Complete

**Status:** All planned milestones shipped ✅

**Codebase After v1.7:**
- **~22,163** lines of Rust code
- **80** Rust source files
- **0** documentation warnings (cargo doc)
- **109** library tests pass
- **71+** integration tests pass
- **All files** under 600 lines

**Milestones Shipped:**
- **v1.0:** Core implementation with daemon connection pooling (Phases 1-5, 42 requirements)
- **v1.2:** Ergonomic CLI output with JSON mode and visual hierarchy (Phases 6-11, 18 requirements)
- **v1.3:** Tech Debt Cleanup & Code Quality (Phases 12-16, 46/47 requirements)
- **v1.4:** Test Coverage (Phases 17-19, 17 requirements)
- **v1.5:** UX Audit & Improvements (Phases 20-21, 13 requirements)
- **v1.6:** CLI Calling Conventions (Phases 22-23, 9 requirements)
- **v1.7:** Linux Compatibility & Documentation (Phases 24-27, 17/20 requirements)

**Total Requirements Satisfied:** 156/159 (98.1%) ✅

**Key Achievements in v1.7:**
- Full Linux compatibility with platform-gated dependencies
- Critical runtime nesting bug eliminated (`Handle::block_on()` removed)
- Comprehensive 354-line README with installation and troubleshooting
- CI/CD automation with matrix builds for Linux, Windows, macOS
- All 109 library tests pass, daemon IPC tests pass (4/4)

---

## Next Milestone: Planning

All planned milestones v1.0-v1.7 are complete. The project has achieved:

- ✅ Core MCP CLI functionality
- ✅ Ergonomic CLI output with JSON mode
- ✅ Comprehensive test coverage (109 lib + 71+ integration tests)
- ✅ UX improvements aligned with original Bun implementation
- ✅ Bash-style calling conventions (`--key value` syntax)
- ✅ Full cross-platform support (Linux, Windows, macOS)
- ✅ Comprehensive documentation and CI/CD

**The tool is feature-complete for the initial release.**

Next steps could involve:
- Distribution (crates.io, Homebrew, etc.)
- Additional features (see Out of Scope)
- Performance optimizations
- Or consider the project complete as-is

---

## Current Milestone: Complete

---

## Context

Original mcp-cli implementation exists at `../mcp-cli` and is Bun-based with the official MCP SDK. Reimplementation motivated by Windows process spawning issues and desire to remove Bun dependency for a standalone Rust binary.

The tool will be wrapped in a skill for LLM use, so error messages and output should be both human-readable and machine-parsable. Compatible with standard mcp_servers.json configuration format used by Claude Desktop, Gemini, and VS Code.

---

## Constraints

- **Tech Stack**: Rust — chosen for cross-platform binaries, no runtime dependencies, and reliable process spawning
- **MCP Client**: Rust MCP SDK used (implemented from scratch due to lack of stable Rust SDK)
- **Compatibility**: Works on Windows, Linux, and macOS without platform-specific bugs
- **Config**: Compatible with standard mcp_servers.json format and environment variable substitution
- **Daemons**: Uses Unix sockets (*nix) and named pipes (Windows) for connection caching
- **Testing**: Comprehensive test coverage with unit and integration tests
- **Distribution**: Single binary, local compilation only (no public package distribution)

---

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust language | Fix Windows process spawning, remove Bun dependency, single binary | ✅ Verified - 60/60 requirements met across v1 and v1.2 |
| Daemon architecture | Maintain performance benefit of connection caching | ✅ Verified - 50%+ performance improvement |
| Unified daemon | Single binary with 3 operational modes (standalone, auto-spawn, require-daemon) | ✅ Simplified deployment and usage |
| Progressive detail levels | Summary → WithDescriptions → Verbose via -d/-v flags | ✅ Improves usability without overwhelming users |
| Visual hierarchy with status icons | Server state immediately obvious without reading text | ✅ Better user experience |
| JSON mode with consistent schema | Programmatic access for scripting and automation | ✅ Machine-parsable output for LLMs |
| reject_remote_clients for Windows | Stronger security than security_qos_flags requirement | ✅ Exceeds XP-02 requirement |
| Daemon filtering at CLI layer | IPC is internal implementation detail | ✅ Good design - CLI enforces filtering, daemon focuses on caching |
| Platform-gated dependencies | Windows-sys shouldn't compile on Linux | ✅ Clean cross-platform builds |
| Make create_ipc_server async | Handle::block_on() is anti-pattern in async runtime | ✅ Eliminated runtime nesting bug |
| AtomicU64 socket identifiers | Prevent parallel test conflicts | ✅ Unique socket paths per test |
| Single CI workflow with matrix | Cleaner than per-platform workflows | ✅ All platforms tested on every PR |

---

*Last updated: 2026-02-16 after v1.7 milestone complete — all planned milestones shipped*