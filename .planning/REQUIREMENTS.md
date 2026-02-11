# Requirements: MCP CLI Rust Rewrite

**Defined:** 2025-02-06
**Core Value:** Reliable cross-platform MCP server interaction without dependencies

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Configuration

- [ ] **CONFIG-01**: Parse server configuration from mcp_servers.toml file with support for both stdio (command, args, env, cwd) and HTTP (url, headers) server definitions
- [ ] **CONFIG-02**: Search for configuration files in priority order: explicit path (MCP_CONFIG_PATH), command line (-c/--config), current directory mcp_servers.toml, home directory .mcp_servers.toml, ~/.config/mcp/mcp_servers.toml
- [ ] **CONFIG-03**: Support environment variable overrides for layered configuration (env vars take precedence over config file values)
- [ ] **CONFIG-04**: Validate TOML structure and display clear errors for missing fields, invalid TOML, or server misconfiguration
- [ ] **CONFIG-05**: Display warning message when no servers are configured

### Server Connections

- [ ] **CONN-01**: Connect to MCP servers via stdio transport (local process spawning)
- [ ] **CONN-02**: Connect to MCP servers via HTTP transport (remote API)
- [ ] **CONN-03**: Handle connection lifecycle (connect, disconnect, timeouts)
- [ ] **CONN-04**: Use tokio::process for async process spawning with kill_on_drop(true) to prevent zombie processes on Windows
- [x] **CONN-05**: Implement connection daemon using Unix sockets (*nix) and Windows named pipes for cross-platform IPC
- [x] **CONN-06**: Spawn daemon lazily on first access with idle timeout (60s default)
- [x] **CONN-07**: Detect configuration changes and spawn new daemon when cached config becomes stale
- [x] **CONN-08**: Cleanup orphaned daemon processes and sockets on startup

### Discovery & Search

- [ ] **DISC-01**: List all configured servers with their available tools when no subcommand is provided
- [ ] **DISC-02**: Display server details including transport type, connection information, tool count, and server instructions
- [ ] **DISC-03**: Display tool details (name, description, input JSON Schema) for inspection
- [ ] **DISC-04**: Search tool names (not server names) using glob patterns with wildcards (*, ?, etc.)
- [x] **DISC-05**: Process servers in parallel with configurable concurrency limits (default 5)
- [ ] **DISC-06**: Support optional display of descriptions via -d/--with-descriptions flag

### Tool Execution

- [ ] **EXEC-01**: Execute tools with JSON arguments provided either inline or via stdin pipe
- [ ] **EXEC-02**: Automatically detect stdin input when TTY is not present (pipe redirection)
- [ ] **EXEC-03**: Format tool call results to extract text content for CLI-friendly display
- [ ] **EXEC-04**: Validate JSON tool arguments and display parse errors with context
- [x] **EXEC-05**: Implement automatic retry logic with exponential backoff for transient errors (network timeouts, HTTP 502/503/504/429)
- [x] **EXEC-06**: Respect overall operation timeout (default 1800s) and stop retries if budget exhausted
- [x] **EXEC-07**: Configure retry limits (max 3 attempts, base 1000ms delay)

### Tool Filtering

- [ ] **FILT-01**: Filter tool availability based on server configuration using allowedTools glob patterns
- [ ] **FILT-02**: Filter tool availability based on server configuration using disabledTools glob patterns
- [ ] **FILT-03**: Ensure disabledTools patterns take precedence over allowedTools patterns when both are defined
- [ ] **FILT-04**: Display error message when user attempts to call disabled tool
- [ ] **FILT-05**: Support glob pattern wildcards (*, ?) in filter rules

### Error Handling

- [ ] **ERR-01**: Provide structured error messages with error type, message, details, and actionable recovery suggestions
- [ ] **ERR-02**: Display context-aware error suggestions (e.g., list available servers when server not found)
- [ ] **ERR-03**: Implement exit code conventions: 0 for success, 1 for client errors, 2 for server errors, 3 for network errors
- [x] **ERR-04**: Display colored terminal output when stdout is a TTY and NO_COLOR is not set
- [ ] **ERR-05**: Capture and forward stderr output from stdio-based MCP servers to the user
- [ ] **ERR-06**: Handle ambiguous commands (e.g., "server tool" without subcommand) and prompt user to specify info vs call
- [x] **ERR-07**: Warn when some servers fail to connect during parallel operations

