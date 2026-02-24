# Runbook (Local Server - No Docker)

## Servisler
- PostgreSQL (system service)
- Redis (system service)
- Dextrabot (systemd)

## Hızlı Kurulum
1. `DB_APP_PASSWORD='...' ./scripts/setup_server.sh`
2. `DATABASE_URL=... ./scripts/check_health.sh`

## Manuel Kurulum
1. `./scripts/bootstrap_server_services.sh`
2. `DB_APP_PASSWORD='...' ./scripts/bootstrap_db.sh`
3. `export DATABASE_URL=...`
4. `./scripts/apply_migrations.sh`
5. `cargo build --release -p bot-runner`
6. systemd unit + env kur

## Başlatma Sırası
1. `systemctl start postgresql`
2. `systemctl start redis-server`
3. `systemctl start dextrabot`

## Doğrulama
- `systemctl is-active postgresql`
- `systemctl is-active redis-server`
- `systemctl is-active dextrabot`
- `DATABASE_URL=... ./scripts/check_health.sh`

## Günlük Operasyon Checklist
- WS stale/reconcile event oranı
- risk event sayısı
- günlük PnL ve drawdown
- servis restart frekansı

## Go/No-Go
1. `export DATABASE_URL=...`
2. `./scripts/go_no_go.sh`
3. Gate geçmeden `LIVE_TRADING_ENABLED=true` açma

## İlk Canlı Koşu (Düşük Notional)
1. `config/bot.toml` -> `mode = "live"`
2. `config/risk.toml` düşük limitlerle doğrula:
- `max_notional_per_market_usdc = 5.0`
- `max_open_orders = 1`
- `max_consecutive_losses = 2`
- `max_daily_loss_usdc = 10.0`
3. `Settings -> Exchange` ekranından credentialları gir (şifreli `enc:v1:` olarak `exchange.toml`'a yazılır)
4. Runtime env'de `CONFIG_ENCRYPTION_KEY` set et (frontend ile aynı key)
5. `LIVE_TRADING_ENABLED=true` ile başlat
6. İlk 15-30 dakika log kontrol et:
- `LIVE_MARKET_DISCOVERED`
- `WS_CONNECT_OK`
- `WS_USER_CONNECT_OK` veya fallback warning
- `LIVE_ENTRY_ACK`
- `RECONCILE` kayıtları

## Durdurma
1. `systemctl stop dextrabot`
2. Açık trade/state tutarlılığını DB'de doğrula
3. Olası incident reason için `risk_events` tablosunu kontrol et

## Incident Kısa Aksiyonlar
- WS kopma: reconnect + REST fallback doğrulaması
- DB sorunu: PostgreSQL health + disk + connection limit
- Risk halt: reason al, root cause çöz, kontrollü restart yap
- Acil durdurma: `manual_kill_switch_active = true` yapıp botu yeniden başlat
