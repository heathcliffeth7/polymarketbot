# Config Reçeteleri

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
