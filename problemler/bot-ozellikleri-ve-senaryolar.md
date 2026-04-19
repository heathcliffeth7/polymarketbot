# Bot Ozellikleri — Ornek Senaryolarla Aciklama

Her ozellik icin 3 farkli senaryo var. Underlying Protection haric.

---

## 1. PTB Guard (Price-to-Beat Korumasi)

**Ne yapar:** Emir gondermeden once BTC/ETH fiyatinin PTB'den ne kadar uzaklastigini kontrol eder. Gap (fark) yeterli degilse emir verilmez.

### Senaryo 1: Gap yeterli → PASSED

```
Market: btc-updown-5m-1773905700
PTB = $70,000 (pencere acilis fiyati)
Anlik BTC = $70,005 (Chainlink)
Gap = $5

PTB Mode = manual, threshold = $3

$5 >= $3 → PASSED → emir gonderilir
Up token fiyati = 55c (BTC biraz yuksek, Up biraz favori)
Bot: Up @ 55c alir
```

### Senaryo 2: Gap yetersiz → BLOCKED, bekler

```
Market: eth-updown-5m-1774013100
PTB = $2,000
Anlik ETH = $2,000.50
Gap = $0.50

PTB Mode = manual, threshold = $2

$0.50 < $2 → BLOCKED → emir verilmez
Bot bekler. ETH hareket edip gap $2'ye ulasana kadar tekrar dener.

retryOnPriceToBeatGuardBlock = true → beklemeye devam
notifyOnPriceToBeatGapBlocked = true → Telegram: "PTB guard blokladi, gap=$0.50, threshold=$2"
```

### Senaryo 3: auto_last_3_avg_excursion modu

```
Market: eth-updown-5m-1774013400
PTB = $2,000
Anlik ETH = $2,004

Son 3 pencerenin hareketleri:
  W-1: open=2000, high=2006 → up excursion = $6
  W-2: open=2003, high=2008 → up excursion = $5
  W-3: open=1999, high=2004 → up excursion = $5
  Ortalama = ($6 + $5 + $5) / 3 = $5.33

PTB Mode = auto_last_3_avg_excursion
Otomatik threshold = $5.33

Gap = $4
$4 < $5.33 → BLOCKED

Gap = $6
$6 >= $5.33 → PASSED
```

---

## 2. Max Price Guard (Maksimum Fiyat Korumasi)

**Ne yapar:** Alis emrinde token fiyati belirli bir seviyenin ustundeyse girmez. "Bu fiyattan pahaliya alma" demek.

### Senaryo 1: Fiyat uygun → alir

```
Up token best ask = 72c
maxPriceCent = 80

72c < 80c → PASSED → emir gonderilir
Bot: Up @ 72c alir, $5 USDC harcar
```

### Senaryo 2: Fiyat pahali → almaz

```
Up token best ask = 85c
maxPriceCent = 80

85c > 80c → BLOCKED
retryOnMaxPriceBlock = true → bekler, fiyat 80'in altina duserse tekrar dener
retryOnMaxPriceBlock = false → emir iptal edilir
```

### Senaryo 3: Fiyat degisiyor → once blok, sonra gec

```
T+0sn:  Up ask = 82c → BLOCKED (82 > 80)
T+30sn: Up ask = 81c → BLOCKED (81 > 80)
T+60sn: Up ask = 79c → PASSED (79 < 80) → emir gonderilir!

retryOnMaxPriceBlock = true oldugu icin bot bekledi ve firsati yakaladi.
```

---

## 3. Execution Floor Guard (Best Ask Tabani)

**Ne yapar:** Best ask fiyati belirli bir seviyenin altindaysa girmez. Orderbook kotu durumda demek.

### Senaryo 1: Orderbook saglikli → gecer

```
Up token orderbook:
  Best ask = 65c, likidite: 200 share
  executionFloorPriceCent = 50

65c > 50c → PASSED
```

### Senaryo 2: Orderbook cok kotu → bloklar

```
Up token orderbook:
  Best ask = 42c, likidite: 2 share (cok az!)
  executionFloorPriceCent = 50

42c < 50c → BLOCKED
Neden? Fiyat cok dusukse piyasa bir sey biliyor olabilir, veya likidite cok az.
Bot: "Bu markette is yapmam, cok riskli" der.
```

