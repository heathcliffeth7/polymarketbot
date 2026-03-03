# Dextrabot - Polymarket Trading Bot

Automated trading bot for Polymarket BTC 5m/15m Up/Down prediction markets.
Rust backend + Next.js 16 frontend. PostgreSQL + Redis.

## Tech Stack

- **Backend**: Rust 2021, tokio, sqlx, reqwest, tokio-tungstenite, ethers, aes-gcm
- **Frontend**: Next.js 16.1, React 19, SWR, @xyflow/react, Tailwind 4, Radix UI, jose
- **Infra**: PostgreSQL 16, Redis 7, systemd services

## Workspace (4 Crates)

| Crate | Path | Role |
|---|---|---|
| bot-core | crates/bot-core/ | Pure domain: types, state machine, risk, strategy. Zero I/O |
| bot-infra | crates/bot-infra/ | Infrastructure: DB, exchange, WS, config, signer, claim |
| bot-runner | crates/bot-runner/ | Orchestration: main loops, market discovery, trade/flow engine |
| mock-exchange | crates/mock-exchange/ | Test fixture: in-memory mock exchange (axum) |

## Build & Run

```bash
# Backend
cargo build -p bot-runner --release
cargo run -p bot-runner

# Frontend
cd frontend && npm install && npm run dev

# Database
./scripts/bootstrap_db.sh
./scripts/apply_migrations.sh

# Health check
./scripts/check_health.sh
./scripts/go_no_go.sh
```

## Architecture Rules

1. **No DB bypass**: All state changes go through `StateRepository` trait. Never raw SQL in runner.
2. **Transitions via repo**: Trade state changes only via `transition_trade_state()` which enforces `can_transition()`.
3. **WS idempotent**: Every WS event processed with `idempotency_keys` table. Duplicate `fill_id` silently skipped.
4. **Config encryption**: Sensitive values use `enc:v1:` prefix (AES-256-GCM). Env var fallback with `_env` suffix.

## State Machine (11 States)

Idle -> WaitingEntry -> EntryPlaced -> EntryPartiallyFilled -> EntryFilled -> TpPlaced -> SlArmed -> ExitFilled -> Settled -> Idle
Any state can transition to Halted on risk breach.

## Execution Modes

- **Paper**: Simulated fills, no real orders (default)
- **Live**: Real CLOB orders with EIP-712 signing on Polygon (chain_id=137)
- **Dual-Side DCA**: Both YES/NO legs, basket P&L management

## Frontend Structure

- Pages: Dashboard, Login, Trades, Orders, Risk, Market, Trade Builder, Settings, Control
- API routes: `frontend/src/app/api/` (50+ endpoints)
- Hooks: `frontend/src/hooks/` (SWR-based polling)
- Components: `frontend/src/components/` (Radix UI + Tailwind)
- Queries: `frontend/src/lib/queries/` (SQL builders)
- Auth: JWT via jose, httpOnly cookie, AUTH_SECRET env var

## Config Files (config/)

| File | Purpose |
|---|---|
| bot.toml | Execution mode, market scope, loop interval |
| strategy.toml | Entry/exit params, DCA settings, TP/SL percentages |
| risk.toml | Daily loss limits, kill switch, max notional |
| execution.toml | Order type, retry logic, reconcile interval |
| exchange.toml | CLOB/Gamma endpoints, encrypted credentials |
| claim.toml | Auto-claim settings for resolved positions |

## Key Environment Variables

DATABASE_URL, BOT_CONFIG_DIR, CONFIG_ENCRYPTION_KEY, LIVE_TRADING_ENABLED,
AUTH_SECRET, RUST_LOG, PM_API_KEY, PM_API_SECRET, PM_SIGNER_PRIVATE_KEY

## Detailed Architecture

@mimari/plan.md
@mimari/architecture.md
@mimari/state-machine.md
@mimari/db-schema.md
