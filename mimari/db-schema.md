# Database Schema

## PostgreSQL Tabloları

### 1) `markets`
- `id` (pk)
- `market_slug` (unique)
- `starts_at`
- `ends_at`
- `status`

### 2) `orders`
- `id` (pk)
- `trade_id` (fk)
- `exchange_order_id` (unique)
- `intent` (`entry|tp|sl`)
- `side`
- `price`
- `size`
- `status`
- `client_order_id` (nullable unique)
- `last_exchange_status` (nullable)
- `created_at`
- `updated_at`

### 3) `fills`
- `id` (pk)
- `order_id` (fk)
- `fill_id` (unique)
- `price`
- `size`
- `fee`
- `filled_at`

### 4) `trades`
- `id` (pk)
- `market_id` (fk)
- `state`
- `entry_price`
- `exit_price`
- `notional_usdc`
- `realized_pnl`
- `opened_at`
- `closed_at`

### 5) `positions`
- `id` (pk)
- `trade_id` (fk)
- `token_id`
- `qty`
- `avg_price`
- `status`

### 6) `risk_events`
- `id` (pk)
- `trade_id` (nullable fk)
- `event_type`
- `decision` (`allow|block|halt`)
- `details`
- `created_at`

### 7) `bot_runs`
- `id` (pk)
- `mode` (`paper|live`)
- `version`
- `started_at`
- `stopped_at`
- `reason`

### 8) `config_snapshots`
- `id` (pk)
- `run_id` (fk)
- `config_hash`
- `payload_json`
- `created_at`

### 9) `idempotency_keys`
- `id` (pk)
- `event_key` (unique)
- `created_at`

## Redis Kullanımı
- `lock:trade:{market_id}`: concurrent entry engeli
- `cache:last_price:{market_id}`: son fiyat
- `health:ws`: websocket canlılık

## İndeksler
- `orders(trade_id,status)`
- `fills(order_id,filled_at)`
- `trades(market_id,state)`
- `risk_events(created_at,event_type)`

## Veri Tutarlılığı
- Fill eventlerinde `fill_id` unique ile idempotency
- Order update'leri transaction içinde state transition ile birlikte yazılır
