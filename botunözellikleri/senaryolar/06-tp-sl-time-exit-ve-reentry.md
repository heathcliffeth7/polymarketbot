# 06 - TP, SL, Time Exit ve Re-Entry

Bu dosya pozisyon açıldıktan sonra nasıl çıkılacağını anlatır: take profit, stop loss, PTB stop-loss, time exit, window end auto-sell ve re-entry.

## Amaç

5 dakikalık marketlerde entry kadar exit de belirleyicidir. TP çok uzakta olursa pozisyon resolution'a kalır; SL çok agresif olursa normal noise içinde pozisyon kapanır; re-entry kontrolsüzse zarar seri hale gelir.

## Çıkış Ailesi

| Özellik | Ne yapar |
|---|---|
| Hard TP | Tek fiyat seviyesinde pozisyonu kapatır |
| Staged TP | Birden fazla seviyede parça parça satar |
| Hard SL | Tek zarar seviyesinde kapatır |
| Staged SL | Zarar seviyelerini parça parça uygular |
| PTB stop-loss | Token fiyatı yerine price-to-beat gap bozulunca çıkar |
| Time exit | Belirli süre geçince pozisyon azaltır |
| Window end auto-sell | Market kapanmadan kalan pozisyonu satmaya çalışır |
| Re-entry | SL sonrası yeni giriş dener |

## Senaryo A: Hard TP ve Hard SL

```json
{
  "side": "buy",
  "executionMode": "market",
  "sizeUsdc": 20,
  "tpRules": [
    {"price": 0.70, "sizePct": 100}
  ],
  "slRules": [
    {"price": 0.35, "sizePct": 100}
  ]
}
```

Akış:

1. Buy fill olur.
2. TP sell order 0.70 seviyesine kurulur.
3. SL sell order 0.35 seviyesine kurulur.
4. İlk gerçekleşen çıkış pozisyonu kapatır.

## Senaryo B: Staged TP

```json
{
  "tpRules": [
    {"priceCent": 65, "sizePct": 40},
    {"priceCent": 80, "sizePct": 40},
    {"priceCent": 95, "sizePct": 20}
  ]
}
```

Bu yapı momentum güçlü ise kârı taşır; zayıf ise en azından ilk kademeden realized profit alabilir.

Dikkat:

- Çok fazla kademe düşük likiditede fill karmaşası yaratabilir.
- Kardeş emir politikası kalan TP/SL büyüklüklerini doğru güncellemelidir.

## Senaryo C: Staged SL

```json
{
  "slRules": [
    {"priceCent": 48, "sizePct": 50},
    {"priceCent": 38, "sizePct": 50}
  ],
  "slSiblingPolicy": "resize_remaining"
}
```

Beklenen:

- İlk SL pozisyonun yarısını kapatır.
- Kalan kardeş TP/SL emirleri kalan pozisyona göre resize edilir.
- `cancel_all` policy seçilirse hard çıkışta tüm kardeşler iptal edilebilir.

## SL Trigger Price Mode

| Mod | Davranış |
|---|---|
| `best_bid` | Hızlıdır, orderbook bid'e bakar |
| `composite_safe` | Daha güvenli, manipülasyona daha dayanıklı |
| `composite_fast` | Daha agresif composite sinyal |
| `last_trade` | Son trade fiyatını kullanır |

Pratik:

- Çok hızlı koruma için `best_bid`.
- Yanlış SL riskini azaltmak için `composite_safe`.
- Geç kalmayı azaltmak için `composite_fast`.

## PTB Stop-Loss

PTB stop-loss token fiyatından çok "entry'nin dayandığı underlying avantaj bozuldu mu?" sorusuna bakar.

Örnek:

```json
{
  "ptbStopLossEnabled": true,
  "ptbStopLossGapUsd": 0,
  "ptbStopLossTimeDecayMode": "tighten"
}
```

Modlar:

| `ptbStopLossTimeDecayMode` | Anlam |
|---|---|
| `none` | Threshold zamanla değişmez |
| `tighten` | Market sonuna doğru daha hassas olur |
| `relax` | Market sonuna doğru daha toleranslı olur |

## Time Exit

Time exit, pozisyonu sadece fiyatla değil zamanla da yönetir.

```json
{
  "timeExitRules": [
    {"elapsedMinutes": 3, "remainingPct": 50},
    {"elapsedMinutes": 4, "remainingPct": 0}
  ]
}
```

Yorum:

- 3. dakikada pozisyonun yarısı kalacak şekilde satış yapılır.
- 4. dakikada tamamı kapatılmaya çalışılır.
- Time exit, TP/SL olmayan pozisyonların resolution'a kalma riskini azaltır.

## Re-Entry

```json
{
  "reenterOnSlHit": true,
  "reentryMaxAttempts": 2,
  "reentryCooldownSec": 10,
  "reentrySkipCurrentWindow": true,
  "reentryThresholdDecay": 0.8,
  "reentryMaxPriceTightenBps": 500
}
```

Anlam:

- SL sonrası en fazla 2 yeni giriş dene.
- 10 saniye bekle.
- Aynı current window'u atla.
- PTB threshold ve max price koşullarını re-entry için sıkılaştır.

Validation notları:

- `reentrySkipCurrentWindow=true`, `reenterOnSlHit=true` gerektirir.
- `reentryThresholdDecay`, re-entry ve PTB guard açıkken anlamlıdır.
- `reentryMaxPriceTightenBps` 0-10000 aralığında olmalıdır.

## Staged SL Re-Entry

Staged SL kullanırken hemen re-entry açmak bazen yanlıştır; çünkü pozisyonun kalan kısmı hâlâ açık olabilir.

Yaklaşımlar:

- Tüm SL kademeleri bitmeden re-entry yok.
- Dust pozisyon kalmışsa re-entry serbest.
- Hemen re-entry sadece çok agresif stratejilerde kullanılır.

## Exit Price Cap

Sell tarafında `exit_price_capped` benzeri davranış, satış fiyatını çok düşükten göndermeyi engeller. TP'de faydalı olabilir; SL'de aşırı cap fill'i engelleyebilir.

Pratik:

- TP için cap koruyucu olabilir.
- SL için fill önceliği daha önemli olabilir.

## Operatör Checklist

- TP/SL toplam `sizePct` mantıklı mı?
- Staged TP ve staged SL birlikteyse sibling policy ne?
- SL trigger mode hızlı mı güvenli mi seçildi?
- PTB stop-loss token fiyatı yerine underlying gap'e göre çıkış istendiğinde açık mı?
- Time exit window sonuna çok mu yakın?
- Re-entry aynı window içinde zarar serisine dönüşüyor mu?
- `reentrySkipCurrentWindow` ve bump birlikte kullanılıyor mu?

## Entry Sonrası Lifecycle

Bir buy fill olduğunda bot artık sadece pozisyon tutmaz; aynı zamanda çıkış planı kurar.

Tipik lifecycle:

```text
buy fill
  -> source trade güncellenir
  -> pozisyon qty hesaplanır
  -> TP child order hazırlanır
  -> SL child order hazırlanır
  -> PTB SL kuralı varsa izleme başlar
  -> time exit schedule varsa zamanlayıcı kurulur
  -> re-entry policy snapshot alınır
```

Bu nedenle fill sonrası TP/SL gelmiyorsa sadece "exit yok" diye bakma. Önce buy fill gerçekten oluştu mu, source trade bağlandı mı, child order oluşturma event'i var mı kontrol et.

## TP ve SL Aynı Anda Varken Öncelik

TP ve SL piyasa hareketine göre yarışır. Staged yapıda bu yarış daha karmaşıktır.

Örnek:

