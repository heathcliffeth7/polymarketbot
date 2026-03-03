---
phase: 07-price-data-integrity-fixes
plan: 01
subsystem: bot-runner
tags: [rust, price-integrity, cross-token-contamination, previous-price, flow-engine]

# Dependency graph
requires:
  - phase: 06-trigger-evaluation-fixes
    provides: confirmation gate fix and tests (TRIG-01, TRIG-02, TRIG-03)
provides:
  - Per-token-only previous_price reads in all three lookup sites (WS enqueue, trigger.market_price, trigger.open_positions)
  - Removal of .or_else legacy fallback that allowed cross-token price contamination
  - Three unit tests proving PRCE-01 safety guarantee
affects:
  - any future phases touching trigger evaluation or flow node state

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "PRCE-01: Use format!(\"previous_price_{}\", token_id) as exclusive read key for previous price; never fall back to bare \"previous_price\""
    - "Write-side parity: only write per-token key, bare key writes removed to stay in sync with read-side"

key-files:
  created: []
  modified:
    - crates/bot-runner/src/main.rs

key-decisions:
  - "PRCE-01: Bare previous_price key is now dead — removed from both read and write paths to prevent silent cross-token contamination"
  - "Dead writes removed at both write sites (trigger.market_price ~5546 and trigger.open_positions ~6182) to avoid confusion in future readers"

patterns-established:
  - "Per-token previous_price pattern: all reads must use format!(\"previous_price_{}\", token_id)"
  - "Market rotation cleanup still removes bare key for legacy data hygiene (safe, harmless)"

requirements-completed: [PRCE-01]

# Metrics
duration: 20min
completed: 2026-03-03
---

# Phase 7 Plan 01: Price Data Integrity Fixes Summary

**Eliminated cross-token price contamination by removing the legacy `previous_price` fallback in all three WS/step-based trigger lookup sites in bot-runner**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-03T11:10:00Z
- **Completed:** 2026-03-03T11:30:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Removed `.or_else(|| flow_node_state(..., "previous_price"))` fallback from WS enqueue path (Site 1)
- Replaced bare `"previous_price"` read with per-token `format!("previous_price_{}", token_id)` in step-based `trigger.market_price` (Site 2)
- Replaced bare `"previous_price"` read with per-token key in step-based `trigger.open_positions` (Site 3)
- Removed two dead bare-key write statements that were no longer read by anything
- Added three unit tests (`prce01_no_legacy_fallback_for_previous_price`, `prce01_per_token_key_works_independently`, `prce01_no_cross_token_contamination`) proving PRCE-01 safety guarantee

## Task Commits

1. **Task 1: Remove legacy previous_price fallback from all three lookup sites** - `c0bf02e` (fix)

## Files Created/Modified

- `crates/bot-runner/src/main.rs` - Three read-side fixes, two write-side cleanups, three new unit tests in `price_integrity_tests` module

## Decisions Made

- Removed bare "previous_price" key writes alongside the read-side fix so the dead writes don't confuse future readers — write-side now mirrors read-side exclusively
- Market rotation cleanup (`remove_flow_node_state(..., "previous_price")`) at line 4088 was intentionally left in place: it removes legacy data if any was written by an older binary version, which is harmless and slightly protective

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PRCE-01 fix complete; all three price lookup sites now use only per-token keys
- 49/49 tests pass, clean release build
- Ready for phase 07 plan 02 (if it exists) or phase completion

---
*Phase: 07-price-data-integrity-fixes*
*Completed: 2026-03-03*
