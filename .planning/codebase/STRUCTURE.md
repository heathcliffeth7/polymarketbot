# Codebase Structure

**Analysis Date:** 2026-03-02

## Directory Layout

```
polymarketbot/
├── crates/                    # Workspace with 4 Rust crates
│   ├── bot-core/              # Domain: state machine, strategy, risk (zero I/O)
│   │   └── src/
│   │       ├── lib.rs         # Re-exports
│   │       ├── types.rs       # TradeState enum, ExecutionMode, OrderStatus
│   │       ├── state_machine.rs  # can_transition() validator
│   │       ├── strategy.rs    # Strategy trait, PriceThresholdStrategy
│   │       ├── risk.rs        # RiskPolicy trait, DefaultRiskPolicy
│   │       └── market_cycle.rs  # MarketCycleId and cycle utilities
│   ├── bot-infra/             # Infrastructure: DB, WebSocket, REST clients
│   │   └── src/
│   │       ├── lib.rs         # Re-exports
│   │       ├── db.rs          # PostgresRepository, TradeFlowRuntime, queries
│   │       ├── ws.rs          # ClobWsClient, WsEvent, WebSocket parsing
│   │       ├── market_data.rs # MarketDataProvider trait, snapshot logic
│   │       ├── exchange.rs    # ClobRestClient, GammaClient, REST calls
│   │       ├── signer.rs      # EIP-712 signing, HMAC headers
│   │       ├── config.rs      # AppConfig loading, encryption/decryption
│   │       ├── contracts.rs   # StateRepository, OrderExecutor traits
│   │       ├── claim.rs       # AutoClaimService for resolved positions
│   │       └── reconcile.rs   # reconcile_tick_and_snapshot() dedup logic
│   ├── bot-runner/            # Orchestration: main loop, flow executor
│   │   └── src/
│   │       ├── main.rs        # 8500+ lines: event loop, trigger eval, flow DAG
│   │       └── dca.rs         # Dual-side DCA order building
│   └── mock-exchange/         # Test fixture: Axum mock CLOB server
│       └── src/
│           └── lib.rs         # Mock endpoints
├── frontend/                  # Next.js 16 frontend
│   ├── src/
│   │   ├── app/               # Pages and API routes
│   │   │   ├── api/           # 50+ API endpoints to bot-runner
│   │   │   ├── dashboard/     # Dashboard page
│   │   │   ├── trade-builder/ # Trade flow builder UI
│   │   │   ├── control/       # Bot control panel
│   │   │   └── login/         # JWT auth
│   │   ├── components/        # React components
│   │   │   ├── ui/            # Radix UI wrappers
│   │   │   ├── dashboard/     # Dashboard widgets
│   │   │   ├── trade-builder/ # Flow editor components
│   │   │   └── layout/        # Page layout
│   │   ├── hooks/             # SWR data hooks
│   │   └── lib/
│   │       └── queries/       # SQL query builders
│   └── package.json
├── config/                    # Runtime configuration files (TOML)
│   ├── bot.toml               # Execution mode, market scope, loop interval
│   ├── strategy.toml          # Entry/exit params, DCA settings, TP/SL %
│   ├── risk.toml              # Daily loss, kill switch, max notional
│   ├── exchange.toml          # CLOB/Gamma endpoints, encrypted credentials
│   ├── execution.toml         # Order type, retry logic
│   └── claim.toml             # Auto-claim settings
├── migrations/                # PostgreSQL schema migrations
│   ├── 001_init.sql           # Tables: markets, orders, trades, fills
│   ├── 002_live_hardening.sql # Idempotency keys, bot_runs
│   ├── ...
│   ├── 010_trade_flow_engine.sql  # trade_flow_definitions, runs, steps
│   └── 015_trade_flow_steps_claim_index.sql # Latest schema
├── mimari/                    # Architecture documentation (Turkish)
│   ├── plan.md                # Master plan phases
│   ├── architecture.md        # Component overview
│   ├── state-machine.md       # State transitions
│   └── db-schema.md           # Database tables
├── scripts/                   # Utility scripts
│   ├── bootstrap_db.sh        # Create DB
│   ├── apply_migrations.sh    # Run sqlx migrations
│   └── check_health.sh        # Health check
├── .claude/                   # Claude agent rules
│   └── rules/
│       └── rust-backend.md    # Architecture rules, traits, patterns
├── .planning/                 # GSD planning documents (this analysis)
│   └── codebase/
│       ├── ARCHITECTURE.md    # This file's twin
│       └── STRUCTURE.md       # This file
├── Cargo.toml                 # Workspace manifest
├── Cargo.lock                 # Dependency lock
├── README.md                  # Overview
└── CLAUDE.md                  # Project brief
```

