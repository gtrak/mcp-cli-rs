# UX Audit: Rust CLI vs Original Bun CLI

## Task 1: Rust CLI --help Output

### Main Help
```
MCP CLI client for tool discovery and execution

Usage: mcp-cli-rs.exe [OPTIONS] [COMMAND]

Commands:
  daemon    Start the connection daemon
  shutdown  Shutdown the running daemon
  list      List all servers and their available tools (CLI-01, DISC-01)
  info      Show details for a specific server (DISC-02)
  tool      Show details for a specific tool (DISC-03)
  call      Execute a tool (EXEC-01, EXEC-02)
  search    Search for tools by name pattern (DISC-04)

Options:
  -c, --config <CONFIG>  Path to configuration file (mcp_servers.toml)
      --no-daemon        Run without daemon (direct mode) - **currently recommended for daemon-mode issues**
      --auto-daemon      Auto-spawn daemon if not running (default behavior - **has known issues on Windows**)
      --require-daemon   Require daemon to be already running (fail if not running)
      --json             Output results as JSON for programmatic use
  -h, --help             Print help
```

### Subcommand Help Summary

| Command | Arguments | Flags | Notes |
|---------|-----------|-------|-------|
| daemon | none | --ttl, --socket-path | Internal --socket-path hidden |
| shutdown | none | none | No additional flags |
| list | none | -d/--describe, -v/--verbose | -d shows descriptions |
| info | <NAME> (server) | none | Simple server lookup |
| tool | <TOOL> | -d/--describe, -v/--verbose | Tool identifier |
| call | <TOOL> [ARGS] | none | Accepts JSON args |
| search | <PATTERN> | -d/--describe, -v/--verbose | Glob pattern |

### Issues Identified