### Senaryo 3: Best ask yok → bloklar

```
Up token orderbook:
  Hic ask yok (boss)
  Best ask = None

executionFloorGuardEnabled = true
executionFloorPriceCent = 50

Best ask yok → BLOCKED
Neden? Orderbook tamamen bos, kimse satmiyor.
```

---

## 4. Trigger Price Guard (Fiyat Tetik Korumasi)

**Ne yapar:** Bot sadece fiyat belirli bir seviyenin ustundeyken girer. "Fiyat bu seviyeye gelirse gir" demek.

### Senaryo 1: Fiyat ustunde → gecer

```
Up token fiyati = 62c
guardTriggerPrice = 55c

62c > 55c → PASSED → emir gonderilir
```

### Senaryo 2: Fiyat altinda → bloklar

```
Up token fiyati = 48c
guardTriggerPrice = 55c

48c < 55c → BLOCKED
Neden? Fiyat cok dusuk, piyasa Down lehine hareket ediyor.
Bot: "Fiyat 55'e yukselene kadar bekle" der.
```

### Senaryo 3: Fiyat yukselir → gec gecer

```
T+0sn:   Up = 45c → BLOCKED (45 < 55)
T+30sn:  Up = 50c → BLOCKED (50 < 55)
T+60sn:  Up = 56c → PASSED (56 >= 55) → emir gonderilir

retryOnTriggerGuardBlock = true oldugu icin bot bekledi.
```

---

## 5. Take Profit (TP) — Kar Al

**Ne yapar:** Alis emri doldugunda otomatik satim emri olusturur. Fiyat hedefe ulasinca satar.

### Senaryo 1: Hard TP — Tek seviye

```
Alis: Up @ 60c, 10 share, $6.00
tpEnabled = true, tpPriceCent = 98

Fiyat 98'i gecince:
  SAT Up @ 98c, 10 share → $9.80 alir
  Kar: $9.80 - $6.00 = +$3.80

TP fiyat tabani (exit_price_capped):
  floor = 0.98 - 0.05 = 0.93
  Emir fiyati 0.93'ten asagi olamaz.
  Eger best bid 0.90 ise → satim 0.93'den yapilir (capped)
```

### Senaryo 2: Staged TP — Kademeli

```
Alis: Up @ 50c, 10 share, $5.00
tpRules = [
  { priceCent: 70, sizePct: 40 },   → 4 share @ 70c = $2.80
  { priceCent: 85, sizePct: 30 },   → 3 share @ 85c = $2.55
  { priceCent: 98, sizePct: 30 }    → 3 share @ 98c = $2.94
]

Toplam satis: $2.80 + $2.55 + $2.94 = $8.29
Kar: $8.29 - $5.00 = +$3.29

Neden staged? Ilk kademe erken satis → garanti mini kar.
             Son kademe yuksek fiyat → buyuk kar sansi.
```

### Senaryo 3: TP trigger olmazsa — pencere kapanir

```
Alis: Up @ 55c, 10 share
tpEnabled = true, tpPriceCent = 98

Fiyat 5 dakika boyunca max 75'e cikti → 98'e ulasmadi.
Pencere kapandi:
  BTC yukseldi → Up = $1.00 → 10 share × $1.00 = $10.00 → +$4.50 kar
  BTC dustu    → Up = $0.00 → 10 share × $0.00 = $0.00  → -$5.50 zarar

TP tetiklenmese bile resolution sonucu belirlenir.
```

---

## 6. Stop Loss (SL) — Zarar Kes

**Ne yapar:** Fiyat belirli seviyenin altina inince otomatik satar.

### Senaryo 1: Hard SL — Tek seviye

```
Alis: Up @ 60c, 10 share, $6.00
slEnabled = true, slPriceCent = 45

Fiyat 45'in altina inince:
  SAT Up @ market price
  Orderbook'ta best bid = 38c → SAT @ 38c (slippage!)
  Gercek kayip: 60c - 38c = 22c per share = -$2.20

Dikkat: SL tetik fiyati 45c ama gercek fill 38c. 
        7 cent slippage var cunku orderbook'ta likidite az.
```

