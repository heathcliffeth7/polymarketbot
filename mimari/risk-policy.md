# Risk Policy

## Hard Guardrails
1. `max_daily_loss_usdc`
2. `max_consecutive_losses`
3. `max_notional_per_market_usdc`
4. `max_open_orders`
5. `max_stale_data_ms`

## Decision Model
- `allow`: normal işlem
- `block`: ilgili order aksiyonunu reddet
- `halt`: botu trade açısından durdur

## Evaluation Sırası
1. Kill-switch aktif mi?
2. Data stale mi?
3. Daily loss limiti aşıldı mı?
4. Consecutive loss limiti aşıldı mı?
5. Market notional limiti uygun mu?
6. Open order limiti uygun mu?

## Halt Davranışı
- Yeni entry order yasak
- Açık orderlar güvenli şekilde iptal/koruma moduna alınır
- Event log + risk_event kaydı zorunlu

## Önerilen Varsayılanlar (V1)
- `max_daily_loss_usdc = 30`
- `max_consecutive_losses = 3`
- `max_notional_per_market_usdc = 10`
- `max_open_orders = 4`
- `max_stale_data_ms = 3000`

## Edge Cases
- Market bitimine çok az zaman kalmışsa entry engellenebilir
- Risk state unknown durumunda fail-closed (`block`) uygulanır

## Manual/Conditional Order Risk Integration
- Trade Builder orderları da aynı risk policy üzerinden değerlendirilir.
- `risk_check_manual_order` event_type ile değerlendirme sonucu kaydedilir.
- Kill-switch, open-order limiti, günlük zarar ve ardışık kayıp limitleri manual order akışında da geçerlidir.
