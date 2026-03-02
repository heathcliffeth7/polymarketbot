# External Integrations

**Analysis Date:** 2026-03-02

## APIs & External Services

**Polymarket CLOB API:**
- Service: Polymarket prediction market CLOB (Central Limit Order Book)
- What it's used for: Real-time price streams, order placement, order status, fill events, market discovery
- SDK/Client: `reqwest` HTTP client + custom `ClobRestClient` wrapper
  - Location: `crates/bot-infra/src/exchange.rs`
- Endpoints:
  - REST: `https://clob.polymarket.com`
  - WebSocket: `wss://ws-subscriptions-clob.polymarket.com/ws/`
- Auth: HMAC-SHA256 header signing + API passphrase
  - Headers required: `POLY_ADDRESS`, `POLY_SIGNATURE`, `POLY_TIMESTAMP`, `POLY_API_KEY`, `POLY_PASSPHRASE`
  - Signature algorithm: HMAC-SHA256 over message format `{timestamp}{method}{path}{body}` with base64url-decoded secret
  - Implementation: `crates/bot-infra/src/signer.rs` - `ClobHeaderSigner` trait

**Polymarket Gamma API:**
- Service: Polymarket market metadata and pricing
- What it's used for: Market discovery (active 5m/15m markets), market details, snapshot pricing
- SDK/Client: `reqwest` HTTP client
  - Location: `crates/bot-infra/src/exchange.rs`
- Endpoints:
  - REST: `https://gamma-api.polymarket.com`
- Auth: None (public API)
- Key data structures: `GammaMarket` (slug, active flag, token IDs, fees)

**Ethereum/Polygon JSON-RPC:**
- Service: Polygon blockchain (chain_id=137)
- What it's used for: EIP-712 order signing, auto-claim transaction execution
- SDK/Client: `ethers` library with `LocalWallet`
- Env vars:
  - `CLAIM_RPC_URL` - RPC endpoint for claim operations (e.g., `https://polygon-rpc.com`)
  - `CLAIMER_PRIVATE_KEY` - Private key for signer (0x-prefixed hex)
- Operations:
  - EIP-712 signing: Domain name="Polymarket CTF Exchange", chainId=137, verifyingContract=`ctf_exchange_address`
  - Auto-claim: Submitting claim transactions to CTF Exchange contract
  - Location: `crates/bot-infra/src/signer.rs` - `sign_order_eip712()`

## Data Storage

**Databases:**
- Type: PostgreSQL 13+
  - Connection: `DATABASE_URL` environment variable (format: `postgres://user:pass@host:port/dbname`)
  - Client: sqlx with async tokio-rustls driver
  - Location: `crates/bot-infra/src/db.rs` - `PostgresRepository` trait impl
  - Schema: 15 migrations (001_init.sql through 015_trade_flow_steps_claim_index.sql)
  - Tables (~30): markets, trades, orders, fills, positions, risk_events, bot_runs, config_snapshots, idempotency_keys, reconcile_runs, leg_positions, position_exit_rules, pressure_snapshots, trade_builder_*, trade_flow_*, auto_claim_*
  - Queries: Read/write via `StateRepository` trait (enforces atomic transitions)

**Caching:**
- Type: Redis 6+
  - Connection: Default localhost:6379 (hardcoded)
  - Client: `redis` crate with tokio-compatible features
  - Purpose: Idempotent WebSocket event processing, distributed locking
  - Key patterns:
    - `lock:trade:{market_id}` - Mutex lock for concurrent entry prevention
    - `cache:last_price:{market_slug}` - Latest market price
    - `health:ws` - WebSocket connectivity flag

**File Storage:**
- Type: Local filesystem only
  - Config files: `./config/` directory (bot.toml, strategy.toml, risk.toml, etc.)
  - Encrypted config values: AES-256-GCM prefixed with `enc:v1:`
  - Migration scripts: `./migrations/` directory (SQL files)

## Authentication & Identity

