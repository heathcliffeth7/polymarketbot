# Config Reçeteleri

Güncelleme tarihi: 2026-05-01

Bu dosya yaygın trade flow kurulumları için kopyalanabilir JSON parçaları içerir. Alanları kendi flow builder şemanıza göre ilgili node config'ine yerleştirin.

## 1. Auto-Scope Trigger + Entry Timing

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "outcomeLabel": "Up",
    "triggerCondition": "cross_above",
    "triggerPrice": 0.54,
    "repeatMode": "once",
    "priceToBeatTriggerEnabled": true,
    "priceToBeatMode": "manual",
    "entryTimingProfiles": [
      {
        "startRemainingSec": 240,
        "endRemainingSec": 120,
        "maxPriceCent": 58,
        "priceToBeatTriggerMinGap": 10,
        "sizeUsdc": 5
      },
      {
        "startRemainingSec": 120,
        "endRemainingSec": 30,
        "maxPriceCent": 64,
        "priceToBeatTriggerMinGap": 22,
        "priceToBeatTriggerMaxGap": 80,
        "sizeUsdc": 8
      }
    ]
  }
}
```

Kullanım:

- Erken daha ucuz, geç daha seçici giriş.
- Action tarafında `sizeUsdc` boş bırakılırsa profil sizing fallback olur.

## 2. `iv_mismatch_edge` Guard'lı Buy

```json
{
  "side": "buy",
  "executionMode": "market",
  "kind": "immediate",
  "sizeUsdc": 10,
  "maxPrice": 0.66,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvDepthGuardEnabled": true,
  "priceToBeatIvDepthMaxSlippage": 0.03,
  "priceToBeatIvRequireBinanceFreshUnderSec": 60,
  "priceToBeatIvBinanceMaxStaleMs": 2000,
  "priceToBeatIvRequireBinanceSameDirection": true,
  "priceToBeatIvTimeRules": [
    {
      "startRemainingSec": 180,
      "endRemainingSec": 90,
      "maxPriceCent": 62,
      "minEdge": 0.08,
      "minGapStrength": 0.85,
      "minExpectedMoveUsd": 10
    },
    {
      "startRemainingSec": 90,
      "endRemainingSec": 30,
      "maxPriceCent": 68,
      "minEdge": 0.12,
      "minGapStrength": 1.1,
      "minExpectedMoveUsd": 18
    }
  ],
  "retryOnPriceToBeatGuardBlock": true,
  "notifyOnPriceToBeatGapBlocked": true,
  "notifyOnOrderSubmitted": true
}
```

## 3. TP/SL + Re-Entry

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 15,
  "tpRules": [
    {"priceCent": 68, "sizePct": 50},
    {"priceCent": 86, "sizePct": 50}
  ],
  "slRules": [
    {"priceCent": 45, "sizePct": 50},
    {"priceCent": 35, "sizePct": 50}
  ],
  "slTriggerPriceMode": "composite_safe",
  "slSiblingPolicy": "resize_remaining",
  "reenterOnSlHit": true,
  "reentryMaxAttempts": 2,
  "reentryCooldownSec": 10,
  "reentrySkipCurrentWindow": true,
  "notifyOnTpHit": true,
  "notifyOnSlHit": true
}
```

## 4. PTB Stop-Loss Bump

```json
{
  "priceToBeatGuardEnabled": true,
  "priceToBeatStopLossBumpEnabled": true,
  "priceToBeatStopLossBumpMode": "loss_table",
  "priceToBeatStopLossBumpUnit": "cent",
  "priceToBeatStopLossBumpMaxValue": 40,
  "priceToBeatStopLossBumpScope": "per_scope",
  "priceToBeatStopLossBumpDecayWindows": 2,
  "priceToBeatStopLossBumpLossRules": [
    {"lossUsd": 1, "bumpValue": 10},
    {"lossUsd": 2, "bumpValue": 20},
    {"lossUsd": 5, "bumpValue": 40}
  ]
}
```

## 5. Max Price Relax

```json
{
  "priceToBeatGuardEnabled": true,
  "priceToBeatMaxPriceRelaxEnabled": true,
  "priceToBeatMaxPriceRelaxMissCount": 3,
  "priceToBeatMaxPriceRelaxHistoryCount": 8,
  "priceToBeatMaxPriceRelaxMinValue": 55,
  "priceToBeatMaxPriceRelaxMinUnit": "cent",
  "priceToBeatMaxPriceRelaxMinDepthUsd": 20,
  "priceToBeatMaxPriceRelaxStepMode": "percent",
  "priceToBeatMaxPriceRelaxStepValue": 5
}
```

Not:

- Global `strategy.max_price_relax_enabled` kapalıysa node config tek başına yetmez.
- `/api/relax` ile global durum kontrol edilebilir.

## 6. Pair Lock Binding + `edge_pairlock_v1`