```text
pozisyon = 100 share
TP1 = 65 cent, size 50
TP2 = 80 cent, size 50
SL1 = 45 cent, size 50
SL2 = 35 cent, size 50
```

Akış:

1. Fiyat 65'e çıkar, TP1 50 share satar.
2. Kalan pozisyon 50 share.
3. Fiyat 45'e düşer, SL1 artık kalan pozisyona göre hesaplanmalıdır.
4. `slSiblingPolicy="resize_remaining"` ise kardeş emirler kalan qty ile uyumlu hale gelir.

Yanlış sibling policy, kalan pozisyondan daha fazla sell denemesine veya korumasız pozisyona yol açabilir.

## SL Trigger Mode Seçimi

`best_bid`:

- En hızlı sinyali verir.
- Spread genişse veya bid kısa süreli düşerse gereksiz SL tetikleyebilir.

`composite_safe`:

- Daha dayanıklıdır.
- Çok hızlı düşüşte geç kalabilir.

`composite_fast`:

- Safe ile best bid arasında agresif davranış sağlar.
- Momentum stratejilerinde kullanılabilir ama noise riski vardır.

Operatör kararı:

- Amaç sermayeyi hızlı korumaksa `best_bid`.
- Amaç fake wick yüzünden çıkmamaksa `composite_safe`.
- İkisi arasında denge isteniyorsa `composite_fast`.

## PTB Stop-Loss Sayısal Örnek

Up token alındı:

```text
entry sırasında PTB gap = +35 USD
ptbStopLossGapUsd = 0
current gap = -2 USD
```

Yorum:

- Token fiyatı hâlâ SL seviyesine gelmemiş olabilir.
- Ama underlying avantaj bitmiş ve tersine geçmiş olabilir.
- PTB stop-loss pozisyonu kapatmayı deneyebilir.

Bu özellik, token fiyatının geç tepki verdiği durumlarda underlying bozulmayı erken yakalamak için kullanılır.

## Time Exit ve Window End Farkı

Time exit:

- Pozisyon açıldıktan sonra geçen süreye göre çalışır.
- "Bu trade 3 dakika açık kaldı, yarısını azalt" gibi davranır.

Window end auto-sell:

- Market kapanışına kalan süreye göre çalışır.
- "Bu market bitiyor, elde pozisyon kalmasın" davranışıdır.

İkisi birlikte kullanılabilir. Time exit erken risk azaltır; window end auto-sell son güvenlik ağıdır.

## Re-Entry Riskleri

Re-entry kârlı olabilir ama zarar serisini de büyütebilir.

Riskli kurulum:

```json
{
  "reenterOnSlHit": true,
  "reentryMaxAttempts": 5,
  "reentrySkipCurrentWindow": false,
  "reentryCooldownSec": 0
}
```

Problem:

- Aynı chop window içinde arka arkaya giriş yapılabilir.
- Her giriş aynı kötü piyasa koşuluna yakalanabilir.
- SL serisi kısa sürede büyür.

Daha kontrollü kurulum:

```json
{
  "reenterOnSlHit": true,
  "reentryMaxAttempts": 2,
  "reentrySkipCurrentWindow": true,
  "reentryCooldownSec": 10,
  "reentryMaxPriceTightenBps": 500
}
```

Bu yapı aynı window'u atlar, girişleri sınırlar ve yeniden giriş fiyatını sıkılaştırır.

## Yanlış Yorumlar

| Yanlış yorum | Doğru yorum |
|---|---|
| SL vurduysa strateji kesin kötü | SL beklenen risk kontrolüdür; oran ve slippage önemlidir |
| TP kurulduysa kâr garanti | TP order fill olmak zorundadır |
| Re-entry daha çok şans demektir | Re-entry kötü ortamda zarar tekrarına dönüşebilir |
| PTB SL token fiyatı SL ile aynı | PTB SL underlying gap bozulmasını izler |
| Time exit ve window end aynı şey | Biri pozisyon süresine, diğeri market kapanışına bakar |
