# 08 - Risk Guardları ve Hata Durumları

Bu dosya order üretmeden önce veya üretim sırasında botun neden block/skip/retry kararı verdiğini açıklar.

## Amaç

"Trigger geçti ama order yok" problemi çoğu zaman bug değildir. Action node içinde guard katmanları sırayla çalışır ve herhangi biri block ederse order üretilmez veya retry planlanır.

## Guard Sırası

Genel buy akışı:

```text
1. Stale market kontrolü
2. Underlying protection
3. Trigger price guard
4. Max price guard
5. Execution floor guard
6. Price-to-beat guard
7. Existing order/reentry guard
8. Risk gate
9. Builder order creation
10. Submit/fill lifecycle
```

Sıra config ve flow tipine göre küçük farklar gösterebilir, ama operatör için önemli olan her block sebebini ayrı okumaktır.

## Stale Market

Durum:

- Auto-scope yeni markete geçti.
- Context hâlâ eski market slug'ını taşıyor.

Beklenen:

- Action eski markete buy göndermemelidir.
- Stale market skip veya context refresh event'i görülür.

Kontrol:

- Trigger output `marketSlug`.
- Action payload `market_slug`.
- Boundary zamanı.

## Underlying Protection

Underlying protection, seçili outcome ile spot/underlying hareketi uyumlu mu diye bakar.

Örnek:

- Up token almak istiyorsun.
- BTC spot fiyatı hızla aşağı gidiyor.
- Protection açık ise buy block edebilir.

Bu guard PTB'den ayrıdır. PTB gap iyi görünse bile underlying ters ise giriş engellenebilir.

## Trigger Price Guard

Action seviyesinde fiyat tetik koruması, trigger'dan sonra fiyatın hâlâ uygun olup olmadığını kontrol eder.

Senaryo:

1. Trigger 0.55 üstü geçer.
2. Action çalışana kadar fiyat 0.62 olur.
3. Trigger guard veya max price guard pahalı giriş diye block eder.

Retry alanları:

- `retryOnTriggerPriceGuardBlock`
- `notifyOnTriggerPriceBlocked`

## Max Price Guard

`maxPrice` veya entry profile'dan gelen `selectedEntryMaxPrice`, alım tavanı olarak davranır.

Örnek:

- `maxPrice=0.60`.
- Best ask 0.63.
- Guard block eder.

Not:

- `priceToBeatMaxPriceRelax*` açıksa ve şartlar sağlanırsa effective tavan kontrollü gevşeyebilir.
- Max price block ile PTB block karıştırılmamalıdır.

## Execution Floor Guard

Execution floor guard, orderbook fiyatı veya derinliği sağlıksızsa alımı engeller.

Tipik block nedenleri:

- Best ask yok.
- Ask fiyatı çok kötü.
- Minimum depth yok.
- VWAP hedef notional için aşırı pahalı.

Retry alanları:

- `retryOnExecutionFloorGuardBlock`
- `notifyOnExecutionFloorBlocked`
- `counterLegExecutionFloorGuardEnabled`

## Price-to-Beat Guard

PTB guard, giriş kalitesini underlying gap ve edge açısından değerlendirir.

Block nedenleri:

- Gap min threshold altında.
- Gap max threshold üstünde ve hareket şüpheli.
- `iv_mismatch_edge` edge yetersiz.
- Binance stale veya ters.
- Depth guard fail.
- Adaptive regime red.

Retry alanları:

- `retryOnPriceToBeatGuardBlock`
- `notifyOnPriceToBeatGapBlocked`
- `counterLegRetryOnPriceToBeatGuardBlock`

## Re-Entry Guard

SL sonrası re-entry açıksa bile guard tekrar girişe izin vermeyebilir.

Kontroller:

- `reentryMaxAttempts` doldu mu?
- `reentryCooldownSec` geçti mi?
- `reentrySkipCurrentWindow=true` ise hâlâ aynı market window'unda mıyız?
- Fill lock serbest bırakıldı mı?
- PTB bump sonrası yeni threshold artık daha mı sıkı?

## Risk Gate

Risk gate sistem limitlerini korur.

Tipik nedenler:

- Market başına notional limit aşıldı.
- Günlük limit aşıldı.
- Aynı token/market için exposure fazla.
- Emir büyüklüğü minimum/maksimum sınır dışında.

Sell tarafında risk genellikle daha gevşektir, çünkü pozisyon azaltır.

## Retry ve Notification

| Alan | Ne zaman kullanılır |
|---|---|
| `retryOnMaxPriceBlock` | Fiyat pahalıysa sonra tekrar dene |
| `retryOnTriggerPriceGuardBlock` | Trigger sonrası fiyat guard block ederse |
| `retryOnExecutionFloorGuardBlock` | Orderbook/floor kötü ise |
| `retryOnPriceToBeatGuardBlock` | PTB şartı geçmezse |
| `notifyOnMaxPriceBlocked` | Max price block bildirimi |
| `notifyOnPriceToBeatGapBlocked` | PTB block bildirimi |