## Directory Purposes

**crates/bot-core/src/:**
- Purpose: Pure domain logic, no dependencies on tokio or sqlx
- Contains: State enum (11 states), strategy interfaces, risk policy interface, market cycle types
- Key files: `types.rs` (types), `state_machine.rs` (validation), `strategy.rs` (trading logic), `risk.rs` (risk checks)

**crates/bot-infra/src/:**
- Purpose: Infrastructure: databases, HTTP clients, WebSocket, config, signing
- Contains: PostgreSQL queries via sqlx, CLOB/Gamma REST clients, WebSocket subscription, AES-256 config encryption
- Key files: `db.rs` (2000+ lines, all DB queries and ORM-like structures), `ws.rs` (WebSocket client), `exchange.rs` (REST clients)

**crates/bot-runner/src/:**
- Purpose: Main application logic coordinating everything
- Contains: Market discovery loop, event loop for price ticks, flow DAG executor, trigger evaluation, state machine transitions
- Key files: `main.rs` (8500+ lines, split into logical sections by comment headers), `dca.rs` (multi-leg order building)

**frontend/src/app/:**
- Purpose: Next.js 13+ App Router pages and API routes
- Contains: Dashboard, trade builder UI, control panel, settings, login
- API routes: `frontend/src/app/api/` handles all bot interaction via HTTP (50+ endpoints)

**config/:**
- Purpose: Runtime configuration, encrypted when needed
- Encryption: Values prefixed with `enc:v1:` are AES-256-GCM encrypted. Fallback to env var with `_env` suffix (e.g., `enc:v1:...` → `EXCHANGE_CREDS_env`)

**migrations/:**
- Purpose: PostgreSQL schema migrations using sqlx
- Pattern: Numbered 001..015, applied sequentially. Table names include scope (`trade_flow_*`, `auto_claim_*`)

**mimari/:**
- Purpose: Architecture documentation in Turkish (reference for developers)
- Linked from: CLAUDE.md with `@mimari/` references

## Key File Locations

**Entry Points:**
- `crates/bot-runner/src/main.rs`: Binary entry point. Async main() initializes config, DB pool, tokio runtime, spawns market discovery and event loops.

**Configuration:**
- `crates/bot-infra/src/config.rs`: Loads TOML files, decrypts AES values, resolves env var overrides.
- `config/bot.toml`: Execution mode (paper/live), market scope, loop interval.
- `config/exchange.toml`: CLOB/Gamma URLs, encrypted API credentials.

**Core Logic:**
- `crates/bot-core/src/state_machine.rs`: State transition rules (11 states, 13 valid transitions).
- `crates/bot-core/src/strategy.rs`: `PriceThresholdStrategy` implements `entry_signal()` as `current_price >= entry_price`.
- `crates/bot-core/src/risk.rs`: `DefaultRiskPolicy` checks notional, daily loss, kill switch.

**Price Trigger Logic:**
- `crates/bot-runner/src/main.rs:234-270`: `evaluate_trigger_market_price_condition()` checks cross_above/cross_below.
- `crates/bot-runner/src/main.rs:4170-4246`: Trigger evaluation per price tick (uses previous_price from context).
- `crates/bot-runner/src/main.rs:3716-3784`: `sync_trigger_market_auto_scope_context()` resolves market slug + token IDs.

**Database Queries:**
- `crates/bot-infra/src/db.rs`: All queries via `sqlx::query!()` macro (compile-time checked against schema).
- Transactions: Wrapped with `db_tx()` for atomicity (e.g., simultaneous order + state update).

**Testing:**
- Test fixtures: `crates/bot-runner/src/main.rs` has inline `#[cfg(test)]` sections (100+ tests).
- Mock provider: `crates/bot-infra/src/market_data.rs:MockMarketDataProvider` simulates price stream.

## Naming Conventions

