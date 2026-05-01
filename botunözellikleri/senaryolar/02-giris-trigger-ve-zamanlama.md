# 02 - Giriş Trigger ve Zamanlama

Bu dosya `trigger.market_price` node'unun fiyat koşullarını, once/repeat davranışını, PTB trigger gate'i ve `entryTimingProfiles` özelliğini senaryolarla açıklar.

## Amaç

Trigger node'u trade flow'un kapısıdır. Yanlış trigger çok erken, çok geç veya yanlış markette order üretir. Doğru trigger ise yalnızca fiyat, zaman ve market bağlamı uygun olduğunda downstream node'a geçiş verir.

## Trigger Koşulları

| Koşul | Kullanım |
|---|---|
| `cross_above` | Fiyat aşağıdan yukarı eşiği kırınca |
| `cross_below` | Fiyat yukarıdan aşağı eşiği kırınca |
| `level_above` | Fiyat eşik üstünde kaldığı sürece |
| `level_below` | Fiyat eşik altında kaldığı sürece |

`cross_*` koşulları önceki fiyatı da kullanır. `level_*` koşulları anlık fiyat seviyesine bakar ve genellikle `repeatMode="once"` ile daha güvenlidir.

## Once ve Repeat

| Mod | Davranış |
|---|---|
| `once` | Koşul bir kere geçince idempotency state yazılır, aynı run/window içinde tekrar tetiklemez |
| `repeat` | Koşul geçmezse `minIntervalMs` veya schedule ile tekrar denenir |

5 dakikalık marketlerde `once` çoğu alım flow'u için daha güvenlidir. `repeat`, izleme veya çok parçalı giriş için kullanılır.

## Senaryo A: Basit Up Trigger

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

Akış:

1. Auto-scope aktif BTC 5m marketini çözer.
2. Up token fiyatı 0.55 üstüne kırarsa `pass=true` olur.
3. Context'e market, token, outcome ve trigger fiyat bilgisi yazılır.
4. Downstream `action.place_order` çalışır.

## Senaryo B: Pair Lock Binding

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

Bu modda fiyat koşulu ana amaç değildir. Trigger YES/NO tokenlarını downstream pair lock node'una bağlar. `action.place_order mode="pair_lock"` için upstream doğrudan ve tek olmalıdır.

## Entry Timing Profiles

`entryTimingProfiles`, auto-scope marketin bitişine kalan süreye göre farklı giriş profili seçer.

Geçerli olduğu temel kombinasyon:

- `marketMode="auto_scope"`
- `repeatMode="once"`
- `cycleWindowMode` boş veya kapalı
- Profil aralıkları çakışmaz
- Her profil için `startRemainingSec > endRemainingSec`

Profil alanları:

| Alan | Anlam |
|---|---|
| `startRemainingSec` | Profilin başladığı kalan saniye |
| `endRemainingSec` | Profilin bittiği kalan saniye |
| `maxPriceCent` | Bu aralıkta downstream için seçilen tavan fiyat |
| `priceToBeatTriggerMinGap` | Manual PTB trigger min gap override |
| `priceToBeatTriggerMaxGap` | Manual PTB trigger max gap override |
| `sizeUsdc` | Action node'da explicit sizing yoksa fallback büyüklük |

## Senaryo C: Erken Ucuz, Geç Güçlü Giriş

```json
{
  "marketMode": "auto_scope",
  "marketScope": "btc_5m_updown",
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
      "sizeUsdc": 8
    }
  ]
}
```

Yorum:

- Erken bölümde fiyat tavanı düşük tutulur.
- Sonlara doğru market daha hızlı hareket ettiği için daha yüksek fiyat kabul edilir ama PTB gap şartı sıkılaşır.
- `action.place_order` kendi `sizeUsdc` değerini vermiyorsa seçili profil `sizeUsdc` fallback olur.

## PTB Trigger Gate

Trigger seviyesinde PTB gate, action'a geçmeden önce "bu fiyat hareketi gerçekten anlamlı mı?" sorusunu sorar.

Örnek kullanım:

- `priceToBeatTriggerEnabled=true`
- `priceToBeatMode="manual"`
- `priceToBeatTriggerMinGap=20`
- `priceToBeatTriggerMaxGap=80`

Beklenen:

- Gap min altında kalırsa trigger geçmez.
- Gap max üstünde ise hareket aşırı/şüpheli sayılabilir.
- Entry profile seçiliyse min/max değerleri profil tarafından override edilebilir.

## Runtime Output

Entry timing seçilirse şu alanlar context veya output içinde aranır:

- `selectedEntryTimingProfile`
- `selectedEntryTimingProfileIndex`
- `selectedEntryRemainingSec`
- `selectedEntryMaxPrice`
- `selectedEntrySizeUsdc`

Trigger'ın temel output alanları:

- `marketSlug`
- `tokenId`
- `outcomeLabel`
- `triggeredOutcomeLabel`
- `price`
- `previousPrice`
- `pass`

## Sık Block Durumları

