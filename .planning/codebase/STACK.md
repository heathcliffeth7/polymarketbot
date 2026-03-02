# Technology Stack

**Analysis Date:** 2026-03-02

## Languages

**Primary:**
- Rust 2021 Edition - Backend runtime (bot-runner, bot-infra, bot-core, mock-exchange crates)
- TypeScript 5 - Frontend and API routes

**Secondary:**
- JavaScript (via TypeScript/TSX) - React components and Next.js configuration
- SQL - PostgreSQL migrations and database schema (15+ migrations)

## Runtime

**Environment:**
- Rust async runtime: tokio 1.x with multi-threaded scheduler and macros support
- Node.js (via Next.js 16.1) - Frontend development server and production build

**Package Manager:**
- Cargo - Rust workspace manager with 4-crate monorepo
- npm - Node.js package manager for frontend dependencies
- Lockfile: `Cargo.lock` (present), `package-lock.json` (present in frontend/)

## Frameworks

**Backend:**
- tokio 1.x - Async runtime with features: `rt-multi-thread`, `macros`, `time`
- axum 0.7 - Web server framework (used in mock-exchange crate with WebSocket support)
- sqlx 0.8 - Async SQL toolkit with compile-time query verification
  - Features: `runtime-tokio-rustls`, `postgres`, `uuid`, `chrono`

**Frontend:**
- Next.js 16.1.6 - React meta-framework with server-side rendering and API routes
- React 19.2.3 - UI library
- TypeScript 5 - Type safety for JavaScript

**Testing:**
- Not configured in workspace (mock-exchange serves as test fixture instead)

**Build/Dev:**
- Cargo (Rust build system) - Workspace compilation
- webpack (via Next.js) - Frontend bundling
- tsc (TypeScript compiler) - Type checking
- ESLint 9 - JavaScript/TypeScript linting
- Tailwind CSS 4 - Utility-first CSS framework with PostCSS integration
- SWR 2.4.0 - Client-side data fetching with polling

## Key Dependencies

**Critical (Workspace-shared):**
- serde 1.x - Serialization/deserialization with `derive` feature
- serde_json 1.x - JSON handling
- tokio 1.x - Core async runtime
- tracing 0.1 - Structured logging framework
- tracing-subscriber 0.3 - Log output with features: `env-filter`, `json`
- chrono 0.4 - DateTime handling with `serde` support
- uuid 1.x - Unique identifiers with features: `v4`, `serde`
- anyhow 1.x - Error handling wrapper
- thiserror 1.x - Derive macro for domain error types

**Cryptography & Signing:**
- ethers 2.x - Ethereum library with features: `abigen`, `rustls`
  - Used for: EIP-712 structured signing, LocalWallet, address/U256 types
- aes-gcm 0.10 - AES-256-GCM encryption for config values
- hmac 0.12 - HMAC-SHA256 for Polymarket API request signing
- sha2 0.10 - SHA256 hashing
- base64 0.22 - Base64 URL-safe encoding/decoding

**HTTP & WebSocket:**
- reqwest 0.12 - Async HTTP client with features: `json`, `rustls-tls`, `socks`
  - SOCKS5 proxy support via environment variable
- tokio-tungstenite 0.24 - WebSocket client/server with features: `rustls-tls-webpki-roots`
- futures-util 0.3 - Future utilities (SinkExt, StreamExt for WS)

**Data Storage:**
- sqlx 0.8 - PostgreSQL async query executor
- redis 0.27 - Redis client with features: `tokio-comp` (tokio-compatible)
- toml 0.8 - TOML configuration parsing

**Concurrency & Async:**
- async-trait 0.1 - Trait support for async functions

**Frontend Dependencies:**
- @xyflow/react 12.10.1 - Flow/diagram visualization (trade builder canvas)
- lucide-react 0.564.0 - Icon library
- radix-ui 1.4.3 - Unstyled component primitives
- sonner 2.0.7 - Toast notifications
- next-themes 0.4.6 - Dark mode support
- jose 6.1.3 - JSON Web Token handling for auth
- clsx 2.1.1 - Conditional CSS class utility
- tailwind-merge 3.4.1 - Tailwind class conflict resolution
- pg 8.18.0 - PostgreSQL client (for API routes connecting to database)
- @iarna/toml 2.2.5 - TOML parsing in frontend
- @types/pg 8.16.0 - TypeScript definitions for pg client

## Configuration

**Environment:**
- `DATABASE_URL` - PostgreSQL connection string (required)
- `BOT_CONFIG_DIR` - Path to config directory (default: `./config`)
- `CONFIG_ENCRYPTION_KEY` - Base64-encoded 32-byte AES key (required for live mode)
- `RUST_LOG` - Tracing log level filter (e.g., `info,bot_runner=debug`)
- `LIVE_TRADING_ENABLED` - Boolean flag to enable real order placement (must be `true` for live)
- `SOCKS5_PROXY_URL` - Optional HTTP proxy for requests
- `AUTH_SECRET` - Frontend authentication secret (required for login)
- `AUTH_COOKIE_SECURE` - Boolean flag for secure cookie handling in HTTP deployments

**Build:**
- `Cargo.toml` - Rust workspace manifest with shared dependencies
- `Cargo.lock` - Pinned dependency versions
- `frontend/package.json` - Node.js dependencies
- `frontend/next.config.ts` - Next.js configuration
- `frontend/tsconfig.json` - TypeScript configuration
- `frontend/eslint.config.mjs` - ESLint rules
- `frontend/postcss.config.mjs` - PostCSS for Tailwind CSS
- TOML files in `config/` directory:
  - `bot.toml` - Execution mode, market scope, loop interval
  - `strategy.toml` - Entry/exit parameters, DCA settings, TP/SL percentages
  - `risk.toml` - Daily loss limits, kill switch, max notional
  - `execution.toml` - Order type, retry logic, reconciliation interval
  - `exchange.toml` - CLOB/Gamma endpoints, encrypted API credentials
  - `claim.toml` - Auto-claim settings for resolved positions

## Platform Requirements

**Development:**
- Rust 1.70+ (for 2021 edition features)
- PostgreSQL 13+ with pgcrypto extension
- Redis 6+
- Node.js 18+ (for frontend development)
- systemd (for service deployment)

**Production:**
- Rust binary compiled with `--release` flag
- PostgreSQL 13+ on separate host or localhost
- Redis 6+ for caching and locks
- systemd for bot service management
- Optional: systemd for frontend Next.js service
- Polygon RPC endpoint (for chain_id=137 signing and claim operations)

## Crate Organization

**Workspace Structure** (`Cargo.toml`):
- `crates/bot-core` - Pure domain logic (state machine, types, risk rules, strategy)
- `crates/bot-infra` - Infrastructure layer (database, HTTP clients, WebSocket, signing, config)
- `crates/bot-runner` - Orchestration and main event loops
- `crates/mock-exchange` - axum-based mock exchange server (testing/dev fixture)

---

*Stack analysis: 2026-03-02*
