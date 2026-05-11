# Volatilite Capture Stratejisi — Sorun Tespiti

---

## Bolum 0 — Polymarket 5 Dakikalik Up/Down Marketleri Nasil Calisir?

### 0.1 Temel Mantik

Polymarket'te BTC, ETH, SOL gibi kripto varliklar icin **5 dakikalik** ve **15 dakikalik** prediksiyon (tahmin) piyasalari var.

**Soru sorulur:** "BTC son 5 dakikada yukseldi mi?"

**Iki cevap secenegi:**
- **Up (YES):** BTC yukseldi
- **Down (NO):** BTC dusmus veya ayni kalmis

```
Ornek:
  Pencere acilis: 12:35:00 UTC
  Pencere kapanis: 12:40:00 UTC

  BTC fiyati 12:35:00'da = $70,000 (bu PTB = Price-to-Beat)
  BTC fiyati 12:40:00'da = $70,050

  Sonuc: BTC yukseldi → Up (YES) kazanir, Down (NO) kaybeder
```

### 0.2 Token Yapisi

Her marketin iki tokeni vardir:

| Token | Ne Zaman $1.00 Deger | Ne Zaman $0.00 Deger |
|-------|----------------------|----------------------|
| **Up (YES)** | BTC pencere sonunda PTB'den yuksekse | BTC pencere sonunda PTB'den dusukse veya esitse |
| **Down (NO)** | BTC pencere sonunda PTB'den dusukse veya esitse | BTC pencere sonunda PTB'den yuksekse |

**Onemli:** Up ve Down tokenleri birbirinin tam tersidir. Biri $1.00 ise digeri $0.00.

### 0.3 PTB (Price-to-Beat) Nedir?

PTB = pencerenin acildigi saniyede o varligin (BTC/ETH/SOL) fiyati.

```
Ornek BTC 5m:
  12:35:00 - Pencere acildi
  PTB = $70,000.00 (tam bu saniyede BTC fiyati)

  12:40:00 - Pencere kapandi
  BTC son fiyat = $70,050

  Fark = +$50 (BTC yukseldi)
  Sonuc: Up = $1.00, Down = $0.00
```

```
Ornek ETH 5m:
  12:35:00 - PTB = $2,000.00
  12:40:00 - ETH son fiyat = $1,998.00
  Fark = -$2 (ETH dustu)
  Sonuc: Up = $0.00, Down = $1.00
```

```
Ornek (esit):
  12:35:00 - PTB = $70,000.00
  12:40:00 - BTC son fiyat = $70,000.00
  Fark = $0 (degismedi)
  Sonuc: Up = $0.00, Down = $1.00 (esitlikte Down kazanir)
```

### 0.4 Market Isimleri (Slug)

Her marketin benzersiz bir slug adi vardir:

```
btc-updown-5m-1773905700    → BTC 5 dakikalik, pencere baslangic timestamp'i
btc-updown-5m-1773906000    → BTC 5 dakikalik, bir sonraki pencere
btc-updown-15m-1773905700   → BTC 15 dakikalik
eth-updown-5m-1773905700    → ETH 5 dakikalik
sol-updown-5m-1773905700    → SOL 5 dakikalik
```

**5 dakikalik:** Her 5 dakikada yeni pencere acilir (gun boyu, araliksiz).
**15 dakikalik:** Her 15 dakikada yeni pencere acilir.

Gunluk pencere sayisi:
- 5m: 24 × 60 / 5 = **288 pencere/gun**
- 15m: 24 × 60 / 15 = **96 pencere/gun**

### 0.5 Token Fiyatlari Nasil Belirlenir?

Token fiyatlari **piyasanin beklentisini** yansitir. Ticaret yapan kisiler/botlar belirler:

```
BTC 12:35'te $70,000, 12:37'de $70,080'e cikti:
  Piyasa: "BTC yukseliyor, Up kazanacak" diyor
  Up token fiyati: 65c (%65 olasilikla yukselecek)
  Down token fiyati: 35c (%35 olasilikla dususecek)

BTC 12:38'de $69,950'ye dustu:
  Piyasa: "BTC geri geldi, Down kazanabilir" diyor
  Up token fiyati: 45c
  Down token fiyati: 55c
```

**Temel kural:** `Up fiyati + Down fiyati ≈ 1.00` (her zaman)

```
Up = 65c + Down = 35c = 100c ✓
Up = 50c + Down = 50c = 100c ✓
Up = 80c + Down = 20c = 100c ✓
```

Neden? Cunku biri $1.00, digeri $0.00 olacak. Toplam getiri her zaman $1.00.

### 0.6 Fiyat Hareketleri 5 Dakika Icinde

5 dakikalik bir pencerede BTC fiyati surekli hareket eder:

