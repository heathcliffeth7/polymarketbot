# Troubleshooting Checklist

Bu dosya canlı operasyonda görülen yaygın belirtiler için hızlı kontrol adımları verir.

## 1. Trigger Geçti Ama Order Yok

Kontrol sırası:

1. Trigger event gerçekten `pass=true` mi?
2. Action node çalışmış mı?
3. Action payload içinde block reason var mı?
4. Stale market skip var mı?
5. Max price guard block etmiş mi?
6. Execution floor block etmiş mi?
7. PTB guard block etmiş mi?
8. Risk gate block etmiş mi?
9. Existing order reuse olduğu için yeni order açılmamış mı?
10. `buyFillLockEnabled` ikinci buy'ı engellemiş mi?

İlk bakılacak dosyalar:

- [../senaryolar/08-risk-guardlari-ve-hata-durumlari.md](../senaryolar/08-risk-guardlari-ve-hata-durumlari.md)
- [../senaryolar/09-telegram-telemetri-ve-analiz.md](../senaryolar/09-telegram-telemetri-ve-analiz.md)

## 2. Submit Var Ama Fill Yok

Kontrol:

1. Order CLOB'a gönderildi mi?
2. Limit price current ask ile uyumlu mu?
3. Best ask depth yeterli mi?
4. Market çok hızlı kaydı mı?
5. Order status pending/expired/cancelled mı?
6. `notifyOnOrderNotFilled` mesajı var mı?

Yorum:

- Submit guard pass anlamına gelir.
- Fill olmaması CLOB, fiyat, limit veya liquidity problemidir.

## 3. Auto-Scope Eski Markete Bakıyor

Kontrol:

1. Trigger `marketMode="auto_scope"` mu?
2. `marketScope` doğru asset/timeframe mi?
3. Boundary sonrası context temizlenmiş mi?
4. WS staleness var mı?
5. Gamma yeni marketi geç mi yayınladı?
6. Action stale market skip atmış mı?

Bak:

- [../senaryolar/01-market-dongusu-ve-auto-scope.md](../senaryolar/01-market-dongusu-ve-auto-scope.md)

## 4. Entry Timing Profil Seçilmiyor

Kontrol:

1. `repeatMode="once"` mi?
2. `cycleWindowMode` kapalı mı?
3. Kalan süre profil aralığına düşüyor mu?
4. Aralıklar çakışıyor mu?
5. `startRemainingSec > endRemainingSec` mi?
6. Action explicit `sizeUsdc` verdiği için profile size fallback kullanılmıyor olabilir mi?

## 5. PTB Sürekli Block Ediyor

Kontrol:

1. `priceToBeatMode` hangi mod?
2. Manual threshold çok yüksek mi?
3. Bump effective threshold'u artırmış mı?
4. `iv_mismatch_edge` adaptive regime orange/red mi?
5. Binance stale veya ters mi?
6. Depth guard fail mi?
7. Max price block PTB block sanılıyor olabilir mi?

Bak:

- [../senaryolar/04-ptb-guard-ve-iv-mismatch.md](../senaryolar/04-ptb-guard-ve-iv-mismatch.md)
- [../senaryolar/05-ptb-bump-ve-max-price-relax.md](../senaryolar/05-ptb-bump-ve-max-price-relax.md)

## 6. Relax Açık Ama Gevşeme Yok

Kontrol:

1. Node'da `priceToBeatMaxPriceRelaxEnabled=true` mı?
2. Global `strategy.max_price_relax_enabled=true` mı?
3. `/api/relax` açık durum döndürüyor mu?
4. `priceToBeatMaxPriceRelaxMissCount` doldu mu?
5. Geçmiş markette tradeable saniye var mı?
6. `priceToBeatMaxPriceRelaxMinDepthUsd` depth şartı sağlanmış mı?
7. Analytics `relax_miss_reason` ne diyor?

## 7. SL Sonrası Bot Çok Seçici Oldu

Kontrol:

1. `priceToBeatStopLossBumpEnabled=true` mı?
2. Bump mode `fixed` mi `loss_table` mı?
3. Scope `global` olduğu için başka asset etkileniyor mu?
4. `priceToBeatStopLossBumpMaxValue` tavanına ulaşıldı mı?
5. `priceToBeatStopLossBumpDecayWindows` beklenen hızda azaltıyor mu?

## 8. Re-Entry Çalışmıyor

Kontrol:

1. `reenterOnSlHit=true` mı?
2. `reentryMaxAttempts` doldu mu?
3. `reentryCooldownSec` henüz bitmedi mi?
4. `reentrySkipCurrentWindow=true` olduğu için aynı window atlanıyor mu?
5. Fill lock serbest bırakılmadı mı?
6. PTB bump sonrası threshold artık geçilemiyor mu?

## 9. Pair Lock Kurulmuyor

Kontrol:

1. Upstream tek ve doğrudan `trigger.market_price` mi?
2. Upstream `bindingMode="pair_lock_only"` mi?
3. Action `mode="pair_lock"` mi?
4. `side="buy"` ve USDC sizing mi?
5. `pairMaxTotalCent` çok sıkı mı?
6. Counter leg token çözüldü mü?
7. `edge_pairlock_v1` için `priceToBeatMode="iv_mismatch_edge"` mi?
8. Decision `single_edge`, `fresh_equal_pair`, `position_counter_lock` veya no decision mı?

## 10. Telegram Sessiz

Kontrol:

1. İlgili `notifyOn*` flag açık mı?
2. Event var ama bildirim kapalı mı?
3. Telegram servis/env sorunu var mı?
4. Sadece block bildirimi kapalı, submit bildirimi açık olabilir mi?
5. Counter leg bildirimi için `counterLegNotify*` alanı gerekli mi?

## 11. Hızlı Veri Paketi

Bir sorunu incelerken şu bilgileri birlikte topla:

- Flow id ve node key.
- Market slug.
- Window zamanı.
- Outcome ve token id.
- Trigger output.
- Action event payload.
- Builder order id ve status.
- Telegram mesajı.
- Analytics query zaman aralığı.
- Guard telemetry.

## 12. Hangi Dosyayı Okumalı?

| Sorun | Dosya |
|---|---|
| Market/window | [../senaryolar/01-market-dongusu-ve-auto-scope.md](../senaryolar/01-market-dongusu-ve-auto-scope.md) |
| Trigger/profile | [../senaryolar/02-giris-trigger-ve-zamanlama.md](../senaryolar/02-giris-trigger-ve-zamanlama.md) |
| Order/sizing/fill | [../senaryolar/03-emir-gonderimi-sizing-ve-fill.md](../senaryolar/03-emir-gonderimi-sizing-ve-fill.md) |
| PTB/IV | [../senaryolar/04-ptb-guard-ve-iv-mismatch.md](../senaryolar/04-ptb-guard-ve-iv-mismatch.md) |
| Bump/relax | [../senaryolar/05-ptb-bump-ve-max-price-relax.md](../senaryolar/05-ptb-bump-ve-max-price-relax.md) |
| Exit/re-entry | [../senaryolar/06-tp-sl-time-exit-ve-reentry.md](../senaryolar/06-tp-sl-time-exit-ve-reentry.md) |
| Pair lock | [../senaryolar/07-pair-lock-ve-edge-pairlock.md](../senaryolar/07-pair-lock-ve-edge-pairlock.md) |
| Bildirim/analytics | [../senaryolar/09-telegram-telemetri-ve-analiz.md](../senaryolar/09-telegram-telemetri-ve-analiz.md) |
