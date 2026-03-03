---
phase: 07-price-data-integrity-fixes
verified: 2026-03-03T12:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification: false
---

# Phase 7: Price Data Integrity Fixes — Verification Report

**Phase Goal:** Price lookups never silently return stale data from a different token, and WS/REST tie-break behavior is deterministic and tested
**Verified:** 2026-03-03T12:00:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | When per-token key `previous_price_{token_id}` is absent, lookup returns None — never falls back to bare `previous_price` | VERIFIED | Site 1 (line 4174): `.and_then(value_as_f64)` with no `.or_else`. `or_else` fallback confirmed absent via grep. |
| 2 | No cross-token price contamination: stale price from token A cannot be used when evaluating token B | VERIFIED | All three read sites use `format!("previous_price_{}", token_id)` exclusively. Test `prce01_no_cross_token_contamination` passes. |
| 3 | WS enqueue path, step-based trigger.market_price, and trigger.open_positions all use only per-token state keys | VERIFIED | Site 1 line 4173-4175; Site 2 line 5499-5504; Site 3 line 6107-6112. All confirmed by direct code read. |
| 4 | When WS tick and REST snapshot have the same timestamp, `reconcile_tick_and_snapshot` consistently selects WS data | VERIFIED | `t.ts >= snapshot.ts` at line 25 of reconcile.rs. Test `ws_wins_at_equal_timestamp` passes with `source="ws"` assertion. |
| 5 | The `>=` comparison in reconcile logic has an inline code comment explaining WHY WS is preferred at equal timestamps | VERIFIED | Lines 19-24 of reconcile.rs contain PRCE-02 multi-line comment with rationale. |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/bot-runner/src/main.rs` | Removed legacy `previous_price` fallback from all three price lookup sites; three unit tests in `price_integrity_tests` module | VERIFIED | All three sites confirmed. Tests at lines 9067-9136 (3 functions). Dead bare-key writes also removed. |
| `crates/bot-infra/src/reconcile.rs` | Equal-timestamp tie-break test and PRCE-02 documentation comment on `>=` | VERIFIED | Comment lines 19-24, tests lines 77-129 (3 new tests: `ws_wins_at_equal_timestamp`, `rest_wins_when_strictly_newer`, `rest_fallback_when_no_tick`). |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` WS enqueue path (line 4173) | `flow_node_state` context | `previous_price_{token_id}` lookup returns None when absent; no `.or_else` fallback | WIRED | Line 4174: `flow_node_state(..., &prev_key).and_then(value_as_f64)` — no `.or_else`. Grep for `or_else.*previous_price` returns zero results. |
| `main.rs` step-based trigger.market_price (line 5499) | `resolve_ws_previous_price` | `state_previous_price` sourced from per-token key `previous_price_{token_id}` | WIRED | Lines 5499-5504: `token_id.as_deref().filter(...).map(|tid| format!("previous_price_{}", tid)).and_then(...)`. Per-token key confirmed. |
| `main.rs` step-based trigger.open_positions (line 6107) | `resolve_ws_previous_price` | `state_previous_price` sourced from per-token key `previous_price_{token_id}` | WIRED | Lines 6107-6112: `if !token_id.is_empty() { let key = format!("previous_price_{}", token_id); ... }`. Per-token key confirmed. |
| `reconcile.rs reconcile_tick_and_snapshot` | WS-preferred price selection | `>=` operator selects WS at equal timestamps | WIRED | Line 25: `if t.ts >= snapshot.ts`. Comment at lines 19-24 documents rationale. `ws_wins_at_equal_timestamp` test passes. |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| PRCE-01 | 07-01-PLAN.md | `previous_price` legacy fallback removed; per-token `previous_price_{token_id}` key is the sole read source; no cross-token contamination possible | SATISFIED | Three sites in main.rs use only per-token keys. No `.or_else` fallback to bare key exists. Three unit tests pass: `prce01_no_legacy_fallback_for_previous_price`, `prce01_per_token_key_works_independently`, `prce01_no_cross_token_contamination`. |
| PRCE-02 | 07-02-PLAN.md | WS/REST equal-timestamp tie-break deterministic and tested; `>=` comparison documented inline | SATISFIED | `>=` at reconcile.rs line 25 with PRCE-02 comment. `ws_wins_at_equal_timestamp` test explicitly asserts WS wins. `rest_wins_when_strictly_newer` and `rest_fallback_when_no_tick` cover the complementary cases. |

**Orphaned requirements:** None. REQUIREMENTS.md maps exactly PRCE-01 and PRCE-02 to Phase 7, both claimed by plans.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `main.rs` | 7320, 7330 | `placeholder` string variable | INFO | Template variable interpolation logic (`{{vars.x}}`), not stub code. Pre-existing, unrelated to phase 07. |

No blockers or warnings identified.

---

### Pre-Existing Test Failure (Out of Scope)

`bot-infra::exchange::tests::place_and_reconcile_against_mock_exchange` fails in `crates/bot-infra/src/exchange.rs` at assertion `!fills.is_empty()`. Verified via `git stash` that this failure existed before any phase 07 changes were applied. The summary (07-02) explicitly notes this and categorizes it as pre-existing. It is not caused by phase 07 and does not affect PRCE-01 or PRCE-02 coverage. The 4 reconcile-specific tests all pass.

---

### Human Verification Required

None. All goal assertions are verifiable programmatically through code inspection and test execution.

---

## Test Results

**PRCE-01 (bot-runner price_integrity_tests):**
```
test price_integrity_tests::prce01_no_cross_token_contamination ... ok
test price_integrity_tests::prce01_no_legacy_fallback_for_previous_price ... ok
test price_integrity_tests::prce01_per_token_key_works_independently ... ok
test result: ok. 3 passed; 0 failed
```

**PRCE-02 (bot-infra reconcile tests):**
```
test reconcile::tests::prefers_newer_ws_tick ... ok
test reconcile::tests::rest_fallback_when_no_tick ... ok
test reconcile::tests::rest_wins_when_strictly_newer ... ok
test reconcile::tests::ws_wins_at_equal_timestamp ... ok
test result: ok. 4 passed; 0 failed
```

**Commits verified:**
- `c0bf02e` — fix(07-01): remove legacy previous_price fallback to prevent cross-token contamination
- `c5ad4f6` — test(07-02): add equal-timestamp tie-break test and document >= choice

---

## Summary

Phase 07 fully achieves its goal. Both requirements are satisfied:

**PRCE-01:** The legacy `.or_else` fallback to the bare `"previous_price"` key is removed from all three read sites in `crates/bot-runner/src/main.rs`. The WS enqueue path (line 4174), step-based `trigger.market_price` (lines 5499-5504), and step-based `trigger.open_positions` (lines 6107-6112) each use only the per-token `format!("previous_price_{}", token_id)` key. Dead bare-key write statements are also removed. Three unit tests prove the no-fallback and no-cross-contamination guarantees.

**PRCE-02:** The `reconcile_tick_and_snapshot` function in `crates/bot-infra/src/reconcile.rs` now has an explicit PRCE-02 multi-line comment at the `>=` comparison (line 25) documenting why WS is preferred at equal timestamps. Three tests cover the full boundary: equal timestamps (WS wins), strictly newer REST (REST wins), and no-tick fallback (REST wins).

---

_Verified: 2026-03-03T12:00:00Z_
_Verifier: Claude (gsd-verifier)_
