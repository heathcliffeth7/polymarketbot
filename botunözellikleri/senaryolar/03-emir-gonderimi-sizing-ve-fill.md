# 03 - Emir Gönderimi, Sizing ve Fill

Bu dosya `action.place_order` node'unun buy/sell order üretimini, sizing mantığını, existing order reuse/rearm davranışını ve `buyFillLockEnabled` korumasını açıklar.

## Amaç

Trigger geçtikten sonra botun görevi doğru token için doğru büyüklükte builder order üretmektir. Bu aşamada fiyat korumaları, risk limitleri, source trade, existing order ve fill idempotency kritik hale gelir.

## Temel Akış

```text
1. Upstream context'ten marketSlug/tokenId/outcomeLabel çözülür.
2. Stale market kontrolü yapılır.
3. Buy tarafında underlying protection ve guard'lar çalışır.
4. sourceTradeId çözülür veya buy için otomatik oluşturulur.
5. Existing order varsa reuse veya rearm kararı verilir.
6. Sizing hesaplanır.
7. Risk gate çalışır.
8. Builder order oluşturulur.
9. immediate ise CLOB submit yapılır, conditional ise pending kalır.
```

## Buy ve Sell Farkı

| Alan | Buy | Sell |
|---|---|---|
| `sourceTradeId` | Yoksa otomatik oluşturulabilir | Zorunlu bağlam gerekir |
| Sizing | `sizeUsdc`, `targetNotionalUsdc`, `triggerSizes`, fallback profile | Pozisyondaki kalan qty veya yüzde |
| Guard | PTB, max price, execution floor, underlying | Genellikle çıkış güvenliği ve pozisyon kalanı |
| Risk | Yeni exposure artırır | Exposure azaltır |

## Senaryo A: Basit Market Buy

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 10,
  "kind": "immediate"
}
```

Beklenen:

1. Trigger context'ten token çözülür.
2. `sizeUsdc=10` ile notional belirlenir.
3. Risk gate izin verirse builder order `triggered` olur.
4. Inline submit açıksa CLOB'a hemen gönderilir.

## Senaryo B: Conditional Limit Buy

```json
{
  "side": "buy",
  "executionMode": "limit",
  "kind": "conditional",
  "triggerCondition": "cross_below",
  "triggerPrice": 0.50,
  "maxPrice": 0.55,
  "sizeUsdc": 15
}
```

Bu kurulumda action node order'ı hemen markete göndermez. Builder order pending kalır; koşul sağlanınca limit submit yapılır.

Operatör notu:

- Conditional order bekliyorsa "order yok" değil, "pending order var" durumudur.
- `triggerPrice` ve `maxPrice` birbirine çok yakınsa fill şansı düşebilir.

## Sizing Seçenekleri

| Alan | Davranış |
|---|---|
| `sizeUsdc` | Buy için doğrudan USDC büyüklüğü |
| `targetNotionalUsdc` | Hedef notional |
| `sizePct` / `sizePercent` | Kaynak pozisyon veya source notional yüzdesi |
| `triggerSizes` | Çoklu tetiklerde her tetik için farklı büyüklük |
| `selectedEntrySizeUsdc` | Entry timing profile fallback'i |

Sizing önceliği pratikte explicit action config değerlerinin fallback context değerlerinden daha güçlü olmasıdır. Entry profile sizing bekleniyorsa action tarafında `sizeUsdc` boş bırakılmalıdır.

## Senaryo C: Parçalı Giriş

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeMode": "usdc",
  "maxTriggers": 3,
  "triggerSizes": [20, 15, 15]
}
```

Beklenen:

- İlk trigger 20 USDC.
- İkinci trigger 15 USDC.
- Üçüncü trigger 15 USDC.
- `maxTriggers` dolduktan sonra yeni buy üretilmez.

Risk:

- 5 dakikalık markette çok parçalı giriş window sonuna taşarsa fill kalitesi bozulabilir.
- Pair lock modunda `maxTriggers=1` beklenir.

## Existing Order Reuse ve Rearm

Bot aynı bağlamda aktif emir görürse yeni emir açmak yerine onu tekrar kullanabilir. Hata durumundaki sell order için rearm yapılabilir.

Kontrol edilmesi gerekenler:

- Aynı `sourceTradeId` var mı?
- Aktif order aynı token ve side için mi?
- Order status terminal mi, pending mi, failed mı?
- Sell order pozisyon kalanı ile uyumlu mu?

## Buy Fill Lock

`buyFillLockEnabled=true`, aynı market cycle içinde aynı gruptan ikinci buy girişini engeller.

İlgili alanlar:

| Alan | Anlam |
|---|---|
| `buyFillLockEnabled` | Fill sonrası aynı market/grup için yeni buy kapısını kilitler |
| `releaseBuyFillLockOnStopLoss` | Stop-loss sonrası kilidi açabilir |

Senaryo:

1. Bot BTC 5m Up için buy gönderir.
2. Emir fill olur.
3. Aynı markette ikinci trigger tekrar geçer.
4. Fill lock açıksa ikinci buy block edilir.

Ne zaman kullanılır:

- Aynı window içinde gereksiz double entry engellenecekse.
- Re-entry sadece SL sonrası istenecekse.
- Multi-trigger strateji kullanılmıyorsa.

## Max Price ve Execution Floor

Buy order üretiminde iki fiyat koruması sık görülür:

- `maxPrice`: Daha pahalıya alma.
- Execution floor: Best ask veya orderbook kalitesi çok kötü ise girme.