1. **Warning text in help**: "--no-daemon" and "--auto-daemon" have warning text about issues
2. **No --version flag**: --version is not implemented (CLI-02 says handled by clap but doesn't work)
3. **Internal flags exposed**: --socket-path is hidden but shows in some contexts
4. **No examples**: No usage examples in any help text
5. **No environment variable documentation**: No mention of MCP_DAEMON_TTL etc.

---

## Task 2: Original Bun CLI Help Text

### Full Help Output (from printHelp())
```
mcp-cli v0.3.0 - A lightweight CLI for MCP servers

Usage:
  mcp-cli [options]                              List all servers and tools
  mcp-cli [options] info <server>                Show server details
  mcp-cli [options] info <server> <tool>         Show tool schema
  mcp-cli [options] grep <pattern>               Search tools by glob pattern
  mcp-cli [options] call <server> <tool>         Call tool (reads JSON from stdin if no args)
  mcp-cli [options] call <server> <tool> <json>  Call tool with JSON arguments

Formats (both work):
  mcp-cli info server tool                       Space-separated
  mcp-cli info server/tool                       Slash-separated

Options:
  -h, --help               Show this help message
  -v, --version            Show version number
  -d, --with-descriptions  Include tool descriptions
  -c, --config <path>      Path to mcp_servers.json config file

Examples:
  mcp-cli                                        # List all servers
  mcp-cli -d                                     # List with descriptions
  mcp-cli grep "*file*"                          # Search for file tools
  mcp-cli info filesystem                        # Show server tools
  mcp-cli info filesystem read_file             # Show tool schema
  mcp-cli call filesystem read_file '{}'        # Call tool

Environment Variables:
  MCP_NO_DAEMON=1        Disable connection caching
  MCP_DAEMON_TIMEOUT=N   Set daemon idle timeout in seconds
```

### Key Differences

| Aspect | Rust CLI | Bun CLI |
|--------|----------|---------|
| Command style | Explicit subcommands | Implicit (default: info) |
| Search command | `search` | `grep` |
| Version flag | NOT WORKING | `-v, --version` |
| Help examples | NONE | YES - comprehensive |
| Env vars in help | NONE | YES |
| Format docs | NONE | YES - explains slash vs space |
| Default command | list | info (server name required) |

---

## Task 3: UX Gap Comparison

### 1. Command Structure Differences

| Gap | Severity | Rust CLI | Bun CLI |
|-----|----------|----------|---------|
| Search naming | LOW | `search` | `grep` |
| Implicit vs explicit | MEDIUM | Requires explicit subcommand | Defaults to `info` |

**Gap 1: Search command name inconsistency**
- Rust: `mcp search <pattern>`
- Bun: `mcp grep <pattern>`
- Recommendation: Consider aliasing `search` to `grep` or adding grep as alias

### 2. Help Text Clarity Gaps

**Gap 2: No examples in Rust CLI help**
- Rust: No examples anywhere
- Bun: Comprehensive examples section
- Recommendation: Add EXAMPLES section to main help

**Gap 3: No environment variable documentation**
- Rust: Not documented
- Bun: Shows MCP_NO_DAEMON, MCP_DAEMON_TIMEOUT
- Recommendation: Add ENV VARS section to help

**Gap 4: No format explanation**
- Bun explains both "server/tool" and "server tool" formats
- Rust doesn't explain this
- Recommendation: Add to help text

### 3. Flag Differences

**Gap 5: Missing --version flag**
- Rust: --version does NOT work (error: unexpected argument)
- Bun: -v, --version works
- Recommendation: Implement --version per CLI-02

**Gap 6: Version conflicts with verbose**
- Bun uses -v for --version
- Rust uses -v for --verbose
- Resolution: Keep -v for verbose (more common in CLI), keep --version

**Gap 7: No stdin support for tool args**
- Bun: Supports `mcp-cli call server tool` without args (reads from stdin)
- Rust: Requires JSON argument
- Recommendation: Consider adding stdin support

### 4. Warning Text in Help

**Gap 8: Warning text about daemon issues in help**
- Current: "--no-daemon - **currently recommended for daemon-mode issues**"
- This is developer-focused, not user-focused
- Recommendation: Remove warnings from help text, document in separate place

---

## Task 4: Error Message Audit

### Error Scenarios Tested

| Scenario | Rust CLI Error | Bun CLI Error |
|----------|----------------|---------------|
| Missing required arg (info) | "the following required arguments were not provided: <NAME>" | "Error [MISSING_ARGUMENT]: Missing required argument for info: server" |
| Unknown subcommand | "unrecognized subcommand 'run'" | Error [UNKNOWN_SUBCOMMAND]: Unknown subcommand: "run" with suggestion |
| Invalid JSON args | "unexpected argument 'not json' found" | "Error [INVALID_JSON_ARGUMENTS]: Invalid JSON in tool arguments" with suggestion |
| Unknown option | (not tested) | "Error [UNKNOWN_OPTION]: Unknown option: -x" with suggestion |

### Gap 9: Error Message Quality

**Rust Error Messages:**
- Generic clap errors
- No error codes
- No suggestions for fixes
- No available alternatives shown

**Bun Error Messages:**
- Structured error format with type, details, suggestion
- Error codes (CLIENT_ERROR, SERVER_ERROR, NETWORK_ERROR)
- Actionable suggestions
- Lists available alternatives

**Recommendation:** Enhance Rust error messages to match Bun quality

### Gap 10: Server Not Found Error

- Rust: Not tested yet
- Bun: Shows available servers in error message
- Recommendation: Show available servers when server not found

---

## Task 5: Phase 21 Fix List

### Priority 1: High Impact (User Experience)

1. **[FIX-01] Add --version flag**
   - Location: src/cli/entry.rs Cli struct
   - Add `#[derive(Parser)]` with version
   - Expected: `mcp --version` outputs "mcp-cli-rs 0.1.0"

2. **[FIX-02] Add examples to help text**
   - Location: src/cli/command_router.rs Commands enum
   - Add long_about with examples to each command
   - Expected: Each subcommand --help shows examples

3. **[FIX-03] Remove warning text from help**
   - Location: src/cli/entry.rs Cli struct
   - Remove "**currently recommended**" and "**has known issues**" text
   - Expected: Clean help without developer notes

4. **[FIX-04] Add environment variable docs to help**
   - Location: src/cli/entry.rs or help text
   - Document MCP_DAEMON_TTL, MCP_NO_DAEMON
   - Expected: Help shows env vars

### Priority 2: Medium Impact (Error Handling)

5. **[FIX-05] Add "Did you mean?" suggestions for unknown subcommands**
   - Location: error handling
   - When user types "run" suggest "call"
   - Expected: "Unknown command 'run'. Did you mean 'call'?"

6. **[FIX-06] Show available servers in error messages**
   - Location: ServerNotFound error
   - Show list of configured servers
   - Expected: "Server 'xyz' not found. Available: a, b, c"

7. **[FIX-07] Improve JSON argument error messages**
   - Location: InvalidJson error handling
   - Add suggestion about valid JSON format
   - Expected: More helpful error with example

### Priority 3: Low Impact (Nice to Have)

8. **[FIX-08] Add grep as alias for search**
   - Location: src/cli/command_router.rs
   - Support both `search` and `grep` commands
   - Expected: `mcp grep "*file*"` works

9. **[FIX-09] Support stdin for tool args**
   - Location: src/cli/call.rs
   - Accept no-args and read from stdin
   - Expected: `echo '{}' | mcp call server tool` works

10. **[FIX-10] Add format explanation to help**
    - Document "server/tool" vs "server tool" formats
    - Expected: Help explains both formats

---

## Summary

### Must Fix (Phase 21)
- --version flag implementation
- Examples in help text
- Remove warning text from help
- Error message improvements with suggestions

### Should Fix
- Unknown subcommand suggestions
- Server availability in errors
- grep alias for search

### Nice to Have
- stdin support
- Format documentation
