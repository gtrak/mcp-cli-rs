# Feature Research

**Domain:** MCP CLI tools (Model Context Protocol client)
**Researched:** 2025-02-06
**Confidence:** HIGH

**Sources:**
- Original mcp-cli implementation (Bun-based) at ../mcp-cli
- MCP Inspector (official development tool) - GitHub: modelcontextprotocol/inspector
- Complete specification document (46 functional requirements)
- MCP protocol documentation - https://modelcontextprotocol.io/docs/concepts/

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist in an MCP CLI. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Server connection (stdio & HTTP) | Must connect to MCP servers to be useful | MEDIUM | MCP supports both local (stdio) and remote (HTTP/transport) protocols - essential for universal compatibility |
| Server & tool listing | Users need to see what's available | LOW | Without discovery, users can't use the tool at all |
| Tool inspection (schema display) | Users need to understand tool parameters | LOW | JSON Schema must be displayed exactly as server provides it |
| Tool execution with arguments | Primary use case - calling tools | MEDIUM | Must support both inline JSON and stdin input |
| Config file support (mcp_servers.json) | Standard format used by Claude Desktop, VS Code, Gemini | MEDIUM | Must parse mcpServers object with stdio and HTTP server definitions |
| Environment variable substitution | Secure credential management | LOW | `${VAR_NAME}` syntax required for secrets and environment-specific values |
| Error messages with suggestions | Users expect actionable error recovery | MEDIUM | Must list available servers/tools when something's not found |
| Shell-compatibility (stdin, pipes) | CLI tools must work in pipelines | LOW | Auto-detecting stdin input when TTY not present |
| Tool search (glob patterns) | Finding tools across many servers quickly | LOW | Critical for productivity when many tools exist |
| Connection timeout handling | Operations shouldn't hang forever | LOW | Default 30-minute timeout from MCP spec context |
| Help & version flags | Every CLI tool has these | LOW | `--help`, `--version` are universally expected |
| Exit code conventions | Scripts need to know success/failure | LOW | 0=success, non-zero=error with different codes for error types |

### Differentiators (Competitive Advantage)

Features that set mcp-cli-rs apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Connection daemon (lazy caching) | 50%+ performance improvement for repeated calls | HIGH | Auto-spawns on first use, self-terminates on idle timeout (60s default) |
| Exponential backoff retry | Automatic recovery from transient failures | MEDIUM | Retries 3x with jitter for network timeouts, HTTP 502/503/504/429 |
| Tool filtering (allowed/disabled) | Security sandboxing for production environments | LOW | Glob patterns to restrict dangerous operations (delete_*, write_*, etc.) |
| Structured error messages | AI agent compatibility - machine-parsable errors | MEDIUM | Error code, details, suggestion fields for both humans and agents |
| Concurrent parallel connections | Faster when listing/searching many servers | MEDIUM | Configurable concurrency limit (default 5) |
| Cross-platform single binary | No runtime dependencies (Bun/Node) | MEDIUM | Rust compilation provides Windows/Linux/macOS binaries |
| Connection daemon with cross-platform IPC | Uses Unix sockets (*nix) and named pipes (Windows) | HIGH | Solves Windows process spawning issues in Bun implementation |
| Environment variable defaults (strict vs non-strict) | Flexibility for different environments | LOW | STRICT mode errors on missing vars, non-strict warns and continues |
| Server instructions display | Context-aware usage guidance | LOW | Shows server-provided instructions when available |
| Auto-detect stdin input | No explicit `-` flag needed for pipes | LOW | Checks if TTY is absent, reads stdin automatically |
| Colored output (with NO_COLOR support) | Better readability, respects terminal preferences | LOW | Colors when stdout is TTY, unless `NO_COLOR` is set |
| Graceful signal handling | Proper cleanup of connections/daemons | MEDIUM | SIGINT/SIGTERM cleanup for no orphaned processes |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| MCP server implementation | Users sometimes think CLI tool should also be a server | Out of scope - mcp-cli is a client only. Server implementation adds massive complexity and blurs the architecture boundary. | Use official MCP servers from https://github.com/modelcontextprotocol/servers or build separate server project |
| Interactive REPL mode | "Would be nice to explore tools interactively" | CLI tools are meant for scripting and automation. REPL creates complexity in command-line argument handling, state management, and output formatting. | Use MCP Inspector (UI mode) for interactive exploration - that's its purpose |
| Shell completion scripts | Tab completion for servers/tools would save time | Requires maintaining separate completion scripts for bash/zsh/fish, keeping them in sync with changing config, and handling dynamic tool discovery. | Use `mcp-cli` for discovery, then tools for auto-completion in your editor's LLM integration |
| Persistent connection storage (database) | "Save favorite tool calls to reuse" | Adds database dependency, complicates configuration management, and doesn't align with stateless CLI philosophy. | Use shell scripts or makefiles to document common command patterns |
| Real-time tool output streaming | "See tool results as they happen" | Most MCP tools return complete results, not streaming. Supporting streaming adds complexity to output formatting and breaks JSON parsability. | Use `--json` output mode and process results line-by-line if server supports streaming |
| Tool aliasing/shortcuts | "Rename filesystem read_file to just 'read'" | Creates configuration complexity, doesn't work with tool discovery, and breaks when servers change. | Use shell functions in `.bashrc` or `.zshrc` for personal shortcuts (e.g., `alias mcp-read='mcp-cli call filesystem read_file'`) |
| Multi-server transactions | "Roll back operations across multiple servers if one fails" | MCP doesn't support transactions. Implementing this requires complex state tracking and can't be guaranteed when servers are remote. | Use error handling in your scripts to handle failures gracefully (check exit codes, parse error messages) |
| Tool timeout presets per server | "GitHub API tools should timeout in 10s, filesystem in 60s" | Overcomplicates configuration. MCP protocol already has server-side timeouts. Client should just enforce a single reasonable default (30 min) with override via env var. | Use `MCP_TIMEOUT` environment variable for timeouts when needed, let servers handle their own timeouts |
| Tool output caching | "Don't re-fetch the same file if I just read it" | Adds cache invalidation complexity. Developers will be confused when stale data is served, and clearing cache adds another command. | Tools like filesystem can implement their own caching; CLI should remain stateless |

