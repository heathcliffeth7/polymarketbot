# Yeni Özellikler

Güncelleme tarihi: 2026-04-26

Bu dosya mevcut çalışma ağacındaki yeni trade flow, guard, pair lock ve analiz özelliklerini özetler. Temel node davranışı için ana referans: [node-ozellikleri.md](./node-ozellikleri.md).

## Hızlı Özet

- `trigger.market_price` artık auto-scope marketlerde kalan süreye göre `entryTimingProfiles` seçebilir.
- `action.place_order` PTB guard tarafında `iv_mismatch_edge` modu, zaman kuralları, Binance doğrulaması, book/depth koruması, participation credit ve adaptive volume rejimleri var.
- PTB stop-loss sonrası `priceToBeatStopLossBump*` alanları threshold'u kademeli artırabilir; `priceToBeatMaxPriceRelax*` alanları art arda kaçan marketlerden sonra threshold'u kontrollü gevşetebilir.
- `mode=pair_lock` tarafında `edge_pairlock_v1` stratejisi, açık pozisyonu counter ile kilitleme, yeni çift açma veya tek taraflı edge alma kararını otomatik verebilir.
- `buyFillLockEnabled` aynı market cycle içinde aynı gruptan ikinci buy girişini engeller.
- `notifyOnOrderSubmitted`, no-order tanısı, auto-scope analiz zaman aralıkları ve `/api/relax` kontrolü eklendi.

---

## 1. Entry Timing Profiles

`trigger.market_price` için `entryTimingProfiles`, auto-scope marketin bitişine kalan süreye göre tetik parametrelerini seçer. Sadece şu kombinasyonda geçerlidir:

- `marketMode = "auto_scope"`
- `repeatMode = "once"`
- `cycleWindowMode` boş veya `"off"`
- Profil aralıkları çakışmaz; her profil için `startRemainingSec > endRemainingSec`

### Profil Alanları

| Alan | Tip | Açıklama |
|---|---|---|
| `startRemainingSec` | int | Profilin aktif olmaya başladığı kalan saniye. `> 0` olmalı |
| `endRemainingSec` | int | Profilin bittiği kalan saniye. `>= 0` olmalı |
| `maxPriceCent` | number | Seçili zaman aralığında kullanılacak tavan fiyat |
| `priceToBeatTriggerMinGap` | number | Manual PTB trigger gate için min gap override |
| `priceToBeatTriggerMaxGap` | number | Manual PTB trigger gate için max gap override |
| `sizeUsdc` | number | Downstream `action.place_order` için fallback buy büyüklüğü |

PTB gap override alanları yalnızca `priceToBeatTriggerEnabled=true` ve `priceToBeatMode="manual"` iken kullanılabilir.

### Runtime Çıktıları

Seçilen profil flow context'e ve trigger çıktısına şu alanlarla yazılır:

- `selectedEntryTimingProfile`
- `selectedEntryTimingProfileIndex`
- `selectedEntryRemainingSec`
- `selectedEntryMaxPrice`
- `selectedEntrySizeUsdc`

`action.place_order`, kendi `sizeUsdc` veya `targetNotionalUsdc` alanı yoksa `selectedEntrySizeUsdc` değerini fallback olarak kullanır.