**Files:**
- Rust modules: snake_case (`state_machine.rs`, `market_data.rs`).
- Feature flags: Feature branches use prefix `feat/` (e.g., `feat/dual-dca-support`).

**Functions:**
- Camel case operations: `evaluate_trigger_market_price_condition()`, `reconcile_tick_and_snapshot()`.
- Prefixes: `process_*` (main loop iterations), `sync_*` (fetch/update external), `apply_*` (apply state change).

**Variables & Types:**
- States: PascalCase enum variants (`Idle`, `EntryPlaced`, `TpPlaced`).
- Constants: SCREAMING_SNAKE_CASE (`DEFAULT_DROP_SELL_PCT`, `CONFIG_ENC_PREFIX`).
- Flow context keys: snake_case strings (`"previous_price_{token_id}"`, `"market_slug"`).

**Database Objects:**
- Tables: snake_case plural (`trades`, `orders`, `trade_flow_runs`).
- Columns: snake_case with `_at` suffix for timestamps (`filled_at`, `updated_at`).
- Indexes: Composite names reflect columns (`orders(trade_id,status)`).

## Where to Add New Code

**New Trigger Type (e.g., trigger.position_size):**
1. Add enum variant to `WsEventType` in `bot-infra/src/ws.rs`.
2. Add parsing logic in `parse_ws_event()` in `bot-runner/src/main.rs`.
3. Add condition evaluation in `evaluate_trigger_market_price_condition()` or new `evaluate_trigger_position_size()`.
4. Update flow specs in `open_position_ws_price_node_specs()` to handle new node type.
5. Tests: Add inline `#[test]` section in `main.rs` testing condition logic.

**New Risk Check (e.g., max_consecutive_losses):**
1. Add field to `RiskLimits` struct in `bot-core/src/risk.rs`.
2. Implement check in `DefaultRiskPolicy::check_pre_trade()` or `check_post_trade()`.
3. Load config from `config/risk.toml` in `crates/bot-infra/src/config.rs`.
4. Return `RiskDecision::Block` or `Halt` from policy.
5. Bot-runner calls policy before order placement: check result before `OrderExecutor::place_order()`.

**New Market Data Source (e.g., REST polling instead of WebSocket):**
1. Create new struct implementing `MarketDataProvider` trait in `bot-infra/src/market_data.rs`.
2. Implement `next_tick()` and `snapshot()` methods.
3. Update bot-runner to instantiate provider based on config mode (paper vs. live).
4. Tick flow unchanged: provider output → trigger evaluation → flow execution.

**New Database Table (e.g., trade_signals):**
1. Create migration in `migrations/016_trade_signals.sql`.
2. Define sqlx schema file or update `sqlx-data.json` (if using offline mode).
3. Add ORM struct to `bot-infra/src/db.rs` and query methods.
4. Call new query from bot-runner event loop as needed (e.g., after trigger fires).

**Frontend Page (e.g., signals dashboard):**
1. Create file `frontend/src/app/signals/page.tsx`.
2. Create API route `frontend/src/app/api/signals.ts` to fetch from bot-runner.
3. Add hook in `frontend/src/hooks/useSignals.ts` using SWR for polling.
4. Add navigation link in `frontend/src/components/layout/Navbar.tsx`.
5. Style using Tailwind 4 + Radix UI components from `frontend/src/components/ui/`.

## Special Directories

**target/:**
- Purpose: Compiled binaries and artifacts (auto-generated)
- Generated: Yes (cargo build output)
- Committed: No (in .gitignore)
- Release binary location: `target/release/bot-runner`

**.planning/codebase/:**
- Purpose: GSD codebase analysis documents (this directory)
- Generated: Yes (by GSD mappers)
- Committed: Yes (reference for future tasks)

**.git/:**
- Purpose: Git version control
- Hooks: None configured (pre-commit hooks not installed)

**.claude/rules/:**
- Purpose: Agent-readable architecture rules (YAML frontmatter style)
- File: `rust-backend.md` with Path: field linking to relevant code

**config/ (runtime, not source):**
- Purpose: Configuration consumed at bot startup
- Committed: TOML templates, not secrets (encrypted values use `enc:v1:` prefix)
- Secrets: Use env var fallback with `_env` suffix (never committed)

---

*Structure analysis: 2026-03-02*
