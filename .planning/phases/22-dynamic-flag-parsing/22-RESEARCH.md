---
phase: 22
type: research
focus: dynamic flag parsing implementation
---

## Research: Dynamic Flag Parsing for `mcp call`

### Implementation Approach

**Challenge:** Clap uses static type definitions, but we need dynamic `--key value` parsing.

**Solution:** Use clap's `ArgMatches::remaining_args()` or `trailing_var_arg` to capture unrecognized flags.

### Key Techniques

1. **Use `...rest` to capture trailing arguments:**
   ```rust
   Call {
       tool: String,
       #[arg(last = true, allow_hyphen_values = true)]
       args: Option<String>,
   }
   ```

2. **Use `ArgMatches::args_present()`** to detect if any dynamic args were provided

3. **Iterate remaining args** to build JSON object:
   ```rust
   for arg in matches.args().keys() {
       // Parse --key and value pairs
   }
   ```

### Parsing Logic

For `--key value` → `{"key": "value"}`:
- Detect `--` prefix on keys
- Next argument is the value
- Handle `=` separator separately
- Parse JSON values with `serde_json::from_str`

### Backward Compatibility

- If no `--` flags provided, use existing JSON argument path
- If `--` flags provided, ignore JSON argument (or error if both provided)

### File Changes Required

1. `src/cli/command_router.rs` - Modify Call command structure
2. `src/cli/call.rs` - Update argument parsing logic (if needed)

### Testing Considerations

- Unit tests for parsing `--key value`, `--key=value`, `--key {"a":1}`
- Integration tests for CLI invocation