---

## Feature Dependencies

```
[Config parsing/Env substitution]
    └──requires──> [Transport detection (stdio/HTTP)]
                   └──requires──> [Server connection/lifecycle]
                                   ├──requires──> [Tool discovery (list tools)]
                                   │              ├──enhances──> [Tool search (grep)]
                                   │              └──requires──> [Tool inspection (info)]
                                   │                              └──enhances──> [Tool execution (call)]
                                   └──requires──> [Tool execution (call)]
                                                  ├──requires──> [JSON argument validation]
                                                  ├──enhances──> [Error handling/messages]
                                                  └──enhances──> [Retry logic]

[Connection daemon]
    ├──conflicts──> [Direct connection mode]
    └──enhances──> [Tool execution (call)] (via cached connections)

[Tool filtering (allowed/disabled)]
    ├──filters──> [Tool discovery (list tools)]
    ├──filters──> [Tool search (grep)]
    └──blocks──> [Tool execution (call)] for filtered tools

[Concurrent parallel connections]
    └──requires──> [Server connection/lifecycle]
```

### Dependency Notes

- **Config parsing requires transport detection**: Need to determine stdio vs HTTP before establishing connections. stdio requires command/args fields, HTTP requires url/headers.
- **Transport detection requires server connection**: Can't connect until you know which transport to use and have validated config fields.
- **Tool discovery enhances tool search**: You can't search for tools without first discovering them. Search is just filtered discovery.
- **Tool inspection requires tool discovery**: You need the tool schema from discovery to display it in `info` command.
- **Tool execution requires tool inspection**: While you can call a blind tool, good UX requires showing schema first so users know what arguments are needed.
- **Retry logic requires tool execution**: Retries only make sense during actual execution, not during discovery/inspection.
- **Connection daemon conflicts with direct connection mode**: Either you use the daemon for caching (default) or force direct connections with `MCP_NO_DAEMON=1`. These are mutually exclusive operating modes.
- **Tool filtering affects all tool operations**: Filtering happens at discovery level, so it affects list, grep, and call commands uniformly.
- **Concurrent connections require server connection lifecycles**: Parallel listing/searching requires robust connection handling that can manage multiple simultaneous connections without race conditions.
- **Error handling enhances execution**: Good error messages are critical for user experience during tool execution, where most errors occur.