```
Dusuk volatilite penceresi:
  12:35:00 → BTC = $70,000  (PTB)
  12:36:00 → BTC = $70,010
  12:37:00 → BTC = $70,005
  12:38:00 → BTC = $70,015
  12:39:00 → BTC = $70,008
  12:40:00 → BTC = $70,012  (KAPANIS)
  Sonuc: Up kazandi (+$12)

  Up token fiyatlari:
  12:35 → 50c (belirsiz)
  12:36 → 52c (biraz yuksek)
  12:37 → 51c (geri geldi)
  12:38 → 53c
  12:39 → 52c
  12:40 → ~55c (yakin, yuksek ihtimal kazanir)

Yuksek volatilite penceresi:
  12:35:00 → BTC = $70,000  (PTB)
  12:36:00 → BTC = $70,200  ↑↑
  12:37:00 → BTC = $69,950  ↓↓
  12:38:00 → BTC = $70,150  ↑↑
  12:39:00 → BTC = $69,900  ↓↓
  12:40:00 → BTC = $70,100  ↑ (KAPANIS)
  Sonuc: Up kazandi (+$100)

  Up token fiyatlari:
  12:35 → 50c (belirsiz)
  12:36 → 75c (cok yuksek!)
  12:37 → 35c (cok dustu!)
  12:38 → 68c
  12:39 → 30c
  12:40 → ~62c (yuksek ihtimal kazanir)
```

### 0.7 Islemler (Nasil Alinir/Satilir?)

Polymarket bir **orderbook borsasi**dir:

```
Up Token Orderbook:
  ASKS (satislar):  55c × 100 adet | 54c × 50 adet | 53c × 200 adet
  --------------------------------- fiyat ----------------------------
  BIDS (alislar):   50c × 80 adet  | 49c × 120 adet | 48c × 60 adet

Almak istersen:
  Market buy (hemen): en dusuk ask'ten alirsin → 53c'den alirsin
  Limit buy @ 50c:   bid tarafina eklenir, biri 50c'den satarsa alirsin
  Limit buy @ 40c:   cok asagiya koyarsin, fiyat 40'a dusmedikce dolmaz
```

**Emir tipleri:**

| Tip | Aciklama | Garanti |
|-----|----------|---------|
| **GTC** (Good-Till-Canceled) | Emir dolana veya iptal edilene kadar acikta kalir | Zaman garantisi yok |
| **IOC** (Immediate-Or-Cancel) | Hemen doldurulabilen kismi al, kalanini iptal et | Kismi dolma olabilir |
| **FOK** (Fill-Or-Kill) | Ya tamamen dol ya da hic dolma | Tam dolma veya hic |

### 0.8 Fee (Komisyon) Yapisi

Polymarket'te alisveris yaparken komisyon alinir:

```
fee_rate_bps = 1000 (varsayilan)
fee_curve_rate = 1000 / 4000 = 0.25

Fee miktari token fiyatina gore degisir:
  50c fiyatta: daha yuksek fee (fiyat ortada, belirsizlik yuksek)
  90c fiyatta: daha dusuk fee (fiyat yuksek, belirsizlik dusuk)
  10c fiyatta: daha dusuk fee (fiyat dusuk, belirsizlik dusuk)

Ornek: 10 share Up @ 50c alirsan:
  Gross: 10 share
  Fee: ~0.31 share
  Net: ~9.69 share (elde tutulan)
```

**Fee satis yaparken de alinir.** Toplam fee kaybi islem basina ~%2-4.

### 0.9 Resolve (Sonuclanma)

Pencere kapandiginda:

```
BTC kapanis fiyati PTB'den yuksekse:
  Up token → $1.00 degerinde olur (otomatik)
  Down token → $0.00 degerinde olur (otomatik)

BTC kapanis fiyati PTB'den dusukse veya esitse:
  Up token → $0.00 degerinde olur (otomatik)
  Down token → $1.00 degerinde olur (otomatik)
```

Elinde 9.69 share Up varsa ve Up kazandiysa → **$9.69 alirsin.**
Elinde 12.05 share Down varsa ve Down kazandiysa → **$12.05 alirsin.**

### 0.10 Ozet: Para Akisi

```
1. Bot Up token alir: 10 share × 50c = $5.00 odenir
2. Bot Down token alir: 10 share × 40c = $4.00 odenir
3. Toplam maliyet: $9.00

4. Pencere kapanir, BTC yukseldi:
   Up → $1.00 × 9.69 (fee sonrasi) = $9.69 alir
   Down → $0.00 × 12.05 = $0.00
   Toplam donus: $9.69

5. Net kar: $9.69 - $9.00 = +$0.69 (fee oncesi +$1.00)
```

---

## Bolum 1 — Bot'un Elindeki Araclar (action.place_order Node Ozellikleri)

Bot'ta mevcut **trade flow** sistemi graph tabanlidir. Her islem akisi **node**'lardan olusur.
Node'lar birbirine baglanir, veri akisi saglanir. Temel iki node tipi:

1. **trigger.market_price** — Fiyat dinler, kosul saglaninca tetikler
2. **action.place_order** — Emir gonderir (alis/satis, market/limit)

