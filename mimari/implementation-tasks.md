# Implementation Tasks (MVP Backlog)

## Sprint 0 - Foundation

1. Rust workspace oluştur (`bot-core`, `bot-runner`, `bot-infra`).
2. Ortak domain type'ları tanımla:
- `ExecutionMode`
- `TradeState`
- `OrderIntent`
- `RiskDecision`
- `MarketCycleId`
3. Config loader yaz (`bot.toml`, `strategy.toml`, `risk.toml`, `execution.toml`).
4. Structured logger (JSON) ve correlation id desteği ekle.
5. Postgres ve Redis bağlantı katmanını hazırla.

## Sprint 1 - Data + State

1. `markets`, `trades`, `orders`, `fills`, `risk_events`, `bot_runs`, `config_snapshots` migrationlarını yaz.
2. State repository (atomic transition) implement et.
3. Market discovery modülünü yaz (BTC 5m market seçimi).
4. WebSocket istemcisi yaz (connect, subscribe, heartbeat, reconnect).
5. REST snapshot fallback modülünü yaz.
6. WS + REST reconcile akışını implement et.

## Sprint 2 - Execution Pipeline

1. Signal engine yaz (entry price tetikleme + pencere kontrolü).
2. Execution engine yaz:
- entry order place
- TP order place
- cancel/replace
- fill event işleme
3. Partial fill state geçişlerini tamamla.
4. Trade lifecycle state machine'i canlı eventlerle bağla.
5. Audit event tiplerini log + DB’ye yaz.

## Sprint 3 - Risk + Safety

1. Risk engine implement et:
- daily loss
- consecutive losses
- max notional
- max open orders
- stale data block
2. Kill-switch (manual + policy halt) ekle.
3. Halt davranışını uygula:
- yeni entry engelle
- açık orderlar için güvenli aksiyon
4. Nonce/signature hata sınıflandırması ve recovery hook’ları ekle.

## Sprint 4 - SL/TP Behavior Hardening

1. Aggressive SL manager yaz (renewal interval + cancel/replace loop).
2. Timeout ve market-end yakınında davranış kuralları ekle.
3. TP/SL çatışma durumlarında deterministic öncelik kuralı uygula.
4. Duplicate fill/idempotency korumasını güçlendir.

## Sprint 5 - Paper Trading Acceptance

1. Paper mode'da en az 100 trade topla.
2. Acceptance testlerini çalıştır (`acceptance-tests.md`).
3. Kritik metrikleri raporla:
- fill rate
- reject rate
- reconcile error rate
- drawdown
- expectancy
4. Go/No-Go kararı çıkar.

## Sprint 6 - Controlled Live Rollout

1. Live mode’u düşük notional ile aç.
2. Risk limitlerini daha sıkı değerlere çek.
3. Günlük operasyon runbook’una göre takip et.
4. Incident playbook tatbikatlarını uygula.

## Technical Deliverables Checklist

- [ ] Tüm migrationlar çalışıyor
Kanıt: `scripts/go_no_go.sh` migration gate (DB bağlantısı ile doğrulanmalı).
- [x] State machine transition testleri geçti
Kanıt: `crates/bot-core/src/state_machine.rs` içindeki `allows_valid_transition_path` ve `rejects_invalid_transition_path`.
- [x] WS reconnect + REST reconcile testi geçti
Kanıt: `crates/bot-infra/src/exchange.rs` içindeki `place_and_reconcile_against_mock_exchange`.
- [x] Risk policy breach’te halt doğrulandı
Kanıt: `crates/bot-core/src/risk.rs` içindeki `halts_on_daily_loss` ve `halts_on_manual_kill_switch`.
- [ ] Paper acceptance kriterleri sağlandı
Kanıt: `mimari/acceptance-tests.md` maddeleri için 100+ trade paper raporu bekleniyor.
- [ ] `scripts/go_no_go.sh` geçti
Kanıt: script artık metrik eşiklerini fail/pass ile enforce ediyor; DB ile çalıştırılarak onaylanmalı.
- [ ] Live rollout checklist tamamlandı
Kanıt: `mimari/runbook.md` canlı koşu checklist adımları operasyonel onay bekliyor.

## Definition of Done (MVP)

1. Bot paper mode’da stabil çalışır.
2. State tutarlılığı bozulmadan restart/recover eder.
3. Risk guardrail’leri gerçek zamanlı enforcement sağlar.
4. Acceptance testleri geçer.
5. Live mode düşük notional ile kontrollü şekilde çalışır.
