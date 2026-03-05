# Architecture

**Analysis Date:** 2026-03-02

## Pattern Overview

**Overall:** Event-driven, layered microservices-like Rust backend with WebSocket price streaming, deterministic state machine transitions, and trade flow DAG execution.

**Key Characteristics:**
- **Event-driven**: Price ticks from CLOB WebSocket drive trigger evaluation and state transitions
- **Stateful**: Trade and flow state persisted to PostgreSQL; in-memory context for decision-making
- **Deterministic**: State transitions validated against explicit state machine rules (11 states)
- **Async/concurrent**: Tokio runtime with multi-threaded executor for parallel market monitoring
- **Clean separation**: Domain logic (bot-core) isolated from infrastructure (bot-infra) and orchestration (bot-runner)

## Layers

**Domain (bot-core):**
- Purpose: Pure business logic, zero I/O dependencies
- Location: `crates/bot-core/src/`
- Contains: State machine rules, strategy interfaces, risk policies, types
- Depends on: anyhow, thiserror, serde (no tokio, no sqlx)
- Used by: bot-infra, bot-runner for decision-making

**Infrastructure (bot-infra):**
- Purpose: External integrations, data persistence, trait implementations
- Location: `crates/bot-infra/src/`
- Contains: DB repository (PostgreSQL), WebSocket client (CLOB), REST clients (Gamma/Polymarket), config loading, signing
- Depends on: tokio, sqlx, reqwest, tokio-tungstenite, bot-core
- Used by: bot-runner for all I/O operations

**Orchestration (bot-runner):**
- Purpose: Glue layer coordinating market discovery, flow execution, price streaming, and state transitions
- Location: `crates/bot-runner/src/`
- Contains: Main event loop, trade flow DAG executor, market cycle management, reconciliation logic
- Depends on: bot-core, bot-infra, tokio
- Used by: systemd service as executable entry point

**Test fixtures (mock-exchange):**
- Purpose: In-memory HTTP mock for testing without hitting real CLOB
- Location: `crates/mock-exchange/src/`
- Contains: Axum server simulating Polymarket order/fill responses
- Depends on: axum, tokio, serde_json

## Data Flow

**Price Trigger → Execution → State Transition:**

1. **Price Stream**: WebSocket client (`bot-infra/src/ws.rs`) subscribes to market CLOB channels
2. **Tick Buffering**: Ticks queued in `WsEvent` with `PriceTick` payload (price, timestamp, market slug)
3. **Trigger Evaluation** (`main.rs:4170-4202`):
   - Previous price retrieved from flow context state: `previous_price_{token_id}`
   - Current tick price vs. trigger price evaluated: `evaluate_trigger_market_price_condition()`
   - Condition: `"cross_above"` (prev < trigger, current ≥ trigger) or `"cross_below"`
   - For `once` mode + auto_scope: confirmation gate requires price to STAY in trigger zone for N seconds
4. **State Storage**: Current price stored as `previous_price_{token_id}` for next tick's evaluation
5. **Flow Execution**: If trigger fires, flow step enqueued via `TradeFlowRunStep` table
6. **Order Placement**: Step execution → `OrderExecutor` trait → REST POST to CLOB with EIP-712 signature
7. **Fill Tracking**: WebSocket `Fill` events → `reconcile_tick_and_snapshot()` deduplicates via `fill_id` UNIQUE constraint
8. **State Transition**: Fill event triggers `transition_trade_state()` → validates rule via `can_transition()` → persists to DB

**Stale Price Fallback:**

- If WebSocket stale (no ticks for 5s), snapshot client falls back: `snapshot_client()` polls REST `GET /book/{market}`
- Snapshot price used if fresher than last tick; recorded in same flow state for next evaluation

## Key Abstractions

**Strategy (trait):**
- Purpose: Encapsulates entry/exit price computation logic
- Examples: `PriceThresholdStrategy` (file: `crates/bot-core/src/strategy.rs`)
- Pattern: Entry signal checks `current_price >= entry_price`; TP/SL computed as percentage deltas

**DualSideStrategy (trait):**
- Purpose: Multi-leg DCA decision-making (YES + NO positions simultaneously)
- Pattern: `SymmetricDualDcaStrategy` tracks last fill price, requires price distance ≥ step_pct before next DCA leg
- Used by: Dual-side workflows for basket P&L management

**RiskPolicy (trait):**
- Purpose: Pre-trade and post-trade risk checks
- Examples: `DefaultRiskPolicy` (file: `crates/bot-core/src/risk.rs`)
- Pattern: Returns `RiskDecision { allow | block | halt }` based on notional, daily loss limits, kill switch

**MarketDataProvider (trait):**
- Purpose: Abstraction over live vs. mock price data
- Location: `bot-infra/src/market_data.rs`
- Implementations: `ClobWsClient` (live), `MockMarketDataProvider` (testing)