```
Ornek akis:
  [trigger.market_price] → [action.place_order]
       (fiyat kosulu)        (emir gonder)
```

---

### 1.1 action.place_order — Temel Parametreler

| Parametre | Aciklama | Ornek Deger |
|---|---|---|
| `side` | Alis veya satis | `"buy"` veya `"sell"` |
| `executionMode` | Market veya limit emir | `"market"` veya `"limit"` |
| `marketSlug` | Hangi market | `"btc-updown-5m-1773905700"` |
| `tokenId` | Hangi token (Up veya Down) | `"12345..."` |
| `outcomeLabel` | Up veya Down | `"Up"` veya `"Down"` |
| `sizeUsdc` | Ne kadar USDC harcanacak | `5.0` ($5) |
| `sizeMode` | Boyut modu | `"usdc"`, `"pct"`, `"shares"` |
| `sizePct` | Onceki trade'in yuzdesi olarak | `50.0` (%50'si) |
| `executionFloorPriceCent` | Minimum alis fiyati | `0` (sinir yok) |
| `maxPriceCent` | Maksimum alis fiyati | `80` (80 centten fazla verme) |
| `triggerCondition` | Ne zaman tetiklensin | `"cross_above"`, `"cross_below"` |
| `triggerPriceCent` | Hangi fiyatta tetiklensin | `80` (80 centi gecince) |

---

### 1.2 PTB Guard (Price-to-Beat Korumasi)

Emir gonderilmeden once bot PTB guard kontrolu yapar. Bu, **giris kalitesini** artirmak icin var.

#### Ne yapar?

```
PTB = $70,000 (pencere baslangic BTC fiyati)
Anlik Chainlink fiyati = $70,003
Gap = $3

Threshold = $2

Soru: $3 >= $2 mu?
Cevap: EVET → PASSED → emir verilir
Cevap: HAYIR → BLOCKED → emir verilmez, bekler
```

#### Config Parametreleri

| Parametre | Aciklama | Ornek |
|---|---|---|
| `priceToBeatGuardEnabled` | PTB guard aktif mi | `true` |
| `priceToBeatMode` | Manual veya otomatik | `"manual"`, `"auto_last_3_avg_excursion"`, `"auto_vol_pct"` |
| `priceToBeatMaxDiff` | Manual modda gap esigi (USD) | `2.0` |
| `priceToBeatMaxDiffUnit` | Birim | `"usd"` |
| `retryOnPriceToBeatGuardBlock` | Bloklaninca tekrar dene mi | `true` |
| `notifyOnPriceToBeatGapBlocked` | Telegram bildirimi | `true` |

#### 3 PTB Modu

**Manual:** Sabit threshold. Ornegin her zaman $2 fark gerekli.

**auto_last_3_avg_excursion:** Son 3 tamamlanmis pencerenin ortalama hareketini threshold yapar.
```
Son 3 pencere (Up yonu):
  W-1: high - open = $5
  W-2: high - open = $3
  W-3: high - open = $4
  Ortalama = $4 → threshold = $4
```

**auto_vol_pct:** Volatiliteye gore dinamik threshold. (Planlanmis, dokuman: `problemler/ptb-threshold-problemi.md`)

---

### 1.3 Trigger Price Guard (Fiyat Tetik Korumasi)

Alis emirlerinde **fiyat belirli bir seviyenin altindaysa** girmeyi engeller.

```
Ornek: Up token fiyati 45 cent
guard_trigger_price = 55 cent

45 cent < 55 cent → BLOCKED (fiyat cok dusuk, girme)
```

| Parametre | Aciklama |
|---|---|
| `guardTriggerPrice` | Alt sinir fiyati (cent) |
| `retryOnTriggerGuardBlock` | Bloklaninca bekle |

---

### 1.4 Max Price Guard (Maksimum Fiyat Korumasi)

Alis emirlerinde **fiyat belirli bir seviyenin ustundeyse** girmeyi engeller.

```
Ornek: Up token fiyati 85 cent
maxPrice = 80 cent

85 cent > 80 cent → BLOCKED (cok pahali, alma)
```

| Parametre | Aciklama |
|---|---|
| `maxPrice` | Ust sinir fiyati (cent) |
| `retryOnMaxPriceBlock` | Bloklaninca bekle |

---

### 1.5 Execution Floor Guard (Best Ask Tabani)

Alis emirlerinde **best_ask belirli bir seviyenin altindaysa** girmeyi engeller.

```
Ornek: Best ask = 45 cent
best_ask_floor_price = 50 cent

45 cent < 50 cent → BLOCKED (ask cok dusuk, orderbook kotu)
```

| Parametre | Aciklama |
|---|---|
| `executionFloorGuardEnabled` | Aktif mi |
| `executionFloorPriceCent` | Best ask tabani |
| `retryOnExecutionFloorGuardBlock` | Bloklaninca bekle |

---

