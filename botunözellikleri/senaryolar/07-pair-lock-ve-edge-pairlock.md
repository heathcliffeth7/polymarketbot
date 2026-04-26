# 07 - Pair Lock ve Edge Pairlock

Bu dosya klasik pair lock davranışını ve `pairLockStrategy="edge_pairlock_v1"` karar modelini açıklar.

## Amaç

Pair lock, aynı binary markette YES ve NO bacaklarını maliyet tavanı altında birlikte yönetmeye çalışır. Hedef ya risk kilidi kurmak ya da iki tarafın toplam maliyeti düşükken yapısal edge yakalamaktır.

## Temel Kısıtlar

`action.place_order mode="pair_lock"` için beklenen yapı:

- Doğrudan tek upstream node `trigger.market_price`.
- Upstream trigger `bindingMode="pair_lock_only"`.
- `side="buy"`.
- `kind` boş veya `immediate`.
- `executionMode` `market` veya `limit`.
- USDC sizing kullanılmalı; `sizePct` desteklenmez.
- `maxTriggers` verilirse `1` olmalı.

## Upstream Binding Örneği

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "bindingMode": "pair_lock_only",
    "repeatMode": "once"
  }
}
```

Bu trigger fiyat koşulu yerine YES/NO tokenlarını çözer ve pair lock action'a aktarır.

## Klasik Pair Lock

Legacy davranışta bot iki bacak toplam maliyeti ve budget'a bakar.

Örnek alanlar:

- `pairMaxTotalCent`
- `pairTotalBudgetUsdc`
- `counterLegEnabled`
- `auto_remaining_budget`

Senaryo:

1. Up bacağı 47 cent.
2. Down bacağı 48 cent.
3. Toplam 95 cent.
4. `pairMaxTotalCent=96` ise pair açılabilir.

Risk:

- Tek bacak fill olup counter fill olmazsa orphan pozisyon oluşur.
- Counter market bitmişse lock kurulamaz.
- Budget yanlışsa iki bacak dengesiz kalır.

## `edge_pairlock_v1`

`edge_pairlock_v1`, sadece toplam maliyete değil, `iv_mismatch_edge` çıktısına da bakar.

Zorunlu alanlar:

- `pairLockStrategy="edge_pairlock_v1"`
- `priceToBeatGuardEnabled=true`
- `priceToBeatMode="iv_mismatch_edge"`

Opsiyonel karar alanları:

| Alan | Anlam |
|---|---|
| `pairLockDecisionQty` | Edge hesabında kullanılacak share miktarı |
| `pairLockSingleEdgeThreshold` | Pair açılamazsa tek taraf için min edge |
| `pairLockCostBuffer` | Ask + fee üstüne maliyet tamponu |
| `pairMaxTotalCent` | YES+NO toplam maliyet tavanı |

## Karar Sırası

```text
1. Açık tek taraflı pozisyon var mı?
   Evet -> counter leg ile toplam maliyet tavan altında mı?
      Evet -> position_counter_lock

2. Yeni YES ve NO birlikte alınabiliyor mu?
   Evet -> fresh_equal_pair

3. Pair yok ama seçili bacakta güçlü edge var mı?
   Evet -> single_edge

4. Hiçbiri yok
   -> pair_lock_edge_no_decision, retry/skip
```

## Senaryo A: Açık Pozisyona Counter Lock

Durum:

- Bot daha önce Up bacağı aldı.
- Down ask uygun hale geldi.
- Toplam maliyet `pairMaxTotalCent` altında.

Beklenen:

- Decision `position_counter_lock`.
- Counter leg order üretilir.
- Pair session locked duruma geçer.

## Senaryo B: Fresh Equal Pair

Durum:

- Açık pozisyon yok.
- Up ask 47 cent.
- Down ask 48 cent.
- Toplam 95 cent ve tavan 96.

Beklenen:

- Decision `fresh_equal_pair`.
- İki bacak budget'a göre açılır.
- `notifyOnPairLocked` açıksa Telegram mesajı gelir.

## Senaryo C: Single Edge

Durum:

- İki bacak toplam maliyeti tavan üstünde.
- Ama Up bacağında `iv_mismatch_edge` güçlü.
- Edge `pairLockSingleEdgeThreshold` üstünde.

Beklenen:

- Decision `single_edge`.
- Sadece güçlü bacak alınır.
- Orphan riski bilerek kabul edilir.

## Senaryo D: No Decision

Durum:

- Pair tavan üstünde.
- Tek taraf edge zayıf.
- Depth yetersiz.

Beklenen:

- Order üretilmez.
- Event/telemetry içinde `pair_lock_edge_no_decision` veya benzeri karar görünür.
- Retry ayarlarına göre tekrar denenebilir.

## Pair Lock Çıkışları

İlgili alanlar:

- `pairProtectiveUnwindEnabled`
- `pairIgnoreStopLossAfterLocked`
- `counterLegTpEnabled`
- `counterLegSlEnabled`
- `counterLegPtbStopLossEnabled`
- `counterLegPtbStopLossTimeDecayMode`
- `notifyOnPairLocked`
- `notifyOnPairUnwind`

Pratik yorum:

- Pair locked olduktan sonra normal SL kuralları bazen iki bacağı bozabilir.
- `pairIgnoreStopLossAfterLocked` lock sonrası SL davranışını sınırlamak için kullanılır.
- Protective unwind orphan veya bozulmuş lock riskinde devreye alınır.

## Telemetry

Pair lock analizinde şu alanlar aranır:

- `pair_lock_strategy`
- `pair_lock_edge_decision`
- `pair_lock_edge`
- `pair_session_id`
- `counter_builder_order_id`
- `pair_total`
- `target_qty`
- `counter_leg_token_id`
- `counter_leg_price`

## Operatör Checklist

- Upstream trigger doğrudan ve tek mi?
- Upstream `bindingMode="pair_lock_only"` mi?
- `side="buy"` ve USDC sizing kullanılıyor mu?
- `edge_pairlock_v1` için PTB guard ve `iv_mismatch_edge` açık mı?
- `pairMaxTotalCent` çok sıkı olduğu için hiç pair kurulmuyor olabilir mi?
- Tek bacak fill riskine karşı protective unwind açık mı?
- Locked session sonrası SL/TP kuralları iki bacağı bozuyor mu?