Retry açıkken block final failure değildir; bot yeniden schedule edebilir.

## Hızlı Teşhis Tablosu

| Belirti | İlk bakılacak yer |
|---|---|
| Trigger pass ama order yok | Action event payload, no-order analytics |
| Telegram PTB block diyor | `price_to_beat_guard` telemetry |
| Max price block çok sık | Entry profile `maxPriceCent`, relax global toggle |
| Execution floor block | Best ask/depth/VWAP |
| Re-entry hiç çalışmıyor | attempt count, cooldown, current window skip |
| Pair lock no decision | pair total, single edge, depth, counter market |

## Operatör Checklist

- Block eden guard'ın adı net mi?
- Retry açık mı, yoksa block terminal mi?
- Telegram bildirimi kapalı olduğu için block görünmüyor olabilir mi?
- Analytics zaman aralığı doğru market window'larını kapsıyor mu?
- Aynı anda max price, PTB ve execution floor block'ları karıştırılıyor mu?
- Bump/relax effective threshold'u değiştirmiş mi?

## Block, Skip ve Retry Farkı

| Sonuç | Anlam | Operatör yorumu |
|---|---|---|
| Block | Guard karar verdi ve order üretmedi | Config veya market şartı incelenir |
| Skip | Bu cycle'da işlem yapılmadı | Stale market, idempotency veya uygun olmayan bağlam olabilir |
| Retry | Şart geçici kötü, tekrar denenecek | Terminal failure değildir |
| Reuse | Yeni order yerine mevcut order kullanıldı | Duplicate order engellenmiş olabilir |
| Pending | Order var ama submit/fill bekliyor | Conditional lifecycle kontrol edilir |

Bu ayrım yapılmazsa her şey "order yok" kategorisine düşer ve yanlış config değişikliği yapılır.

## Guard Katmanlarını Birlikte Okuma

Bir action payload'ında birden fazla guard bilgisi olabilir. Operatör şu soruyu sormalıdır:

```text
Order'ı durduran ilk kesin karar hangisi?
```

Örnek:

- Trigger price uygun.
- Max price pahalı.
- PTB hesaplanmış ama order zaten max price'ta block olmuş.

Bu durumda PTB değerlerini değiştirmek sorunu çözmez. İlk terminal block max price ise max price veya relax tarafına bakılır.

## Sayısal Örnek: Üç Guard Aynı Anda

```text
selected max price = 0.62
best ask = 0.64
PTB gap = 35 USD
manual PTB min = 20 USD
execution depth = yeterli
```

Yorum:

- PTB pass.
- Execution floor pass.
- Max price block.

Doğru aksiyon:

- `maxPrice`/entry profile tavanı veya max price relax incelenir.
- PTB min gap düşürülmez.

Ters örnek:

```text
selected max price = 0.66
best ask = 0.61
PTB gap = 12 USD
manual PTB min = 20 USD
execution depth = yeterli
```

Yorum:

- Max price pass.
- Execution floor pass.
- PTB block.

Doğru aksiyon:

- PTB threshold, IV edge, bump veya underlying veri kalitesi incelenir.

## Retry Ayarlarının Riski

Retry kullanışlıdır ama sınırsız retry kötü piyasa koşulunda gereksiz gürültü üretir.

İyi retry:

- Guard geçici bir sebeple block ediyor.
- Market hızlı düzelebilir.
- Retry interval makul.
- Telegram gürültüsü kontrol altında.

Riskli retry:

- Max price tavanı bilinçli olarak düşük.
- PTB threshold stratejik olarak sıkı.
- Retry sürekli aynı block'u üretiyor.
- Operatör gerçek sorunu göremiyor.

Bu yüzden retry açılırken analytics ve notification birlikte düşünülmelidir.

## Risk Gate Detaylı Okuma

Risk gate genellikle strateji kalitesinden değil, sistem limitlerinden bahseder.

Bakılacak sorular:

- Aynı markette mevcut exposure var mı?
- Günlük limit doldu mu?
- Market başına notional limiti aşıldı mı?
- Minimum order büyüklüğü altında mı kalındı?
- Sell order exposure azaltıyor mu?

Risk gate block'u PTB veya price guard gibi optimize edilmez. Burada doğru aksiyon limit politikasını veya sizing'i gözden geçirmektir.

## Debug İçin Event Sırası

1. Trigger event.
2. Action started event.
3. Guard decision event.
4. Builder order created/reused event.
5. Submit event.
6. Fill event.
7. Exit child order event.

Bu sıralamada ilk eksik halka kök neden adayını verir. Örneğin submit event yoksa fill tarafına bakmak zaman kaybıdır.
