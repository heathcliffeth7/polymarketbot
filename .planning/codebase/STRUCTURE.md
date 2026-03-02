# Codebase Structure

**Analysis Date:** 2026-03-02

## Directory Layout

```
polymarketbot/
├── crates/                      # Rust workspace (4 crates)
│   ├── bot-core/                # Pure domain: types, state machine, risk, strategy
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs         # TradeState enum, ExecutionMode, etc.
│   │       ├── state_machine.rs # State transitions rules
│   │       ├── risk.rs          # RiskPolicy trait, DefaultRiskPolicy
│   │       ├── strategy.rs      # Strategy trait implementations
│   │       └── market_cycle.rs  # Market cycle identifier
│   ├── bot-infra/               # Infrastructure & integrations
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── contracts.rs     # OrderExecutor, StateRepository traits
│   │       ├── db.rs            # PostgresRepository implementation
│   │       ├── exchange.rs      # CLOB REST client
│   │       ├── market_data.rs   # MarketDataProvider trait
│   │       ├── ws.rs            # WebSocket client (CLOB)
│   │       ├── config.rs        # Config structs (TOML parsing)
│   │       ├── signer.rs        # EIP-712 signing, HMAC headers
│   │       ├── claim.rs         # Auto-claim service for resolved positions
│   │       └── reconcile.rs     # Fill event + snapshot merging
│   ├── bot-runner/              # Orchestration & main loop
│   │   └── src/
│   │       ├── main.rs          # Entry point, market discovery, scheduler
│   │       └── dca.rs           # Dollar-cost averaging logic
│   └── mock-exchange/           # Test fixture (axum server)
│       └── src/lib.rs
├── frontend/                    # Next.js 16 dashboard & trade builder
│   ├── src/
│   │   ├── app/                 # Next.js app router
│   │   │   ├── layout.tsx       # Root layout with AppShell
│   │   │   ├── page.tsx         # Dashboard page
│   │   │   ├── login/           # Login page
│   │   │   ├── control/         # Bot control page
│   │   │   ├── settings/        # Configuration page
│   │   │   ├── trade-builder/   # Visual flow editor
│   │   │   └── api/             # Backend API routes (50+ endpoints)
│   │   │       ├── auth/        # Login/logout
│   │   │       ├── bot/         # Bot status/control
│   │   │       ├── trades/      # Trade queries
│   │   │       ├── orders/      # Order queries
│   │   │       ├── fills/       # Fill queries
│   │   │       ├── markets/     # Market listing
│   │   │       ├── trade-builder/  # Workflow CRUD
│   │   │       ├── trade-flow/  # Flow definition CRUD
│   │   │       ├── dashboard/   # Summary stats
│   │   │       └── (other endpoints...)
│   │   ├── components/          # React components
│   │   │   ├── layout/          # AppShell, nav, sidebar
│   │   │   ├── dashboard/       # Trade summary, charts
│   │   │   ├── control/         # Bot control buttons
│   │   │   ├── settings/        # Config editor
│   │   │   ├── trade-builder/   # Flow canvas (@xyflow/react)
│   │   │   └── ui/              # Radix UI + Tailwind components
│   │   ├── hooks/               # SWR-based custom hooks
│   │   │   ├── use-dashboard.ts
│   │   │   ├── use-bot-status.ts
│   │   │   ├── use-trade-builder.ts
│   │   │   ├── use-trade-flow.ts
│   │   │   └── (polling, canvas, config hooks)
│   │   └── lib/                 # Utilities & queries
│   │       ├── auth.ts          # JWT creation, cookie handling
│   │       ├── config.ts        # Config type definitions
│   │       ├── http-client.ts   # Fetch wrapper
│   │       ├── db.ts            # Database connection (unused - routes use pg directly)
│   │       ├── types.ts         # Frontend type definitions
│   │       ├── queries/         # SQL query builders
│   │       │   ├── trades.ts
│   │       │   ├── orders.ts
│   │       │   ├── fills.ts
│   │       │   ├── trade-builder.ts
│   │       │   ├── trade-flow.ts
│   │       │   └── (other queries)
│   │       ├── trade-flow-config-mappers.ts  # Graph ↔ config conversion
│   │       └── (utility functions)
│   ├── package.json
│   └── tailwind.config.ts
├── config/                      # Runtime configuration (TOML)
│   ├── bot.toml                 # Execution mode, market scope, loop timing
│   ├── strategy.toml            # Entry/exit params, DCA settings
│   ├── risk.toml                # Daily loss limit, kill switch, max notional
│   ├── execution.toml           # Order type, retry logic
│   ├── exchange.toml            # CLOB/Gamma endpoints, encrypted creds
│   └── claim.toml               # Auto-claim settings
├── migrations/                  # PostgreSQL migration scripts
├── scripts/                     # Utility scripts
│   ├── bootstrap_db.sh          # Create database & schema
│   ├── apply_migrations.sh      # Run migrations
│   ├── check_health.sh          # Health check
│   └── (other scripts)
├── deploy/                      # Systemd service files
│   └── systemd/
├── mimari/                      # Architecture documentation (Turkish)
│   ├── plan.md
│   ├── architecture.md
│   ├── state-machine.md
│   └── db-schema.md
├── .planning/codebase/          # GSD codebase analysis (this directory)
├── Cargo.toml                   # Rust workspace definition
├── Cargo.lock                   # Dependency lock
└── README.md
```