### Örnek

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "repeatMode": "once",
    "priceToBeatTriggerEnabled": true,
    "priceToBeatMode": "manual",
    "entryTimingProfiles": [
      {
        "startRemainingSec": 180,
        "endRemainingSec": 90,
        "maxPriceCent": 58,
        "priceToBeatTriggerMinGap": 10,
        "sizeUsdc": 5
      },
      {
        "startRemainingSec": 90,
        "endRemainingSec": 30,
        "maxPriceCent": 63,
        "priceToBeatTriggerMinGap": 20,
        "priceToBeatTriggerMaxGap": 80,
        "sizeUsdc": 8
      }
    ]
  }
}
```

---

## 2. PTB Guard: `iv_mismatch_edge`

`priceToBeatMode="iv_mismatch_edge"` fiyatı yalnızca PTB gap ile değil, implied probability, Chainlink hareketi, Binance teyidi, orderbook, depth ve kalan süreye göre değerlendirir. Çıktı `price_to_beat_guard.iv_mismatch_edge` altında detaylı telemetry üretir.

### Temel Kullanım

```json
{
  "side": "buy",
  "executionMode": "market",
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvDepthGuardEnabled": true,
  "priceToBeatIvRequireBinanceFreshUnderSec": 60,
  "priceToBeatIvBinanceMaxStaleMs": 2000,
  "priceToBeatIvTimeRules": [
    {
      "startRemainingSec": 120,
      "endRemainingSec": 60,
      "maxPriceCent": 65,
      "minEdge": 0.08,
      "minGapStrength": 0.85,
      "minExpectedMoveUsd": 12
    }
  ]
}
```

### Önemli Config Grupları

| Grup | Alanlar | Ne işe yarar |
|---|---|---|
| Zaman kuralları | `priceToBeatIvTimeRules[]` | Kalan süreye göre `maxPriceCent`, `minEdge`, `minGapStrength`, `minExpectedMoveUsd`, margin kuralları |
| Stale/velocity cezası | `priceToBeatIvStalePenaltyMs`, `priceToBeatIvStaleGapStrengthPenalty`, `priceToBeatIvNegativeVelocityGapStrengthPenalty` | Eski Chainlink verisi veya ters velocity durumunda edge/gap şartını zorlaştırır |
| Binance teyidi | `priceToBeatIvRequireBinanceFreshUnderSec`, `priceToBeatIvBinanceMaxStaleMs`, `priceToBeatIvRequireBinanceSameDirection`, `priceToBeatIvBinanceDisagreement*` | Binance fiyatı taze ve aynı yönde değilse block veya penalty uygular |
| Book koruması | `priceToBeatIvProtectionMode`, `priceToBeatIvBookLeadGuardEnabled`, `priceToBeatIvBookLeadUnderSec`, `priceToBeatIvOppositeMidBlockCent`, `priceToBeatIvBlockOnOppositeBookLead` | YES/NO book tarafı seçili yönü desteklemiyorsa engeller veya threshold artırır |
| Model-book uyumu | `priceToBeatIvModelBookGapWarn`, `priceToBeatIvModelBookGapHard`, `priceToBeatIvModelBookWarnThresholdPenalty`, `priceToBeatIvModelBookWarnGapPenalty` | Model olasılığı book fiyatından aşırı ayrışıyorsa hard block veya soft penalty |
| Depth guard | `priceToBeatIvDepthGuardEnabled`, `priceToBeatIvDepthMaxSlippage` | Hedef miktar için orderbook VWAP slippage kabul edilemezse block eder |
| Geç ve pahalı giriş | `priceToBeatIvLateHighPrice*` | Market sonuna yakın yüksek ask veya zayıf mid teyidinde threshold'u sıkılaştırır |
| Participation credit | `priceToBeatIvParticipationCreditEnabled`, `priceToBeatIvParticipationAfterMinutes`, `priceToBeatIvParticipationLongAfterMinutes`, `priceToBeatIvParticipationCredit`, `priceToBeatIvParticipationLongCredit` | Uzun süre fill yoksa threshold'a sınırlı kredi uygular |
| Adaptive volume | `priceToBeatIvVolumeBaselineMode`, `priceToBeatIvVolumeBaselineLookbackDays`, `priceToBeatIvVolumeWindowSec`, `priceToBeatIvLowHourlyVolumeRatio`, `priceToBeatIvHighHourlyVolumeRatio`, `priceToBeatIvExtremeHourlyVolumeRatio` | Son hacmi saatlik geçmiş baseline ile kıyaslayıp green/orange/red rejim üretir |

### Adaptive Rejimleri

| Rejim | Tipik sebep | Davranış |
|---|---|---|
| `green` | Book seçili yönü destekliyor, Binance aynı yönde, hacim normal/yüksek | `minEdge` ve `minGapStrength` gevşeyebilir |
| `orange` | Book karşı yönde veya extreme volume/chop yumuşak risk üretiyor | Edge ve gap şartları sıkılaşır |
| `red` | Yüksek hacim + güvenilir karşı book ya da extreme chop | `price_to_beat_guard` block eder |

### Telemetry Alanları

Öne çıkan alanlar:

- `q`, `q_final`, `edge`, `cost`, `threshold`, `dynamic_threshold`
- `gap_strength`, `required_gap_strength`, `gap_usd_margin`
- `binance_price`, `binance_staleness_ms`, `binance_same_direction`
- `depth_guard_result`, `estimated_avg_fill`, `vwap_slippage`
- `adaptive_regime`, `hourly_volume_ratio`, `book_reliability`

---

## 3. PTB Stop-Loss Bump ve Max Price Relax

### Stop-Loss Bump

`priceToBeatStopLossBumpEnabled=true`, PTB stop-loss ile zarar yazan marketlerden sonra sonraki girişlerde gereken PTB threshold'u artırır. Örneğin manual PTB threshold `80 cent`, bump `10 cent` ve 1 SL sonrası efektif threshold `90 cent` olur.

| Alan | Açıklama |
|---|---|
| `priceToBeatStopLossBumpEnabled` | Özelliği açar. Sadece `side=buy` ve `priceToBeatGuardEnabled=true` ile geçerli |
| `priceToBeatStopLossBumpMode` | `"fixed"` veya `"loss_table"` |
| `priceToBeatStopLossBumpAmount` | Fixed modda her SL sonrası eklenecek miktar |
| `priceToBeatStopLossBumpLossRules` | Loss table modda `{lossUsd, bumpValue}` kuralları; `lossUsd` strict artmalı |
| `priceToBeatStopLossBumpMaxValue` | Toplam bump tavanı |
| `priceToBeatStopLossBumpUnit` | `"usd"` veya `"cent"` |
| `priceToBeatStopLossBumpScope` | `"global"` veya `"per_scope"`; per scope anahtarı `asset:timeframe:direction` |
| `priceToBeatStopLossBumpDecayWindows` | Her yeni window'da bump etkisini azaltma adımı |

### Max Price Relax

`priceToBeatMaxPriceRelaxEnabled=true`, art arda trade kaçırılan marketlerden sonra threshold'u kontrollü gevşetir. Gevşeme yalnızca geçmiş completed marketlerde gerçek tradeable fırsat görülmüşse ve min depth şartı sağlanmışsa anlamlı hale gelir.

| Alan | Açıklama |
|---|---|
| `priceToBeatMaxPriceRelaxMissCount` | Relax başlamadan önce gereken ardışık kaçırma sayısı |
| `priceToBeatMaxPriceRelaxHistoryCount` | Geçmişte kaç completed market incelenecek |
| `priceToBeatMaxPriceRelaxMinValue` / `MinUnit` | Relax sonrası threshold bunun altına inmez |
| `priceToBeatMaxPriceRelaxMinDepthUsd` | Geçmiş fırsatın tradeable sayılması için ask depth tabanı |
| `priceToBeatMaxPriceRelaxStepMode` | `"percent"` veya `"absolute"` |
| `priceToBeatMaxPriceRelaxStepValue` / `StepUnit` | Her ekstra miss için gevşeme adımı |

Global strateji anahtarı `strategy.max_price_relax_enabled` false yapılırsa relax devre dışı kalır. Frontend'de `/api/relax` bu anahtarı okur/yazar ve servis kontrolü uygunsa restart dener.

---

## 4. Pair Lock Geliştirmeleri

`action.place_order` için `mode="pair_lock"` hâlâ şu temel kısıtlarla çalışır:

- Doğrudan tek upstream node `trigger.market_price` olmalı.
- Upstream trigger `bindingMode="pair_lock_only"` olmalı.
- `side="buy"`, `kind="immediate"`, `maxTriggers=1` ve USDC sizing kullanılmalı.
- Binary up/down market veya upstream auto-scope trigger gerekli.

### `edge_pairlock_v1`

`pairLockStrategy="edge_pairlock_v1"` otomatik edge değerlendirmesi yapar. Zorunlu alanlar:

- `priceToBeatGuardEnabled=true`
- `priceToBeatMode="iv_mismatch_edge"`

Opsiyonel karar alanları:

| Alan | Varsayılan | Açıklama |
|---|---:|---|
| `pairLockDecisionQty` | `5` | Edge kararı için share adedi |
| `pairLockSingleEdgeThreshold` | `0.10` | Pair açılamazsa tek taraflı giriş için minimum edge |
| `pairLockCostBuffer` | `0.005` | Ask + taker fee üstüne eklenen maliyet tamponu |
| `pairMaxTotalCent` | `95` UI default | İki bacağın toplam cost tavanı |

Karar sırası:

1. Açık tek taraflı pozisyon varsa ve counter leg ile toplam cost `pairMaxTotalCent` altında kalıyorsa `position_counter_lock`.
2. YES ve NO birlikte alınabiliyor ve toplam cost tavan altındaysa `fresh_equal_pair`.
3. Pair yok ama tek taraf edge `pairLockSingleEdgeThreshold` üstündeyse `single_edge`.
4. Hiçbiri olmazsa `pair_lock_edge_no_decision` ile retry planlanır.

Output ve event alanlarında `pair_lock_strategy`, `pair_lock_edge_decision`, `pair_lock_edge`, `pair_session_id`, `counter_builder_order_id`, `pair_total`, `target_qty` gibi telemetry görülür.

### Pair Lock Çıkışları

Yeni/önemli alanlar:

- `pairProtectiveUnwindEnabled`: Lead fill sonrası counter terminal olursa koruyucu unwind planlansın mı. Varsayılan UI değeri `true`.
- `pairIgnoreStopLossAfterLocked`: Pair locked olduktan sonra stop-loss yüzeyini kapatma davranışı.
- `notifyOnPairLocked`, `notifyOnPairUnwind`: Pair state bildirimleri.
- Counter leg TP: `counterLegTpEnabled`, `counterLegTpPriceCent`, `counterLegTpRules`, `counterLegNotifyOnTpHit`.
- Counter leg hard/PTB SL: `counterLegSlEnabled`, `counterLegSlPriceCent`, `counterLegSlTriggerPriceMode`, `counterLegPtbStopLossEnabled`, `counterLegPtbStopLossGapUsd`, `counterLegPtbStopLossGapUnit`, `counterLegPtbStopLossTimeDecayMode`, `counterLegNotifyOnSlHit`.

Pair lock, primary staged SL ve primary/counter TP ile temel hard/PTB SL alanlarını destekler; counter staged exits, time exits ve advanced re-entry alanları desteklenmez.

---

## 5. Buy Fill Lock

`buyFillLockEnabled=true`, aynı `buyFillLockGroup` içindeki başka bir buy node aynı market cycle içinde fill aldıysa yeni buy açmayı engeller.

| Alan | Açıklama |
|---|---|
| `buyFillLockEnabled` | Sadece `side=buy` için geçerli |
| `buyFillLockGroup` | Lock grubunun adı. Zorunlu |
| `releaseBuyFillLockOnStopLoss` | Parent pozisyon stop-loss sonrası tamamen kapanırsa lock temizlenebilir |

Davranış:

1. Buy order fill olunca flow context altında `__buyFillLocks[group]` kaydedilir.
2. Aynı group + aynı market için sonraki buy node `buy_fill_lock_blocked` sebebiyle atlanır.
3. `releaseBuyFillLockOnStopLoss=true` ise stop-loss parent pozisyonu sıfırlayınca `buy_fill_lock_released_on_stop_loss` event'i ile lock kalkar.

Output alanları:

- `buy_fill_lock`
- `buy_fill_lock_group`
- `reason = "buy_fill_lock_blocked"` block durumunda

---

## 6. Bildirim ve Analiz Ekleri

### Submit Bildirimi

`notifyOnOrderSubmitted=true`, emir CLOB'a başarıyla gönderildiğinde Telegram bildirimi üretir. `order_submitted` idempotency ile aynı submit payload'ı tekrar bildirilmez.

Bildirim payload'ında seçili fiyat, dynamic threshold, participation credit, adjusted margin, EV tahminleri, depth guard sonucu, spread ve stale bilgisi gibi alanlar yer alır.

### No-Order Tanısı

Market window bittiği halde order yoksa veya order fill'e dönmediyse no-order tanısı üretilebilir:

- `why_no_order_summary`
- `order_created`, `order_submitted`, `order_filled`
- Son guard adı/kodu/state
- `execution_floor`, `best_ask_at_window_end`, `floor_distance`, `floor_wait_ms`
- Liquidity rejimi, `hourly_volume_ratio`, 30s volume, 60s trade count
- Book quote durumu: selected/up/down bid/ask/mid, `book_side`, `book_mid_diff`

Auto-scope analiz ekranı ve export artık bu no-order telemetry alanlarını taşır.

### Analiz Zaman Aralığı

Auto-scope analiz endpointleri şu zaman aralıklarını destekler:

- `all`
- `custom` (`from`, `to`)
- `3h`, `6h`, `12h`, `24h`

---

## Kaynak Dosyalar

| Özellik | Dosyalar |
|---|---|
| Entry timing profiles | `crates/bot-runner/src/trade_flow/triggers/market_price_entry_timing.rs`, `frontend/src/lib/trade-flow-config-mappers/entry-timing-profiles.ts` |
| IV mismatch edge | `crates/bot-runner/src/trade_flow/guards/price_to_beat/iv_mismatch_edge.rs`, `iv_mismatch_runtime_config.rs`, `iv_mismatch_adaptive.rs`, `iv_mismatch_depth.rs`, `iv_mismatch_protection.rs` |
| PTB bump/relax | `crates/bot-runner/src/trade_builder/ptb_stop_loss_bump.rs`, `crates/bot-runner/src/trade_flow/guards/price_to_beat/max_price_relax.rs` |
| Pair lock edge | `crates/bot-runner/src/trade_builder/pair_lock_edge_strategy.rs`, `frontend/src/lib/trade-flow-config-mappers/pair-lock.ts` |
| Buy fill lock | `crates/bot-runner/src/trade_builder/buy_fill_lock.rs`, `frontend/src/lib/queries/trade-flow/validation-action-place-order-buy-fill-lock.ts` |
| Submit notification | `crates/bot-runner/src/trade_builder/submitted_notification.rs` |
| No-order analytics | `crates/bot-runner/src/trade_flow/missed_market_no_order_diagnosis.rs`, `missed_market_notifications.rs`, `frontend/src/lib/queries/trade-flow/auto-scope-analysis-extras.ts` |
| Relax toggle | `frontend/src/app/api/relax/route.ts`, `frontend/src/components/control/relax-toggle.tsx`, `config/strategy.toml` |