**Exchange API:**
- Auth Provider: Polymarket (custom HMAC + API credentials)
  - Implementation: `crates/bot-infra/src/signer.rs` - `ApiCredentials` struct + `ClobHeaderSigner`
  - Config location: `config/exchange.toml`
  - Fields:
    - `api_address` - Polymarket address (UUID format)
    - `api_key` - API key for CLOB (UUID format)
    - `api_secret` - Base64url-encoded secret
    - `api_passphrase` - Account passphrase (hex string)
  - Environment variable fallback: `PM_API_KEY`, `PM_API_SECRET`, `PM_API_PASSPHRASE`, `PM_POLY_ADDRESS`

**Order Signing:**
- Auth Provider: Ethereum/Polygon (EIP-712)
  - Signer: LocalWallet from ethers library
  - Private key source: `config/exchange.toml` (`signer_private_key` with env var fallback `PM_SIGNER_PRIVATE_KEY`)
  - Signature format: EIP-712 structured data (domain hash + order hash)
  - Contract: CTF Exchange at `ctf_exchange_address` (0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E for regular markets)
  - Gnosis Safe support: Optional proxy wallet (`gnosis_safe_address`)

**Frontend Web:**
- Auth Provider: Custom JWT-based (no OAuth)
  - Implementation: `jose` library for JWT signing
  - Session storage: httpOnly secure cookie (AUTH_COOKIE_SECURE environment variable controls flag)
  - Secret: `AUTH_SECRET` environment variable (required)
  - Auth routes: `frontend/src/app/api/auth/*` endpoints
  - No user database (single-user deployment, hardcoded password via AUTH_SECRET)

## Monitoring & Observability

**Error Tracking:**
- Service: None configured (errors logged to stdout/journalctl)

**Logging:**
- Approach: Structured JSON logging via tracing ecosystem
  - Crate: `tracing` + `tracing-subscriber` 0.3 with `env-filter`, `json` features
  - Configuration: `RUST_LOG` environment variable (e.g., `info`, `debug`, `bot_runner=trace`)
  - Output: stdout (captured by systemd journalctl or docker logs)
  - Frontend: JavaScript console.error() for errors, no structured logging

**Health Checks:**
- Bot service: `./scripts/check_health.sh` - Queries `bot_runs` table for recent activity
- Frontend: `/api/bot/status` endpoint checks:
  - systemd service status via `systemctl` command
  - Last bot run timestamp in database
  - Market discovery state from database queries
  - Market selection status
- WebSocket health: Redis key `health:ws` updated on connection

## CI/CD & Deployment

**Hosting:**
- Platform: Self-hosted Linux server (systemd-based)
  - Backend service: `dextrabot.service` unit (runs bot-runner binary as `dextrabot` system user)
  - Frontend service: `dextrabot-frontend.service` unit (runs Next.js via npm start)
  - Config location: `/etc/dextrabot/dextrabot.env` for environment variables

**Deployment Steps:**
1. Rust: `cargo build --release -p bot-runner`
2. Binary placement: systemd ExecStart points to compiled binary
3. Frontend: `npm run build && npm run start` (or via systemd service)
4. Database migrations: `./scripts/apply_migrations.sh` (runs .sql files via psql)
5. Service management: `systemctl start|stop|restart|status dextrabot` and `dextrabot-frontend`

**CI Pipeline:**
- Service: None (manual deployment)
- Build process: Local `cargo build --release` before copying to server

## Environment Configuration