| Belirti | Muhtemel neden |
|---|---|
| Profil seçilmiyor | Kalan süre hiçbir profile düşmüyor |
| Validation hata veriyor | Profil aralıkları çakışıyor veya `startRemainingSec <= endRemainingSec` |
| Action sizing beklenenden farklı | Action'da explicit `sizeUsdc` var, profil fallback kullanılmıyor |
| Trigger geçiyor ama action block ediyor | Trigger gate geçse bile action guard'ları ayrıca çalışır |

## Operatör Checklist

- `marketMode` auto-scope ise `marketScope` doğru mu?
- `repeatMode` alım flow'u için `once` mı?
- `entryTimingProfiles` aralıkları çakışmıyor mu?
- Geç giriş profili daha yüksek `maxPriceCent` içeriyorsa PTB/IV şartı da sıkı mı?
- Trigger output'ta seçili profil ve kalan süre beklenen aralıkta mı?

## Detaylı Karar Mantığı

Trigger node'u üç soruya sırayla cevap verir:

1. Hangi market ve token izleniyor?
2. Fiyat koşulu geçiyor mu?
3. Geçiş downstream için yeterli bağlam üretiyor mu?

Bu yüzden trigger debug ederken sadece `triggerPrice` değerine bakmak eksiktir. Market yanlışsa doğru fiyat koşulu bile yanlış token için geçer. Token doğru ama once idempotency doluysa fiyat koşulu tekrar geçse bile route verilmeyebilir. Entry profile seçilmemişse action beklenen `maxPrice` veya `sizeUsdc` fallback'ini alamayabilir.

## Sayısal Örnek: Cross ve Level Farkı

Örnek fiyat akışı:

```text
12:01:00 fiyat 0.52
12:01:05 fiyat 0.54
12:01:10 fiyat 0.56
12:01:15 fiyat 0.57
```

`triggerCondition="cross_above"`, `triggerPrice=0.55`:

- 12:01:10 anında geçer.
- Çünkü fiyat 0.54'ten 0.56'ya eşiği kırmıştır.
- 12:01:15 tekrar geçmez; çünkü tekrar cross yoktur.

`triggerCondition="level_above"`, `triggerPrice=0.55`:

- 12:01:10 ve 12:01:15 anlarında koşul doğrudur.
- `repeatMode="loop"` ise çoklu tetik riski doğar.
- `repeatMode="once"` ise ilk geçişten sonra idempotency korur.

## Entry Timing Tasarım Örneği

Tek markette üç ayrı karar alanı kurulabilir:

| Kalan süre | Amaç | Örnek ayar |
|---:|---|---|
| 240-150 sn | Sadece ucuz giriş | `maxPriceCent=56`, `sizeUsdc=4` |
| 150-60 sn | Normal fırsat | `maxPriceCent=62`, `sizeUsdc=7` |
| 60-20 sn | Sadece güçlü momentum | `maxPriceCent=68`, `priceToBeatTriggerMinGap=30`, `sizeUsdc=5` |

Bu modelde son bölüm daha pahalı fiyat kabul eder ama daha yüksek gap ister. Eğer son bölümde hem fiyat pahalı hem gap zayıf kabul edilirse entry timing risk azaltmak yerine FOMO davranışı üretir.

## Başarısız Profil Seçimi Örneği

```json
{
  "entryTimingProfiles": [
    {"startRemainingSec": 180, "endRemainingSec": 90, "maxPriceCent": 60},
    {"startRemainingSec": 120, "endRemainingSec": 30, "maxPriceCent": 65}
  ]
}
```

Problem:

- 120-90 saniye aralığı iki profile de girer.
- Operatör hangi max price'ın geçerli olacağını yanlış yorumlayabilir.

Düzeltme:

```json
{
  "entryTimingProfiles": [
    {"startRemainingSec": 180, "endRemainingSec": 120, "maxPriceCent": 60},
    {"startRemainingSec": 120, "endRemainingSec": 30, "maxPriceCent": 65}
  ]
}
```

Sınır değerlerinde runtime'ın nasıl değerlendirdiği kodda tanımlıdır; doküman tarafında amaç çakışmayı tamamen ortadan kaldırmaktır.

## Trigger Geçti Ama Action Neden Farklı Davrandı?

Trigger ve action arasında zaman geçer. Bu kısa aralıkta:

- Best ask değişebilir.
- Market boundary yaklaşabilir.
- PTB gap daralabilir.
- Entry profile'dan gelen max price action guard tarafından override değil, input olarak kullanılabilir.
- Action explicit `maxPrice` veya `sizeUsdc` veriyorsa trigger profile fallback'i kullanılmayabilir.

Bu yüzden trigger output ile action guard payload aynı timestamp gibi okunmamalıdır. İkisi farklı karar anlarını temsil eder.

## Telemetry Okuma Örneği

Aranacak alanlar:

```text
selectedEntryTimingProfileIndex = 1
selectedEntryRemainingSec = 74
selectedEntryMaxPrice = 0.64
selectedEntrySizeUsdc = 8
marketSlug = btc-updown-5m-...
pass = true
```

Yorum:

- Kalan süre 74 saniye olduğu için ikinci profil seçilmiş.
- Action explicit size vermediyse 8 USDC fallback olabilir.
- Action max price block ederse karşılaştırılacak tavan 0.64 olmalıdır.