**StateRepository (trait):**
- Purpose: Atomic state transitions with validation
- Location: `bot-infra/src/contracts.rs`
- Implementor: `PostgresRepository`
- Pattern: `transition_trade_state()` calls `can_transition()` before UPDATE, prevents invalid state sequences

**OrderExecutor (trait):**
- Purpose: Order placement/cancel/replace abstraction
- Location: `bot-infra/src/contracts.rs`
- Implementor: `ClobRestClient`
- Pattern: Blanket impl wraps `PlaceOrderRequest` → EIP-712 signing → REST POST

**TradeFlowRuntime (struct):**
- Purpose: In-memory DAG execution context for flow automation
- Location: `bot-infra/src/db.rs`
- Contains: Node specs, edges, flow context (market slug, prices, token IDs), node state per token
- Pattern: Context persisted to `trade_flow_runs` table on each iteration

## Entry Points

**Main Binary:**
- Location: `crates/bot-runner/src/main.rs`
- Triggers: `systemctl start dextrabot` (systemd service)
- Responsibilities:
  1. Parse config from `$BOT_CONFIG_DIR` (encrypted AES-256-GCM values with `enc:v1:` prefix)
  2. Initialize DB pool (PostgreSQL via `sqlx`)
  3. Spawn async runtime (tokio multi-threaded)
  4. Launch market discovery task (polls Gamma API for active markets)
  5. Launch main event loop: price stream → trigger eval → flow execution → order placement

**Market Discovery Loop:**
- Runs every 30s (cached)
- Polls Gamma API for markets matching configured scope (`btc_5m_updown`, `eth_15m_updown`, etc.)
- Populates flow context with live market slug, token IDs, timeframe
- Prevents stale market references in auto_scope triggers

**Flow Processing Loop:**
- Triggered by price tick arrival or poll interval (< 1s)
- Reads pending `TradeFlowRun` rows with `status = 'processing'`
- For each run: evaluates node conditions, enqueues steps to next node, updates `flow_context`
- Commits `context_dirty` back to DB via `update_trade_flow_run_context()`

## Error Handling

**Strategy:** Fail-safe with logging and DB fallback.

**Patterns:**

1. **Config Load Failure**: Bot refuses to start. Error logged, process exits non-zero.
   - Example: Encrypted config value corrupted → `aes_gcm.decrypt()` fails → `?` operator in `decrypt_value()`

2. **DB Connection Loss**: All infra operations return `anyhow::Result<T>`. Caller decides retry vs. graceful shutdown.
   - Example: WebSocket fill event → idempotency check fails (DB unreachable) → event buffered locally

3. **Exchange API Timeout**: HTTP client has 30s timeout per request. Retries via `max_retries` field on `ClobWsClient` and `ClobRestClient`.
   - Example: POST `/submit_order` times out → log warn, retry with exponential backoff

4. **Invalid State Transition**: `can_transition()` returns `TransitionError::Invalid { from, to }`. Transition silently blocked, event logged with context.
   - Example: Attempting `Idle → TpPlaced` (invalid) → error caught, DB state unchanged

5. **Risk Policy Breach**: `RiskPolicy::check()` returns `RiskDecision::Halt`. Trade halted, no further orders placed.
   - Example: Daily loss limit exceeded → new entry blocked, `Halted` state persisted

## Cross-Cutting Concerns

**Logging:**
- Tool: `tracing` with `tracing-subscriber` (JSON output for production)
- When to log:
  - Every price tick: `info!(market_slug=%slug, price=%tick.price, "PRICE_TICK")`
  - Every trigger evaluation: `info!(trigger_fired=%crossed, condition=%eval_mode, "TRIGGER_EVAL")`
  - State transitions: `info!(from_state=?old_state, to_state=?new_state, "STATE_TRANSITION")`
  - Risk decisions: `warn!(decision=?risk_decision, notional=%notional, "RISK_CHECK")`

**Validation:**
- Config validation: Happens at startup in `load_config()`. All required fields checked; missing fields = startup failure.
- Order validation: `PlaceOrderRequest` validated for non-zero size, non-negative price before signing.
- State validation: State machine rules encoded in `can_transition()` switch statement; all invalid paths explicitly rejected.

**Authentication:**
- Header signing: All CLOB REST requests signed with HMAC-SHA256 over payload, timestamp, nonce.
  - Headers: `POLY_ADDRESS`, `POLY_SIGNATURE`, `POLY_TIMESTAMP`, `POLY_PASSPHRASE`, `POLY_API_KEY`
  - Implementation: `bot-infra/src/signer.rs:sign_request_headers()`
- WebSocket auth: None (market data public); user channel requires API credentials in subscription message.

**Idempotency:**
- Fill deduplication: Every `fill_id` checked against `idempotency_keys` table before processing. Duplicate = silent skip.
- Flow step deduplication: Step enqueued with `idempotency_key`; re-processing a step with same key is a no-op (checked at row lock).
- Trigger deduplication: Once-fired nodes tracked with `node_state["once_fired"] = true`; subsequent conditions ignored.

---

*Architecture analysis: 2026-03-02*
