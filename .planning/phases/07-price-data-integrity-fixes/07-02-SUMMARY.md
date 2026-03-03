---
phase: 07-price-data-integrity-fixes
plan: 02
subsystem: testing
tags: [rust, reconcile, price-data, ws, rest, tie-break, unit-tests]

requires:
  - phase: 07-price-data-integrity-fixes/07-01
    provides: reconcile_tick_and_snapshot function with >= comparison

provides:
  - Equal-timestamp tie-break test (ws_wins_at_equal_timestamp) explicitly asserting WS wins at equal timestamps
  - Complementary tests: rest_wins_when_strictly_newer, rest_fallback_when_no_tick
  - PRCE-02 inline documentation comment on the >= comparison explaining rationale

affects:
  - Any future changes to reconcile_tick_and_snapshot must maintain >= tie-break behavior

tech-stack:
  added: []
  patterns:
    - "TDD: tests written alongside documentation to make implicit behavior explicit and deterministic"
    - "PRCE-02 comment pattern: requirement ID + rationale inline at decision point"

key-files:
  created: []
  modified:
    - crates/bot-infra/src/reconcile.rs

key-decisions:
  - "PRCE-02: WS wins at equal timestamps (>= comparison) - documented inline in reconcile.rs"
  - "Three tests cover the full boundary: equal (WS wins), strictly-newer-REST (REST wins), no-tick (REST fallback)"

patterns-established:
  - "Requirement ID comment pattern: // PRCE-02: ... rationale ... above the decision-point line"

requirements-completed: [PRCE-02]

duration: 7min
completed: 2026-03-03
---

# Phase 7 Plan 02: Equal-Timestamp Tie-Break Test and Documentation Summary

**WS/REST equal-timestamp tie-break behavior made explicit with PRCE-02 inline comment and three boundary-covering unit tests in reconcile.rs**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-03T11:14:09Z
- **Completed:** 2026-03-03T11:15:48Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added PRCE-02 multi-line documentation comment to the `>=` comparison in `reconcile_tick_and_snapshot` explaining why WS is preferred at equal timestamps
- Added `ws_wins_at_equal_timestamp` test explicitly asserting source="ws" when tick.ts == snapshot.ts
- Added `rest_wins_when_strictly_newer` test asserting source="rest" when snapshot.ts > tick.ts
- Added `rest_fallback_when_no_tick` test asserting source="rest" when no tick is available

## Task Commits

Each task was committed atomically:

1. **Task 1: Add equal-timestamp tie-break test and document the >= choice** - `c5ad4f6` (test)

## Files Created/Modified

- `/home/heathcliff/polymarketbot/crates/bot-infra/src/reconcile.rs` - Added PRCE-02 comment to >= comparison and three new unit tests

## Decisions Made

- WS preference at equal timestamps (`>=`) is the correct design: WS ticks arrive real-time via CLOB WebSocket and represent most-recent market activity. REST snapshots are polled and may lag. Equal timestamp means WS is at least as fresh.
- Three tests chosen to cover all boundary conditions: equal (WS wins), strictly-newer-REST (REST wins), no-tick (REST fallback).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Pre-existing test failure `exchange::tests::place_and_reconcile_against_mock_exchange` exists in bot-infra (confirmed pre-dates this plan via git stash). Out of scope per deviation rules - logged to deferred items.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- PRCE-02 requirement fully satisfied: behavior documented, tested, and deterministic
- All reconcile tests pass (4/4): `prefers_newer_ws_tick`, `ws_wins_at_equal_timestamp`, `rest_wins_when_strictly_newer`, `rest_fallback_when_no_tick`
- Phase 07 plans 01 and 02 complete

## Self-Check: PASSED

- FOUND: crates/bot-infra/src/reconcile.rs (modified with PRCE-02 comment and 3 new tests)
- FOUND: .planning/phases/07-price-data-integrity-fixes/07-02-SUMMARY.md
- FOUND: commit c5ad4f6 (test(07-02): add equal-timestamp tie-break test and document >= choice)
- FOUND: 4/4 reconcile tests passing (prefers_newer_ws_tick, ws_wins_at_equal_timestamp, rest_wins_when_strictly_newer, rest_fallback_when_no_tick)

---
*Phase: 07-price-data-integrity-fixes*
*Completed: 2026-03-03*
