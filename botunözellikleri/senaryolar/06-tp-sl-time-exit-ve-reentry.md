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