### CLI Support

- [ ] **CLI-01**: Display help information when -h/--help flag is provided
- [ ] **CLI-02**: Display version information when -v/--version flag is provided
- [ ] **CLI-03**: Support custom config file path via -c/--config command line option
- [x] **CLI-04**: Gracefully handle signals (SIGINT, SIGTERM) with proper cleanup of connections and resources
- [ ] **CLI-05**: Support both space-separated (server tool) and slash-separated (server/tool) argument formats for info, grep, and call commands

### Cross-Platform Support

- [x] **XP-01**: Test stdio process spawning on Windows and ensure no zombie processes remain
- [ ] **XP-02**: Implement Windows named pipe security flags (security_qos_flags) to prevent privilege escalation
- [ ] **XP-03**: Ensure MCP protocol compliance for stdio transport (newline-delimited messages, no embedded newlines)
- [ ] **XP-04**: Validate connection daemon functionality on Linux, macOS, and Windows

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### TBD
(None identified yet)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Bug-for-bug compatibility with Bun implementation | Opportunity to improve where reasonable; focus on core functionality parity |
| Public distribution/crates.io publishing | For personal use only, local compilation sufficient |
| MCP server implementation | This tool is a client only; server implementation is separate project |
| Interactive REPL mode | CLI tools are for scripting/automation; MCP Inspector provides UI-based exploration |
| Shell completion scripts | Maintenance burden for dynamic tool discovery; use editor integrations instead |
| Persistent connection storage (database) | Adds database dependency; CLI should remain stateless |
| SSE and Streamable HTTP transports | Original implementation uses basic HTTP only; defer to post-MVP |
| Tool aliasing/shortcuts | Config complexity without clear benefit over shell aliases |
| Multi-server transactions | MCP doesn't support transactions; not feasible to implement generically |
| Tool output caching | Cache invalidation complexity; tools can implement their own caching if needed |
| Environment variable substitution within config (${VAR}) | Using env vars to override layered config instead (simpler pattern) |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CONFIG-01 | Phase 1 | Pending |
| CONFIG-02 | Phase 1 | Pending |
| CONFIG-03 | Phase 1 | Pending |
| CONFIG-04 | Phase 1 | Pending |
| CONFIG-05 | Phase 1 | Pending |
| CONN-01 | Phase 1 | Pending |
| CONN-02 | Phase 1 | Pending |
| CONN-03 | Phase 1 | Pending |
| CONN-04 | Phase 1 | Pending |
| CONN-05 | Phase 2 | Complete |
| CONN-06 | Phase 2 | Complete |
| CONN-07 | Phase 2 | Complete |
| CONN-08 | Phase 2 | Complete |
| DISC-01 | Phase 1 | Pending |
| DISC-02 | Phase 1 | Pending |
| DISC-03 | Phase 1 | Pending |
| DISC-04 | Phase 1 | Pending |
| DISC-05 | Phase 3 | Complete |
| DISC-06 | Phase 1 | Pending |
| EXEC-01 | Phase 1 | Pending |
| EXEC-02 | Phase 1 | Pending |
| EXEC-03 | Phase 1 | Pending |
| EXEC-04 | Phase 1 | Pending |
| EXEC-05 | Phase 3 | Complete |
| EXEC-06 | Phase 3 | Complete |
| EXEC-07 | Phase 3 | Complete |
| FILT-01 | Phase 4 | Pending |
| FILT-02 | Phase 4 | Pending |
| FILT-03 | Phase 4 | Pending |
| FILT-04 | Phase 4 | Pending |
| FILT-05 | Phase 4 | Pending |
| ERR-01 | Phase 1 | Pending |
| ERR-02 | Phase 1 | Pending |
| ERR-03 | Phase 1 | Pending |
| ERR-04 | Phase 3 | Complete |
| ERR-05 | Phase 1 | Pending |
| ERR-06 | Phase 1 | Pending |
| ERR-07 | Phase 3 | Complete |
| CLI-01 | Phase 1 | Pending |
| CLI-02 | Phase 1 | Pending |
| CLI-03 | Phase 1 | Pending |
| CLI-04 | Phase 3 | Complete |
| CLI-05 | Phase 4 | Pending |
| XP-01 | Phase 8 | Complete |
| XP-02 | Phase 4 | Pending |
| XP-03 | Phase 1 | Pending |
| XP-04 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 42 total
- Mapped to phases: 42 ✅
- Unmapped: 0

