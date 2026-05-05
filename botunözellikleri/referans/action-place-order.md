# Referans - `action.place_order`

Güncelleme tarihi: 2026-05-01

Bu dosya `action.place_order` node'unun config gruplarını, guard alanlarını, output/event davranışını ve geçerli kombinasyonlarını özetler.

## Görev

`action.place_order`, upstream trigger'dan gelen bağlama göre builder order üretir veya mevcut order'ı yönetir.

Desteklenen ana davranışlar:

- Buy ve sell order.
- Market veya limit execution.
- Immediate veya conditional order.
- Sizing ve parçalı trigger.
- PTB, max price, execution floor ve underlying guard.
- TP/SL/PTB SL/time exit/re-entry.
- Pair lock ve counter leg.
- DCA live generic slug/outcome buy ladder.
- Telegram ve analytics telemetry.

## Temel Alanlar

| Alan | Açıklama |
|---|---|
| `side` | `buy` veya `sell` |
| `executionMode` | `market` veya `limit` |
| `kind` | `immediate` veya `conditional` |
| `mode` | `single`, `pair_lock` veya `dca_live_v1` |
| `marketSlug` | Config veya upstream context |
| `tokenId` | Config veya upstream context |
| `outcomeLabel` | Config veya upstream context |
| `sourceTradeId` | Sell ve lifecycle bağlamı |

## Sizing Alanları

| Alan | Kullanım |
|---|---|
| `sizeMode` | `usdc`, `shares` veya `pct` |
| `sizeUsdc` | Buy notional |
| `targetNotionalUsdc` | Alternatif buy notional hedefi |
| `targetQty` | Share hedefi veya DCA initial size fallback'i |
| `sizePct` / `sizePercent` | Sell veya kaynak pozisyon yüzdesi |
| `triggerSizes` | Çoklu tetik başına farklı buy büyüklüğü |
| `maxTriggers` | Maksimum tetik sayısı |

Entry timing fallback'i:

- Action explicit size vermiyorsa trigger context'teki `selectedEntrySizeUsdc` kullanılabilir.

## DCA Live Alanları

`mode="dca_live_v1"` generic market/outcome DCA modudur.

| Alan | Açıklama |
|---|---|
| `marketSelectionMode` | `manual_slug`, `manual_slug_list`, `auto_group_top_n`, `auto_scope` |
| `manualSlug`, `manualSlugs` | Manual market kaynağı |
| `marketGroup`, `candidateSlugLimit`, `maxActiveSlugs` | Group/top-n market seçimi |
| `sideMode` | `one_sided`, `two_sided_pair`, `multi_outcome_basket` |
| `selectedOutcomes` | Explicit outcome listesi |
| `initialOrderShares`, `firstDcaShares` | İlk DCA size |
| `dcaEntryMinPriceCent`, `dcaEntryMaxPriceCent` | Entry fiyat bandı |
| `dcaLevels`, `dcaLevelSpacingCent`, `dcaLevelSpacingMultiplier` | Ladder yapısı |
| `dcaOrderSizeMultiplier` | Kademe size çarpanı |
| `targetPairCostCent`, `pairBufferCent` | Binary pair DCA cost hedefi |
| `counterOnlyIfPairCostOk` | Counter bacak sadece pair cost uygunsa |
| `maxTotalCostPerSlugUsdc`, `maxTotalCostAllSlugsUsdc` | Budget guard |
| `maxUnmatchedCostUsdc`, `maxOpenOrdersAllSlugs` | Exposure/open order guard |
| `noNewOrdersBeforeEndSec`, `cancelOpenOrdersBeforeEndSec` | Market sonu risk kontrolü |

Geçerli kullanım için upstream trigger `bindingMode="dca_live_only"` olmalıdır. Ayrıntı: [11-dca-live-ve-trigger-binding.md](../senaryolar/11-dca-live-ve-trigger-binding.md).

## Buy Guard Alanları

| Guard | Alanlar |
|---|---|
| Max price | `maxPrice`, `maxPriceCent`, `retryOnMaxPriceBlock`, `notifyOnMaxPriceBlocked` |
| Trigger price | `triggerCondition`, `triggerPrice`, `retryOnTriggerPriceGuardBlock`, `notifyOnTriggerPriceBlocked` |
| Execution floor | execution floor config, `retryOnExecutionFloorGuardBlock`, `notifyOnExecutionFloorBlocked` |
| PTB | `priceToBeatGuardEnabled`, `priceToBeatMode`, `priceToBeatCurrentPriceSource`, `retryOnPriceToBeatGuardBlock`, `notifyOnPriceToBeatGapBlocked` |
| Underlying | underlying protection config |
| Fill lock | `buyFillLockEnabled`, `releaseBuyFillLockOnStopLoss` |

## PTB Mode Alanları

