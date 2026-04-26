# 05 - PTB Bump ve Max Price Relax

Bu dosya stop-loss sonrası daha seçici giriş için PTB bump mekanizmasını ve uzun süre trade kaçırınca kontrollü gevşeme için max price relax mekanizmasını açıklar.

## Amaç

Bot bazen çok kolay giriş alıp ardışık SL yer. Bazen de şartlar fazla sıkı olduğu için hiç trade alamaz. Bu iki durum için iki karşı ağırlık vardır:

- PTB stop-loss bump: zarar sonrası eşiği yükseltir.
- Max price relax: ardışık kaçan market sonrası eşiği kontrollü düşürür.

Bu iki özellik aynı anda kullanılabilir ama biri risk azaltır, diğeri fırsat kaçırmayı azaltır. Telemetry'de hangisinin baskın olduğunu okumak gerekir.

## PTB Stop-Loss Bump

`priceToBeatStopLossBumpEnabled=true`, PTB stop-loss ile zarar yazıldıktan sonra sonraki buy girişlerinde gereken PTB threshold'unu artırır.

Örnek:

- Base PTB threshold: 80 cent.
- Bump amount: 10 cent.
- Bir PTB SL sonrası effective threshold: 90 cent.
- Max value: 110 cent ise daha fazla bump tavanı aşamaz.

## Bump Config Alanları

| Alan | Anlam |
|---|---|
| `priceToBeatStopLossBumpEnabled` | Özelliği açar |
| `priceToBeatStopLossBumpMode` | `fixed` veya `loss_table` |
| `priceToBeatStopLossBumpAmount` | Fixed modda her SL sonrası ek miktar |
| `priceToBeatStopLossBumpLossRules` | Loss büyüklüğüne göre `{lossUsd, bumpValue}` kuralları |
| `priceToBeatStopLossBumpMaxValue` | Toplam bump tavanı |
| `priceToBeatStopLossBumpUnit` | `usd` veya `cent` |
| `priceToBeatStopLossBumpScope` | `global` veya `per_scope` |
| `priceToBeatStopLossBumpDecayWindows` | Yeni window'larda bump azaltma adımı |

Validation notları:

- Sadece `side="buy"` için geçerlidir.
- `priceToBeatGuardEnabled=true` gerektirir.
- `loss_table` modunda loss rules zorunludur ve `lossUsd` değerleri artan sırada olmalıdır.
- `fixed` modunda `priceToBeatStopLossBumpAmount` gerekir.

## Senaryo A: Fixed Bump

```json
{
  "priceToBeatGuardEnabled": true,
  "priceToBeatStopLossBumpEnabled": true,
  "priceToBeatStopLossBumpMode": "fixed",
  "priceToBeatStopLossBumpAmount": 10,
  "priceToBeatStopLossBumpMaxValue": 30,
  "priceToBeatStopLossBumpUnit": "cent",
  "priceToBeatStopLossBumpScope": "per_scope",
  "priceToBeatStopLossBumpDecayWindows": 2
}
```

Akış:

1. BTC 5m Up trade PTB SL ile zarar yazar.
2. Scope `btc:5m:up` için bump count artar.
3. Bir sonraki BTC 5m Up girişinde threshold 10 cent sıkılaşır.
4. Yeni window'lar geçtikçe decay bump etkisini azaltır.

## Senaryo B: Loss Table Bump

```json
{
  "priceToBeatStopLossBumpEnabled": true,
  "priceToBeatStopLossBumpMode": "loss_table",
  "priceToBeatStopLossBumpUnit": "cent",
  "priceToBeatStopLossBumpLossRules": [
    {"lossUsd": 1, "bumpValue": 10},
    {"lossUsd": 2, "bumpValue": 20},
    {"lossUsd": 5, "bumpValue": 40}
  ]
}
```

Yorum:

- Küçük zarar küçük sıkılaşma üretir.
- Büyük zarar daha büyük sıkılaşma üretir.
- Son eşleşen kural kullanılır.

