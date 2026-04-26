# Referans - `trigger.market_price`

Bu dosya `trigger.market_price` node'unun config, output ve operasyon davranışını özetler.

## Görev

`trigger.market_price`, market fiyatını veya market binding bilgisini izler ve uygun durumda downstream node'a route verir.

Kullanım biçimleri:

- Sabit market slug ve token ile fiyat tetikleme.
- Auto-scope ile aktif 5m marketi seçme.
- Multi-outcome koşul izleme.
- Pair lock için YES/NO token binding.
- Entry timing profile seçme.
- PTB trigger gate ile action'a geçmeden giriş kalitesi kontrolü.

## Ana Config Grupları

| Grup | Alanlar |
|---|---|
| Market seçimi | `marketMode`, `marketScope`, `marketSlug`, `tokenId`, `outcomeLabel` |
| Binding | `bindingMode` |
| Fiyat koşulu | `triggerCondition`, `triggerPrice`, `outcomeConditions` |
| Tekrar | `repeatMode`, `minIntervalMs` |
| Cycle window | `cycleWindowMode`, eligible zaman alanları |
| Entry timing | `entryTimingProfiles[]` |
| PTB trigger | `priceToBeatTriggerEnabled`, `priceToBeatMode`, `priceToBeatTriggerMinGap`, `priceToBeatTriggerMaxGap` |
| Protection | underlying protection alanları |

## Market Seçimi

| Alan | Açıklama |
|---|---|
| `marketMode="fixed"` | Config'teki `marketSlug` ve token bilgisi kullanılır |
| `marketMode="auto_scope"` | `marketScope` üzerinden aktif market seçilir |
| `marketScope` | `btc_5m_updown`, `eth_5m_updown`, `sol_5m_updown` gibi kapsam |
| `marketSlug` | Sabit market kimliği |
| `tokenId` | Sabit token kimliği |
| `outcomeLabel` | `Up`, `Down`, `YES`, `NO` gibi outcome |

Auto-scope kullanılacaksa sabit slug'a güvenmek yerine scope ve outcome kullanılmalıdır.

## Binding Mode

| Değer | Davranış |
|---|---|
| `standard` veya boş | Fiyat koşulu değerlendirir |
| `pair_lock_only` | Fiyat tetiklemeden YES/NO token binding yapar |

`pair_lock_only`, `action.place_order mode="pair_lock"` için beklenen upstream davranıştır.

## Trigger Condition

| Değer | Açıklama |
|---|---|
| `cross_above` | Önceki fiyat eşik altındayken yeni fiyat eşik üstüne çıkarsa |
| `cross_below` | Önceki fiyat eşik üstündeyken yeni fiyat eşik altına inerse |
| `level_above` | Anlık fiyat eşik üstündeyse |
| `level_below` | Anlık fiyat eşik altındaysa |

`outcomeConditions[]` ile aynı markette birden fazla outcome koşulu izlenebilir.

## Repeat Mode

| Değer | Açıklama |
|---|---|
| `once` | Pass sonrası aynı idempotency bağlamında tekrar tetiklemez |
| `repeat` | Koşul geçmezse interval ile tekrar dener |

Alım flow'larında yanlış double-entry riskini azaltmak için `once` tercih edilir.

## Entry Timing Profiles

Geçerli temel şartlar:

- `marketMode="auto_scope"`
- `repeatMode="once"`
- `cycleWindowMode` kapalı veya boş
- Profil aralıkları çakışmaz
- `startRemainingSec > endRemainingSec`

Profil alanları:

| Alan | Tip | Açıklama |
|---|---|---|
| `startRemainingSec` | int | Aktif olmaya başladığı kalan saniye |
| `endRemainingSec` | int | Aktif olduğu son kalan saniye |
| `maxPriceCent` | number | Action için seçilen tavan fiyat |
| `priceToBeatTriggerMinGap` | number | Manual PTB trigger min gap override |
| `priceToBeatTriggerMaxGap` | number | Manual PTB trigger max gap override |
| `sizeUsdc` | number | Action sizing fallback'i |

## Output Alanları

Temel output:

- `pass`
- `route`
- `marketSlug`
- `tokenId`
- `outcomeLabel`
- `triggeredOutcomeLabel`
- `price`
- `previousPrice`
- `source`

Auto-scope output:

- `marketScope`
- `resolvedMarketSlug`
- `remainingSec`
- `windowStart`
- `windowEnd`

Entry timing output:

- `selectedEntryTimingProfile`
- `selectedEntryTimingProfileIndex`
- `selectedEntryRemainingSec`
- `selectedEntryMaxPrice`
- `selectedEntrySizeUsdc`

Pair binding output:

- YES/NO token bilgileri.
- Binary outcome mapping.
- Pair lock downstream context.

## Başarılı Akış Örneği

```json
{
  "nodeType": "trigger.market_price",
  "config": {
    "marketMode": "auto_scope",
    "marketScope": "btc_5m_updown",
    "outcomeLabel": "Up",
    "triggerCondition": "cross_above",
    "triggerPrice": 0.55,
    "repeatMode": "once"
  }
}
```

Beklenen:

1. Aktif BTC 5m market seçilir.
2. Up token fiyatı 0.55 üstüne kırar.
3. `pass=true`.
4. Downstream action'a route verilir.

## Pair Binding Örneği

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

Beklenen:

- Fiyat koşulu ana karar değildir.
- YES/NO tokenları context'e yazılır.
- Pair lock action node'u bu bağlamla çalışır.

## Validasyon ve Dikkat

- `level_*` koşulları repeat ile kullanılırsa çok sık tetik riski oluşabilir.
- Entry profile aralıkları boşluk bırakabilir; bu durumda kalan süre hiçbir profile düşmez.
- Pair lock için upstream trigger tek ve doğrudan olmalıdır.
- Auto-scope boundary anında birkaç saniyelik veri gecikmesi normaldir.
- Trigger pass, action order garantisi değildir; downstream guard'lar ayrıca block edebilir.