---

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [ ] **Server connection (stdio & HTTP)** - Core capability without which nothing works
- [ ] **Config parsing** - Parse mcp_servers.json with both stdio and HTTP server definitions
- [ ] **Environment variable substitution** - `${VAR_NAME}` for credentials and secrets
- [ ] **Server/tool listing** - `mcp-cli` (or `mcp-cli info list`) to show all servers and tools
- [ ] **Tool inspection** - `mcp-cli info <server> <tool>` to display JSON Schema
- [ ] **Tool execution** - `mcp-cli call <server> <tool> <json>` and stdin mode
- [ ] **Tool search** - `mcp-cli grep <pattern>` with glob matching
- [ ] **Basic error messages** - Clear errors for missing servers/tools, invalid JSON
- [ ] **Help & version** - `--help`, `--version` flags
- [ ] **Exit codes** - Non-zero on error for scripting compatibility

**Why this set:** These features enable the complete workflow: configure servers → discover tools → inspect schemas → execute tools → handle errors. Anything beyond these is performance optimization or convenience features.

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Connection daemon** - Performance optimization for cached connections
- [ ] **Retry logic** - Reliability improvement for transient errors
- [ ] **Concurrent parallel connections** - Speed improvement for listing/searching
- [ ] **Structured error messages** - AI agent compatibility
- [ ] **Tool filtering** - Security feature for production environments
- [ ] **Colored output** - Readability improvement
- [ ] **Graceful signal handling** - Proper cleanup on SIGINT/SIGTERM

**Trigger for adding:** Core functionality is working, users are using it, and real-world feedback indicates performance/reliability pain points or security concerns.

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Server-specific timeout configuration** - Advanced performance tuning
- [ ] **Config validation command** - Pre-flight checking without connecting
- [ ] **Tool benchmarking** - Measure tool execution times
- [ ] **Diff tool results** - Compare tool outputs between calls
- [ ] **Batch tool execution** - Execute multiple tools in one command
- [ ] **Tool chaining** - Pipe tool output as input to another tool
- [ ] **Configuration templates** - Pre-built configs for common server setups

**Why defer:** These are convenience features for power users. The core value proposition is reliable MCP server interaction, which the MVP covers. These features don't prevent users from being productive with the tool.

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Server connection (stdio & HTTP) | HIGH | MEDIUM | P1 |
| Server/tool listing | HIGH | LOW | P1 |
| Tool inspection | HIGH | LOW | P1 |
| Tool execution | HIGH | MEDIUM | P1 |
| Config parsing | HIGH | MEDIUM | P1 |
| Tool search (grep) | HIGH | LOW | P1 |
| Environment variable substitution | MEDIUM | LOW | P1 |
| Basic error messages | HIGH | MEDIUM | P1 |
| Help & version | HIGH | LOW | P1 |
| Exit codes | MEDIUM | LOW | P1 |
| Connection daemon | MEDIUM | HIGH | P2 |
| Retry logic | MEDIUM | MEDIUM | P2 |
| Structured error messages | MEDIUM | MEDIUM | P2 |
| Tool filtering | MEDIUM | LOW | P2 |
| Concurrent parallel connections | LOW | MEDIUM | P2 |
| Colored output | LOW | LOW | P2 |
| Graceful signal handling | MEDIUM | MEDIUM | P2 |
| Server-specific timeouts | LOW | LOW | P3 |
| Config validation command | LOW | MEDIUM | P3 |
| Tool benchmarking | LOW | LOW | P3 |
| Diff tool results | LOW | MEDIUM | P3 |
| Batch tool execution | MEDIUM | HIGH | P3 |
| Tool chaining | HIGH | HIGH | P3 |
| Configuration templates | LOW | MEDIUM | P3 |

**Priority key:**
- **P1**: Must have for MVP - core functionality without which tool is unusable
- **P2**: Should have, add when possible - performance, reliability, security improvements
- **P3**: Nice to have, future consideration - convenience features for power users

---

## Competitor Feature Analysis