Bu guard'lar PTB'den bağımsızdır. PTB geçse bile max price veya execution floor block edebilir.

## Sık Hatalar

| Belirti | Muhtemel neden |
|---|---|
| Trigger geçti ama order yok | Action guard block, risk block veya stale market |
| Order pending kaldı | `kind="conditional"` veya existing pending order |
| Sizing beklenenden küçük | `triggerSizes` veya profile fallback devrede |
| İkinci buy gelmiyor | `buyFillLockEnabled` kilit tuttu |
| Sell hata veriyor | `sourceTradeId` veya pozisyon bağlamı yok |

## Operatör Checklist

- Action `side` ve `executionMode` doğru mu?
- Buy için explicit sizing var mı, yoksa entry profile fallback bekleniyor mu?
- `maxPrice` cent/decimal beklentisiyle doğru verildi mi?
- Existing order reuse sebebiyle yeni order açılmıyor olabilir mi?
- Fill lock açıkken aynı markette ikinci buy gerçekten isteniyor mu?
- Sell flow'unda source trade ve kalan pozisyon mevcut mu?

## Action İçindeki Karar Noktaları

Action node'u tek bir "order gönder" komutu gibi görünse de içeride birkaç ayrı karar verir:

1. Bağlam çözümü: market, token, outcome, source trade.
2. Güvenlik: stale market ve side/execution uyumu.
3. Giriş kalitesi: max price, execution floor, PTB, underlying.
4. Idempotency: existing order reuse veya rearm.
5. Büyüklük: USDC, pct, trigger size veya profile fallback.
6. Lifecycle: immediate submit, conditional pending, TP/SL çocuk emirleri.

Bu kararlar ayrı ayrı loglanmadığında operatör "order yok" der. Doğru teşhis, hangi adımda durduğunu bulmaktır.

## Sizing Önceliği İçin Pratik Örnek

Durum:

- Trigger entry profile `selectedEntrySizeUsdc=8` üretmiş.
- Action config içinde `sizeUsdc=10` var.

Beklenen:

- Action explicit değer olan 10 USDC kullanır.
- Profile fallback kullanılmaz.

Durum:

- Trigger `selectedEntrySizeUsdc=8`.
- Action config içinde `sizeUsdc` yok.
- Action buy ve USDC sizing bekliyor.

Beklenen:

- Action 8 USDC fallback kullanabilir.

Operatör hatası:

- UI'da profile size değiştirip action'daki explicit `sizeUsdc` değerini unutmak.

## Fill ve Submit Ayrımı

Submit aşaması:

```text
builder order oluşturuldu
should_inline_submit=true
CLOB submit denendi
notifyOnOrderSubmitted mesajı geldi
```

Fill aşaması:

```text
CLOB order matched
fill event geldi
pozisyon oluştu
TP/SL child order kurulabilir
notifyOnFill veya fill mesajı geldi
```

Submit mesajı fill garantisi değildir. Özellikle limit order, hızlı market ve düşük depth durumlarında submit sonrası fill olmayabilir.

## Sayısal Örnek: Max Price ve Fill

Action config:

```json
{
  "executionMode": "limit",
  "maxPrice": 0.60,
  "sizeUsdc": 20
}
```

Orderbook:

```text
ask 0.59 depth 4 USDC
ask 0.61 depth 20 USDC
```

Yorum:

- Best ask max price altında görünüyor.
- Ama 20 USDC almak için 0.61 seviyesine çıkmak gerekebilir.
- Execution floor veya depth guard açıksa block edebilir.
- Guard yoksa partial fill veya kötü fill riski oluşur.

## Existing Order Reuse Detayı

Reuse istenen bir davranıştır. Aynı flow aynı markette tekrar tetiklendiğinde her seferinde yeni order açmak exposure'ı şişirebilir.

Reuse olduğunda:

- Yeni builder order id beklenmeyebilir.
- Mevcut pending/active order döndürülebilir.
- Operator bunu "action çalışmadı" diye yorumlamamalıdır.

Rearm farklıdır:

- Özellikle sell tarafında failed order tekrar kullanılabilir.
- Pozisyon kalanı yeniden hesaplanmalıdır.
- Rearm sonrası eski hata sebebinin devam edip etmediği kontrol edilmelidir.

## Buy Fill Lock Detaylı Örnek

Durum:

1. BTC 5m Up için 10 USDC buy fill oldu.
2. Aynı markette fiyat tekrar trigger koşulunu geçti.
3. `buyFillLockEnabled=true`.
4. `releaseBuyFillLockOnStopLoss=false`.

Beklenen:

- İkinci buy block edilir.
- Bu block, risk azaltma davranışıdır.

Eğer SL sonrası tekrar giriş isteniyorsa:

```json
{
  "buyFillLockEnabled": true,
  "releaseBuyFillLockOnStopLoss": true,
  "reenterOnSlHit": true
}
```

Bu durumda kilit sadece stop-loss sonrası kontrollü re-entry için açılabilir.

## Yanlış Yorumlar

| Yanlış yorum | Doğru yorum |
|---|---|
| Builder order oluştuysa pozisyon açıldı | Fill event olmadan pozisyon oluşmaz |
| Conditional order yok demektir | Pending lifecycle bekliyor olabilir |
| Profile size çalışmıyor | Action explicit size override ediyor olabilir |
| Reuse bug'dır | Aynı bağlamda duplicate order engelleniyor olabilir |
| Fill lock order sistemini bozuyor | Aynı markette ikinci buy riskini engelliyor |