Trigger:

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "eth_5m_updown",
    "bindingMode": "pair_lock_only",
    "repeatMode": "once"
  }
}
```

Action:

```json
{
  "mode": "pair_lock",
  "pairLockStrategy": "edge_pairlock_v1",
  "side": "buy",
  "executionMode": "market",
  "kind": "immediate",
  "sizeUsdc": 10,
  "pairMaxTotalCent": 96,
  "pairLockDecisionQty": 5,
  "pairLockSingleEdgeThreshold": 0.10,
  "pairLockCostBuffer": 0.005,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "pairProtectiveUnwindEnabled": true,
  "notifyOnPairLocked": true,
  "notifyOnPairUnwind": true
}
```

## 7. No-Order Teşhisi İçin Bildirimler

```json
{
  "notifyOnOrderSubmitted": true,
  "notifyOnOrderPlaced": true,
  "notifyOnOrderNotFilled": true,
  "notifyOnTriggerPriceBlocked": true,
  "notifyOnExecutionFloorBlocked": true,
  "notifyOnPriceToBeatGapBlocked": true,
  "notifyOnMaxPriceBlocked": true,
  "retryOnTriggerPriceGuardBlock": true,
  "retryOnExecutionFloorGuardBlock": true,
  "retryOnPriceToBeatGuardBlock": true,
  "retryOnMaxPriceBlock": true
}
```

Kullanım:

- Yeni stratejiyi canlı izlerken kısa süreli aç.
- Gürültü fazla olursa sadece ilgili block tipini açık bırak.

## 8. Conservative Entry Profili

Amaç: Az ama daha kaliteli trade almak.

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 5,
  "maxPrice": 0.58,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvRequireBinanceSameDirection": true,
  "priceToBeatIvDepthGuardEnabled": true,
  "priceToBeatIvDepthMaxSlippage": 0.02,
  "reenterOnSlHit": false,
  "notifyOnOrderSubmitted": true,
  "notifyOnPriceToBeatGapBlocked": true
}
```

Ne zaman kullanılır:

- Yeni strateji denenirken.
- SL serisi varsa.
- Düşük depth döneminde.

Beklenen yan etki:

- Trade sayısı azalır.
- No-order sayısı artabilir.

## 9. Aggressive Momentum Profili

Amaç: Geç window'da güçlü hareketi yakalamak.

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 8,
  "maxPrice": 0.68,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvRequireBinanceSameDirection": true,
  "priceToBeatIvMomentumProtectionEnabled": true,
  "priceToBeatIvDepthGuardEnabled": true,
  "tpRules": [
    {"priceCent": 78, "sizePct": 50},
    {"priceCent": 92, "sizePct": 50}
  ],
  "slRules": [
    {"priceCent": 50, "sizePct": 100}
  ],
  "timeExitRules": [
    {"elapsedMinutes": 1.5, "remainingPct": 50}
  ]
}
```

Dikkat:

- Daha pahalı giriş kabul ettiği için IV edge ve depth şartları açık olmalıdır.
- Time exit yoksa geç giriş resolution riskini artırır.

## 10. SL Serisi Sonrası Fren Profili

```json
{
  "priceToBeatGuardEnabled": true,
  "priceToBeatStopLossBumpEnabled": true,
  "priceToBeatStopLossBumpMode": "fixed",
  "priceToBeatStopLossBumpAmount": 10,
  "priceToBeatStopLossBumpMaxValue": 40,
  "priceToBeatStopLossBumpUnit": "cent",
  "priceToBeatStopLossBumpScope": "per_scope",
  "priceToBeatStopLossBumpDecayWindows": 3,
  "reentrySkipCurrentWindow": true,
  "reentryMaxPriceTightenBps": 500
}
```

Ne yapar:

- Aynı scope'ta SL sonrası girişleri sıkılaştırır.
- Yeni window'lar geçtikçe etki azalır.
- Re-entry aynı window'da daha dikkatli hale gelir.

## 11. Relax Debug Profili

Amaç: Bot gerçekten fırsat mı kaçırıyor, yoksa depth mi yok anlamak.

```json
{
  "priceToBeatGuardEnabled": true,
  "priceToBeatMaxPriceRelaxEnabled": true,
  "priceToBeatMaxPriceRelaxMissCount": 3,
  "priceToBeatMaxPriceRelaxHistoryCount": 10,
  "priceToBeatMaxPriceRelaxMinValue": 55,
  "priceToBeatMaxPriceRelaxMinUnit": "cent",
  "priceToBeatMaxPriceRelaxMinDepthUsd": 20,
  "priceToBeatMaxPriceRelaxStepMode": "percent",
  "priceToBeatMaxPriceRelaxStepValue": 5,
  "notifyOnMaxPriceBlocked": true,
  "notifyOnPriceToBeatGapBlocked": true
}
```

Bakılacak analytics:

- `relax_miss_reason`
- `tradable_seconds_count`
- `price_ok_depth_fail_count`
- `max_fillability_score`

## 12. Pair Lock Güvenli Başlangıç Profili

```json
{
  "mode": "pair_lock",
  "pairLockStrategy": "edge_pairlock_v1",
  "side": "buy",
  "executionMode": "market",
  "kind": "immediate",
  "sizeUsdc": 5,
  "pairMaxTotalCent": 95,
  "pairLockDecisionQty": 5,
  "pairLockSingleEdgeThreshold": 0.12,
  "pairLockCostBuffer": 0.0075,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "pairProtectiveUnwindEnabled": true,
  "pairIgnoreStopLossAfterLocked": true,
  "notifyOnPairLocked": true,
  "notifyOnPairUnwind": true
}
```

Ne zaman kullanılır:

- Önce küçük notional ile pair lock davranışı doğrulanırken.
- Orphan riskini azaltmak istenirken.
- Locked pair sonrası normal SL'nin pair yapısını bozması istenmiyorsa.

## 13. DCA Live Trigger Binding

Trigger:

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "bindingMode": "dca_live_only",
    "repeatMode": "once",
    "onceScope": "market",
    "cycleWindowMode": "custom_range",
    "cycleWindowStartSec": 15,
    "cycleWindowEndSec": 240
  }
}
```

