# 11 - DCA Live ve Trigger Binding

Güncelleme tarihi: 2026-05-01

## Amaç

`action.place_order mode="dca_live_v1"` klasik tek crypto 5m akışından farklıdır. Bu mod generic Polymarket slug/outcome seçimiyle bir veya daha fazla outcome üzerinde DCA ladder üretir. Trigger fiyatı tek token seçmek için değil, DCA action'a market bağlamı taşımak için kullanılabilir.

## Ne Zaman Kullanılır?

- Aynı stratejiyi tek slug, slug listesi veya market grubu üzerinde çalıştırmak istiyorsan.
- Binary markette iki taraflı pair DCA yapmak istiyorsan.
- Multi-outcome markette basket DCA kurmak istiyorsan.
- Crypto auto-scope window bilgisini DCA emir zamanı olarak kullanmak istiyorsan.

Ne zaman kullanılmaz:

- Sadece tek markette tek buy istiyorsan `mode="single"` daha basittir.
- Pair lock cost/edge kararı gerekiyorsa `mode="pair_lock"` kullan.
- Sell, TP/SL child veya re-entry lifecycle'ını DCA action'ın kendisinden bekleme.

## Trigger Binding Modeli

`trigger.market_price` tarafında DCA için özel binding gerekir:

```json
{
  "marketMode": "auto_scope",
  "marketScope": "btc_5m_updown",
  "bindingMode": "dca_live_only",
  "repeatMode": "once",
  "onceScope": "market"
}
```

Bu modda trigger:

- Outcome condition seçmez.
- `priceToBeatTrigger*` gate kullanmaz.
- Tek reachable downstream `action.place_order mode="dca_live_v1"` bekler.
- Araya logic/guard ve notification node'ları girebilir.
- `cycleWindowMode="custom_range"` varsa DCA action sadece bu pencere içinde emir üretir.

## Action Minimum Yapı

```json
{
  "mode": "dca_live_v1",
  "side": "buy",
  "executionMode": "limit",
  "marketSelectionMode": "manual_slug",
  "manualSlug": "example-market-slug",
  "sideMode": "one_sided",
  "selectedOutcomes": [
    { "marketSlug": "example-market-slug", "outcomeLabel": "Yes" }
  ],
  "initialOrderShares": 5,
  "dcaLevels": 3,
  "dcaLevelSpacingCent": 2,
  "dcaOrderSizeMultiplier": 1,
  "maxTotalCostPerSlugUsdc": 25,
  "maxTotalCostAllSlugsUsdc": 50
}
```

Runtime beklentileri:

- `side` sadece `buy` olmalıdır.
- `executionMode` `limit` veya `market` olabilir.
- Başlangıç size için `initialOrderShares`, `firstDcaShares` veya `targetQty` gerekir.
- `dcaLevels` 1-20 aralığındadır.
- Budget guard yoksa DCA ladder yanlışlıkla fazla notional açabilir.

## Market Seçimi

| Mode | Anlamı |
|---|---|
| `manual_slug` | Tek slug veya URL'den market seçer |
| `manual_slug_list` | Birden çok slug üzerinde aynı DCA mantığını dener |
| `auto_group_top_n` | Market grubu içinden adayları seçer |
| `auto_scope` | Crypto auto-scope bağlamından slug alır |

Manual modlarda `selectedOutcomes` açık verilirse runtime hangi outcome'a order açacağını doğrudan bilir. Auto-scope modunda trigger context marketi taşır.

## Side Mode

| Mode | Davranış |
|---|---|
| `one_sided` | Tam olarak bir outcome seçilir |
| `two_sided_pair` | Binary pair-eligible markette iki bacak birlikte planlanır |
| `multi_outcome_basket` | En az iki selected outcome basket olarak kullanılır |

`two_sided_pair` için pair cost kontrolleri önemlidir:

- `targetPairCostCent`
- `pairBufferCent`
- `counterOnlyIfPairCostOk`
- `maxUnmatchedCostUsdc`

Bu alanlar yoksa tek bacak dolup counter tarafın alınamaması daha olasıdır.

## Ladder ve Bütçe

Temel DCA alanları:

- `dcaEntryMinPriceCent`
- `dcaEntryMaxPriceCent`
- `dcaLevels`
- `dcaLevelSpacingCent`
- `dcaLevelSpacingMultiplier`
- `dcaOrderSizeMultiplier`
- `maxOpenOrdersAllSlugs`
- `noNewOrdersBeforeEndSec`
- `cancelOpenOrdersBeforeEndSec`

Pratik yorum:

- `dcaLevelSpacingCent` küçükse emirler birbirine çok yakın olur.
- `dcaOrderSizeMultiplier > 1` seviyeler ilerledikçe riskin büyümesine neden olur.
- `noNewOrdersBeforeEndSec` market sonundaki düşük kaliteli girişleri azaltır.
- `cancelOpenOrdersBeforeEndSec` resolution'a yakın açık limit order bırakmamaya yarar.

## Runtime Akışı

```text
trigger.market_price bindingMode=dca_live_only
  -> market/window context üretir
  -> action.place_order dca_live_v1 çalışır
  -> selected outcomes çözülür
  -> ladder seviyeleri üretilir
  -> budget ve window guard uygulanır
  -> tek, pair veya basket buy order'ları açılır
```

Başarılı akışta payload içinde `mode="dca_live_v1"` ve seçilen outcome listesi aranmalıdır.

## Sık Block Durumları

| Belirti | Muhtemel sebep |
|---|---|
| Validation `dca_live_requires_dca_binding_mode` | Upstream trigger `bindingMode="dca_live_only"` değildir |
| Validation `dca_live_requires_single_market_price_binding` | Birden fazla upstream trigger var veya hiç yok |
| Outcome yok | `selectedOutcomes`, `marketSlug/tokenId/outcomeLabel` veya auto-scope context eksik |
| Pair mode çalışmıyor | Market binary/pair eligible değildir |
| Window içinde emir yok | `custom_range`, `noNewOrdersBeforeEndSec` veya budget guard bloklamıştır |

## Operatör Checklist

1. Trigger `bindingMode` gerçekten `dca_live_only` mı?
2. Downstream action `mode="dca_live_v1"` mı?
3. `sideMode` ile selected outcome sayısı uyumlu mu?
4. Budget guard değerleri küçük notional ile test edildi mi?
5. Custom range varsa market kalan süresi o aralıkta mı?
6. Submit yoksa validation, action payload ve budget block birlikte okunmalı.

## Kaynak Notu

Kod referansları:

- `frontend/src/lib/queries/trade-flow/validation-action-place-order-dca.ts`
- `frontend/src/lib/queries/trade-flow/validation-trigger-market-price.ts`
- `frontend/src/lib/dca-bot/`
- `crates/bot-runner/src/trade_builder/dca_live.rs`

Polymarket yüzeyi: market discovery için Gamma/Data API, order execution için CLOB, gasless onchain işlem gerekiyorsa Builders Relayer ayrı değerlendirilir.
