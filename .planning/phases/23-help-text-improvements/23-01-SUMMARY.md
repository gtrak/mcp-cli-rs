---
phase: 23-help-text-improvements
plan: 01
subsystem: cli
tags: [cli, help-text, error-messages, user-experience]

# Dependency graph
requires:
  - phase: 22-cli-calling-conventions
    provides: Dynamic flag parsing with parse_arguments function
provides:
  - Improved error messages showing valid JSON format hint
  - Call command help documenting both JSON and --args formats
  - List command showing calling hint
affects: [future phases, user-experience]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  modified:
    - src/cli/call.rs - Error message updated with JSON format hint
    - src/cli/command_router.rs - Call command help enhanced
    - src/cli/formatters.rs - List output includes calling hint

key-decisions:
  - "Error message now shows valid JSON format hint"

patterns-established:
  - "Error messages should guide users toward valid usage"

# Metrics
duration: 1min 24sec
completed: 2026-02-14
---

# Phase 23 Plan 1: Help Text Improvements Summary

**Improved CLI error messages and help text with JSON format hints and calling guidance**

## Performance

- **Duration:** 1min 24sec
- **Started:** 2026-02-14T14:54:46Z
- **Completed:** 2026-02-14T14:56:10Z
- **Tasks:** 4/4
- **Files modified:** 3

## Accomplishments
- Error message in argument parsing shows valid JSON format hint
- Call command help now clearly documents both JSON and --args flag formats
- List command output shows hint for calling tools: `mcp call <server>/<tool> --key value`

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix error message to show valid JSON format** - `cd52c27` (fix)
2. **Task 2: Document both formats in call help** - `c7ba4fd` (docs)
3. **Task 3: Add flag usage example to call help** - `c7ba4fd` (docs) [combined with Task 2]
4. **Task 4: Update list command to show calling hint** - `650a951` (feat)

**Plan metadata:** (included above)

## Files Created/Modified
- `src/cli/call.rs` - Error message now shows: "Use JSON format: {\"key\": \"value\"} or flags: --key value"
- `src/cli/command_router.rs` - Call help now includes description: "Supports two argument formats: JSON or flag style"
- `src/cli/formatters.rs` - Added calling hint: "Call with: 'mcp call <server>/<tool> --key value'"

## Decisions Made
- Error message enhancement: Changed from "Expected --key value pair" to show both JSON and flag formats
- Help text enhancement: Added clear description of supported argument formats

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Help text improvements complete
- Ready for any remaining v1.6 work

---
*Phase: 23-help-text-improvements*
*Completed: 2026-02-14*