### 1.6 Take Profit (TP) — Kar Al

Alis emri doldugunda otomatik olarak **satim emri** olusturur.

#### Hard TP (Tek Seviye)

```
Up@50 alindi → TP @ 98 cent olusturuldu
Fiyat 98'i gecince (cross_above) → SAT @ 98 cent
Kar: 98 - 50 = 48 cent per share
```

| Parametre | Aciklama |
|---|---|
| `tpEnabled` | TP aktif mi |
| `tpPriceCent` | TP fiyati (cent) |

#### Staged TP (Kademeli)

Birden fazla TP kademesi:

```
TP-0: SAT @ 70c, %50 pozisyon
TP-1: SAT @ 85c, %30 pozisyon
TP-2: SAT @ 98c, %20 pozisyon

Not: TP fiyatlari yukselen sirada olmali
     sizePct toplami %100 olmali
```

| Parametre | Aciklama |
|---|---|
| `tpRules` | Kademeli TP kurallari (JSON array) |

```json
{
  "tpRules": [
    { "priceCent": 70, "sizePct": 50 },
    { "priceCent": 85, "sizePct": 30 },
    { "priceCent": 98, "sizePct": 20 }
  ]
}
```

---

### 1.7 Stop Loss (SL) — Zarar Kes

Alis emri doldugunda otomatik olarak **satim emri** olusturur.

#### Hard SL (Tek Seviye)

```
Up@50 alindi → SL @ 35 cent olusturuldu
Fiyat 35'in altina inince (cross_below) → SAT @ market price
Zarar: 50 - 35 = 15 cent per share (gercekte daha fazla, slippage var)
```

| Parametre | Aciklama |
|---|---|
| `slEnabled` | SL aktif mi |
| `slPriceCent` | SL fiyati (cent) |

#### Staged SL (Kademeli)

Birden fazla SL kademesi:

```
SL-0: SAT @ 50c, %50 pozisyon (cross_below 50)
SL-1: SAT @ 40c, %50 pozisyon (cross_below 40)

Not: SL fiyatlari dusen sirada olmali
     sizePct toplami %100 olmali
```

| Parametre | Aciklama |
|---|---|
| `slRules` | Kademeli SL kurallari (JSON array) |

```json
{
  "slRules": [
    { "priceCent": 50, "sizePct": 50 },
    { "priceCent": 40, "sizePct": 50 }
  ]
}
```

#### SL Trigger Fiyat Modu

SL tetiklendiginde hangi fiyat kaynagini kullanacagini belirler:

| Mod | Aciklama | Davranis |
|---|---|---|
| `best_bid` (varsayilan) | Sadece best_bid | Hizli tetik, flash crash'e duyali |
| `composite` | best_bid + last_trade filtreler | Dengeli |
| `composite_safe` | Iki kaynak da SL altinda olmali | En guvenli, gec tetik |
| `composite_fast` | best_bid ve last_trade minimumu | Agresif |
| `last_trade` | Sadece son trade fiyati | Nadir |

---

### 1.8 PTB Stop Loss (Fiyat Bazli Zarar Kes)

Sabit fiyat SL yerine **PTB gap** bazli SL. Karsi token fiyatina bakmadan, underlying fiyat ile PTB referansi arasindaki directional gap'i izleyip sat.

#### Mantik

```
Up token alindi (BTC yukselecek seklinde bahis)
PTB = $70,000

BTC dustu: $70,000 → $69,998
directional_gap (Up icin) = $69,998 - $70,000 = -$2

threshold_gap_usd = $0 (sifir, yani gap sifira inince sat)

-$2 <= $0 → EVET → SATIS YAP
BTC daha dustu: $69,995
directional_gap (Up icin) = $69,995 - $70,000 = -$5

-$5 <= $0 → EVET → SATIS YAP
```

#### Config

| Parametre | Aciklama | Ornek |
|---|---|---|
| `ptbStopLossEnabled` | PTB SL aktif mi | `true` |
| `ptbStopLossGapUsd` | Directional gap threshold (USD). Negatif deger karsi yone overshoot bekler | `0.0`, `5.0`, `-20.0` |
| `ptbStopLossTimeDecayMode` | Zamanla threshold degisimi | `"tighten"`, `"relax"`, `"none"` |
| `ptbStopLossRules` | Kademeli PTB SL | JSON array |

#### Time Decay Modlari

```
Pencere suresi: 300sn

tighten (varsayilan): Threshold zamanla azalir
  Baslangic: threshold = $5
  150sn gecti: threshold = $5 × (1 - 0.5) = $2.50
  270sn gecti: threshold = $5 × (1 - 0.9) = $0.50
  Sonuc: Gectikce daha hassas, erken cikis

relax: Threshold zamanla artar
  Baslangic: threshold = $5
  150sn gecti: threshold = $5 × (1 + 0.5) = $7.50
  Sonuc: Gectikce daha toleransli, gec cikis

none: Sabit threshold
  Her zaman $5

Not: Negatif thresholdlerde time decay uygulanmaz.
```