### Senaryo 2: Staged SL — Kademeli

```
Alis: Up @ 60c, 10 share, $6.00
slRules = [
  { priceCent: 50, sizePct: 50 },   → 5 share SAT @ ~43c (slippage)
  { priceCent: 40, sizePct: 50 }    → 5 share SAT @ ~34c (slippage)
]

SL-0 tetiklendi (fiyat 50'nin altina dustu):
  5 share @ 43c = $2.15 aldi (gercek fill 43c, tetik 50c degil!)
  Zarar: 5 × (60-43) = -$0.85

SL-1 tetiklendi (fiyat 40'in altina dustu):
  5 share @ 34c = $1.70 aldi
  Zarar: 5 × (60-34) = -$1.30

Toplam zarar: -$2.15 (maliyet $6.00, geri $3.85)
```

### Senaryo 3: SL tetiklenmez — fiyat yukselir

```
Alis: Up @ 60c, 10 share, $6.00
slEnabled = true, slPriceCent = 45

Fiyat 5 dakika boyunca min 52'ye dustu → 45'in altina inmedi.
SL tetiklenmedi. Pencere kapandi:
  BTC yukseldi → Up = $1.00 → +$4.00 kar
  SL gereksizdi, fiyat hic o kadar dusmedi.
```

---

## 7. PTB Stop Loss (Fiyat Bazli Zarar Kes)

**Ne yapar:** Sabit fiyat SL yerine PTB gap bazli SL. BTC fiyati PTB'ye donunce satar.

### Senaryo 1: Gap sifira iner → satis

```
Market: eth-updown-5m-1774013100
Up token alindi (BTC yukselecek bahsi)
PTB = $2,000, alis aninda BTC = $2,005 (gap = +$5)

ptbStopLossEnabled = true
ptbStopLossGapUsd = 0 (gap sifira inince sat)

10sn sonra: BTC = $2,001 → gap = +$1 → bekle
20sn sonra: BTC = $2,000 → gap = $0 → SATIS!

directional_gap (Up icin) = BTC - PTB = $2,000 - $2,000 = $0
$0 <= $0 → TRIGGER → sat
```

### Senaryo 2: Time decay = tighten — gectikce hassaslasir

```
Market: btc-updown-5m-1773905700
PTB = $70,000, alista BTC = $70,080 (gap = +$80)

ptbStopLossGapUsd = $50
ptbStopLossTimeDecayMode = "tighten"

Pencere baslangic threshold = $50
60sn gecti (%20 sure) → threshold = $50 × (1 - 0.2) = $40
120sn gecti (%40 sure) → threshold = $50 × (1 - 0.4) = $30
180sn gecti (%60 sure) → threshold = $50 × (1 - 0.6) = $20
240sn gecti (%80 sure) → threshold = $50 × (1 - 0.8) = $10

BTC = $70,030 (hala yuksek ama gap azaldi)
Gap = $30, threshold = $30 → TRIGGER → sat

Neden? Gectikce "hala yuksek mi?" kontrolu sikilasir.
```

### Senaryo 3: Kademeli PTB SL

```
ptbStopLossRules = [
  { gapUsd: 12.5, sizePct: 25 },   → %25 pozisyon gap $12.5'in altina inince sat
  { gapUsd: 3.0, sizePct: 75 }     → %75 pozisyon gap $3'un altina inince sat
]

PTB = $70,000, alista BTC = $70,050

T+30sn: BTC = $69,985 → gap = -$15 (negatif!) → -$15 < $12.5 → KADEME 1 SATIS
  25% pozisyon satildi (ilk kademe)

T+60sn: BTC = $69,998 → gap = -$2 → -$2 < $3 → KADEME 2 SATIS
  Kalan 75% pozisyon satildi

Toplam: Hemen hemen tum pozisyon PTB'nin altina dustugu icin satildi.
```

---

## 8. Re-Entry (Yeniden Giris)

**Ne yapar:** SL tetiklendiginde otomatik olarak ayni piyasadan tekrar girer.

### Senaryo 1: 2 kez SL, 3. giriste TP

