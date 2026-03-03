# Roadmap: Dextrabot

## Milestones

- ✅ **v1.0 Initial Build** - Phases 1-5 (shipped pre-GSD tracking)
- 🚧 **v1.1 tickhatası** - Phases 6-7 (in progress)

## Phases

<details>
<summary>✅ v1.0 Initial Build (Phases 1-5) - SHIPPED pre-GSD</summary>

Phases 1-5 shipped before GSD tracking was introduced. See MILESTONES.md for shipped features.

Shipped:
- Rust backend with 4-crate workspace (bot-core, bot-infra, bot-runner, mock-exchange)
- 11-state trade state machine
- CLOB WebSocket + REST price data
- Paper + Live execution modes
- Dual-side DCA strategy
- Risk policy with daily loss limits
- Auto-claim for resolved positions
- Trade builder workflow engine
- Next.js frontend dashboard
- JWT auth, config encryption

</details>

### 🚧 v1.1 tickhatası (In Progress)

**Milestone Goal:** Fix tick handling correctness — confirmation timer resets, first_tick+confirmation safety, and price data integrity (stale cross-token data, WS/REST tie-break)

## Phase Details

### Phase 6: Trigger Evaluation Fixes
**Goal**: Confirmation gate and first_tick interaction are correct and tested
**Depends on**: Phase 5 (v1.0 complete)
**Requirements**: TRIG-01, TRIG-02, TRIG-03
**Success Criteria** (what must be TRUE):
  1. When price leaves the trigger zone, cross_pending_at is cleared and the confirmation timer stops accumulating out-of-zone time
  2. In auto_scope+once mode, if price is already above threshold at market open, the confirmation gate reliably rejects the false trigger without race conditions
  3. Unit tests pass for: out-of-zone reset, re-entry timing after reset, and first_tick+confirmation interaction
  4. No false trigger fires in the scenario: price crosses, leaves zone, re-enters — confirmation must restart from zero on re-entry
**Plans**: TBD

### Phase 7: Price Data Integrity Fixes
**Goal**: Price lookups never silently return stale data from a different token, and WS/REST tie-break behavior is deterministic and tested
**Depends on**: Phase 6
**Requirements**: PRCE-01, PRCE-02
**Success Criteria** (what must be TRUE):
  1. If per-token key `previous_price_{token_id}` is absent from Redis, the lookup returns None (or an explicit error) — it never falls back to bare `previous_price` and never uses another token's price
  2. When a WS tick and REST snapshot carry the same timestamp, the bot consistently selects WS data and a test asserts this behavior
  3. Reconcile logic behavior at equal timestamps is documented in code with a comment explaining the `>=` choice
**Plans**: TBD

## Progress

**Execution Order:** 6 → 7

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 6. Trigger Evaluation Fixes | 1/2 | In Progress|  | - |
| 7. Price Data Integrity Fixes | v1.1 | 0/TBD | Not started | - |