**Phase Distribution:**
- Phase 1: 25 requirements (Configuration, Core Connections, Discovery, Basic Execution, Basic Errors, CLI Foundation)
- Phase 2: 4 requirements (Connection Daemon, IPC)
- Phase 3: 6 requirements (Concurrency, Retry, Colored Output, Signal Handling)
- Phase 4: 7 requirements (Tool Filtering, Argument Formats, Windows Validation, Cross-Platform Daemon)

## v1.2 Requirements

Requirements for ergonomic CLI output improvements.

### Output Formatting

- [ ] **OUTP-01**: Tool listing shows parameter overview (names, types, required/optional status) in help-style format
- [ ] **OUTP-02**: Progressive detail levels via flags: default (summary) → `-d` (with descriptions) → `-v` (verbose with full schema)
- [ ] **OUTP-03**: Default `list` command shows tool count and brief descriptions per server
- [ ] **OUTP-04**: Multi-server listings have clear visual hierarchy (server headers, grouped tools)
- [ ] **OUTP-05**: Consistent formatting across all commands (list, info, grep, call)
- [ ] **OUTP-06**: Parameter display uses standard CLI conventions (e.g., `name <type>` for required, `name [type]` for optional)

### Output Modes

- [x] **OUTP-07**: JSON output mode (`--json` flag) for programmatic use and scripting
- [x] **OUTP-08**: JSON output includes complete tool metadata (name, description, parameters, schema)
- [x] **OUTP-09**: Plain text mode (`--no-color` or when piped) works correctly for all commands
- [x] **OUTP-10**: Machine-readable output follows consistent schema across all commands

### Tool Discovery UX

- [ ] **OUTP-11**: Tool descriptions are prominently displayed (not truncated in default view)
- [ ] **OUTP-12**: Usage hints shown in tool listings (e.g., "Use `mcp info server tool` for full schema")
- [ ] **OUTP-13**: Server status clearly indicated (connected, failed, disabled tools present)
- [ ] **OUTP-14**: Tool search (grep) results show context (server name + tool description)
- [ ] **OUTP-15**: Empty states have helpful messages (no servers configured, no tools found)

### Error & Warning Display

- [ ] **OUTP-16**: Error messages maintain consistent format with context and suggestions
- [ ] **OUTP-17**: Warnings are visually distinct but not overwhelming
- [ ] **OUTP-18**: Partial failures (some servers down) show which succeeded and which failed

## Traceability (v1.2)

| Requirement | Phase | Status |
|-------------|-------|--------|
| OUTP-01 | Phase 6 | Complete |
| OUTP-02 | Phase 6 | Complete |
| OUTP-03 | Phase 6 | Complete |
| OUTP-04 | Phase 6 | Complete |
| OUTP-05 | Phase 6 | Complete |
| OUTP-06 | Phase 6 | Complete |
| OUTP-07 | Phase 7 | Complete |
| OUTP-08 | Phase 7 | Complete |
| OUTP-09 | Phase 7 | Complete |
| OUTP-10 | Phase 7 | Complete |
| OUTP-11 | Phase 6 | Complete |
| OUTP-12 | Phase 6 | Complete |
| OUTP-13 | Phase 6 | Complete |
| OUTP-14 | Phase 6 | Complete |
| OUTP-15 | Phase 6 | Complete |
| OUTP-16 | Phase 6 | Complete |
| OUTP-17 | Phase 6 | Complete |
| OUTP-18 | Phase 6 | Complete |

**v1.2 Coverage:**
- v1.2 requirements: 18 total
- Mapped to phases: 18 ✅ (roadmap complete)
- Unmapped: 0

**Phase Distribution:**
- Phase 6: 14 requirements (Output Formatting, Tool Discovery UX, Error Display)
- Phase 7: 4 requirements (JSON Output, Machine-Readable Modes)

---
*Requirements defined: 2025-02-06 (v1), 2026-02-10 (v1.2)*
*Last updated: 2026-02-10 after v1.2 milestone initialization*
