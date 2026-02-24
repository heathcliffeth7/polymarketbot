# Config Spec

## `bot.toml`
- `mode`: `paper|live` (live rollout profile uses `live`)
- `market_scope`: `btc_5m_updown`
- `loop_interval_ms`

## `strategy.toml`
- `entry_price` (default `0.60`)
- `tp_pct` (default `0.12`)
- `base_sl_pct` (default `0.08`)
- `aggressive_sl_pct` (default `0.30`)
- `entry_window_sec`
- `max_hold_sec`
- `sl_renew_interval_ms`

## `risk.toml`
- `max_daily_loss_usdc`
- `max_consecutive_losses`
- `max_notional_per_market_usdc`
- `max_open_orders`
- `max_stale_data_ms`
- `kill_switch_mode` (`disabled|manual_only|manual_or_policy`)
- `manual_kill_switch_active`

## `execution.toml`
- `order_type` (`limit`)
- `time_in_force`
- `retry_count`
- `retry_backoff_ms`
- `reconcile_interval_ms`

## `exchange.toml`
- `gamma_base_url`
- `clob_base_url`
- `clob_ws_url`
- `chain_id`
- `api_address` (şifreli `enc:v1:` önerilir)
- `api_key` (şifreli `enc:v1:` önerilir)
- `api_secret` (şifreli `enc:v1:` önerilir)
- `api_passphrase` (şifreli `enc:v1:` önerilir)
- `api_address_env` (legacy fallback, optional)
- `api_key_env` (legacy fallback, optional)
- `api_secret_env` (legacy fallback, optional)
- `api_passphrase_env` (legacy fallback, optional)

## Config Kuralları
- Runtime'da config hash snapshot alınır
- Invalid config -> process start fail
- Live mode'da risk config zorunlu

## Position Exit Rules (DB-backed)
- `position_exit_rules` tablosu ile trade + leg bazlı `drop_sell_pct` tutulur.
- Varsayılan: yeni dual trade için YES/NO leg başına `%15` seed edilir.
- Dashboard kartından leg bazlı değerler runtime sırasında güncellenebilir.

## Trade Builder (Manual/Conditional)
- `trade_builder_orders` tablosu manual ve conditional order tanımlarını saklar.
- `min_price_distance_cent` zorunludur; dolmayan kısım için dinamik reprice davranışında kullanılır.
- Conditional order için `trigger_condition`, `trigger_price`, `expires_at` alanları kullanılır.
- `max_triggers` ile tekrar sayısı sınırlandırılır (1..20).
