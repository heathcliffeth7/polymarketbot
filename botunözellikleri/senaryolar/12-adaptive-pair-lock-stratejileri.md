# 12 - Adaptive Pair Lock Stratejileri

Güncelleme tarihi: 2026-05-01

## Amaç

Pair lock artık tek bir `edge_pairlock_v1` kararından ibaret değildir. Yeni stratejiler farklı problemi çözer:

- `adaptive_max_price_v1`: geçmiş iyi miss kanıtına göre max price'ı kontrollü gevşetir.
- `manual_adaptive_risk_v1`: manual PTB modunda hacim, trend ve SL geçmişine göre sıkılaştırır veya gevşetir.
- `biased_hedge_v1`: OlympusX tarzı erken dominant primary + sınırlı hedge yaklaşımını uygular.

## Strateji Seçimi

| `pairLockStrategy` | Ana fikir | PTB beklentisi |
|---|---|---|
| `legacy` | Basit iki bacak pair lock | Özel PTB zorunluluğu yok |
| `edge_pairlock_v1` | Cost/edge karar sırası | Genellikle `iv_mismatch_edge` |
| `adaptive_max_price_v1` | Good-miss kanıtıyla max price relax | `priceToBeatMode="iv_mismatch_edge"` |
| `manual_adaptive_risk_v1` | Manual PTB + hacim/trend/SL risk ayarı | `priceToBeatMode="manual"` |
| `biased_hedge_v1` | Dominant primary, küçük hedge, bias invalidation | Stratejiye göre IV/manual alanlar |

Yanlış seçim:

- `manual_adaptive_risk_v1` ile `iv_mismatch_edge` beklemek.
- `adaptive_max_price_v1` ile PTB guard kapalı kullanmak.
- `biased_hedge_v1`'i eşit pair açma stratejisi sanmak.

## `adaptive_max_price_v1`

Bu strateji "bot iyi fırsatları max price yüzünden sürekli kaçırıyor mu?" sorusuna cevap verir. Geçmiş miss'ler analiz edilir; yeterli sayıda iyi miss varsa max price kontrollü gevşer.

Önemli alanlar:

- `adaptiveMaxPriceMissCount`
- `adaptiveMaxPriceRequiredGoodMissCount`
- `adaptiveMaxPriceRelaxCreditCent`
- `adaptiveMaxPriceMaxRelaxCreditCent`
- `adaptiveMaxPriceHardCapCent`
- `adaptiveMaxPriceExtraBufferCent`
- `adaptiveMaxPricePairBufferCent`
- `adaptiveMaxPriceSizeMultiplier`
- `adaptiveMaxPriceWindowStartSec`
- `adaptiveMaxPriceWindowEndSec`
- `adaptiveMaxPriceLateRiskEnabled`
- `adaptiveMaxPriceSlCooldownMarkets`

Karar mantığı:

```text
good miss sayısı yeterli mi?
  hayır -> relax yok
  evet -> relax credit hesapla
    -> hard cap ve pair buffer uygula
    -> late risk aktifse extra buffer/size multiplier sıkılaşır
    -> SL cooldown varsa gevşeme durur
```

Notification alanları:

- `notifyOnAdaptiveMaxPriceEvaluated`
- `notifyOnAdaptiveMaxPriceRelax`
- `notifyOnAdaptiveMaxPriceRelaxSl`
- `notifyOnAdaptiveMaxPriceNoRelaxImportant`
- `notifyOnAdaptiveMaxPriceMissResolved`
- `notifyOnAdaptiveMaxPriceCooldown`
- `notifyOnAdaptiveMaxPriceSummary`
- `adaptiveMaxPriceNotifyMinIntervalSec`
- `adaptiveMaxPriceSummaryEveryMarkets`

## `manual_adaptive_risk_v1`

Bu strateji manual PTB threshold kullanan pair lock'larda piyasa rejimine göre davranışı değiştirir. Amaç her markette aynı max price ve aynı PTB gap ile girmemektir.

Önemli alan aileleri:

- Window: `manualAdaptiveWindowStartSec`, `manualAdaptiveWindowEndSec`
- Hacim eşikleri: `manualAdaptiveVolumeNormalLt`, `manualAdaptiveVolumeElevatedLt`, `manualAdaptiveVolumeHighLt`
- Trend: `manualAdaptiveTrendDeltaUsd`, `manualAdaptiveTrendDeltaUsdByScope`
- Normal/elevated/high rejim ayarları: `manualAdaptive*MaxPrice*`, `manualAdaptive*SizeMultiplier`, `manualAdaptive*PtbGapAddCent`
- Miss relax: `manualAdaptiveMissRelaxEnabled`, `manualAdaptiveMissRelaxAfterNoOrderMarkets`
- SL tighten: `manualAdaptiveSlTightenEnabled`, `manualAdaptivePtbSlBump*`, `manualAdaptiveMaxPriceSlPenalty*`
- Lockdown: `manualAdaptiveConsecutiveSlLockdownAfter`, `manualAdaptiveLockdownReleaseCleanMarkets`, `manualAdaptiveLockdownMaxMarkets`
- Decay: `manualAdaptiveCleanMarketDecayEnabled`, `manualAdaptive*Decay*`

