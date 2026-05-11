  # PTB (Price-to-Beat) Threshold Problemi — Detayli Aciklama

  ## 1. Polymarket Up/Down Marketleri Nedir?

  Polymarket'te BTC, ETH, SOL gibi kripto varliklar icin **5 dakikalik** veya **15 dakikalik** prediction marketler var.

  - **Soru:** "BTC son 5 dakikada yukseldi mi?"
  - **Cevap:** Yes (yukseldi) veya No (dusmedi / sabit)
  - **Sen:** Yes token'i ucuza al, dogru tahmin edersen para kazaniyorsun

  ---

  ## 2. PTB (Price-to-Beat) Nedir?

  Her 5 dakikalik veya 15 dakikalik pencerenin bir **baslangic fiyati** var. Bu fiyata **PTB (Price-to-Beat)** deniyor.

  PTB, pencerenin acildigi saniyede o varligin (BTC/ETH/SOL) fiyatidir.

  ```
  Ornek ETH 5m:
  Pencere baslangici: 12:35:00
  PTB = $2,000.00  (tam 12:35:00'da ETH fiyati)
  Pencere bitisi:   12:40:00
  ```

  PTB'nin kaynagi: Onceki pencerenin kapanis fiyati (Polymarket crypto-price API'den dogrulanir) veya Chainlink baslangic tick'i.

  ---

  ## 3. PTB Guard Ne Yapar?

  Bot, bir Yes/No token'i almadan once **PTB guard** adinda bir kontrol yapar.

  ### Kontrol mantigi:

  ```
  PTB (pencere baslangic fiyati) = $2,000.00
  Su anki Chainlink fiyati        = $2,003.00
  Gap (fark)                      = $3.00

  Threshold (esik)                = $2.00

  Soru: 3.00 >= 2.00 mu?
  Cevap: EVET → PASSED → emir verilir
  ```

  ```
  PTB = $2,000.00
  Su anki fiyat = $2,001.00
  Gap = $1.00

  Threshold = $2.00

  Soru: 1.00 >= 2.00 mu?
  Cevap: HAYIR → BLOCKED → emir verilmez, bekler
  ```

  ### Threshold = "en az bu kadar fark olmali ki gireyim"

  - Yuksek threshold → daha secici (sadece buyuk hareketlerde gir)
  - Dusuk threshold → daha agresif (kucuk hareketlerde bile gir)

  ---

  ## 4. Mevcut Sistem: 3 Mod Var

  ### Mod 1: Manual
  Kullanici sabit bir threshold belirler. Ornegin `priceToBeatMaxDiff = 2 USD`.
  - Arti: basit, tahmin edilebilir
  - Eksi: piyasa kosullarindan bagimsiz, her zaman ayni

  ### Mod 2: auto_last_3_avg_excursion
  Son 3 tamamlanmis pencerenin **yonlu hareket ortalamasini** threshold yapar.
  - Up yonu icin: her pencerenin `high - open` degerlerinin ortalamasi
  - Down yonu icin: her pencerenin `open - low` degerlerinin ortalamasi

  ```
  Ornek:
  W-1: open=2000, high=2005 → up excursion = 5
  W-2: open=2003, high=2008 → up excursion = 5
  W-3: open=1999, high=2004 → up excursion = 5

  Avg = (5+5+5)/3 = 5 → threshold = $5
  ```

  - Arti: piyasa kosullarina gore degisir
  - Eksi: sadece 3 pencereye bakar (5m'de sadece son 15 dk), volatilite/hacim duyarli degil

### Mod 3: auto_vol_pct (eklenmesi onerilen yeni mod)
 Otomatik, volatiliteye ve pencere temposuna gore dinamik threshold hesaplar.

  ---

  ## 5. Sorunun Koku

  ### Senaryo: ETH 5m Up

  **Dun (yuksek hacimli gun):**
  ```
  PTB = $2,000.00
  Anlik Chainlink fiyati = $2,002.00
  Gap = $2.00

  Manual threshold = $2.00
  2.00 >= 2.00 → PASSED

  Polymarket Yes token fiyati = ~80 cent
  → ALDI (iyi fiyat, iyi giris)
  ```

  **Bugun (dusuk hacimli gun):**
  ```
  PTB = $2,000.00
  Anlik Chainlink fiyati = $2,002.00
  Gap = $2.00 (ayni!)

  Manual threshold = $2.00 (hala sabit)
  2.00 >= 2.00 → PASSED

  Polymarket Yes token fiyati = ~90 cent
  → ALAMADI (pahali, edge yok)
  ```

  **Ayni gap ($2), ayni threshold ($2), farkli sonuclar.**

  ### Neden oluyor?

  - **Yuksek hacimde:** Fiyat hizli hareket ediyor, Polymarket probability fiyati hizli tepki veriyor, Yes token fiyati dusuyor (80 cent) → iyi alis
  - **Dusuk hacimde:** Fiyat yavas hareket ediyor, Polymarket probability fiyati yuksek kaliyor (90 cent) → kotu alis

  ### Ozet:

  | | Gap | Threshold | Sonuc | Polymarket Fiyat | Alis? |
  |---|---|---|---|---|---|
  | Dun (yuksek hacim) | $2 | $2 | PASSED | ~80 cent | EVET |
  | Bugun (dusuk hacim) | $2 | $2 | PASSED | ~90 cent | HAYIR |

  Sabit threshold, farkli piyasa kosullarinda ayni gap icin farkli sonuclar uretiyor.

  ---

  ## 6. Hacim/Volatilite Nedir ve Nasil Olculur?

  ### Volatilite = Bir pencerede ne kadar hareket oldugu

  Her 5 dakikalik pencerede Chainlink'ten 4 fiyat geliyor:
  - **open** = pencere baslangic fiyati
  - **high** = pencere icindeki en yuksek fiyat
  - **low** = pencere icindeki en dusuk fiyat
  - **close** = pencere bitis fiyati

  ```
  Volatilite = high - low
  ```

  Ornekler:
  ```
  Pencere A: open=2000, high=2005, low=1998, close=2003
  Volatilite = 2005 - 1998 = $7 (hareketli pencere)

  Pencere B: open=2003, high=2004, low=2002, close=2003
  Volatilite = 2004 - 2002 = $2 (sakin pencere)
  ```

  ### Volatilite yuksekse = hacim yuksek demektir

  Cok hareket eden bir piyasada:
  - Islem hacmi yuksek
  - Fiyat hizli degisiyor
  - Buyuk gap'ler olusuyor

  Az hareket eden bir piyasada:
  - Islem hacmi dusuk
  - Fiyat yavas degisiyor
  - Kucuk gap'ler olusuyor

  ---

  ## 7. Her Varligin Volatilitesi Farkli

  Ayni 5 dakikalik pencerede, farkli varliklar cok farkli volatilite gosterir:

```
+----------------+-------------+------------------+---------------------+
| Varlik         | Fiyat       | 5m Ortalama Vol  | Ornek (dusuk/normal/yuksek) |
+----------------+-------------+------------------+---------------------+
| ETH            | ~$2,000     | $3 - $8          | $2 / $5 / $15       |
| BTC            | ~$70,000    | $100 - $400      | $50 / $200 / $800   |
| SOL            | ~$82        | $0.03 - $0.15    | $0.03 / $0.06 / $0.10+ |
+----------------+-------------+------------------+---------------------+
```

**BTC'nin absolute 5m volatilitesi ETH'in ~30-50 kati.**
**ETH'in absolute 5m volatilitesi SOL'un ~50-150 kati.**

  Bu, threshold hesaplamasinda cok onemli cunku ayni formul her varlikta farkli sonuc veriyor.

  ---

  ## 8. Matematiksel Sorun

  Diyelim ki threshold'u **ortalama volatilite × carpan** olarak hesapliyoruz:

  ```
  threshold = avg_vol × ratio
  ```

  Ayni ratio (ornegin 0.4) her varlikta:

  ```
ETH: avg_vol = $5    → 5 × 0.4    = $2.00
BTC: avg_vol = $200  → 200 × 0.4  = $80.00
SOL: avg_vol = $0.06 → 0.06 × 0.4 = $0.024
  ```

  ### ETH icin: $2.00 — mantikli
ETH'de 5m'de $2 fark demek, fiyat %0.1 hareket etmis demek. Bu makul bir giris esigi.

  ### BTC icin: $80.00 — cok fazla
  BTC'de 5m'de $80 fark demek, fiyat %0.11 hareket etmis demek. Bu cok buyuk bir hareket, nadiren olur. Bot neredeyse hic alis yapamaz.

### SOL icin: $0.024 — cok kucuk
SOL'da 5m'de 2.4 cent fark demek, tipik dusuk hacim gurultusune cok yakin bir seviyedir. Bu threshold botu fazla agresiflestirir.

  ---

  ## 9. Neden Tek Bir Carpan Tum Varliklari Kapsayamiyor?

  Volatilite farklari cok buyuk:

```
BTC vol / ETH vol = ~200/5 = 40x
BTC vol / SOL vol = ~200/0.06 = ~3333x
ETH vol / SOL vol = ~5/0.06 = ~83x
```

Eger ratio'yu ETH'e gore ayarlarsan (ratio=0.4 → ETH'te $2 threshold):
- BTC'te $80 olur (cok buyuk)
- SOL'da $0.024 olur (cok kucuk)

Eger ratio'yu BTC'e gore ayarlarsan (ratio=0.05 → BTC'te $10 threshold):
- ETH'te $0.25 olur (cok kucuk, her seyi alir)
- SOL'da $0.003 olur (mikroskopik, her seyi alir)

Eger ratio'yu SOL'a gore ayarlarsan (ratio=0.67 → SOL'da ~$0.04 threshold):
- ETH'te $3.35 olur (ETH icin daha secici)
- BTC'te $134 olur (cok buyuk)

**Matematiksel gercek:** Fiyat olcekleri arasindaki fark (BTC $70K, ETH $2K, SOL $82) ve absolute volatilite farklari, tek bir carpanin tum varliklari kapsayamamasina neden oluyor.

  ---

  ## 10. Mevcut ETH Node'u Nasil Calisiyor?

  Bir ETH 5m Up/Down trade flow su node'lardan olusur:

  ```
  [trigger.market_price] → [action.place_order] → [action.place_order (TP/SL)]
  ```

  ### Node 1: trigger.market_price

  Bu node ETH fiyatini dinler ve belirli kosullar saglaninca tetikler.

  **Typical ETH node config:**
  ```toml
  # trigger.market_price node
  marketMode = "auto_scope"          # otomatik market secimi (eth-updown-5m-*)
  priceMode = "composite"            # Polymarket + Chainlink bileşik fiyat
  outcomeLabel = "Up"                 # ETH yukselecek mi?

  # Fiyat kosulu (opsiyonel):
  triggerCondition = "cross_above"    # fiyat ustunu gecince tetikle
  triggerPriceCent = 80               # 80 centten asagi dustugunde

  # PTB Gate (tetik seviyesi):
  priceToBeatTriggerEnabled = true    # PTB gate aktif
  priceToBeatMode = "manual"          # su an manual kullaniyorsun
  priceToBeatTriggerMinGap = 2        # minimum 2 USD fark gerekli
  priceToBeatTriggerUnit = "usd"      # birim USD

  # Tekrar ayarlari:
  repeatMode = "once"                 # her markette bir kez tetikle
  ```

  **Bu node ne yapar?**
  1. Her yeni ETH 5m pencere acildiginda (ornegin `eth-updown-5m-1774016400`):
    - PTB'yi getirir (o pencerenin baslangic fiyati)
    - Chainlink'den anlik ETH fiyatini alir
    - Gap hesaplar: `gap = anlik_fiyat - PTB`
  2. Gap, threshold'dan buyukse → tetiklenir → bir sonraki node'a gecer
  3. Gap, threshold'dan kucukse → bekler, yeni fiyat verisi geldikce tekrar dener

  ### Node 2: action.place_order

  Bu node Polymarket'te Yes token'i alir.

  **Typical config:**
  ```toml
  # action.place_order node
  side = "buy"
  outcomeLabel = "Up"

  # Fiyat sinirlari:
  executionFloorPriceCent = 0        # minimum alis fiyati
  maxPriceCent = 80                   # MAXIMUM 80 centten al (onemli!)

  # PTB Guard (alis oncesi son kontrol):
  priceToBeatGuardEnabled = true      # PTB guard aktif
  priceToBeatMode = "manual"          # manual mod
  priceToBeatMaxDiff = 2              # 2 USD fark gerekli
  priceToBeatMaxDiffUnit = "usd"

  # Bekleme davranisi:
  retryOnPriceToBeatGuardBlock = true # bloklanirsa bekle, tekrar dene
  notifyOnPriceToBeatGapBlocked = true # Telegram bildirimi gonder
  ```

  **Bu node ne yapar?**
  1. trigger.market_price tetikleyince calisir
  2. **PTB guard** tekrar kontrol eder (fiyat degismis olabilir):
    - Gap >= threshold mu? → Evet → devam
    - Gap < threshold mu? → Hayir → bekle, tekrar dene
  3. Polymarket orderbook'a bakar, en iyi fiyati bulur
  4. **maxPrice kontrolu:** Yes token fiyati 80 centten yuksekse → alma
  5. Emri gonderir

  ### Akis Ornegi (ETH 5m Up, dun):

  ```
  12:35:00 - Yeni pencere acildi: eth-updown-5m-1774016400
            PTB = $2,000.00 (bu saniyede ETH fiyati)

  12:35:02 - Chainlink: ETH = $2,002.00
            Gap = $2.00, threshold = $2.00 → PASSED (trigger)

  12:35:02 - action.place_order basladi
            PTB guard: gap=$2.00 >= threshold=$2.00 → PASSED
            Polymarket Yes token = 78 cent (< 80 cent max) → OK
            Emir gonderildi: BUY Yes @ 78 cent, $5 USDC

  12:37:30 - ETH $2,005'e cikti
            TP tetiklendi, satildi → KAR
  ```

  ### Ayni Akis Ornegi (ETH 5m Up, bugun dusuk hacim):

  ```
  12:35:00 - Yeni pencere acildi: eth-updown-5m-1774016400
            PTB = $2,000.00

  12:35:02 - Chainlink: ETH = $2,002.00
            Gap = $2.00, threshold = $2.00 → PASSED (trigger)

  12:35:02 - action.place_order basladi
            PTB guard: gap=$2.00 >= threshold=$2.00 → PASSED
            Polymarket Yes token = 90 cent (> 80 cent max) → BLOCKED!
            "Fiyat 80 centi asti, almiyorum"
            
            → Emir verilmedi, beklemede...
            → 5 dakika bitti, pencere kapandi
            → Islem yapilmadi
  ```

  ### Iki PTB Kontrol Noktasi

  Onemli: PTB **iki yerde** kontrol ediliyor:

  1. **trigger.market_price** uzerindeki `priceToBeatTriggerEnabled` + `priceToBeatTriggerMinGap`
    - Bu, tetiklemeye karar verir
    - "Gap yeterli mi? Tetikleyeyim mi?"

  2. **action.place_order** uzerindeki `priceToBeatGuardEnabled` + `priceToBeatMaxDiff`
    - Bu, emir vermeye karar verir (son kontrol)
    - "Gap hala yeterli mi? Emin misin?"

  Her ikisinin de ayri ayri gecmesi gerekiyor.

  ---

  ## 11. threshold'un Onemi

  Threshold, bot'un **ne zaman girecegini** belirleyen en onemli parametre:

  ```
  Cok dusuk threshold (ornegin $0.10):
    → Her kucuk hareket tetiklenir
    → Cok fazla islem yapilir
    → Ama Polymarket fiyatlari yuksek olur (cok erken girdi)
    → Sonuc: cok islem, az kar

  Cok yuksek threshold (ornegin $10):
    → Sadece buyuk hareketlerde tetiklenir
    → Cok az islem yapilir
    → Polymarket fiyatlari dusuk olur (gec girdi, iyi fiyat)
    → Ama firsatlarin cogu kacirilir

  Dengeli threshold (ornegin $2-3):
    → Orta seviye hareketlerde tetiklenir
    → Makul sayida islem
    → Polymarket fiyatlari makul (75-85 cent arasi)
    → Sonuc: iyi denge
  ```

  **Sorun:** "Dengeli" threshold, piyasa kosullarina gore degisiyor. Dün $2 dengeliydi, bugün $2 çok düşük (çünkü düşük hacimde $2 fark anlamlı değil ama fiyat hala 90 cent).

  ---

  ## 12. auto_vol_pct — Onerilen Formul (Duzeltilmis)

  ### Mantik

  - **Dusuk hacim** → fiyat hareketleri kucuk → threshold **duser** → kucuk gap'leri yakala → firsatlari kacirma
  - **Yuksek hacim** → fiyat hareketleri buyuk → threshold **artar** → sadece buyuk gap'leri al → gurultuyu filtrele
  - **Oran bazli** → her varlik (BTC/ETH/SOL) kendi fiyat olceginde otomatik calisir

  ### Formul (7 adim)

  ```
1. Her pencere icin:   range_pct = (high - low) / open
2. Baseline:           baseline_pct = EWMA(son 20 range_pct, span=20)
3. Current tempo:      current_pct = weighted_mean(son 3 range_pct, [0.2, 0.3, 0.5])
4. Ham oran:           raw_factor = current_pct / baseline_pct
5. Guvenli faktor:     vol_factor = sqrt(clip(raw_factor, 0.1, 10.0))
6. Threshold yuzdesi:  threshold_pct = base_pct_asset × vol_factor
7. Threshold USD:      threshold_usd = clamp(PTB × threshold_pct, min_usd, max_usd)
```

### Neden current / baseline (ters cevrildi)?

  ```
  Dusuk hacim: current_pct KUCUK → current/baseline KUCUK → vol_factor < 1 → threshold DUSER
  Yuksek hacim: current_pct BUYUK → current/baseline BUYUK → vol_factor > 1 → threshold ARTAR
  ```

### Neden kok alma (0.5)?

  Asiri dalgalanmayi onler:
  ```
vol_factor = 2.0 → 2.0^0.5 = 1.41 (degil 2.0)
vol_factor = 0.5 → 0.5^0.5 = 0.71 (degil 0.5)
```

### Neden EWMA baseline?

- Son 20 pencereye bakmaya devam eder ama en yeni pencerelere daha fazla agirlik verir
- Uzun sure yuksek volatilite goruldukten sonra baseline'in asiri sisip yeni normu gec yansitmasini azaltir
- `lookback=20` ve `span=20`, 5m markette yaklasik son 100 dakikayi takip eder

### Safe vol_factor notlari

```
Eger baseline_pct <= 0 veya current_pct <= 0 ise:
  vol_factor = 1.0

Eger raw_factor cok uc degerdeyse:
  raw_factor = clip(raw_factor, 0.1, 10.0)
```

### Neden yuzde bazli?

Her varlik kendi fiyat olceginde calisir, ayri scale factor gerekmez:
```
ETH: PTB=$2,000 × 0.1% = $2.00
BTC: PTB=$70,000 × 0.1% = $70
SOL: PTB=$82 × 0.05% = $0.041
```

  ### Parametreler

| Varlik | base_pct | floor_usd | max_usd | Mantik |
|---|---|---|---|---|
| ETH | 0.12% | $1.00 | $5.00 | tipik range ~0.25%, yarisi |
| BTC | 0.09% | $10.00 | 150 USD | daha az gurultu |
| SOL | 0.05% | $0.02 | $0.05 | 3-10 cent arasi hareketlere uyumlu |

  ### Floor (Alt Sinir) Nedir?

  **Floor**, threshold'un hicbir zaman altina dusmemesi gereken degerdir.

  - Cok dusuk hacimde vol_factor sifira yakin olsa bile, threshold floor degerinin altina inmez
  - Floor, bot'un "cok kucuk gap'leri bile almayayim" dedigi sinirdir
  - Ornegin ETH'de floor=$1.00 ise, hicbir durumda $1'in altindaki gap'leri almaz

  Neden farkli floor'lar?
  ```
ETH floor = $1.00  → ETH ~$2,000 fiyatinda $1 = %0.05 hareket
BTC floor = $10.00 → BTC ~$70,000 fiyatinda $10 = %0.014 hareket
SOL floor = $0.02  → SOL ~$82 fiyatinda $0.02 = %0.024 hareket
```

  Her varligin dogal fiyat olcegine gore minimum anlamli hareket farkli.

  ---

  ## 13. ORNEK: ETH 5m Up — Dun (yuksek hacimli gun)

  ### Pencere verileri:

  ```
Son 20 pencere (baseline):
  Pencere | Open    | High    | Low     | range_pct = (H-L)/O
  W-20    | 1998    | 2004    | 1994    | 0.50%
  W-19    | 2002    | 2007    | 1998    | 0.45%
  W-18    | 2000    | 2005    | 1996    | 0.45%
  ...     | (diger 17 pencere benzer, ortalama ~0.45%)

  baseline_pct = 0.45% (EWMA span=20)
```

```
Son 3 pencere (current tempo — hareketli!):
  W-3     | 2003    | 2010    | 1999    | 0.55%
  W-2     | 2001    | 2008    | 1995    | 0.65%
  W-1     | 1999    | 2011    | 1996    | 0.75%

  current_pct = (0.55 × 0.2) + (0.65 × 0.3) + (0.75 × 0.5) = 0.68%
```

  ### Hesaplama adim adim:

  ```
Adim 1: range_pct hesaplandi (yukarida)
Adim 2: baseline_pct = 0.45%
Adim 3: current_pct  = 0.68%

Adim 4: vol_factor = (0.68 / 0.45) ^ 0.5
                  = (1.511) ^ 0.5
                  = 1.229

Adim 5: threshold_pct = 0.12% × 1.229 = 0.147%

Adim 6: PTB = $2,000.00
        threshold_usd = $2,000 × 0.147% = $2.95

Adim 7: clamp($2.95, $1.00, $5.00) = $2.95
```

**Dun threshold = $2.95**

```
Gap = $2.00 → $2.00 < $2.95 → BLOCKED (gurultu filtre)
Gap = $3.00 → $3.00 >= $2.95 → PASSED → Yes ~78 cent → IYI GIRIS
```

  Yuksek hacimde bot sadece buyuk gap'leri alir. Kucuk gap'ler geciyor, gurultu olarak filtreleniyor.

  ---

  ## 14. ORNEK: ETH 5m Up — Bugun (dusuk hacimli gun)

  ### Pencere verileri:

  ```
Son 20 pencere (baseline):
  baseline_pct = 0.40% (EWMA span=20, hafta boyu karisik)

  Son 3 pencere (current tempo — sakin!):
    W-3     | 2000.50 | 2001.50 | 1999.50 | 0.10%
    W-2     | 2001.00 | 2002.00 | 2000.00 | 0.10%
    W-1     | 2000.00 | 2001.00 | 1999.00 | 0.10%

  current_pct = 0.10% (agirlikli son 3 pencere, dun 0.68% idi!)
  ```

  ### Hesaplama:

  ```
  vol_factor = (0.10 / 0.40) ^ 0.5
            = (0.25) ^ 0.5
            = 0.50

  threshold_pct = 0.12% × 0.50 = 0.06%

  threshold_usd = $2,000 × 0.06% = $1.20

  clamp($1.20, $1.00, $5.00) = $1.20
  ```

**Bugun threshold = $1.20** (dun $2.95 idi, simdi $1.20'ye dustu!)

  ```
  Gap = $2.00 → $2.00 >= $1.20 → PASSED → Yes ~85-90 cent
  ```

  Dusuk hacimde bot kucuk gap'leri de yakalar. **Ama** Polymarket fiyati hala 85-90 cent olabilir — bu durumda `maxPrice=80 cent` bloklar. Threshold duzgun calisiyor ama Polymarket fiyatinin 80'in altina dusmesi icin gap'in daha da buyumesi gerekiyor.

  ---

  ## 15. ORNEK: BTC 5m Up — Normal gun

```
baseline_pct = 0.30% (EWMA span=20, BTC tipik 5m range)
current_pct  = 0.35% (agirlikli son 3 pencere, hafif hareketli)

  vol_factor = (0.35 / 0.30) ^ 0.5 = 1.167^0.5 = 1.080

  threshold_pct = 0.09% × 1.080 = 0.097%

  PTB = $70,000
  threshold_usd = $70,000 × 0.097% = $67.90

clamp($67.90, $10, 150 USD) = $67.90
  ```

  **BTC normal threshold = $67.90**

  ---

  ## 16. ORNEK: BTC 5m Up — Cok hareketli gun

```
baseline_pct = 0.30% (EWMA span=20)
current_pct  = 1.20% (agirlikli son 3 pencere, cilgin hareketli!)

  vol_factor = (1.20 / 0.30) ^ 0.5 = 4.0^0.5 = 2.00

  threshold_pct = 0.09% × 2.00 = 0.18%

  threshold_usd = $70,000 × 0.18% = $126.00

clamp($126.00, $10, 150 USD) = $126.00
  ```

  **BTC yuksek hacim threshold = $126.00** (normal gunde $67 idi, simdi cikti)

  Yuksek hacimde sadece cok buyuk hareketlerde girer. Gap=$50 olsa bile bloklanir, gap=$130+ bekler.

  ---

## 17. ORNEK: SOL 5m Up — Dusuk hacim

```
baseline_pct = 0.073% (EWMA span=20, normal tempo ~6 cent)
current_pct  = 0.037% (agirlikli son 3 pencere, dusuk hacim ~3 cent)

vol_factor = (0.037 / 0.073) ^ 0.5 = 0.507^0.5 = 0.712

threshold_pct = 0.05% × 0.712 = 0.036%

PTB = $82
threshold_usd = $82 × 0.036% = $0.029 (~2.9 cent)

clamp($0.029, $0.02, $0.05) = $0.029
```

**SOL dusuk hacim threshold = ~2.9 cent**

  ---

## 18. ORNEK: SOL 5m Up — Yuksek hacim

```
baseline_pct = 0.073%
current_pct  = 0.122% (agirlikli son 3 pencere, yuksek hacim ~10 cent)

vol_factor = (0.122 / 0.073) ^ 0.5 = 1.671^0.5 = 1.293

threshold_pct = 0.05% × 1.293 = 0.065%

threshold_usd = $82 × 0.065% = $0.053 (~5.3 cent)

clamp($0.053, $0.02, $0.05) = $0.050
```

**SOL yuksek hacim threshold = 5 cent ceiling** (dusuk hacimde ~2.9 centti, simdi tavana vurdu)

  ---

  ## 19. KARSILASTIRMA TABLOSU

  ```
  +------------------+-------------+-------------+-------------+
|                  | ETH         | BTC         | SOL         |
|                  | base=0.12%  | base=0.09%  | base=0.05%  |
+------------------+-------------+-------------+-------------+
| Dusuk hacim      |             |             |             |
|  current_pct     | 0.10%       | 0.15%       | 0.037%      |
|  baseline_pct    | 0.40%       | 0.30%       | 0.073%      |
|  vol_factor      | 0.50        | 0.71        | 0.71        |
|  threshold_usd   | $1.20       | $44.60      | $0.029      |
+------------------+-------------+-------------+-------------+
| Normal hacim     |             |             |             |
|  current_pct     | 0.45%       | 0.35%       | 0.073%      |
|  baseline_pct    | 0.45%       | 0.30%       | 0.073%      |
|  vol_factor      | 1.00        | 1.08        | 1.00        |
|  threshold_usd   | $2.40       | $67.90      | $0.041      |
+------------------+-------------+-------------+-------------+
| Yuksek hacim     |             |             |             |
|  current_pct     | 0.68%       | 1.20%       | 0.122%      |
|  baseline_pct    | 0.45%       | 0.30%       | 0.073%      |
|  vol_factor      | 1.23        | 2.00        | 1.29        |
|  threshold_usd   | $2.95       | $126.00     | $0.05       |
+------------------+-------------+-------------+-------------+
```

  ---

  ## 20. ETH Dun vs Bugun — Ozet

  ```
  +------------------+------------+------------+
|                  | Dun        | Bugun      |
|                  | Yuksek Vol | Dusuk Vol  |
+------------------+------------+------------+
| current_pct      | 0.68%      | 0.10%      |
| baseline_pct     | 0.45%      | 0.40%      |
| vol_factor       | 1.229      | 0.500      |
| threshold_usd    | $2.95      | $1.20      |
+------------------+------------+------------+
| Gap=$2 sonucu    | BLOCKED    | PASSED     |
| Gap=$3 sonucu    | PASSED     | PASSED     |
+------------------+------------+------------+

Dun (yuksek hacim):  Threshold $2.95 → gap=$3 gecince Yes ~78 cent → IYI
Bugun (dusuk hacim): Threshold $1.20 → gap=$2 gecince Yes ~85-90 cent → maxPrice kontrolu
```

  ---

## 21. Config Alanlari (node-level)

Her iki PTB yuzeyinde (trigger.market_price + action.place_order):

Yeni mod icin:

```toml
priceToBeatMode = "auto_vol_pct"
```

| Alan | Default | ETH | BTC | SOL |
|---|---|---|---|---|
| `priceToBeatAutoBasePct` | `0.0012 (0.12%)` | `0.0012` | `0.0009` | `0.0005` |
| `priceToBeatAutoMinThresholdUsd` | `1.00` | `1.00` | `10.00` | `0.02` |
| `priceToBeatAutoMaxThresholdUsd` | `5.00` | `5.00` | `150.00` | `0.05` |
| `priceToBeatAutoLookbackBaseline` | 20 | 20 | 20 | 20 |
| `priceToBeatAutoLookbackCurrent` | 3 | 3 | 3 | 3 |

5 config alani var. `Default` kolonu asset-specific override yoksa kullanilacak fallback'i, ETH/BTC/SOL kolonlari ise onerilen asset ayarlarini gosterir. lookback degerleri tum varliklarda ayni.

---

## 22. Edge Case'ler ve Cozumleri

`auto_vol_pct` formulunde asagidaki edge case'ler bilincli olarak ele alinmali:

```
1. baseline_pct <= 0
   → sifira bolme riski vardir
   → cozum: vol_factor = 1.0 fallback

2. current_pct <= 0
   → anlik tempo anlamsiz derecede dusuktur
   → cozum: vol_factor = 1.0 fallback

3. Asiri dusuk current
   → threshold teoride cok kuculebilir
   → cozum: floor clamp devreye girer

4. Spike volatility
   → tek pencere outlier'i threshold'u bir anda sisirebilir
   → cozum: current_pct = agirlikli ortalama(last_3, [0.2, 0.3, 0.5])
```

Bu sayede threshold ne sifira cok yaklasir ne de tek pencere spike'i yuzunden gereksiz buyur.

---

## 23. Iyilestirilmis Formul

Kabul edilen formul:

```python
range_pct = (high - low) / open
baseline_pct = ewma(last_20_range_pct, span=20)
current_pct = weighted_mean(last_3_range_pct, weights=[0.2, 0.3, 0.5])

if baseline_pct <= 0 or current_pct <= 0:
    vol_factor = 1.0
else:
    raw_factor = current_pct / baseline_pct
    raw_factor = clip(raw_factor, 0.1, 10.0)
    vol_factor = sqrt(raw_factor)

threshold_pct = base_pct_asset * vol_factor
threshold_usd = clamp(PTB * threshold_pct, floor_usd, ceiling_usd)
```

Bu formulde:
- `current_pct`, en yeni pencereye daha fazla agirlik verir
- `baseline_pct`, daha yavas ama drift'e dayanikli referans uretir
- `clip(0.1, 10.0)` asiri uc degerleri kirpar
- `sqrt(...)` ani threshold sivrilmelerini yumusatir

Not: Momentum factor bilincli olarak EKLENMIYOR. `vol_factor` zaten anlik tempo ile referans tempo arasindaki farki yansittigi icin ekstra momentum carpani gereksiz karmasiklik yaratir.

---

## 24. Baseline Drift ve EWMA

Basit ortalama kullanildiginda soyle bir sorun olur:

```
Uzun sure yuksek volatilite yasandi
→ son 20 pencerenin ortalamasi sisti
→ piyasa sakinlesse bile baseline uzun sure yuksek kaldi
→ threshold gereksiz secici oldu
```

EWMA bu sorunu azaltir:

```python
baseline_pct = EWMA(range_pct, span=20)
```

Mantik:
- son 20 pencereye bakmaya devam eder
- en yeni pencerelere daha fazla agirlik verir
- uzun sureli yuksek volatilite sonrasinda yeni normali daha hizli yakalar

Bu yuzden `lookback_baseline=20` korunurken, baseline hesabi icin basit mean yerine EWMA tercih edilir.

---

## 25. Eski Sistem vs Yeni Sistem — Ornekli Karsilastirma

### 25.1 ETH 5m Up — Yuksek Hacimli Gun

```
PTB = $2,000
Anlik fiyat = $2,003
Gap = $3.00
Polymarket Yes = ~78 cent
```

**Eski sistem (`manual`, threshold=$2.00):**

```
$3.00 >= $2.00 → PASSED
Yes ~78 cent < maxPrice=80 → ALIR
```

**Yeni sistem (`auto_vol_pct`, threshold~$2.95):**

```
baseline_pct = 0.45%
current_pct  = 0.68%
vol_factor   = 1.229
threshold    = ~$2.95

$3.00 >= $2.95 → PASSED
Yes ~78 cent < maxPrice=80 → ALIR
```

Fark: Her ikisi de bu kaliteli firsati alir. Ama yeni sistem gap=$2 oldugunda bloklayarak gurultuyu filtreler.

### 25.2 ETH 5m Up — Dusuk Hacimli Gun

```
PTB = $2,000
Anlik fiyat = $2,002
Gap = $2.00
Polymarket Yes = ~85-90 cent
```

**Eski sistem (`manual`, threshold=$2.00):**

```
$2.00 >= $2.00 → PASSED
Ama Yes > 80 cent → maxPrice bloklar
```

**Yeni sistem (`auto_vol_pct`, threshold=$1.20):**

```
baseline_pct = 0.40%
current_pct  = 0.10%
vol_factor   = 0.50
threshold    = $1.20

$2.00 >= $1.20 → PASSED
Ama Yes > 80 cent → maxPrice yine bloklar
```

Fark: Bu senaryoda asil sorun threshold degil, Polymarket fiyatinin hala pahali olmasi. Yeni sistem threshold'u dogru yone ceker ama emir kalitesi icin `maxPrice` hala belirleyicidir.

### 25.3 BTC 5m Up — Normal Gun

```
PTB = $70,000
Anlik fiyat = $70,060
Gap = $60
Polymarket Yes = ~80 cent
```

**Eski sistem (`manual`, threshold=$15.00):**

```
$60 >= $15 → PASSED
```

**Yeni sistem (`auto_vol_pct`, threshold~$67.90):**

```
baseline_pct = 0.30%
current_pct  = 0.35%
vol_factor   = 1.080
threshold    = ~$67.90

$60 < $67.90 → BLOCKED
```

Fark: Eski sistem BTC'de cok agresif kalir. Yeni sistem sadece gercekten anlamli BTC hareketlerinde girer.

### 25.4 BTC 5m Up — Cok Hareketli Gun

```
PTB = $70,000
Anlik fiyat = $70,130
Gap = $130
Polymarket Yes = ~70 cent
```

**Eski sistem (`manual`, threshold=$15.00):**

```
$130 >= $15 → PASSED
Yes ~70 cent → ALIR
```

**Yeni sistem (`auto_vol_pct`, threshold~$126.00):**

```
baseline_pct = 0.30%
current_pct  = 1.20%
vol_factor   = 2.00
threshold    = ~$126.00

$130 >= $126.00 → PASSED
Yes ~70 cent → ALIR
```

Fark: Her iki sistem de buyuk hareketi yakalar. Yeni sistemin avantaji, ayni gun gap=$40-$80 gibi yari-gurultu hareketlerinde acele etmemesidir.

### 25.5 SOL 5m Up — Dusuk Hacim

```
PTB = $82
Anlik fiyat = $82.03
Gap = $0.03
Polymarket Yes = ~90 cent
```

**Eski sistem (`manual`, ornek threshold=$0.04):**

```
$0.03 < $0.04 → BLOCKED
```

**Yeni sistem (`auto_vol_pct`, threshold~$0.029):**

```
baseline_pct = 0.073%
current_pct  = 0.037%
vol_factor   = 0.712
threshold    = ~$0.029

$0.03 >= $0.029 → PASSED
Ama Yes > 80 cent → maxPrice bloklar
```

Fark: Yeni sistem SOL'un dusuk hacim temposuna daha iyi uyum saglar. Yine de emir kalitesini son sozu `maxPrice` soyler.

### 25.6 SOL 5m Up — Yuksek Hacim

```
PTB = $82
Anlik fiyat = $82.10
Gap = $0.10
Polymarket Yes = ~75 cent
```

**Eski sistem (`manual`, ornek threshold=$0.04):**

```
$0.10 >= $0.04 → PASSED
Yes ~75 cent → ALIR
```

**Yeni sistem (`auto_vol_pct`, threshold=ceiling $0.05):**

```
baseline_pct = 0.073%
current_pct  = 0.122%
vol_factor   = 1.293
ham threshold = ~$0.053
clamp sonrası threshold = $0.05

$0.10 >= $0.05 → PASSED
Yes ~75 cent → ALIR
```

Fark: Yeni sistem yuksek hacimde threshold'u yukari ceker ve micro-noise'i filtreler. Gap=$0.04 olsa eski sistem girerken yeni sistem bekleyebilir.