Temel:

- `priceToBeatGuardEnabled`
- `priceToBeatMode`
- `priceToBeatCurrentPriceSource`: `chainlink` (varsayılan), `binance` veya `coinbase`
- manual threshold alanları
- `priceToBeatIvTimeRules`

`priceToBeatCurrentPriceSource`, PTB referans fiyatını değiştirmez; yalnızca PTB'ye karşı kıyaslanan current underlying fiyat kaynağını seçer. Eski config'lerde alan yoksa `chainlink` kabul edilir. `binance` veya `coinbase` seçiliyken ilgili CEX book/ticker snapshot'ı eksik ya da stale ise Chainlink'e sessiz fallback yapılmaz; mevcut retry/block akışı bekler veya node'u bloklar.

`iv_mismatch_edge` grupları:

- Binance: `priceToBeatIvRequireBinanceFreshUnderSec`, `priceToBeatIvBinanceMaxStaleMs`, `priceToBeatIvRequireBinanceSameDirection`
- Book: `priceToBeatIvProtectionMode`, `priceToBeatIvBookLeadGuardEnabled`, `priceToBeatIvOppositeMidBlockCent`
- Model/book: `priceToBeatIvModelBookGapWarn`, `priceToBeatIvModelBookGapHard`
- Depth: `priceToBeatIvDepthGuardEnabled`, `priceToBeatIvDepthMaxSlippage`
- Late high price: `priceToBeatIvLateHighPrice*`
- Participation: `priceToBeatIvParticipation*`
- Adaptive volume: `priceToBeatIvVolume*`, `priceToBeatIvAdaptive*`

## PTB Bump ve Relax

| Alan | Açıklama |
|---|---|
| `priceToBeatStopLossBumpEnabled` | SL sonrası PTB threshold'u sıkılaştırır |
| `priceToBeatStopLossBumpMode` | `fixed` veya `loss_table` |
| `priceToBeatStopLossBumpAmount` | Fixed bump miktarı |
| `priceToBeatStopLossBumpLossRules` | Loss büyüklüğüne göre bump |
| `priceToBeatStopLossBumpMaxValue` | Bump tavanı |
| `priceToBeatStopLossBumpUnit` | `usd` veya `cent` |
| `priceToBeatStopLossBumpScope` | `global` veya `per_scope` |
| `priceToBeatStopLossBumpDecayWindows` | Window başına decay |
| `priceToBeatMaxPriceRelaxEnabled` | Kaçan market sonrası gevşeme |
| `priceToBeatMaxPriceRelaxMissCount` | Relax başlamadan önce miss sayısı |
| `priceToBeatMaxPriceRelaxHistoryCount` | İncelenecek market sayısı |
| `priceToBeatMaxPriceRelaxMinValue` | Relax tabanı |
| `priceToBeatMaxPriceRelaxMinUnit` | `usd` veya `cent` |
| `priceToBeatMaxPriceRelaxMinDepthUsd` | Tradeable depth tabanı |
| `priceToBeatMaxPriceRelaxStepMode` | `percent` veya `absolute` |
| `priceToBeatMaxPriceRelaxStepValue` | Relax adımı |
| `priceToBeatMaxPriceRelaxStepUnit` | Absolute mod birimi |

## Çıkış Alanları

TP/SL:

- `tpRules`
- `slRules`
- `slTriggerPriceMode`
- `slSiblingPolicy`
- `notifyOnTpHit`
- `notifyOnSlHit`

PTB SL:

- `ptbStopLossEnabled`
- `ptbStopLossGapUsd`
- `ptbStopLossTimeDecayMode`
- `priceToBeatCurrentPriceSource`: fill sonrası üretilen PTB SL child order varsayılan olarak entry PTB current kaynağını miras alır.
- `ptbStopLossCurrentPriceSource`: opsiyonel SL özel override. Boşsa `priceToBeatCurrentPriceSource`, o da boşsa `chainlink` kullanılır. `binance` veya `coinbase` seçiliyken eksik/stale CEX verisinde Chainlink'e sessiz fallback yapılmaz.

Time/window exit:

- `timeExitRules`
- `autoSellOnWindowEnd`

Re-entry:

- `reenterOnSlHit`
- `reentryMaxAttempts`
- `reentryCooldownSec`
- `reentryMinPriceCent`
- `reentryMaxPriceCent`
- `reentryPriceToBeatMaxDiff`
- `reentryPriceToBeatMaxDiffUnit`
- `reentrySkipCurrentWindow`
- `reentryThresholdDecay`
- `reentryMaxPriceTightenBps`

## Pair Lock Alanları