Action:

```json
{
  "nodeType": "action.place_order",
  "config": {
    "mode": "dca_live_v1",
    "side": "buy",
    "executionMode": "limit",
    "marketSelectionMode": "auto_scope",
    "sideMode": "one_sided",
    "selectedOutcomes": [{ "outcomeLabel": "Up" }],
    "initialOrderShares": 5,
    "dcaEntryMinPriceCent": 35,
    "dcaEntryMaxPriceCent": 62,
    "dcaLevels": 3,
    "dcaLevelSpacingCent": 2,
    "dcaOrderSizeMultiplier": 1,
    "maxTotalCostPerSlugUsdc": 25,
    "maxTotalCostAllSlugsUsdc": 50,
    "noNewOrdersBeforeEndSec": 30,
    "cancelOpenOrdersBeforeEndSec": 10
  }
}
```

## 14. Adaptive Max Price Pair Lock

```json
{
  "mode": "pair_lock",
  "pairLockStrategy": "adaptive_max_price_v1",
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 5,
  "pairMaxTotalCent": 96,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "adaptiveMaxPriceMissCount": 3,
  "adaptiveMaxPriceRequiredGoodMissCount": 2,
  "adaptiveMaxPriceRelaxCreditCent": 2,
  "adaptiveMaxPriceMaxRelaxCreditCent": 5,
  "adaptiveMaxPriceHardCapCent": 76,
  "adaptiveMaxPriceSizeMultiplier": 0.5,
  "adaptiveMaxPriceWindowStartSec": 120,
  "adaptiveMaxPriceWindowEndSec": 290,
  "adaptiveMaxPriceLateRiskEnabled": true,
  "adaptiveMaxPriceLateRiskAfterSec": 210,
  "adaptiveMaxPriceSlCooldownMarkets": 3,
  "notifyOnAdaptiveMaxPriceRelax": true,
  "notifyOnAdaptiveMaxPriceSummary": true
}
```

## 15. Manual Adaptive Risk Pair Lock

```json
{
  "mode": "pair_lock",
  "pairLockStrategy": "manual_adaptive_risk_v1",
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 5,
  "pairMaxTotalCent": 96,
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "manual",
  "manualAdaptiveWindowStartSec": 90,
  "manualAdaptiveWindowEndSec": 270,
  "manualAdaptiveTrendDeltaUsd": 5,
  "manualAdaptiveHighMaxPriceCent": 58,
  "manualAdaptiveHighSizeMultiplier": 0.3,
  "manualAdaptiveHighPtbGapAddCent": 25,
  "manualAdaptiveSelfTuneEnabled": true,
  "manualAdaptiveMissRelaxEnabled": true,
  "manualAdaptiveMissRelaxAfterNoOrderMarkets": 3,
  "manualAdaptivePtbRelaxStepCent": 5,
  "manualAdaptiveMaxPriceRelaxHardCapCent": 90,
  "manualAdaptiveSlTightenEnabled": true,
  "manualAdaptivePtbSlBumpMaxCent": 45,
  "manualAdaptiveLockdownMaxMarkets": 5,
  "notifyOnManualAdaptiveRiskBlock": true,
  "notifyOnManualAdaptiveRiskSummary": true,
  "notifyOnManualAdaptiveCounterCap": true
}
```

## 16. Claim Sweep Funds Activation

`config/claim.toml` örneği:

```toml
enabled = true
execution_mode = "relayer_api_key"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
auto_activate_funds = true
activate_min_usdc = 0.01
usdce_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
pusd_token_address = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
collateral_onramp_address = "0x93070a847efEf7F70739046A929D47a521F5B8ee"
min_claim_usdc = 0.0
```

Gerekli env:

```text
CLAIM_RELAYER_ADAPTER_TOKEN=...
CLAIM_RELAYER_ADAPTER_URL=http://127.0.0.1:3000/api/internal/claim/redeem
CLAIM_FUNDS_ACTIVATION_ADAPTER_URL=http://127.0.0.1:3000/api/internal/claim/activate-funds
```