**Required env vars:**
- `DATABASE_URL` - PostgreSQL connection (required by bot and frontend)
- `BOT_CONFIG_DIR` - Config directory path (required by bot)
- `CONFIG_ENCRYPTION_KEY` - Base64-encoded 32-byte AES key (required for live mode)
- `AUTH_SECRET` - Frontend authentication secret (required for login)
- `LIVE_TRADING_ENABLED` - Enable real order placement (must be `true` for live)
- `PM_POLY_ADDRESS` - Polymarket address (or in config/exchange.toml)
- `PM_API_KEY` - API key (or in config/exchange.toml)
- `PM_API_SECRET` - API secret (or in config/exchange.toml)
- `PM_API_PASSPHRASE` - API passphrase (or in config/exchange.toml)
- `PM_SIGNER_PRIVATE_KEY` - Private key for EIP-712 (or in config/exchange.toml)
- `RUST_LOG` - Tracing filter (optional, defaults to info)
- `CLAIM_RPC_URL` - Polygon RPC for auto-claim (optional, e.g., https://polygon-rpc.com)
- `CLAIMER_PRIVATE_KEY` - Private key for claim transactions (optional)
- `AUTH_COOKIE_SECURE` - Force secure cookie (default: true, set false for HTTP deployments)
- `SOCKS5_PROXY_URL` - HTTP SOCKS5 proxy (optional)

**Secrets location:**
- `config/exchange.toml` - Encrypted API credentials with `enc:v1:` prefix
  - Values encrypted with `CONFIG_ENCRYPTION_KEY` using AES-256-GCM
  - Encryption/decryption: `crates/bot-infra/src/config.rs`
- Environment variables as fallback (suffix `_env` in config.toml)
- Never stored in `.env` file (should be set in systemd unit or deployment tool)

## Webhooks & Callbacks

**Incoming:**
- None configured (bot polls CLOB REST API and subscribes to WebSocket channels)

**Outgoing:**
- None configured (no third-party services notified of trade events)

## Data Flow Diagram

```
┌─────────────────────────────────────────────────────────┐
│  External Services                                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  Polymarket CLOB REST/WS                                │
│  ├─ Market discovery (Gamma API)                       │
│  ├─ Price streams (CLOB WS: market channel)           │
│  ├─ Order placement/status/fills (CLOB REST + WS)    │
│  └─ User channel (fill events via WS)                 │
│                                                         │
│  Ethereum/Polygon JSON-RPC                             │
│  ├─ EIP-712 order signing (ethers LocalWallet)       │
│  └─ Auto-claim transactions (if CLAIMER_PRIVATE_KEY) │
└─────────────────────────────────────────────────────────┘
                         ↕ HTTP/WS
┌─────────────────────────────────────────────────────────┐
│  Bot Runner (crates/bot-runner)                        │
├─────────────────────────────────────────────────────────┤
│  ├─ market_discovery loop (Gamma API → markets table) │
│  ├─ price_stream WS listener                          │
│  ├─ signal_engine (PriceThresholdStrategy)            │
│  ├─ execution_engine (ClobRestClient order ops)      │
│  └─ risk_engine (pre/post-trade checks)               │
└─────────────────────────────────────────────────────────┘
                    ↕ SQL (sqlx)
┌─────────────────────────────────────────────────────────┐
│  PostgreSQL 13+                                        │
├─────────────────────────────────────────────────────────┤
│  ├─ trades, orders, fills, positions                   │
│  ├─ risk_events, reconcile_runs                       │
│  ├─ idempotency_keys (WS dedup)                       │
│  └─ bot_runs, config_snapshots                        │
└─────────────────────────────────────────────────────────┘
                    ↕ Redis GET/SET
                   ┌─────────────┐
                   │ Redis 6+    │
                   │ ├─ locks    │
                   │ └─ prices   │
                   └─────────────┘

┌─────────────────────────────────────────────────────────┐
│  Frontend (Next.js 16.1)                               │
├─────────────────────────────────────────────────────────┤
│  ├─ API routes: /api/bot, /api/config, /api/trades    │
│  ├─ SWR hooks: Polling dashboard/trades/orders        │
│  └─ Auth: Jose JWT + httpOnly cookies                 │
└─────────────────────────────────────────────────────────┘
                    ↕ HTTP
           ┌─────────────────────────┐
           │ PostgreSQL (pg client)  │
           │ Queries for dashboard   │
           └─────────────────────────┘
```

---

*Integration audit: 2026-03-02*
