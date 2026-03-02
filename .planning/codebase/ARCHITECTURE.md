# Architecture

**Analysis Date:** 2026-03-02

## Pattern Overview

**Overall:** Multi-crate Rust backend with deterministic state machine orchestration. Frontend (Next.js) provides dashboard and trade builder UI. Event-driven with WebSocket price feeds, REST order execution, and PostgreSQL state persistence.

**Key Characteristics:**
- Strict separation of concerns: bot-core (domain logic, no I/O) → bot-infra (infrastructure contracts) → bot-runner (orchestration)
- State transitions enforced via trait-based contracts, never raw SQL
- Idempotent processing with event deduplication on unique `fill_id`
- Dual-execution modes: Paper (simulated) and Live (real CLOB orders with EIP-712 signing)
- Concurrent trade flows with locked market cycles using Redis + DB advisory locks

## Layers

**Domain Layer (bot-core):**
- Purpose: Pure business logic, type definitions, state machine rules. Zero I/O. Framework: `thiserror` for typed errors.
- Location: `crates/bot-core/src/`
- Contains: TradeState enum (11 states), RiskPolicy trait, Strategy trait, risk evaluation functions
- Depends on: Only std lib + chrono, serde, thiserror
- Used by: bot-infra (trait implementations), bot-runner (state transitions & risk checks)

**Infrastructure Layer (bot-infra):**
- Purpose: External integrations - database, exchange API, WebSocket, configuration, signing. Implements bot-core contracts.
- Location: `crates/bot-infra/src/`
- Contains: PostgreSQL repository, CLOB HTTP/WS clients, market data provider, config loader, EIP-712 signer, reconciliation
- Depends on: bot-core, sqlx, tokio, reqwest, tokio-tungstenite, ethers, redis
- Used by: bot-runner (execution) and frontend API routes (database queries)

**Orchestration Layer (bot-runner):**
- Purpose: Main event loops, market discovery, trade/flow engines, cycle scheduling. Ties domain logic + infra together.
- Location: `crates/bot-runner/src/`
- Contains: main.rs (scheduler), market discovery, fill event processing, state transitions, risk enforcement, DCA logic
- Depends on: bot-core, bot-infra
- Used by: systemd service, CLI invocation

**Frontend Layer (Next.js):**
- Purpose: Dashboard UI, API routes that proxy to database, trade builder UI with visual flow editor.
- Location: `frontend/src/`
- Contains: React components (Radix UI + Tailwind), API routes that query PostgreSQL, SWR-based polling hooks, trade flow definition UI
- Depends on: Next.js, React, SWR, @xyflow/react, jose (JWT), pg (PostgreSQL client)
- Used by: Browser clients

## Data Flow

**Order Execution Flow:**

1. Market discovery identifies active market (e.g., `btc-updown-5m-2025-03-02-10-00`)
2. Price stream (WebSocket) receives real-time ticks from CLOB
3. Signal engine (strategy.rs) evaluates entry condition
4. If risk check passes (risk.rs), execution engine places entry order via CLOB REST API (signed with EIP-712)
5. Fill event arrives via WebSocket, validated with `idempotency_keys` table
6. `transition_trade_state()` enforces valid state path (e.g., `EntryPlaced → EntryPartiallyFilled`)
7. TP/SL orders placed after entry is fully filled
8. Exit fill arrives, final state transition to `Settled`

**State Recovery on Restart:**

1. Bot reads last known trade state from `trades` table (plus `orders` and `fills`)
2. Queries exchange for open orders, reconciles with DB (`reconcile.rs`)
3. On mismatch (e.g., order filled but not recorded), updates DB via `StateRepository`
4. Only after reconciliation is complete, bot resumes normal cycles

**WebSocket → REST Fallback:**

1. Price stream healthy: consume WebSocket ticks
2. If WebSocket stale (>max_stale_data_ms), snapshot client (REST) fetches latest price
3. Both feeds use `reconcile_tick_and_snapshot()` to merge deterministically (timestamp-based)

**State Management:**

- Primary: PostgreSQL `trades`, `orders`, `fills`, `positions` tables
- Cache: Redis for last price, WebSocket liveness checks
- In-memory: Current trade runtime state, strategy parameters
- Advisory locks: `lock:trade:{market_id}` ensures only one bot instance processes a market

## Key Abstractions

**TradeState (11-state machine):**
- Purpose: Deterministic trade lifecycle with explicit transitions
- Examples: `crates/bot-core/src/state_machine.rs` enforces `can_transition()` rules
- Pattern: Enum + function. Invalid transitions return `TransitionError`. Any state → Halted on risk breach.