## Max Price Relax

`priceToBeatMaxPriceRelaxEnabled=true`, art arda kaçan marketlerden sonra threshold'u kontrollü gevşetir. Amaç sürekli "fiyat biraz pahalı" diye hiç trade alamayan flow'ların gerçek fırsat kaçırıp kaçırmadığını ölçmektir.

Relax ancak şu tarz kanıtlarla anlamlıdır:

- Geçmiş completed marketlerde fiyat sonradan kabul edilebilir seviyeye gelmiş.
- Yeterli ask depth görülmüş.
- Kaçırma sayısı `priceToBeatMaxPriceRelaxMissCount` değerine ulaşmış.

Global anahtar:

- `strategy.max_price_relax_enabled`
- Frontend API: `/api/relax`

## Relax Config Alanları

| Alan | Anlam |
|---|---|
| `priceToBeatMaxPriceRelaxEnabled` | Node seviyesinde relax açar |
| `priceToBeatMaxPriceRelaxMissCount` | Relax başlamadan önce gereken ardışık miss |
| `priceToBeatMaxPriceRelaxHistoryCount` | Kaç geçmiş completed market incelenecek |
| `priceToBeatMaxPriceRelaxMinValue` | Threshold bu değerin altına gevşemez |
| `priceToBeatMaxPriceRelaxMinUnit` | `usd` veya `cent` |
| `priceToBeatMaxPriceRelaxMinDepthUsd` | Tradeable fırsat sayılması için depth tabanı |
| `priceToBeatMaxPriceRelaxStepMode` | `percent` veya `absolute` |
| `priceToBeatMaxPriceRelaxStepValue` | Her ekstra miss için gevşeme |
| `priceToBeatMaxPriceRelaxStepUnit` | Absolute modda `usd` veya `cent` |

## Senaryo C: Relax Devreye Girer

Durum:

- Base max price 60 cent.
- 5 market üst üste order yok.
- Kaçan marketlerde fiyat 61-63 cent arası tradeable olmuş.
- Relax miss count 3.
- Step 5 percent.

Beklenen:

1. İlk 3 miss sonrası relax izlemeye başlar.
2. Geçmişte gerçek tradeable fırsat varsa relax credit üretir.
3. Effective threshold biraz gevşer.
4. `max_price_relax_relax_credit_usd` ve miss reason analytics'e düşer.

## Senaryo D: Relax Çalışmaz

Durum:

- Çok miss var.
- Ama geçmiş marketlerde min depth yok.
- Ya da global `max_price_relax_enabled=false`.

Beklenen:

- Relax credit üretilmez.
- Miss reason `depth_fail`, `no_tradeable_second`, `disabled` gibi görünür.
- Node hâlâ base max price/PTB şartıyla çalışır.

## Telemetry

PTB guard payload'ında şu alanlar aranır:

- `stop_loss_bump_count`
- `stop_loss_bump_applied_count`
- `stop_loss_bump_amount`
- `stop_loss_bump_max_value`
- `stop_loss_bump_unit`
- `stop_loss_bump_usd`
- `stop_loss_bump_capped`
- `stop_loss_bump_max_reached`
- `stop_loss_bump_increment_usd`
- `max_price_relax_relax_credit_usd`
- `max_price_relax_miss_reason`
- `max_price_relax_first_tradable_second_ts`
- `max_price_relax_tradable_seconds_count`
- `max_price_relax_price_ok_depth_fail_count`
- `max_price_relax_max_fillability_score`

## Operatör Checklist

- Bump açıkken `side=buy` ve PTB guard açık mı?
- Bump scope global mı, per-scope mu isteniyor?
- Loss table sırası artan mı?
- Relax için global `/api/relax` açık mı?
- Relax node config'i PTB guard ile birlikte mi?
- Analytics'te miss reason fiyat mı, depth mi, disable mı?
- Bump ve relax aynı anda çalışıyorsa effective threshold'un son hali okunuyor mu?