## Directory Purposes

**`crates/bot-core/src/`:**
- Pure domain logic, zero I/O. Types, enums, state machine rules, trait definitions.
- Key files: `types.rs` (TradeState, ExecutionMode, RiskDecision), `state_machine.rs` (transition rules), `risk.rs` (RiskPolicy), `strategy.rs` (Strategy implementations).
- No external dependencies except chrono, serde, thiserror.

**`crates/bot-infra/src/`:**
- Infrastructure layer. Implements bot-core traits. Handles all I/O: DB, HTTP, WebSocket, signing, config.
- Key files: `contracts.rs` (trait implementations), `db.rs` (PostgresRepository), `exchange.rs` (CLOB client), `ws.rs` (WebSocket), `config.rs` (TOML parsing), `signer.rs` (EIP-712).
- Heavy dependency usage: tokio, sqlx, reqwest, tokio-tungstenite, ethers.

**`crates/bot-runner/src/`:**
- Orchestration and main loops. Ties domain + infra together. Market discovery, event processing, trade state machine orchestration.
- Entry point: `main.rs` (~1000+ lines). Also `dca.rs` for dollar-cost averaging.
- Runs as systemd service or CLI.

**`frontend/src/app/`:**
- Next.js app router structure. Pages at top level, API routes in `api/` subdirectory.
- `page.tsx`: Dashboard home. `login/page.tsx`: Authentication. `trade-builder/page.tsx`: Visual flow editor.
- API routes proxy to PostgreSQL or trigger bot operations.

**`frontend/src/components/`:**
- React components using Radix UI + Tailwind. Organized by feature (dashboard, control, settings, trade-builder).
- `layout/app-shell.tsx`: Top-level app layout with navigation.
- `trade-builder/`: Flow canvas editor using @xyflow/react.

**`frontend/src/hooks/`:**
- Custom SWR-based hooks for data fetching and polling. Coupled to API endpoints.
- `use-dashboard.ts`: Fetch trade summary, orders, fills.
- `use-bot-status.ts`: Poll bot health/status.
- `use-trade-builder.ts`: Manage workflow state.
- `use-trade-flow.ts`: Manage flow definitions.

**`frontend/src/lib/queries/`:**
- SQL query builders for database operations in API routes. Each file corresponds to an entity (trades, orders, fills, etc.).
- Files construct parameterized queries, not raw SQL strings in routes.

**`config/`:**
- Runtime TOML configuration files. Loaded by bot-runner on startup.
- `bot.toml`: Scope, execution mode, timing.
- `strategy.toml`: Signal thresholds, TP/SL, DCA.
- `risk.toml`: Loss limits, kill switch, caps.
- `exchange.toml`: API endpoints, encrypted credentials.

**`migrations/`:**
- PostgreSQL schema migrations. Run via `scripts/apply_migrations.sh`.
- Creates tables: `trades`, `orders`, `fills`, `positions`, `risk_events`, `bot_runs`, `config_snapshots`, `idempotency_keys`, etc.

**`deploy/systemd/`:**
- Systemd service files for bot-runner and frontend. Enable via `systemctl --user enable`.

**`mimari/`:**
- Architecture documentation in Turkish. Detailed specs for state machine, database schema, data flow, risk policy.

## Key File Locations

**Entry Points:**
- `crates/bot-runner/src/main.rs`: Rust backend entry point. Loads config, connects to DB/Redis/WS, spawns market cycles.
- `frontend/src/app/page.tsx`: Next.js dashboard home page.
- `frontend/src/app/api/auth/route.ts`: Login API endpoint.

**Configuration:**
- `config/bot.toml`: Execution mode (Paper/Live), market scope, loop interval.
- `config/strategy.toml`: Entry price, TP/SL %, DCA settings.
- `config/risk.toml`: Daily loss limit, consecutive loss limit, max notional.
- `config/exchange.toml`: CLOB/Gamma endpoints, API credentials (encrypted).

**Core Logic:**
- `crates/bot-core/src/types.rs`: TradeState enum, ExecutionMode, OrderStatus, RiskDecision.
- `crates/bot-core/src/state_machine.rs`: State transition rules via `can_transition()` function.
- `crates/bot-core/src/risk.rs`: RiskPolicy trait, risk evaluation logic.
- `crates/bot-core/src/strategy.rs`: Strategy trait, signal generation implementations.

