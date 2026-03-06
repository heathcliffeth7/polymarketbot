# Dextrabot Polymarket Bot (Phase-2 Paper Runtime)

Rust-based paper-trading runtime for BTC 5m Up/Down workflow.

## Prerequisites (Local Server, No Docker)
- Rust (cargo)
- PostgreSQL + psql client
- Redis server
- systemd

## Quick Start (No Docker)
```bash
cd /home/heathcliff/polymarketbot
DB_APP_PASSWORD='strong-password' ./scripts/setup_server.sh
```

## Manual Setup (Step-by-Step)

### 1) Install Services (Ubuntu)
```bash
./scripts/bootstrap_server_services.sh
```

### 2) Bootstrap Database/User
```bash
DB_APP_PASSWORD='strong-password' ./scripts/bootstrap_db.sh
```

### 3) Apply Migrations
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
./scripts/apply_migrations.sh
```

### 4) Run (Paper or Live-Dry)
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
export BOT_CONFIG_DIR=./config
cargo run -p bot-runner
```

### 5) Run as systemd service
```bash
cargo build --release -p bot-runner
sudo useradd --system --create-home --shell /usr/sbin/nologin dextrabot || true
sudo mkdir -p /etc/dextrabot
sudo cp deploy/systemd/dextrabot.env.example /etc/dextrabot/dextrabot.env
sudo cp deploy/systemd/dextrabot.service /etc/systemd/system/dextrabot.service
sudo systemctl daemon-reload
sudo systemctl enable dextrabot
sudo systemctl restart dextrabot
sudo systemctl status dextrabot --no-pager -l
```

Important: after every `cargo build --release -p bot-runner`, always run `sudo systemctl restart dextrabot` so runtime behavior matches the latest binary.
If you see `trigger_ws_price_enqueued (cross_confirmed)` followed by `step_completed(triggered=false, evaluation_mode=no_cross)`, assume stale process/binary mismatch and restart service.

### 6) Frontend on Server IP (`http://<SERVER_IP>:3000`)
```bash
cd /home/heathcliff/polymarketbot
./scripts/setup_frontend_service.sh
```

If build-time internet access is restricted:
```bash
cd /home/heathcliff/polymarketbot
SKIP_FRONTEND_BUILD=true ./scripts/setup_frontend_service.sh
```

Then:
```bash
sudo systemctl status dextrabot-frontend --no-pager -l
```

If port `3000` is currently used by dev mode, stop it first:
```bash
pkill -f '/home/heathcliff/polymarketbot/frontend/node_modules/.bin/next dev --webpack' || true
```

Important: for HTTP-only server IP deployments keep `AUTH_COOKIE_SECURE=false` in `/etc/dextrabot/dextrabot-frontend.env`.
Login is mandatory and uses `AUTH_SECRET` as password.

## Health Check
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
./scripts/check_health.sh
```

## Notes
- Runtime supports `paper` and `live` mode.
- `live` mode defaults to safe behavior. Real order placement requires `LIVE_TRADING_ENABLED=true`.
- `exchange.toml` is required for Gamma/CLOB URLs and encrypted credentials (`api_address`, `api_key`, `api_secret`, `api_passphrase` as `enc:v1:`).
- `telegram.toml` stores the global Telegram `bot_token`; `chatId` remains workflow node-specific.
- Runtime'da `CONFIG_ENCRYPTION_KEY` zorunludur; frontend ile aynı key kullanılmalıdır.
- Published trade flows are processed by a single `bot-runner` process. Do not run `cargo run -p bot-runner` in parallel with the systemd service.
- Architecture and rollout docs are under `mimari/`.
- Docker compose file is kept for optional dev-only usage.

## Live Mode
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
export BOT_CONFIG_DIR=./config
export CONFIG_ENCRYPTION_KEY='BASE64_32_BYTE_KEY'
export LIVE_TRADING_ENABLED=true
# Optional auto-claim
export POLYMARKET_ADDRESS='0xYOUR_POLYMARKET_ADDRESS'
export CLAIMER_PRIVATE_KEY='0xYOUR_PRIVATE_KEY'
export CLAIM_RPC_URL='https://polygon-rpc.com'
cargo run -p bot-runner
```

If `LIVE_TRADING_ENABLED` is not set to `true`, bot works in live-data/dry-order flow.

## Go/No-Go Gate
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
./scripts/go_no_go.sh
```

## First Live Run (Strict)
1. Gate geç:
```bash
export DATABASE_URL='postgres://dextrabot_app:strong-password@127.0.0.1:5432/dextrabot'
./scripts/go_no_go.sh
```
2. Env hazırla:
```bash
export BOT_CONFIG_DIR=./config
export CONFIG_ENCRYPTION_KEY='BASE64_32_BYTE_KEY'
export LIVE_TRADING_ENABLED=true
```
Credentials değerlerini UI'da `Settings -> Exchange` bölümünden gir; dosyada otomatik şifreli saklanır.
Telegram action node için global bot tokeni UI'da `Settings -> Telegram` bölümünden gir; opsiyonel global `chat_id` de aynı ekrandan tanımlanabilir.
Node icinde `chatId` varsa runtime onu kullanir; node bos ise global `chat_id` fallback olur.
Telegram token/chat ayarlari settings autosave tamamlandıktan sonra bir sonraki Telegram action çalışmasında restart olmadan kullanılır.
3. Çalıştır:
```bash
cargo run -p bot-runner
```