```
reenterOnSlHit = true
reentryMaxAttempts = 2 (toplam 3 giris: 1 ilk + 2 re-entry)

Giris 1: Up @ 60c, $5 → SL@45 atesler → -$1.80 zarar
Re-entry 1: Up @ 60c, $5 → SL@45 atesler → -$1.80 zarar
Re-entry 2: Up @ 60c, $5 → TP@98 dolar → +$3.20 kar

Net: -$1.80 - $1.80 + $3.20 = -$0.40 (hala zarar ama tek seferde olsaydi -$1.80 olacakti)
```

### Senaryo 2: Her seferinde SL → toplam zarar

```
reentryMaxAttempts = 2

Giris 1: Up @ 60c, $5 → SL@45 → -$1.80
Re-entry 1: Up @ 60c, $5 → SL@45 → -$1.80
Re-entry 2: Up @ 60c, $5 → SL@45 → -$1.80

Net: 3 × (-$1.80) = -$5.40

Sorun: Ayni parametrelerle 3 kez girdi, 3 kez de SL yedi.
Piyasa Down lehine hareket ediyor, re-entry zarari katliyor.
```

### Senaryo 3: Re-entry devre disi

```
reenterOnSlHit = false

Giris 1: Up @ 60c, $5 → SL@45 → -$1.80
Re-entry yapilmadi.

Net: -$1.80 (tek zarar, re-entry riski yok)

Avantaj: Toplam zarar sinirli.
Dezavantaj: SL sonrasi fiyat geri donerse kacirdi.
```

---

## 9. SL Trigger Price Mode (Fiyat Kaynagi)

**Ne yapar:** SL tetiklendiginde hangi fiyatin kullanilacagini belirler.

### Senaryo 1: best_bid modu (hizli)

```
Up token SL@45 konuldu.
slTriggerPriceMode = "best_bid"

T+10sn: best_bid = 46c → bekle (46 > 45)
T+20sn: best_bid = 44c → TRIGGER (44 < 45)

Sonuc: Hizli tetik. Best bid aninda SL seviyesinin altina dustu.
Risk: Flash crash'te best_bid anida cok dusebilir.
```

### Senaryo 2: composite_safe modu (guvenli)

```
Up token SL@45 konuldu.
slTriggerPriceMode = "composite_safe"

T+10sn: best_bid = 44c, last_trade = 47c
  44 < 45 ama 47 > 45 → bekle (ikisi de altinda degil)

T+20sn: best_bid = 44c, last_trade = 43c
  44 < 45 ve 43 < 45 → TRIGGER (ikisi de altinda!)

Sonuc: Gec ama guvenli tetik. Yanlis pozitif (flash crash) filtrelenir.
```

### Senaryo 3: composite_fast modu (agresif)

```
Up token SL@45 konuldu.
slTriggerPriceMode = "composite_fast"

T+10sn: best_bid = 47c, last_trade = 43c
  min(47, 43) = 43 → 43 < 45 → TRIGGER

Sonuc: Herhangi bir fiyat kaynagi SL altina duserse tetikler.
Cok hizli ama gereksiz tetik riski var.
```

---

## 10. Time Exit (Zaman Bazli Cikis)

**Ne yapar:** Belirli sure gectikten sonra pozisyonu otomatik satar.

### Senaryo 1: 3 dakikada yari satis

```
timeExitRules = [{ elapsedMinutes: 3, remainingPct: 50 }]

Alis: Up @ 55c, 10 share
3 dakika gecti:
  5 share SAT @ market price (ornegin 62c)
  Kalan 5 share hala elde

Pencere kapandiginda:
  Up kazandi: 5 × $1.00 = $5.00 + satis $3.10 = $8.10 (maliyet $5.50) → +$2.60
  Up kaybetti: 5 × $0.00 = $0 + satis $3.10 = $3.10 → -$2.40
```

### Senaryo 2: Kademeli cikis

```
timeExitRules = [
  { elapsedMinutes: 2, remainingPct: 30 },
  { elapsedMinutes: 4, remainingPct: 100 }
]

Alis: Up @ 55c, 10 share

2. dakikada: 3 share SAT (kalann 7)
4. dakikada: 7 share SAT (hepsi satildi)

Sonuc: Erken kismi cikis + gec tam cikis. Risk kademeli azaltilir.
```