#### Kademeli PTB SL

```json
{
  "ptbStopLossRules": [
    { "gapUsd": 12.5, "sizePct": 25 },
    { "gapUsd": 3.0, "sizePct": 75 }
  ]
}
```

```
Gap $12.5'in altina dusunce → %25 pozisyonu sat
Gap $3.0'in altina dusunce → kalan %75'i sat

`-10` ornegi:
- Up/Yes icin fiyat PTB referansinin $10 altina indiginde sat
- Down/No icin fiyat PTB referansinin $10 ustune ciktiginda sat
```

---

### 1.9 Re-Entry (Yeniden Giris)

SL tetiklendiginde otomatik olarak **ayni piyasadan tekrar giris** yapabilir.

```
Up@50 al → SL@35 atesler → -$15c zarar
Re-entry: Up@50 al (AYNI parametreler) → SL@35 atesler → -$15c zarar
Re-entry: Up@50 al (AYNI parametreler) → TP@98 dolar → +48c kar

Net: -15 + (-15) + 48 = +18c (2 zarar 1 karden kurtuldu)
```

| Parametre | Aciklama | Ornek |
|---|---|---|
| `reenterOnSlHit` | SL'den sonra re-entry yap | `true` |
| `reentryMaxAttempts` | Max re-entry sayisi | `2` (1-10 arasi) |

**Dikkat:** Re-entry ayni parametrelerle yapilir (ayni maxPrice, ayni sizeUsdc). SL olmus piyasada ayni sartlarla tekrar girmek genellikle ayni sonucu verir.

---

### 1.10 Time Exit (Zaman Bazli Cikis)

Belirli sure gectikten sonra pozisyonun bir kismini veya tamamini otomatik sat.

```json
{
  "timeExitRules": [
    { "elapsedMinutes": 3, "remainingPct": 50 },
    { "elapsedMinutes": 4, "remainingPct": 100 }
  ]
}
```

```
3. dakikada: Pozisyonun %50'sini sat
4. dakikada: Kalan %100'unu sat (hepsini)
```

| Parametre | Aciklama |
|---|---|
| `timeExitRules` | Kademeli zaman cikis kurallari |
| `elapsedMinutes` | Kac dakika gecti |
| `remainingPct` | Pozisyonun kac yuzdile satilacak |

---

### 1.11 Underlying Protection (Spot Fiyat Korumasi)

Alis yapmadan once spot piyasadaki fiyat hareketini kontrol eder. Cok hizli dusus/yukseliste girmez.

```
BTC spot 10 saniyede %0.5 dustu:
  protection_mode = "divergence"
  delta_10s_pct = 0.5%
  threshold = 0.3%

  0.5% > 0.3% → BLOCKED (cok hizli hareket, girme)
```

---

### 1.12 Staged SL Re-Entry Ayarlari

Staged SL'de re-entry davranisini kontrol eder:

| Parametre | Aciklama |
|---|---|
| `stagedSlReentryOnlyAfterAllStages` | Tum SL kademeleri bitmeden re-entry yapma |
| `stagedSlRetryOnlyDust` | Sadece toz (cok kucuk) pozisyonlari tekrar dene |
| `stagedSlReentryUseSoldNotional` | Satilan miktar kadar yeniden gir |

---

### 1.13 Window End Auto Sell (Pencere Sonu Otomatik Satis)

Pencere kapanirken bekleyen alis emri varsa otomatik olarak satis yapar.

```
Up@50 alindi, pencere kapanisina 30sn kala:
  windowEndAutoSell = true
  → Up token market price satilir
```

---

### 1.14 Bildirimler (Telegram)

Her kritik durumda Telegram bildirimi gonderebilir:

| Parametre | Ne Zaman |
|---|---|
| `notifyOnFill` | Emir doldugunda |
| `notifyOnOrderNotFilled` | Emir dolmadiysa |
| `notifyOnTriggerGuardBlocked` | Trigger guard blokladiysa |
| `notifyOnExecutionFloorBlocked` | Execution floor blokladiysa |
| `notifyOnTpHit` | TP tetiklendiginde |
| `notifyOnSlHit` | SL tetiklendiginde |
| `notifyOnMaxPriceBlocked` | Max price blokladiysa |

---

### 1.15 Ozet: action.place_order Tum Parametreler