| Feature | Original mcp-cli (Bun) | MCP Inspector CLI | Our Approach (Rust rewrite) |
|---------|------------------------|-------------------|---------------------------|
| Server connections | stdio + HTTP | stdio + SSE + Streamable HTTP | stdio + HTTP (match original scope for MVP) |
| Connection daemon | ✅ Unix socket only | ❌ No daemon (direct only) | ✅ Unix sockets + named pipes (cross-platform) |
| Tool filtering | ✅ allowedTools/disabledTools | ❌ Not implemented | ✅ Keep same patterns as original |
| Retry logic | ✅ Exponential backoff | ❌ Basic timeout only | ✅ Same as original (validated pattern) |
| Error messages | ✅ Structured + suggestions | ✅ Structured | ✅ Improve on original with AI-agent friendliness |
| Concurrent connections | ✅ Configurable limit | ❌ Single-threaded | ✅ Same as original |
| Environment variables | ✅ `${VAR}` substitution | ✅ ✅ | ✅ Keep same syntax, add strict/non-strict modes |
| Cross-platform binary | ❌ Bun runtime required | ❌ Node.js runtime required | ✅ Single binary (no runtime deps) |
| Stdin auto-detect | ✅ Yes | ✅ ✅ Yes | ✅ Keep this - it's excellent UX |
| Colored output | ✅ ✅ (with NO_COLOR) | ❌ Not implemented | ✅ Same as original |
| Tool search (grep) | ✅ Glob patterns | ❌ No search command | ✅ Keep - essential for discovery |
| Server instructions display | ✅ ✅ | ✅ Via UI | ✅ Keep from original CLI |

**Key insights from comparison:**
- Original mcp-cli has excellent feature set validated by real-world use - we should keep most of it
- MCP Inspector CLI is more focused on server development (not production use) - less relevant for our goals
- Rust rewrite's main advantage is cross-platform single binary + reliable Windows process spawning
- No gap in original feature set that we need to fill - focus on quality and reliability improvements

---

## MVP Feature Walkthrough

This demonstrates how the MVP features enable a complete user workflow:

```bash
# 1. Create config (environment variables substituted at load time)
cat > mcp_servers.json <<EOF
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "${WORK_DIR}"],
      "env": {
        "AUTH_TOKEN": "${MCP_TOKEN}"
      }
    },
    "github": {
      "url": "https://api.github.com/mcp",
      "headers": {
        "Authorization": "Bearer ${GITHUB_TOKEN}"
      }
    }
  }
}
EOF

export WORK_DIR="/home/user/projects"
export MCP_TOKEN="secret-token"
export GITHUB_TOKEN="ghp_xxx"

# 2. List all servers and tools (discovers available functionality)
$ mcp-cli
filesystem
  • read_file
  • write_file
  • list_directory
github
  • search_repositories
  • get_file_contents
  • create_or_update_file

# 3. Search for specific tools (productivity feature)
$ mcp-cli grep "*file*"
github/get_file_contents
github/create_or_update_file
filesystem/read_file
filesystem/write_file

# 4. Inspect tool schema (understand requirements before calling)
$ mcp-cli info filesystem read_file
Tool: read_file
Server: filesystem

Description:
  Read the contents of a file

Input Schema:
  {
    "type": "object",
    "properties": {
      "path": { "type": "string", "description": "File path to read" }
    },
    "required": ["path"]
  }

# 5. Execute tool with inline JSON
$ mcp-cli call filesystem read_file '{"path": "/home/user/projects/README.md"}'
# [Content extracted and displayed]

# 6. Execute tool with stdin (shell-friendly for complex JSON)
$ cat <<EOF | mcp-cli call github search_repositories
{
  "query": "rust cli tools",
  "per_page": 5
}
EOF
# [Results returned]

# 7. Error recovery when something goes wrong
$ mcp-cli call unknown_server some_tool "{}"
Error [SERVER_NOT_FOUND]: Server "unknown_server" not found in config
  Available servers: filesystem, github
  Suggestion: Use one of: mcp-cli info filesystem, mcp-cli info github
# [Exit code: 1]
```

This walkthrough shows that the MVP features enable a complete, useful CLI tool. Post-MVP features (daemon, retry, filtering, etc.) add performance, reliability, and security but don't change the fundamental functionality.

---

*Feature research for: MCP CLI tools (Model Context Protocol client)*
*Researched: 2025-02-06*