### Senaryo 3: Time exit yok — tum pozisyon resolution'a kalir

```
timeExitRules = [] (bos, time exit yok)

Alis: Up @ 55c, 10 share
5 dakika boyunca pozisyon elde tutulur.
Pencere kapandiginda:
  Up kazandi: 10 × $1.00 = $10.00 → +$4.50 kar
  Up kaybetti: 10 × $0.00 = $0.00 → -$5.50 zarar

Buyuk kar veya buyuk zarar. Time exit riski azaltmaz ama kar sansini da kesmez.
```

---

## 11. Window End Auto Sell (Pencere Sonu Satis)

**Ne yapar:** Pencere kapanirken bekleyen pozisyon varsa otomatik satar.

### Senaryo 1: Pozisyon elde, pencere kapaniyor

```
Up @ 55c alindi, 10 share
TP ve SL hic tetiklenmedi (fiyat 50-60 arasinda kaldi)

windowEndAutoSell = true
Pencere kapanisina 30sn kala:
  Bot 10 share @ market price (ornegin 52c) satar
  $5.20 alir, maliyet $5.50 → -$0.30 zarar (kucuk)

Neden? Resolution riskini almak yerine kucuk zararla cik.
```

### Senaryo 2: Pozisyon yok → bir sey yapmaz

```
Up @ 55c alindi, 10 share
TP @ 98c tetiklendi, 10 share satildi. Pozisyon sifir.

windowEndAutoSell = true
Pencere sonu: Pozisyon yok → bir sey yapmaz.
```

### Senaryo 3: Window end kapali → resolution riski

```
Up @ 55c alindi, 10 share
TP ve SL hic tetiklenmedi.

windowEndAutoSell = false
Pencere kapandi:
  BTC yukseldi → Up = $1.00 → 10 × $1.00 = $10.00 → +$4.50
  BTC dustu → Up = $0.00 → 10 × $0.00 = $0.00 → -$5.50

Buyuk risk ama buyuk kar sansi da var.
```

---

## 12. Staged SL Re-Entry Ayarlari

**Ne yapar:** Kademeli SL sonrasi re-entry davranisini kontrol eder.

### Senaryo 1: Tum kademeler bitmeden re-entry yok

```
stagedSlReentryOnlyAfterAllStages = true
slRules = [
  { priceCent: 50, sizePct: 50 },
  { priceCent: 40, sizePct: 50 }
]

SL-0 tetiklendi (50'nin altina dustu, %50 satildi)
  Re-entry? HAYIR → SL-1 hala beklemede.

SL-1 tetiklendi (40'in altina dustu, kalan %50 satildi)
  Re-entry? EVET → tum kademeler bitti.

Neden? Aradaki kademeler hala calisirken re-entry yapmak kafa karistirici olur.
```

### Senaryo 2: Hemen re-entry

```
stagedSlReentryOnlyAfterAllStages = false

SL-0 tetiklendi (%50 satildi)
  Re-entry? EVET → hemen yeniden girer.

Neden? Bot hizli tepki verir, pery AA varsa tekrar girer.
Risk: SL-1 hala beklemede, eski pozisyonun bir kismi hala riskte.
```

### Senaryo 3: Sadece toz (dust) pozisyonlari tekrar dene

```
stagedSlRetryOnlyDust = true
stagedSlRetryDustMetric = "remaining_usdc"
stagedSlRetryDustValue = 1.0

SL-0 tetiklendi, %50 satildi. Kalan pozisyon = $2.50 (dust degil)
  Re-entry? HAYIR → $2.50 > $1.0 (dust sinirindan buyuk)

SL-0 tetiklendi, %90 satildi. Kalan pozisyon = $0.50 (dust!)
  Re-entry? EVET → $0.50 < $1.0 (cok kucuk, tekrar dene)

Neden? Cok kucuk pozisyonlari yeniden denemek mantikli, buyukleri degil.
```

---

## 13. Bildirimler (Telegram)

**Ne yapar:** Kritik durumlarda Telegram mesaji gonderir.