| Alan | Açıklama |
|---|---|
| `mode="pair_lock"` | Pair lock behavior açar |
| `pairLockStrategy` | `legacy`, `edge_pairlock_v1`, `adaptive_max_price_v1`, `manual_adaptive_risk_v1`, `biased_hedge_v1` |
| `pairLockDecisionQty` | Edge hesabı qty |
| `pairLockSingleEdgeThreshold` | Tek taraf edge eşiği |
| `pairLockCostBuffer` | Cost buffer |
| `pairMaxTotalCent` | YES+NO toplam maliyet tavanı |
| `pairTotalBudgetUsdc` | Pair toplam budget |
| `counterLegEnabled` | Counter leg aç |
| `counterLegTpEnabled` | Counter TP |
| `counterLegSlEnabled` | Counter SL |
| `counterLegPtbStopLossEnabled` | Counter PTB SL |
| `counterLegPtbStopLossCurrentPriceSource` | Counter PTB SL current source override; boşsa `counterLegPriceToBeatCurrentPriceSource` miras alınır |
| `pairProtectiveUnwindEnabled` | Orphan/bozuk pair için unwind |
| `pairIgnoreStopLossAfterLocked` | Lock sonrası SL etkisini sınırla |

Adaptive pair lock alan aileleri:

- `adaptiveMaxPrice*`: good-miss kanıtına göre max price relax.
- `manualAdaptive*`: manual PTB, hacim/trend/SL risk rejimi ve self tuning.
- `biasedHedge*`: dominant primary, sınırlı hedge, bias invalidation ve time exit.
- `notifyOnAdaptiveMaxPrice*`, `notifyOnManualAdaptive*`: strategy notification ve summary throttle.

Primary pair lock re-entry alanları child node'a taşınabilir. Counter re-entry ve counter staged exit desteklenmez.

## Bildirim Alanları

- `notifyOnOrderSubmitted`
- `notifyOnOrderPlaced`
- `notifyOnOrderNotFilled`
- `notifyOnTriggerPriceBlocked`
- `notifyOnExecutionFloorBlocked`
- `notifyOnPriceToBeatGapBlocked`
- `notifyOnMaxPriceBlocked`
- `notifyOnTpHit`
- `notifyOnSlHit`
- `notifyOnPairLocked`
- `notifyOnPairUnwind`
- `notifyOnAdaptiveMaxPriceRelax`
- `notifyOnAdaptiveMaxPriceSummary`
- `notifyOnManualAdaptiveRiskBlock`
- `notifyOnManualAdaptiveRiskSummary`
- `notifyOnManualAdaptiveCounterCap`

## Output ve Event

Temel output:

- `builder_order_id`
- `source_trade_id`
- `market_slug`
- `token_id`
- `side`
- `status`
- `should_inline_submit`
- `size_usdc`
- `target_qty`

Guard telemetry:

- `price_to_beat_guard`
- `max_price_guard`
- `execution_floor_guard`
- `trigger_price_guard`
- `risk_decision`

Pair telemetry:

- `pair_session_id`
- `pair_lock_strategy`
- `pair_lock_edge_decision`
- `counter_builder_order_id`
- `adaptiveMaxPrice`
- `manualAdaptiveRisk`
- `biasedHedge`

DCA telemetry:

- `mode="dca_live_v1"`
- selected outcome listesi
- ladder level bilgisi
- budget/window block nedeni

## Geçerli Kombinasyon Notları

- `priceToBeatStopLossBumpEnabled=true`, `side="buy"` ve `priceToBeatGuardEnabled=true` ister.
- Relax config'i PTB guard olmadan anlamlı değildir.
- `edge_pairlock_v1`, `priceToBeatMode="iv_mismatch_edge"` ister.
- `adaptive_max_price_v1`, `priceToBeatGuardEnabled=true` ve `priceToBeatMode="iv_mismatch_edge"` ister.
- `manual_adaptive_risk_v1`, `priceToBeatGuardEnabled=true` ve `priceToBeatMode="manual"` ister.
- `dca_live_v1`, `side="buy"` ve upstream `bindingMode="dca_live_only"` ister.
- Pair lock `sizePct` desteklemez; USDC sizing gerekir.
- `reentrySkipCurrentWindow=true`, `reenterOnSlHit=true` ister.
- `ptbStopLossTimeDecayMode`, buy tarafında `ptbStopLossEnabled=true` gerektirir.

## Minimal Buy Örneği

```json
{
  "side": "buy",
  "executionMode": "market",
  "kind": "immediate",
  "sizeUsdc": 10,
  "notifyOnOrderSubmitted": true
}
```

