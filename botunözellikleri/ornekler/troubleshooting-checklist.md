# Troubleshooting Checklist

Güncelleme tarihi: 2026-05-01

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
11. DCA live ise upstream `bindingMode="dca_live_only"` mi?
12. Pair adaptive strategy ise node snapshot'ta strategy payload var mı?

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

## 13. Hızlı Karar Ağacı

```text
Sorun var
  -> Pozisyon açıldı mı?
      evet -> exit/re-entry/pair lifecycle incele
      hayır -> submit var mı?
          evet -> fill/liquidity/limit price incele
          hayır -> builder order var mı?
              evet -> pending/reuse/conditional incele
              hayır -> action guard veya trigger routing incele
```

Bu karar ağacı ilk ayrımı doğru yapar: sorun entry öncesi mi, submit sonrası mı, fill sonrası mı?

## 14. Belirtiye Göre İlk Üç Kontrol

| Belirti | 1 | 2 | 3 |
|---|---|---|---|
| Trigger yok | Market scope | Fiyat koşulu | WS/polling staleness |
| Trigger var, action yok | Edge bağlantısı | Node route | Flow state |
| Action var, order yok | Guard block | Risk gate | Fill lock/reuse |
| Submit var, fill yok | Limit price | Depth | CLOB status |
| Fill var, TP yok | TP config | Source trade | Child order event |
| SL sonrası tekrar girmiyor | Re-entry attempt | Cooldown | Current window skip |
| Pair lock yok | Binding mode | Pair max total | Edge decision |
| Relax yok | Global toggle | Miss count | Min depth |
| DCA live yok | Binding mode | Selected outcome | Budget/window guard |
| Adaptive pair gevşemedi | Good miss count | SL cooldown | Strategy window |
| PnL farklı | cashStatus | Activity cash | Node snapshot |
| Claim activation yok | execution_mode | USDC.e balance | relayer adapter |

## 15. Guard Block Sonrası Ne Değiştirilir?

| Block | Önce bak | Sonra değiştir |
|---|---|---|
| Max price | Best ask ve selected max | Entry profile veya max price relax |
| PTB manual | Gap ve threshold | Min gap veya bump/relax |
| IV edge | Edge, regime, Binance, depth | IV time rule veya protection ayarları |
| Execution floor | Depth ve VWAP | Size veya floor threshold |
| Risk gate | Limit ve exposure | Size veya risk limit |
| Stale market | Window ve slug | Auto-scope/boundary ayarı |
| DCA budget | Cost per slug/all slugs | Budget veya level count |
| Adaptive lockdown | SL serisi ve clean market | Lockdown/decay ayarları |
| Funds activation | Safe USDC.e/pUSD balance | Adapter/env/config |

Config'i doğrudan gevşetmeden önce block'un doğru sınıfta olduğundan emin ol. Yanlış guard'ı değiştirmek sorunu çözmez.

## 16. Canlı Olay Notu Şablonu

```text
Tarih/saat:
Market slug:
Window kalan süre:
Flow/node:
Beklenen:
Görülen:
Trigger pass:
Action event:
Guard decision:
Builder order:
Submit:
Fill:
Telegram mesajı:
Analytics query aralığı:
İlk şüphe:
```

Bu şablon ekip içi debug için yeterli bağlam sağlar. Özellikle market slug ve window kalan süre yazılmadan 5m market sorunları sağlıklı incelenemez.

## 17. Güvenli Müdahale Sırası

1. Önce sadece gözlemle: bildirim ve analytics'i aç.
2. Sonra size küçült: kötü ayar büyük zarara dönüşmesin.
3. Sonra guard sebebini düzelt: max price, PTB, depth veya risk.
4. En son strateji davranışını değiştir: re-entry, relax, pair lock, TP/SL.

Bu sıra ters yapılırsa bir debug değişikliği gerçek strateji riskini artırabilir.

## 18. DCA Live Çalışmıyor

Kontrol sırası:

1. Trigger `bindingMode="dca_live_only"` mı?
2. Downstream tek reachable `action.place_order mode="dca_live_v1"` mı?
3. `side="buy"` mı?
4. `sideMode` ile `selectedOutcomes` sayısı uyumlu mu?
5. `initialOrderShares`, `firstDcaShares` veya `targetQty` var mı?
6. `maxTotalCostPerSlugUsdc` ve `maxTotalCostAllSlugsUsdc` budget'ı block ediyor mu?
7. `cycleWindowMode="custom_range"` dışına çıkılmış mı?

## 19. Adaptive Pair Lock Beklenen Kararı Vermiyor

Kontrol sırası:

1. `pairLockStrategy` doğru mu?
2. `adaptive_max_price_v1` için `priceToBeatMode="iv_mismatch_edge"` mi?
3. `manual_adaptive_risk_v1` için `priceToBeatMode="manual"` mı?
4. Strategy window içinde miyiz?
5. SL cooldown, lockdown veya late risk aktif mi?
6. `bot_decision_logs` ve node snapshot aynı market için okunmuş mu?
7. Notification throttle yüzünden Telegram sessiz olabilir mi?

## 20. Analiz PnL Polymarket Profiliyle Uyuşmuyor

Kontrol sırası:

1. 48h, 30d veya custom aralık doğru mu?
2. `cashStatus` pending, redeemed veya unclaimed mı?
3. Activity cash PnL mi diagnostic PnL mi okunuyor?
4. Redeem/claim eventleri tamamlanmış mı?
5. Node snapshot geçmiş config'i gösteriyor mu?

## 21. Claim Funds Activation Hatası

Kontrol sırası:

1. `execution_mode` `builder_relayer` veya `relayer_api_key` mı?
2. Safe üzerinde USDC.e balance `activate_min_usdc` üstünde mi?
3. `CLAIM_RELAYER_ADAPTER_TOKEN` iki tarafta aynı mı?
4. `CLAIM_FUNDS_ACTIVATION_ADAPTER_URL` doğru internal route'a mı gidiyor?
5. Hata `relayer_wallet_activation_required` ise dashboard `Activate Funds` denenmiş mi?
6. Raw hata için `auto_claim_events.payload_json` okunmuş mu?
