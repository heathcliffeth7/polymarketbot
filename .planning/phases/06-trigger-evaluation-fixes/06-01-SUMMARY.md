---
phase: 06-trigger-evaluation-fixes
plan: 01
subsystem: bot-runner/confirmation-gate
tags: [trigger, confirmation, timer-reset, first-tick, rust]
dependency_graph:
  requires: []
  provides: [TRIG-01, TRIG-02]
  affects: [crates/bot-runner/src/main.rs]
tech_stack:
  added: []
  patterns: [remove_flow_node_state on zone exit, CROSS_PENDING_RESET log]
key_files:
  created: []
  modified:
    - crates/bot-runner/src/main.rs
decisions:
  - "Out-of-zone ticks now actively clear all three cross_pending_* keys via remove_flow_node_state"
  - "first_tick_threshold and real crosses use the same confirmation_secs duration"
metrics:
  duration: "~5 minutes"
  completed: "2026-03-03"
---

# Phase 6 Plan 01: Confirmation Gate Out-of-Zone Reset Summary

**One-liner:** Fixed confirmation timer so it resets to zero on zone exit, preventing false triggers from cross-leave-reenter price sequences; documented that first_tick_threshold events also require confirmation_secs sustain before enqueue.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Fix confirmation gate out-of-zone reset (TRIG-01) | 8a214c6 | crates/bot-runner/src/main.rs |
| 2 | Ensure first_tick_threshold triggers pass through confirmation gate (TRIG-02) | 8a214c6 | crates/bot-runner/src/main.rs |

## What Was Done

### Task 1 — TRIG-01: Out-of-zone Reset

The else branch at line 4305 previously contained only a comment explaining that pending state was preserved during out-of-zone ticks. This allowed the confirmation timer to keep accumulating elapsed time even when price had left the trigger zone entirely.

Replaced the comment-only else with three `remove_flow_node_state` calls clearing:
- `cross_pending_at_{token_id}`
- `cross_pending_price_{token_id}`
- `cross_pending_prev_{token_id}`

Also sets `context_dirty = true` and emits a `CROSS_PENDING_RESET` info log at:
- `run_id`, `node_key`, `price`, `trigger`, `market`

On re-entry to the zone, the `crossed` detection fires fresh and the confirmation timer restarts from zero.

### Task 2 — TRIG-02: first_tick_threshold Documentation

Verified by code inspection that `allow_first_tick_threshold` is already set to `true` only when `auto_scope && once_mode` (the exact condition gate the confirmation block also checks). Therefore `first_tick_threshold` triggers already flow into the confirmation gate correctly.

Replaced the one-line comment `// New cross detected → start confirmation period, DON'T enqueue yet` with a four-line comment that explicitly names `first_tick_threshold` as a covered case and explains the rationale (transient opening prices are MORE likely to be false, so equal confirmation duration is appropriate).

No logic change was needed for Task 2 — the existing flow was already correct.

## Deviations from Plan

None - plan executed exactly as written.

## Verification

- `cargo build -p bot-runner` — Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.69s, no errors
- `CROSS_PENDING_RESET` log message present in code at line 4332
- Three `remove_flow_node_state` calls in the new else branch
- `evaluate_trigger_market_price_condition()` and crossing helpers unchanged
- Comment at `if crossed` block now documents first_tick_threshold behavior

## Self-Check: PASSED

- crates/bot-runner/src/main.rs: modified and compiled
- Commit 8a214c6 exists in git log
