# Dextrabot

Dextrabot is a Rust and Next.js trading automation workspace for Polymarket BTC Up/Down markets. It includes a backend runtime, PostgreSQL persistence, Redis-backed coordination, a visual trade-flow builder, analytics screens, Telegram notifications, and optional claim/funds activation tooling.

The default configuration is paper-oriented. Live trading requires explicit credentials and `LIVE_TRADING_ENABLED=true`.

## Features

- Rust runtime for market discovery, workflow execution, order lifecycle management, and risk gates.
- Next.js dashboard for configuration, trade-flow editing, runtime control, order views, analytics, and settings.
- Visual flow builder with market-price triggers, place-order actions, pair-lock strategies, DCA live flows, PTB guards, IV mismatch checks, stop-loss logic, re-entry controls, and Telegram actions.
- PostgreSQL schema migrations for trades, builder orders, workflow runs, decision logs, analytics snapshots, and user settings.
- Polymarket Gamma/CLOB integration, encrypted credential storage, EIP-712 signing support, and optional builder relayer configuration.
- Operational scripts for local service setup, migrations, health checks, and go/no-go validation.

## Architecture

| Area | Path | Purpose |
|---|---|---|
| Core domain | `crates/bot-core` | Shared domain types, risk types, execution modes, and state-machine primitives. |
| Infrastructure | `crates/bot-infra` | PostgreSQL repository, exchange clients, websocket support, config loading, signing, and claim helpers. |
| Runtime | `crates/bot-runner` | Main orchestration loops, trade-flow execution, order handling, guards, notifications, and analytics refresh. |
| Mock exchange | `crates/mock-exchange` | In-memory exchange fixture for tests. |
| Frontend | `frontend` | Next.js dashboard, API routes, flow builder UI, settings, and analytics views. |
| Database | `migrations` | Ordered PostgreSQL migrations. |

## Requirements

- Rust stable toolchain with `cargo`
- Node.js 20+ and npm
- PostgreSQL
- Redis
- `psql` client
- Linux with systemd for service installation scripts

## Configuration

Runtime config files are local files and are not committed. Create them from the examples:

```bash
cp config/bot.toml.example config/bot.toml
cp config/strategy.toml.example config/strategy.toml
cp config/risk.toml.example config/risk.toml
cp config/execution.toml.example config/execution.toml
cp config/exchange.toml.example config/exchange.toml
cp config/claim.toml.example config/claim.toml
cp config/telegram.toml.example config/telegram.toml
```

Sensitive values can be stored encrypted with the `enc:v1:` prefix or supplied through the documented `*_env` fields. The frontend and backend must use the same `CONFIG_ENCRYPTION_KEY` when saving or reading encrypted credentials.

Important environment variables:

```bash
DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot
BOT_CONFIG_DIR=./config
CONFIG_ENCRYPTION_KEY=<base64-encoded-32-byte-key>
AUTH_SECRET=<strong-dashboard-secret>
LIVE_TRADING_ENABLED=false
```

## Local Development

Apply database migrations:

```bash
export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot
./scripts/apply_migrations.sh
```

Run the backend:

```bash
export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot
export BOT_CONFIG_DIR=./config
cargo run -p bot-runner
```

Run the frontend:

```bash
cd frontend
npm install
npm run dev
```

Open `http://localhost:3000`.

## Server Setup

The setup script installs PostgreSQL/Redis dependencies, bootstraps the database role, applies migrations, builds the runtime, installs a systemd unit, and prepares config directory permissions:

```bash
DB_APP_PASSWORD='<strong-password>' ./scripts/setup_server.sh
```

Install the dashboard as a systemd service:

```bash
./scripts/setup_frontend_service.sh
```

For HTTP-only server-IP deployments, set `AUTH_COOKIE_SECURE=false` in the frontend environment file.

## Live Trading

Live trading is disabled unless all of the following are true:

- `config/bot.toml` uses a live-capable mode.
- Exchange credentials are configured in `config/exchange.toml` or matching environment variables.
- `CONFIG_ENCRYPTION_KEY` is set if encrypted values are used.
- `LIVE_TRADING_ENABLED=true` is present in the backend environment.
- `./scripts/go_no_go.sh` passes.

Example:

```bash
export DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot
export BOT_CONFIG_DIR=./config
export CONFIG_ENCRYPTION_KEY=<base64-encoded-32-byte-key>
export LIVE_TRADING_ENABLED=true
./scripts/go_no_go.sh
cargo run -p bot-runner
```

## Verification

```bash
cargo check
cargo build --release -p bot-runner
cd frontend && npm run lint && npm run build
```

Operational checks:

```bash
./scripts/check_health.sh
./scripts/go_no_go.sh
```

## Safety Notice

This project automates prediction-market trading workflows. It is provided for engineering and research use. Live trading can lose funds, and configuration mistakes can create unintended orders. Start in paper mode, review every credential and limit, and use the go/no-go gate before enabling live execution.
