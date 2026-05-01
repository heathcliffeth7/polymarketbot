# Referans - `trigger.market_price`

Güncelleme tarihi: 2026-05-01

Bu dosya `trigger.market_price` node'unun config, output ve operasyon davranışını özetler.

## Görev

`trigger.market_price`, market fiyatını veya market binding bilgisini izler ve uygun durumda downstream node'a route verir.

Kullanım biçimleri:

- Sabit market slug ve token ile fiyat tetikleme.
- Auto-scope ile aktif 5m marketi seçme.
- Multi-outcome koşul izleme.
- Pair lock için YES/NO token binding.
- DCA live için market/window binding.
- Entry timing profile seçme.
- PTB trigger gate ile action'a geçmeden giriş kalitesi kontrolü.

## Ana Config Grupları

| Grup | Alanlar |
|---|---|
| Market seçimi | `marketMode`, `marketScope`, `marketSlug`, `tokenId`, `outcomeLabel` |
| Binding | `bindingMode` |
| Fiyat koşulu | `triggerCondition`, `triggerPrice`, `outcomeConditions` |
| Tekrar/firing | `repeatMode`, `onceScope`, `minIntervalMs` |
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
| `dca_live_only` | Fiyat tetiklemeden DCA live action'a market/window binding yapar |

`pair_lock_only`, `action.place_order mode="pair_lock"` için beklenen upstream davranıştır.
`dca_live_only`, `action.place_order mode="dca_live_v1"` için beklenen upstream davranıştır.

## Trigger Condition

| Değer | Açıklama |
|---|---|
| `cross_above` | Önceki fiyat eşik altındayken yeni fiyat eşik üstüne çıkarsa |
| `cross_below` | Önceki fiyat eşik üstündeyken yeni fiyat eşik altına inerse |
| `level_above` | Anlık fiyat eşik üstündeyse |
| `level_below` | Anlık fiyat eşik altındaysa |

`outcomeConditions[]` ile aynı markette birden fazla outcome koşulu izlenebilir.

## Firing Mode

UI üç modu gösterir; config karşılığı `repeatMode` ve `onceScope` alanlarıdır.

| UI modu | Config | Açıklama |
|---|---|
| Once per market | `repeatMode="once"`, `onceScope="market"` | Auto-scope her yeni markette bir kez tetikleyebilir |
| Once per run | `repeatMode="once"`, `onceScope="run"` | Workflow run boyunca ilk başarılı tetikten sonra tekrar tetiklemez |
| Loop | `repeatMode="loop"` | Koşul geçmezse interval ile tekrar dener |

Alım flow'larında yanlış double-entry riskini azaltmak için `once` tercih edilir. Auto-scope + once varsayılanı market scope'tur.

`level_above` ve `level_below` sürekli true kalabileceği için `repeatMode="once"` ister. `cross_above` ve `cross_below` loop modda kullanılabilir.

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

DCA binding output:

- `bindingMode="dca_live_only"` bilgisi.
- Market slug/window context.
- Custom range zamanları.
- DCA action için route izni.

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

## Alanların Karar Etkisi

| Alan | Karar etkisi | Yanlış ayar belirtisi |
|---|---|---|
| `marketMode` | Sabit market mi auto-scope mu belirler | Eski markete bakma veya hiç market bulamama |
| `marketScope` | Auto-scope ailesini seçer | BTC yerine ETH/SOL marketi izleme |
| `outcomeLabel` | Up/Down veya YES/NO token seçer | Ters yönde token bağlanması |
| `bindingMode` | Fiyat trigger mı, pair binding mi belirler | Pair lock action validation hatası |
| `triggerCondition` | Fiyat geçiş mantığını seçer | Çok sık veya hiç tetiklenmeme |
| `repeatMode` | Tek sefer mi tekrar mı belirler | Double entry veya beklenen retry'nin olmaması |
| `entryTimingProfiles` | Kalan süreye göre action input üretir | Max price veya size beklenen gibi değişmez |

## Config Tasarım Kuralları

Auto-scope alım flow'u:

- `marketMode="auto_scope"` kullan.
- `marketScope` açık ver.
- `repeatMode="once"` ile duplicate giriş riskini azalt.
- Entry timing kullanıyorsan action explicit size/max price override'larını kontrol et.

Pair lock flow'u:

- Trigger fiyat koşulu yerine `bindingMode="pair_lock_only"` kullan.
- Downstream tek action node olmalı.
- Action `mode="pair_lock"` ile uyumlu olmalı.

DCA live flow'u:

- Trigger fiyat koşulu yerine `bindingMode="dca_live_only"` kullan.
- Downstream tek reachable `action.place_order mode="dca_live_v1"` olmalı.
- Outcome condition ve PTB trigger gate bu binding modunda kullanılmaz.

Debug flow'u:

- Önce basit trigger ile market ve token çözümünü doğrula.
- Sonra entry timing veya PTB trigger gate ekle.
- En son action guard'ları sıkılaştır.

## Örnek: Trigger Output'tan Action'a Aktarım

Trigger output:

```json
{
  "pass": true,
  "marketSlug": "btc-updown-5m-1800000000",
  "tokenId": "123",
  "outcomeLabel": "Up",
  "selectedEntryMaxPrice": 0.64,
  "selectedEntrySizeUsdc": 8
}
```

Action tarafında beklenen:

- `marketSlug`, `tokenId`, `outcomeLabel` config'te yoksa context'ten çözülür.
- Action explicit `maxPrice` vermiyorsa `selectedEntryMaxPrice` kullanılabilir.
- Action explicit `sizeUsdc` vermiyorsa `selectedEntrySizeUsdc` fallback olabilir.

Bu aktarımın çalışması için trigger ve action aynı flow context içinde bağlı olmalıdır.

## Sık Validasyon Sorunları

| Sorun | Açıklama |
|---|---|
| Entry profile çakışması | Aynı kalan süre iki profile girer |
| Entry profile boşluğu | Bazı kalan sürelerde hiçbir profil yoktur |
| Pair lock upstream hatası | Trigger `bindingMode="pair_lock_only"` değildir |
| DCA live upstream hatası | Trigger `bindingMode="dca_live_only"` değildir |
| Repeat ile level trigger | Aynı koşul sürekli true olduğu için çoklu tetik riski |
| Fixed market + auto-scope beklentisi | `marketMode` sabit olduğu için yeni window'a geçmez |

## Operasyonel Kural

Trigger dosyası debug edilirken önce market ve token doğrulanır, sonra fiyat koşulu incelenir. Çünkü doğru fiyat koşulu yanlış token üzerinde çalışıyorsa downstream bütün kararlar teknik olarak doğru ama stratejik olarak hatalı olur.