## Guard'lı Buy Örneği

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 10,
  "maxPrice": 0.62,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvDepthGuardEnabled": true,
  "retryOnPriceToBeatGuardBlock": true,
  "notifyOnPriceToBeatGapBlocked": true
}
```

## Alanların Runtime Etkisi

| Alan grubu | Runtime etkisi | Hata olduğunda belirti |
|---|---|---|
| Market/token çözümü | Hangi token için order üretileceğini belirler | Sell source yok, stale market veya yanlış outcome |
| Sizing | Order notional/qty hesaplar | Beklenen büyüklükten farklı order |
| Max price | Pahalı buy girişini engeller | Max price block |
| Execution floor | Orderbook kalitesini kontrol eder | Best ask/depth block |
| PTB guard | Entry edge/gap kalitesini kontrol eder | PTB block veya IV edge block |
| Risk gate | Sistem limitlerini uygular | RiskDecision block |
| TP/SL | Fill sonrası exit child order üretir | Pozisyon açık ama exit yok |
| Pair lock | YES/NO lifecycle kurar | Pair no decision veya orphan leg |
| Notification | Operatöre mesaj yollar | Event var ama Telegram yok |

## Immediate ve Conditional Ayrımı

`kind="immediate"`:

- Action çalışınca order submit için hazır olur.
- Guard geçerse `should_inline_submit=true` görülebilir.
- Submit ve fill ayrı aşamalardır.

`kind="conditional"`:

- Builder order pending kalabilir.
- Kendi trigger fiyatını bekler.
- Operatör bunu no-order sanmamalıdır.

Conditional flow'larda canlı debug için builder order status mutlaka okunmalıdır.

## Buy Flow İçin Adım Adım Referans

```text
1. Context çöz
2. Stale market kontrol et
3. Buy guard'ları çalıştır
4. source trade oluştur veya bul
5. existing order kontrol et
6. sizing hesapla
7. risk gate çalıştır
8. builder order oluştur
9. notification flag'lerini snapshot al
10. immediate submit veya pending lifecycle'a bırak
```

Her adım ayrı bir failure noktasıdır. Örneğin source trade oluşmadan sell order beklemek hatalıdır; risk gate block ederken PTB config değiştirmek etkisizdir.

## Sell Flow İçin Adım Adım Referans

```text
1. sourceTradeId çöz
2. Pozisyon kalan qty hesapla
3. Sell size pct veya full close kararını uygula
4. Existing failed sell varsa rearm değerlendir
5. Builder sell order oluştur
6. Submit/fill lifecycle'a geç
```

Sell flow'unda en sık hata source trade veya pozisyon bağlamının olmamasıdır. Buy tarafındaki otomatik source oluşturma davranışı sell için aynı şekilde düşünülmemelidir.

## Pair Lock İçin Geçerli Minimum Yapı

Trigger:

```json
{
  "type": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "bindingMode": "pair_lock_only"
  }
}
```

Action:

```json
{
  "mode": "pair_lock",
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 10,
  "pairMaxTotalCent": 96
}
```

`edge_pairlock_v1` eklenirse:

```json
{
  "pairLockStrategy": "edge_pairlock_v1",
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge"
}
```

Bu üç alan eksikse edge pairlock beklenmemelidir.

Adaptive strategy eklenirse strategy/PTB uyumu ayrıca kontrol edilmelidir:

```json
{
  "pairLockStrategy": "adaptive_max_price_v1",
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge"
}
```

```json
{
  "pairLockStrategy": "manual_adaptive_risk_v1",
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "manual"
}
```

`biased_hedge_v1` eşit pair yerine primary bias + sınırlı hedge akışı kurar; detaylı karar ağacı için [12-adaptive-pair-lock-stratejileri.md](../senaryolar/12-adaptive-pair-lock-stratejileri.md) okunmalıdır.

## Sık Config Çakışmaları

| Çakışma | Sonuç |
|---|---|
| Pair lock + `sizePct` | Validation hatası veya unsupported davranış |
| DCA live + `side="sell"` | Validation hatası |
| DCA live + standard upstream trigger | `bindingMode="dca_live_only"` beklenir |
| `adaptive_max_price_v1` + PTB guard kapalı | Validation/runtime block |
| `manual_adaptive_risk_v1` + `priceToBeatMode!="manual"` | Strategy uyumsuz |
| Bump açık + PTB guard kapalı | Bump anlamlı değildir |
| Relax config + PTB guard kapalı | Relax çalışmaz |
| `reentrySkipCurrentWindow=true` + re-entry kapalı | Validation hatası |
| Counter re-entry | Pair lock'ta desteklenmez |
| `ptbStopLossTimeDecayMode` + PTB SL kapalı | Geçersiz kombinasyon |
| Notify flag kapalı | Event olabilir ama Telegram gelmez |

## Debug İçin Minimum Payload

Action davranışı incelenirken şu alanlar birlikte alınmalıdır:

- `node_key`
- `market_slug`
- `token_id`
- `side`
- `execution_mode`
- `kind`
- `size_usdc` veya `target_qty`
- guard decision payload
- `builder_order_id`
- `source_trade_id`
- retry/notification flags