Self tuning açıkken:

- Miss serisi varsa PTB/max price gevşeyebilir.
- SL serisi varsa PTB bump ve max price penalty artabilir.
- Clean market geldikçe relax veya SL penalty decay ile azalabilir.
- Lockdown aktifse re-entry veya yeni risk alma geçici durur.

Counter cap bildirimleri:

- `notifyOnManualAdaptiveCounterCap`
- `manualAdaptiveCounterCapNotifyMinDeltaCent`

Bu bildirimler counter bacak tavanı risk rejimi yüzünden değiştiğinde kullanılır.

## `biased_hedge_v1`

Bu strateji eşit iki bacak almak için değil, güçlü görünen primary tarafı erken yakalayıp hedge'i sınırlı tutmak için vardır.

Önemli alanlar:

- `biasedHedgePrimaryBudgetUsdc`
- `biasedHedgeHedgeBudgetUsdc`
- `biasedHedgeMinDominantShare`
- `biasedHedgeMaxHedgeSpendRatio`
- `biasedHedgePrimaryMinEdge`
- `biasedHedgePrimaryMinFinalQ`
- `biasedHedgeMaxPriceCent`
- `biasedHedgeHighPriceCent`
- `biasedHedgeHedgeOnlyIfPrimaryFilled`
- `biasedHedgeDisableNewPrimaryAfterSec`
- `biasedHedgeDisableAnyBuyAfterSec`
- `biasedHedgeMaxSideSwitchCount`
- `biasedHedgeMaxPairedEffectiveCostCent`
- `biasedHedgeStopBiasInvalidationEnabled`
- `biasedHedgeStopTimeExitRulesJson`

Runtime yorumu:

- Primary taraf dominant share ve edge koşullarını geçerse alınır.
- Hedge genellikle primary fill sonrası ve sınırlı bütçeyle açılır.
- Geç window'da yeni primary veya tüm buy'lar kapatılabilir.
- Bias invalidation aktifse q_final/edge bozulunca pozisyonun bir kısmı satılır.
- Time exit rules kalan pozisyonu window ilerledikçe azaltabilir.

## Primary Re-entry Alanları

Pair lock akışı primary re-entry alanlarını child node'a taşır:

- `reentryMinPriceCent`
- `reentryMaxPriceCent`
- `reentryPriceToBeatMaxDiff`
- `reentryPriceToBeatMaxDiffUnit`
- `reentrySkipCurrentWindow`
- `reentryThresholdDecay`
- `reentryMaxPriceTightenBps`

Counter re-entry ve counter staged exit hâlâ desteklenmez. Pair lock'ta re-entry bekleniyorsa bunun primary bacak davranışı olduğu açık okunmalıdır.

## Operatör Checklist

1. `pairLockStrategy` ile `priceToBeatMode` uyumlu mu?
2. Strateji window içinde mi çalışıyor?
3. Miss relax mi, SL tighten mı, lockdown mı aktif?
4. Telegram summary throttle yüzünden bildirim sessiz olabilir mi?
5. Node snapshot payload'ında `adaptiveMaxPrice` veya `manualAdaptiveRisk` uygulanmış mı?
6. Pair locked olduktan sonra `pairIgnoreStopLossAfterLocked` SL child davranışını değiştirmiş mi?

## Yanlış Yorumlar

| Yanlış yorum | Doğru yorum |
|---|---|
| Adaptive max price her miss sonrası gevşer | Sadece good-miss koşulları sağlanırsa gevşer |
| Manual adaptive risk IV edge gibi çalışır | Manual PTB threshold üzerine rejim ekler |
| Biased hedge pair lock maliyetini eşitler | Primary bias alır, hedge'i sınırlı tutar |
| Re-entry pair lock'ta tamamen yasak | Primary re-entry alanları desteklenir, counter re-entry desteklenmez |

## Kaynak Notu

Kod referansları:

- `crates/bot-runner/src/trade_builder/pair_lock_adaptive_max_price.rs`
- `crates/bot-runner/src/trade_builder/pair_lock_manual_adaptive_risk.rs`
- `crates/bot-runner/src/trade_builder/pair_lock_biased_hedge.rs`
- `crates/bot-runner/src/trade_builder/pair_lock_child_nodes.rs`
- `frontend/src/lib/trade-flow-config-mappers/pair-lock.ts`