**Database:**
- `crates/bot-infra/src/db.rs`: PostgresRepository implementation. All DB queries here.
- `crates/bot-infra/src/contracts.rs`: StateRepository trait definition & implementation.
- `frontend/src/lib/queries/`: SQL query builders for API routes.

**Frontend Components:**
- `frontend/src/components/layout/app-shell.tsx`: Root layout, navigation, sidebar.
- `frontend/src/components/dashboard/`: Trade summary, charts, status displays.
- `frontend/src/components/trade-builder/`: Flow canvas, node types, utilities.
- `frontend/src/app/login/page.tsx`: Login page (password auth).

**Hooks & State Management:**
- `frontend/src/hooks/use-dashboard.ts`: Trade data polling.
- `frontend/src/hooks/use-bot-status.ts`: Bot health polling.
- `frontend/src/hooks/use-trade-builder.ts`: Workflow builder state.
- `frontend/src/hooks/use-polling.ts`: Generic polling hook used by others.

## Naming Conventions

**Files:**
- Rust: `snake_case.rs` (e.g., `state_machine.rs`, `market_data.rs`)
- TypeScript: `kebab-case.ts` for utilities, `camelCase.ts` for components (e.g., `use-dashboard.ts`, `app-shell.tsx`)
- TOML configs: `lowercase.toml` (e.g., `bot.toml`, `strategy.toml`)

**Directories:**
- Rust: `snake_case` (e.g., `bot-core`, `bot-infra`, `bot-runner`)
- TypeScript: `kebab-case` (e.g., `trade-builder`, `trade-flow`, `app-shell`)
- Features: Named after domain concept (e.g., `trades`, `orders`, `fills`, `risk-events`)

**Types & Enums:**
- Rust: PascalCase (e.g., `TradeState`, `ExecutionMode`, `RiskPolicy`)
- TypeScript: PascalCase for types/interfaces (e.g., `Trade`, `Order`, `Fill`)

**Functions & Methods:**
- Rust: `snake_case` (e.g., `transition_trade_state()`, `evaluate_risk()`)
- TypeScript: `camelCase` (e.g., `getTradeState()`, `evaluateRisk()`)

## Where to Add New Code

**New Feature (e.g., New Signal Type):**
- Primary code: `crates/bot-core/src/strategy.rs` (implement Strategy trait)
- Config: Add params to `crates/bot-infra/src/config.rs` and `config/strategy.toml`
- Tests: Add unit tests in strategy.rs `#[cfg(test)]` module
- Frontend: Add UI in `frontend/src/components/settings/` if user-configurable

**New Component/Module (e.g., New Integration):**
- Implementation: `crates/bot-infra/src/{feature}.rs`
- Contract: Add trait to `crates/bot-infra/src/contracts.rs` if it's a core abstraction
- Usage: Call from `crates/bot-runner/src/main.rs` main loop or from API routes
- Tests: Add tests in feature file or `crates/bot-infra/tests/` if complex

**New API Endpoint:**
- Implementation: `frontend/src/app/api/{feature}/route.ts`
- Query builder: Add to `frontend/src/lib/queries/{entity}.ts` if database access needed
- Hook: Add to `frontend/src/hooks/use-{feature}.ts` if UI needs to poll
- Component: Add to `frontend/src/components/{feature}/` if new UI required

**Utilities:**
- Shared helpers: `frontend/src/lib/utils.ts`
- Type definitions: `frontend/src/lib/types.ts`
- HTTP utilities: `frontend/src/lib/http-client.ts`

**Database Migrations:**
- Create: `migrations/{YYYYMMDDHHMMSS}_{description}.sql`
- Run: `scripts/apply_migrations.sh` applies all pending migrations
- No down migrations—schema is forward-only

## Special Directories

**`target/`:**
- Purpose: Rust build artifacts. Generated by cargo.
- Generated: Yes (created by `cargo build`)
- Committed: No (in .gitignore)

**`migrations/`:**
- Purpose: PostgreSQL migration scripts. Each file is timestamped.
- Generated: No (manually created)
- Committed: Yes

**`.planning/codebase/`:**
- Purpose: GSD codebase analysis documents (ARCHITECTURE.md, STRUCTURE.md, etc.)
- Generated: Yes (by gsd orchestrator)
- Committed: Yes

**`config/`:**
- Purpose: Runtime TOML configuration. Loaded by bot-runner on startup.
- Generated: No (created manually or via config initialization script)
- Committed: Depends (`.example` files committed, actual values may be encrypted)

**`.claude/`:**
- Purpose: Claude Code configuration and rules.
- Generated: No (manually maintained)
- Committed: Yes (contains CLAUDE.md, rules/ directory)

---

*Structure analysis: 2026-03-02*