```toml
# action.place_order node config ornegi

side = "buy"
executionMode = "market"
outcomeLabel = "Up"
sizeUsdc = 5.0

# Alis korumalari
maxPriceCent = 80
executionFloorGuardEnabled = true
executionFloorPriceCent = 0

# PTB Guard
priceToBeatGuardEnabled = true
priceToBeatMode = "manual"
priceToBeatMaxDiff = 2
priceToBeatMaxDiffUnit = "usd"
retryOnPriceToBeatGuardBlock = true

# Take Profit
tpEnabled = true
tpPriceCent = 98

# veya staged TP:
# tpRules = [{ priceCent = 70, sizePct = 50 }, { priceCent = 98, sizePct = 50 }]

# Stop Loss
slEnabled = true
slPriceCent = 35
slTriggerPriceMode = "composite_safe"

# veya staged SL:
# slRules = [{ priceCent = 50, sizePct = 50 }, { priceCent = 40, sizePct = 50 }]

# PTB Stop Loss
ptbStopLossEnabled = true
ptbStopLossGapUsd = 0
ptbStopLossTimeDecayMode = "tighten"

# Re-entry
reenterOnSlHit = true
reentryMaxAttempts = 2

# Time Exit
# timeExitRules = [{ elapsedMinutes = 3, remainingPct = 50 }]

# Window End
windowEndAutoSell = false

# Bildirimler
notifyOnFill = true
notifyOnSlHit = true
notifyOnTpHit = true
```

---

## Strateji Ozeti

