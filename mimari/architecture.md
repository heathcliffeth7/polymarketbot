# Architecture

## Üst Düzey Bileşenler
1. `market_discovery`: Aktif 5m market tespiti
2. `price_stream`: CLOB websocket aboneliği
3. `snapshot_client`: REST snapshot/backfill
4. `signal_engine`: Entry/exit koşul değerlendirme
5. `execution_engine`: Order place/cancel/replace
6. `sl_manager`: Aggressive SL renewal
7. `risk_engine`: Guardrail evaluasyonu
8. `state_store`: PostgreSQL + Redis erişimi
9. `scheduler`: 5m cycle orchestration
10. `audit_logger`: Structured event log

## Veri Akışı
1. Scheduler yeni market açılışını bekler.
2. Market discovery aktif market kimliğini belirler.
3. Price stream eventlerini toplar.
4. Stream stale/disconnect olursa snapshot client devreye girer.
5. Signal engine entry koşulu üretirse execution engine order gönderir.
6. Fill eventleri state machine transition tetikler.
7. Risk engine her kritik transition öncesi karar verir (`allow|block|halt`).
8. Tüm eventler audit logger ve DB'ye yazılır.

## Interface Kontratları
- `MarketDataProvider`: stream + snapshot
- `OrderExecutor`: place/cancel/replace/status
- `RiskPolicy`: pre-trade ve post-trade check
- `StateRepository`: atomic state transition
- `Strategy`: signal üretimi ve parametre kullanımı

### Kod Eşleşmesi
- `MarketDataProvider`: `crates/bot-infra/src/market_data.rs`
- `OrderExecutor` ve `StateRepository`: `crates/bot-infra/src/contracts.rs`
- `RiskPolicy` ve `Strategy`: `crates/bot-core/src/risk.rs`, `crates/bot-core/src/strategy.rs`

## Mimari Kuralları
- Execution, signal ve risk katmanları birbirine doğrudan DB bypass etmez.
- State transition sadece repository üstünden yapılır.
- Websocket eventleri idempotent işlenir.

## Edge Cases
- Çift fill event: idempotency key ile tekilleştirme
- Restart sonrası in-memory state kaybı: DB replay + reconcile
- WS + REST çakışması: timestamp önceliği + deterministic merge
