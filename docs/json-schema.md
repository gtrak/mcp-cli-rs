# JSON Output Schema

The MCP CLI supports JSON output mode via the `--json` flag for programmatic use.

## Usage

Add `--json` to any command:

```bash
mcp list --json
mcp info server/tool --json
mcp call server/tool '{"arg": "value"}' --json
mcp search "pattern" --json
```

## Schema Reference

### List Command

The `list --json` command displays all configured servers and their tools:

```json
{
  "servers": [
    {
      "name": "server-name",
      "status": "connected",
      "tool_count": 5,
      "tools": [
        {
          "name": "tool-name",
          "description": "Tool description",
          "input_schema": { ... }
        }
      ],
      "error": null
    }
  ],
  "total_servers": 1,
  "connected_servers": 1,
  "failed_servers": 0,
  "total_tools": 5
}
```

**Fields:**

- `servers`: Array of server information objects
- `servers[].name`: Server name from configuration
- `servers[].status`: Connection status ("connected", "failed")
- `servers[].tool_count`: Number of tools available on this server
- `servers[].tools`: Array of tool descriptors (only when connected)
- `servers[].tools[].name`: Tool name
- `servers[].tools[].description`: Tool description
- `servers[].tools[].input_schema`: JSON Schema for tool input
- `servers[].error`: Error message if connection failed (null otherwise)
- `total_servers`: Total number of configured servers
- `connected_servers`: Number of successfully connected servers
- `failed_servers`: Number of servers that failed to connect
- `total_tools`: Total number of tools across all connected servers

### Info Command

The `info server/tool --json` command displays detailed information about a specific tool:

```json
{
  "name": "tool-name",
  "description": "Tool description",
  "server": "server-name",
  "parameters": [
    {
      "name": "param-name",
      "param_type": "string",
      "required": true,
      "description": "Parameter description"
    }
  ],
  "input_schema": { ... }
}
```

**Fields:**

- `name`: Tool name
- `description`: Tool description (may contain newlines)
- `server`: Server name where this tool is available
- `parameters`: Array of parameter information objects
- `parameters[].name`: Parameter name
- `parameters[].param_type`: Parameter type (string, number, boolean, object, array)
- `parameters[].required`: Boolean indicating if parameter is required
- `parameters[].description`: Parameter description
- `input_schema`: Full JSON Schema for tool input validation

### Call Command (Success)

The `call server/tool '{"arg": "value"}' --json` command displays tool execution results:

```json
{
  "server": "server-name",
  "tool": "tool-name",
  "status": "success",
  "result": { ... },
  "metadata": {
    "timestamp": "2026-02-10T10:30:00Z",
    "retry_count": null
  }
}
```

**Fields:**

- `server`: Server name
- `tool`: Tool name
- `status`: Execution status ("success" or "error")
- `result`: Tool execution result (present only on success)
- `metadata`: Execution metadata
- `metadata.timestamp`: ISO 8601 timestamp (UTC)
- `metadata.retry_count`: Number of retries performed (null if none)

### Call Command (Error)

When tool execution fails:

```json
{
  "server": "server-name",
  "tool": "tool-name",
  "status": "error",
  "error": {
    "message": "Error description",
    "code": 123
  },
  "metadata": {
    "timestamp": "2026-02-10T10:30:00Z"
  }
}
```

**Fields:**

- `server`: Server name
- `tool`: Tool name
- `status`: Always "error" on failure
- `error`: Error details
- `error.message`: Error message description
- `error.code`: Optional error code (integer)
- `metadata`: Execution metadata
- `metadata.timestamp`: ISO 8601 timestamp (UTC)

### Search Command

The `search "pattern" --json` command searches for tools matching a pattern:

```json
{
  "pattern": "search-pattern",
  "total_matches": 3,
  "matches": [
    {
      "server": "server-name",
      "tool": "tool-name",
      "description": "Tool description"
    }
  ],
  "failed_servers": []
}
```

**Fields:**

- `pattern`: The search pattern used
- `total_matches`: Total number of matching tools across all servers
- `matches`: Array of match results
- `matches[].server`: Server name where match was found
- `matches[].tool`: Tool name
- `matches[].description`: Tool description
- `failed_servers`: Array of server names that failed during search

## Notes

- **No ANSI color codes**: JSON output never includes ANSI escape sequences for colors
- **Consistent schema**: All JSON outputs follow consistent patterns with status and metadata
- **ISO 8601 timestamps**: All timestamps use ISO 8601 format in UTC
- **Optional fields**: Fields with `null` values are omitted from output (using `skip_serializing_if`)
- **Plain text mode compliance**: JSON output respects OUTP-09 - always produces plain text, never colored

## Examples

### Get list of all servers and tools

```bash
mcp list --json | jq '.servers[] | {name, status, tool_count}'
```

### Find tool information

```bash
mcp info serena/read_file --json | jq '.parameters'
```

### Execute tool and process result

```bash
RESULT=$(mcp call serena/read_file --json '{"path": "README.md"}')
echo "$RESULT" | jq -r '.result'
```

### Search for tools

```bash
mcp search "file" --json | jq -r '.matches[] | "\(.server)/\(.tool)"'
```

## Error Handling

All error responses use the consistent `status: "error"` format:

```json
{
  "status": "error",
  "error": {
    "message": "Error description"
  }
}
```

This makes programmatic error handling straightforward:

```bash
mcp call server/tool --json '{"arg": "value"}' | \
  jq -e 'select(.status == "error") | .error.message'
```