5 dakikalik Up/Down marketinde:
1. **Up limit buy @ 50c** koy (hemen dolar, piyasa acilista ~50/50)
2. **Down limit buy @ 40c** koy (bekler, fiyat 40'a dusunce dolar)
3. Her ikisi dolunca → toplam maliyet 90c → resolution'ta garanti $1.00 → **10c garantili kar**

Fiyat kombinasyonlari (hepsi 90c toplam, 10c garanti):

| Up Fiyat | Down Fiyat | Toplam | Garanti Kar |
|----------|------------|--------|-------------|
| 50c | 40c | 90c | 10c |
| 60c | 30c | 90c | 10c |
| 70c | 20c | 90c | 10c |
| 45c | 45c | 90c | 10c |

---

## Sorun 1 — Tek Taraf Dolma Riski

### Senaryo

Up@50 limit hemen doldu. Down@40 limit 5 dakika boyunca dolmadi.

```
T+0sn:  Up@50 DOLDU, Down@40 beklemede
T+60sn: YES=55, NO=45 → Down@40 henuz dolmadi
T+120sn: YES=58, NO=42 → hala dolmadi
T+240sn: YES=52, NO=48 → NO yukseldi, hic dolmayacak gibi
T+300sn: Pencere kapandi
```

### Sonuc

- Up token'i resolution'ta ya $1.00 ya $0.00 olur
- Eger Up kaybederse → 50c kayip (Down@40 dolmadigi icin koruma yok)
- Eger Up kazansa → 50c kar (100c - 50c maliyet), ama bu garanti degil

### Risk

Tek taraf dolma durumunda, bot duz bir bahis yapmis olur. Garantili kar sansi ortadan kalkar.

---

## Sorun 2 — SL Konamaz

### Neden?

SL koyarsan:

```
Up@50 alindi, SL@35 konuldu.
Fiyat dusmeye basladi:
  YES: 50 → 45 → 40 → 35 → SL ATESLER → -15c ZARAR
  NO:  50 → 55 → 60 → 65 → Down@40 ASLA DOLMAZ
```

**SL atesleyen fiyat hareketi, diger tarafi UZAKLASTIRIR.**

- YES duserse → SL atesler → NO yukselir → Down@40 dolmaz
- YES yukselirse → SL ateslemez → NO duser → Down@40 dolar → ikisi dolunca garanti kar

Yani SL sadece "kotu senaryo"da atesler ve o kotu senaryoda zaten diger taraf dolmazdi. SL zarari garanti eder, kar degil.

### Sonuc

Bu stratejide **stop loss YASAK**. Tek risk yonetimi: zaman bazli flatten (pencere kapanisina X saniye kala dolmayan iptal + dolani sat).

---

## Sorun 3 — Her Zaman 40'a Dusmez

### Sorun

5 dakikalik pencerede BTC/ETH yeterince hareket etmezse, fiyat 50/50 civarinda kalir ve Down hic 40'a dusmez.

### Ornekler

```
Dusuk volatilite:
  BTC 5m: 70,000 → 70,020 → 70,010 → 70,015 → 70,005
  YES: 50 → 52 → 51 → 51 → 50
  NO:  50 → 48 → 49 → 49 → 50
  Down hic 40'a dusmedi → strateji calismadi

Yuksek volatilite:
  BTC 5m: 70,000 → 70,150 → 69,980 → 70,100 → 70,080
  YES: 50 → 72 → 38 → 65 → 62
  NO:  50 → 28 → 62 → 35 → 38
  Down@40 doldu (NO 35'e dustu) → garanti kar
```

### Istatistiksel Olasilik

5 dakikalik BTC marketlerinde:
- YES en az 60'a cikma olasiligi: ~%40-50
- NO en az 40'a dusme olasiligi: ~%40-50 (ayni sey, ters yon)
- **Her iki taraf da 10c+ salinim gosterme olasiligi: ~%35-45**

Yani her 3 pencereden 1'inde bu strateji calisir. 2'sinde tek taraf dolma riski vardir.

---

## Sorun 4 — Fee Etkisi

### Polymarket Fee Yapisi

```
fee_rate_bps = 1000 (varsayilan)
fee_curve_rate = 1000 / 4000 = 0.25
fee_shares = gross_qty * 0.25 * (price * (1 - price))^2 / price
```

### Hesaplama

```
Up@50 al: 10 share × 50c = $5.00 USDC
  fee_shares ≈ 10 × 0.25 × (0.50 × 0.50)^2 / 0.50 = 10 × 0.25 × 0.0625 / 0.50 = 0.31 share
  net_share = 10 - 0.31 = 9.69 share

Down@40 al: 12.5 share × 40c = $5.00 USDC
  fee_shares ≈ 12.5 × 0.25 × (0.40 × 0.60)^2 / 0.40 = 12.5 × 0.25 × 0.0576 / 0.40 = 0.45 share
  net_share = 12.5 - 0.45 = 12.05 share

Resolution: Kazanan taraf $1.00 × net_share
  Eger Up kazansa: 9.69 × $1.00 = $9.69 (maliyet $10.00) → -$0.31
  Eger Down kazansa: 12.05 × $1.00 = $12.05 (maliyet $10.00) → +$2.05

BEKLENEN: 0.5 × 9.69 + 0.5 × 12.05 = 10.87 → +$0.87 kar? HAYIR.

Gercek: Her iki taraf da resolve oldugunda:
  Up=1.00 ise: 9.69 share × $1.00 = $9.69
  Down=0.00 ise: 12.05 share × $0.00 = $0.00
  Toplam donus: $9.69 (maliyet $10.00) → -$0.31 ZARAR!
```

### Fee Sonucu

**Gross kar 10c ama fee ~2-4c yer. Net kar 6-8c.** Fee hesaba katilmazsa strateji karsiz gorunebilir.

Ayrica satista da fee var. Up@50 alip 60'a satmak istersen:
```
Alis: 9.69 net share
Satis: 9.69 × 60c = $5.81 ama satis fee de var
Net satis: ~$5.60
Kar: $5.60 - $5.00 = $0.60 (gross 81c degil)
```

---

## Sorun 5 — Orderbook Likiditesi

### Sorun

Limit order 40'a konur ama o seviyede:
1. **Maker emir olmayabilir** → emir acikta kalir, kimse karsilik vermez
2. **Baska botlar da ayni fiyatta bekliyor olabilir** → sirada beklersin
3. **Spread genis olabilir** → bid@38, ask@42 arasinda, 40'a limit koyarsan ask tarafina denk gelmeyebilir

### Ornek Orderbook

```
Down token orderbook:
  Asks: 45@0.02  44@0.05  43@0.10  42@0.15  41@0.20  40@0.50
  Bids: 39@0.30  38@0.40  37@0.20

Limit buy @ 40 koy → 0.50 share mevcut → DOLAR
Limit buy @ 40 koy ama orderbook:
  Asks: 45@0.02  44@0.05  43@0.10  42@0.15  41@0.20
  Bids: 40@0.10  39@0.30
  → 40'a kimsenin satisi yok → DOLMAZ
```

### Sonuc

Emrin dolmasi icin o fiyatta birinin satmak istemesi lazim. Her zaman garanti degil.

---

## Sorun 6 — Zamanlama ve Acilis Fiyati

### Sorun

Pencere acildiginda fiyat her zaman 50/50 degil.

### Senaryolar

```
Senaryo A - Dengeli acilis:
  Pencere acildi: YES=50, NO=50
  Limit Up@50 → hemen DOLUR
  Limit Down@40 → bekler
  Sonuc: Strateji calisir

Senaryo B - Yonlu acilis:
  Pencere acildi: YES=65, NO=35 (BTC zaten yuksek)
  Limit Up@50 → YES zaten 65'te, 50'ye dusmesini bekle → RISKLI
  Limit Down@40 → NO zaten 35'te, 40'IN ALTINDA → hemen DOLUR

  AMA: Down@40 dolar, Up@50 dolmaz → TEK TARAFLI pozisyon

Senaryo C - Agresif acilis:
  Pencere acildi: YES=80, NO=20
  Limit Up@50 → 30c dusus gerekiyor → cok zor
  Limit Down@40 → NO zaten 20'de → hemen DOLUR
  Sonuc: Sadece Down@40 dolar, Up@50 dolmaz → tek taraf riski
```

### Sonuc

Acilis fiyati 50/50'ye yakin degilse, strateji otomatik olarak tek tarafli pozisyon alir. Bot'un acilis fiyatini kontrol edip limit fiyatlari buna gore ayarlamasi gerekir.

---

## Sorun 7 — Fiyat Kombinasyonu Secimi

### Sorun

50+40=90, ama 45+45=90 da, 55+35=90 da calisir. Hangisi en iyi?

### Karsilastirma

| Kombinasyon | Up Dolma Sansi | Down Dolma Sansi | Her Ikisi | Not |
|---|---|---|---|---|
| 50 + 40 | Yuksek (%80) | Orta (%45) | %36 | Dengeli |
| 45 + 45 | Orta (%60) | Orta (%60) | %36 | Simetrik |
| 55 + 35 | Yuksek (%70) | Dusuk (%30) | %21 | Agresif Up |
| 60 + 30 | Orta (%50) | Dusuk (%20) | %10 | Cok agresif |
| 70 + 20 | Dusuk (%30) | Cok dusuk (%10) | %3 | Neredeyse imkansiz |

*(Yuzdeler tahmini, 5m BTC volatilitesine dayanir)*

### Sonuc

**45+45 simetrik** veya **50+40 hafif asimetrik** en iyi secenek. 60+30 ve otesi cok agresif, her iki taraf dolma olasiligi cok dusuk.

---

## Sorun 8 — Flatten Zamani

### Sorun

Pencere kapanisina kac saniye kala dolmayan emri iptal edip dolani satmali?

### Erken Flatten (60sn once)

```
T+240sn: Sadece Up@50 doldu, <60sn kaldi
  → Down@40 iptal, Up market price sat
  → Up 52'de → +2c kar (kucuk ama pozitif)
  → Up 48'de → -2c zarar (kucuk)
```

**Arti:** Kucuk kayiplarla cikis, buyuk kayip riski yok
**Eksi:** 60sn icinde Down@40 dolabilirdi, kacirdik

### Gec Flatten (15sn once)

```
T+285sn: Sadece Up@50 doldu, <15sn kaldi
  → Down@40 iptal, Up market price sat
  → Up 45'te → -5c zarar (buyuk)
  → Up 55'te → +5c kar
```

**Arti:** Daha fazla bekleme suresi, Down@40 dolma sansi artar
**Eksi:** Son saniyada fiyat hizli hareket edebilir, buyuk kayip riski

### Optimum

```
Pencere suresi: 300sn
Onerilen flatten: 30-45sn once

30sn once → hala 30sn var, fiyat nispeten stabil
45sn once → daha guvenli, ama firsat kacirma riski
```

---

## Sorun 9 — Pencere Boyutu ve Volatilite Eslestirmesi

### Sorun

5m marketlerde volatilite her zaman ayni degil. Dusuk volatilitede 50→40 salinim olmayabilir.

### Volatiliteye Gore Beklenen Sonuclar

```
Dusuk volatilite (BTC 5m range < $30):
  YES salinimi: 48-52 arasi
  Down@40 dolma olasiligi: ~%5
  Sonuc: Strateji neredeyse hic calismaz

Normal volatilite (BTC 5m range $50-150):
  YES salinimi: 40-60 arasi
  Down@40 dolma olasiligi: ~%40
  Sonuc: Strateji 3'te 1 calisir

Yuksek volatilite (BTC 5m range > $200):
  YES salinimi: 25-75 arasi
  Down@40 dolma olasiligi: ~%70
  Sonuc: Strateji sik calisir
```

### Sonuc

Dusuk volatiliteli saatlerde (ornegin UTC 18:00-19:00, bot mevcut verilerde %0 TP orani gosteriyor) bu strateji calismaz. Volatilite filtresi gerekli.

---

## Sorun 10 — Coklu Pencere Kapma

### Sorun

Ayni anda birden fazla 5m market var (BTC, ETH, SOL). Her birinde vol capture calistirilirsa:

```
BTC 5m: Up@50 + Down@40 → $5 USDC
ETH 5m: Up@50 + Down@40 → $5 USDC
SOL 5m: Up@50 + Down@40 → $5 USDC

Toplam bagli sermaye: $15 USDC
Her ikisi dolma olasiligi (her market icin): ~%35
En az 1 markette her iki taraf dolma: ~%73
Hicbirinde dolmama: ~%27
```

### Risk

Birden fazla markette ayni anda tek taraf dolma olursa → toplam $15 pozisyon, hepsi resolution riski tasir. Seraye yonetimi kritik.

---

## Ozet: Stratejinin EV (Beklenen Deger) Analizi

```
Varsayimlar:
  - Up@50 her zaman dolar (cunku piyasa acilista ~50)
  - Down@40 dolma olasiligi: %40
  - Her ikisi dolunca: +10c kar (fee sonrasi ~+7c)
  - Sadece Up dolunca + flatten: ortalama -3c kayip

EV per pencere:
  = (0.40 × +7c) + (0.60 × -3c)
  = +2.8c - 1.8c
  = +1.0c per pencere

Gunluk (BTC 5m: 288 pencere):
  = 288 × 1.0c = ~$2.88/gun (BTC basina)

Aylik: ~$86/gun (BTC 5m tek basina)
```

**Not:** Bu hesaplama kabataslaktir. Gercek dolma oranlari, fee, slippage ve flatten kayiplari sonucu degistirebilir.