### Senaryo 1: Alis doldu + TP doldu

```
notifyOnFill = true
notifyOnTpHit = true

12:35:02 → Telegram: "Up @ 55c alindi, 10 share, $5.50"
12:38:15 → Telegram: "TP tetiklendi! Up @ 98c satildi, kar = +$4.10"
```

### Senaryo 2: PTB guard blokladi

```
notifyOnPriceToBeatGapBlocked = true

12:35:02 → Telegram: "PTB guard blokladi. Gap=$0.50, threshold=$2. Bot bekliyor."
12:35:15 → Telegram: "PTB guard PASSED. Gap=$2.10. Emir gonderildi."
```

### Senaryo 3: SL + re-entry

```
notifyOnSlHit = true
notifyOnFill = true

12:36:00 → Telegram: "SL tetiklendi! Up @ 38c satildi (tetik: 45c, slippage: 7c)"
12:36:02 → Telegram: "Re-entry: Up @ 60c alindi, 10 share, $6.00 (deneme 2/3)"
```

---

## 14. SL Sibling Policy (Kardes Emir Politikasi)

**Ne yapar:** Kademeli SL'de bir kademe doldugunda diger kardes emirlere ne olacagini belirler.

### Senaryo 1: resize_remaining — kalan kardesleri yeniden boyutlandir

```
Alis: Up @ 60c, 10 share
slRules = [
  { priceCent: 50, sizePct: 50 },   → SL-0: 5 share
  { priceCent: 40, sizePct: 50 }    → SL-1: 5 share
]

SL-0 tetiklendi, 5 share satildi.
SL-1 ne yapar? → Boyutunu 5 share'den 10 share'a cikarir (kalan tum pozisyonu kapsar)

Neden? SL-0 satınca SL-1'in tum pozisyonu korumasi gerekir.
```

### Senaryo 2: cancel_all — hard SL'de tum kardesleri iptal et

```
Alis: Up @ 60c, 10 share
tpEnabled = true, tpPriceCent = 98 (hard TP)
slEnabled = true, slPriceCent = 45 (hard SL)

TP @ 98 doldugunda → SL @ 45 iptal edilir (gerek kalmadi)
SL @ 45 doldugunda → TP @ 98 iptal edilir (gerek kalmadi)

Neden? Hard cikislarda biri tetiklenince digeri otomatik iptal olur.
```

### Senaryo 3: Staged TP + staged SL birlikte

```
Alis: Up @ 50c, 10 share
tpRules = [{ priceCent: 70, sizePct: 50 }, { priceCent: 98, sizePct: 50 }]
slRules = [{ priceCent: 40, sizePct: 50 }, { priceCent: 30, sizePct: 50 }]

TP-0 @ 70 tetiklendi → 5 share satildi
  SL-0 ve SL-1 boyutu guncellenir (5 share'den az kalirsa iptal)
  TP-1 @ 98 hala beklemede, boyutu kalan 5 share

TP-1 @ 98 tetiklendi → 5 share satildi → pozisyon bitti
  Tum SL emirleri otomatik iptal
```

---

## 15. Exit Price Cap (Satis Fiyati Tabani)

**Ne yapar:** TP satim fiyatini `trigger_price - 5 cent` ile sinirlar. Cok ucuz satmayi onler.

### Senaryo 1: Normal satis — cap etkisi yok

```
TP @ 98c, best bid = 95c
floor = 98 - 5 = 93c

95c > 93c → cap etkisi yok, satis @ 95c yapilir
```

### Senaryo 2: Cap aktif — fiyati yukseltir

```
TP @ 98c, best bid = 88c
floor = 98 - 5 = 93c

88c < 93c → cap aktif! Satis @ 93c yapilir (88c degil!)
Bot 93c'den satis emri gonderir.
Eger 93c'den alan cikmazsa emir acikta kalir.
```

### Senaryo 3: SL'de cap yok

```
SL @ 45c, best bid = 38c
floor = SL icin UYGULANMAZ (sadece TP icin)

Satis @ 38c yapilir (market price, cap yok)
Neden? SL'de hiz onemli, fiyat sinirlamaya vakit yok.
```