**RiskPolicy trait:**
- Purpose: Pluggable risk evaluation without hardcoding rules
- Examples: `crates/bot-core/src/risk.rs`, `DefaultRiskPolicy` implementation
- Pattern: Evaluate daily loss, consecutive losses, stale data, manual kill switch. Return Allow/Block/Halt.

**Strategy trait:**
- Purpose: Entry/exit signal generation
- Examples: `crates/bot-core/src/strategy.rs` - `PriceThresholdStrategy`, `SymmetricDualDcaStrategy`, `DualSideStrategy`
- Pattern: Evaluate price conditions, return true/false for entry/exit. Dual-side version manages basket P&L across YES/NO legs.

**OrderExecutor trait:**
- Purpose: Unified interface for order operations (place, cancel, replace, status, fills list)
- Examples: `crates/bot-infra/src/contracts.rs`. Blanket implementation on `ClobRestClient`.
- Pattern: Async trait with REST call wrapper. Replace = cancel + place.

**StateRepository trait:**
- Purpose: All state changes must flow through this trait. No direct SQL in bot-runner.
- Examples: `crates/bot-infra/src/contracts.rs`. Implementation: `PostgresRepository::transition_trade_state()`
- Pattern: Methods check `can_transition()` before updating DB. Record state change reason in logs.

**MarketDataProvider trait:**
- Purpose: Abstraction for price stream (WebSocket or mock)
- Examples: `crates/bot-infra/src/market_data.rs`. Mock impl for testing.
- Pattern: `next_tick()` returns Option (sparse stream OK), `snapshot()` returns latest price.

## Entry Points

**Bot Runner:**
- Location: `crates/bot-runner/src/main.rs`
- Triggers: `cargo run -p bot-runner` or systemd service start
- Responsibilities: Load config, establish DB + Redis + WebSocket connections, spawn market cycle tasks, handle graceful shutdown

**Frontend Dashboard:**
- Location: `frontend/src/app/page.tsx`
- Triggers: Browser navigation to `/`
- Responsibilities: Fetch trade summary via SWR, display trades/orders/fills/risk events, show bot status

**Frontend Login:**
- Location: `frontend/src/login/page.tsx`, API route `frontend/src/app/api/auth/route.ts`
- Triggers: Unauthenticated request redirects here
- Responsibilities: Verify AUTH_SECRET password, issue JWT, set httpOnly cookie

**Trade Builder:**
- Location: `frontend/src/app/trade-builder/page.tsx`
- Triggers: User clicks "Trade Builder" nav
- Responsibilities: Visual flow editor (@xyflow/react), serialize to JSON, POST to `POST /api/trade-flow/definitions` to create workflow

**API Routes (Frontend):**
- Location: `frontend/src/app/api/` (50+ routes)
- Triggers: React components call SWR hooks
- Responsibilities: Query PostgreSQL via `pg` client or proxy requests to bot-runner

## Error Handling

**Strategy:** Domain errors in bot-core use `thiserror`, operational errors in bot-infra/bot-runner use `anyhow::Result`.

**Patterns:**
- Soft errors (e.g., RPC timeout): Log warning, retry with exponential backoff
- State machine violation: Panic (invariant broken, bot must restart)
- Config missing/invalid: Return Err early, bot won't start
- Fill event already seen: Idempotency key found in DB → skip silently
- Exchange order rejected: Log reason, record in `risk_events` table, move to Halted if policy dictates
- Database transaction conflict: Retry with backoff or fail fast depending on severity

## Cross-Cutting Concerns

**Logging:** Structured logging via `tracing` + `tracing-subscriber` with JSON output. Entry: `cargo run` with `RUST_LOG=info` or `debug`.

**Validation:** Config load fails early if required fields missing. Trade state transitions validated before DB update. Order size/price validated against limits.

**Authentication:** Frontend uses AUTH_SECRET (simple password) + JWT (jose) + httpOnly cookie. No bot-runner auth (runs locally).

**Configuration:** TOML files in `config/` directory:
- `bot.toml`: Execution mode, market scope, loop interval
- `strategy.toml`: Entry price, TP/SL %, DCA settings
- `risk.toml`: Daily loss limit, kill switch, notional cap
- `execution.toml`: Order type, retry logic
- `exchange.toml`: CLOB/Gamma endpoints, encrypted API credentials (AES-256-GCM with `enc:v1:` prefix)

**Encryption:** Config values prefixed with `enc:v1:` are AES-256-GCM encrypted. Key from CONFIG_ENCRYPTION_KEY env var.

---

*Architecture analysis: 2026-03-02*
