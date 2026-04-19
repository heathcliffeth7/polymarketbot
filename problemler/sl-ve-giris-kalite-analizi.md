# SL ve Giris Kalite Analizi — Detayli Sorun Tespiti

## 1. Polymarket 5 Dakikalik Up/Down Piyasalari Nasil Calisir?

### 1.1 Piyasa Mekanizmasi

Polymarket'te BTC, ETH, SOL gibi kripto varliklar icin **5 dakikalik** veya **15 dakikalik** prediksiyon piyasalari var.

**Soru:** "BTC son 5 dakikada yukseldi mi?"

Her 5 dakikalik pencerenin bir **PTB (Price-to-Beat)** degeri var. PTB, pencerenin acildigi saniyede o varligin fiyatidir. Ornegin:

```
Pencere baslangici: 12:35:00 UTC
PTB = $70,000.00 (tam 12:35:00'da BTC fiyati)

Pencere bitisi: 12:40:00 UTC
Sonuc: BTC > PTB ise YES kazanir, BTC <= PTB ise NO kazanir
```

PTB'nin kaynagi: Polymarket crypto-price API'si veya Chainlink baslangic tick'i.

### 1.2 Token Fiyatlari ve Olasiliklar

YES ve NO token fiyatlari piyasanin yonu hakkindaki beklentiyi yansitir:

```
YES fiyati = 0.60 → Piyasa %60 ihtimalle yuksek diyor
NO fiyati  = 0.40 → Piyasa %40 ihtimalle yuksek diyor

YES + NO = 1.00 (her zaman)
```

**Onemli ozellik:** YES token fiyati PTB gap ile dogrudan iliskilidir:
- Buyuk gap (BTC PTB'den cok yuksek) → YES fiyati yuksek (0.80-0.95)
- Kucuk gap (BTC PTB'den biraz yuksek) → YES fiyati orta (0.55-0.70)
- Negatif gap (BTC PTB'den dusuk) → YES fiyati dusuk (0.20-0.45)

### 1.3 Piyasa Yapisi ve Diger Ozellikler

- **likidite:** Orderbook tabanli. Maker emirleri (limit) ve taker emirleri (market) var.
- **Fee model:** `fee_rate_bps = 1000` (Polymarket varsayilan). Fee, kar uzerinden alinir:
  ```
  fee_curve_rate = fee_rate_bps / 4000 = 0.25
  fee_shares = gross_qty * fee_curve_rate * (price * (1-price))^2 / price
  ```
  Ornegin: 7.25 share @ $0.69 alindiginda fee ≈ 0.12 share. Net = 7.25 - 0.12 = 7.13 share.
- **5 dakikalik silsili:** Ayni varlik icin her 5 dakikada yeni bir piyasa acilir. Ornegin `btc-updown-5m-1773905700`, `btc-updown-5m-1773906000`, vb.
- **Sonuc:** Pencere kapandiginda, BTC fiyati PTB'den yuksekse YES token $1.00'e, dusukse $0.00'e cozulur. NO token ise tersi.

### 1.4 Botun Piyasaya Giris Sekli

Bot, her yeni 5 dakikalik pencerede su adimlari izler:

1. **PTB Guard kontrolu:** Chainlink'den anlik BTC fiyati al, PTB ile karsilastir. Gap >= threshold ise gec.
2. **Emir gonderimi:** YES token al (immediate buy). `maxPrice` ile fiyati sinirla.
3. **Cikis emirleri:** Alim sonrasi TP (take-profit) ve SL (stop-loss) emirleri olustur.
4. **Re-entry:** SL tetiklenirse, `reenterOnSlHit=true` ile ayni piyasadan tekrar gir.

---

## 2. Take Profit (TP) Mekanizmasi Nasil Calisir?

### 2.1 TP Olusturma

Alim emri fill oldugunda, bot otomatik olarak TP satim emri olusturur:

```
Alim: YES @ 64 cent, 7.25 share, $5.00 USDC
TP: SELL YES @ 98 cent (trigger_price=0.98, cross_above)
```

**TP tetik mantigi:** `trigger_price` fiyatini gecince (cross_above) emir gonderilir.

### 2.2 TP Fiyat Sinirlama (`exit_price_capped`)

TP emirlerinde bir fiyat tabani (floor) uygulanir:

```rust
// crates/bot-runner/src/trade_builder/exit_math.rs:362-383
fn trade_builder_exit_sell_price_floor(order: &TradeBuilderOrder) -> Option<f64> {
    if !trade_builder_is_child_exit_sell(order) {
        return None;
    }
    // SL icin floor uygulama - piyasa fiyatindan sat
    if matches!(order.trigger_condition.as_deref(), Some("cross_below")) {
        return None;
    }
    let trigger_price = order.trigger_price?;
    let floored = ((trigger_price - TRADE_BUILDER_EXIT_TRIGGER_BUFFER).max(0.0) * 100.0).round() / 100.0;
    Some(clamp_probability(floored))
}
```

Sabitler: `TRADE_BUILDER_EXIT_TRIGGER_BUFFER = 0.05` (5 cent)

**TP fiyat tabani kurali:**
- `floor = trigger_price - 0.05`
- TP @ 98 cent icin: floor = 0.98 - 0.05 = 0.93
- Emir fiyati 0.93'ten asagi olamaz

**22,567 kez `exit_price_capped` event tetiklenmis.** Bu, TP satim fiyatini sinirlamistir.

### 2.3 TP Gerceklestirme Verileri

| Metrik | Deger |
|--------|-------|
| TP islem sayisi | 668 |
| Ort. alims fiyati | $0.80 |
| Ort. TP satim fiyati (last_seen_price) | $0.94 |
| Ort. getiri | +19.9% |
| Toplam TP geliri | $2,889 |
| TP fill orani | %95.1 (target_qty'nin %95'i gerceklesti) |

TP islemleri kârli ama `size_usdc` alani gercek geliri yansitmiyor:

```
TP satim: size_usdc = $5.00, target_qty = 7.25
Gorunen fiyat: $5.00 / 7.25 = $0.69/share (BU ALIM FIYATI!)
Gercek satim fiyati: last_seen_price = $0.94

Gercek geliri: 7.06 share * $0.94 = $6.64
Stated geliri: $5.00
Fark: $1.64 (size_usdc gercek gelirden dusuk)
```

---

## 3. Stop Loss (SL) Mekanizmasi Nasil Calisir?

### 3.1 Staged SL (Kademeli Stop Loss)

Bot, alim emri fill oldugunda kademeli SL emirleri olusturur. Ornegin:

```
SL-0: SELL YES @ 50 cent, %50 pozisyon (trigger_price=0.50, cross_below)
SL-1: SELL YES @ 40 cent, %50 pozisyon (trigger_price=0.40, cross_below)
```

**SL tetik mantigi:** `trigger_price` fiyatini asagigi gecince (cross_below) emir gonderilir.

Kod kaynaklari:
- SL kurali olusturma: `crates/bot-runner/src/trade_builder/exit_ladders.rs:242-252`
- Kademe boyutu: `size_pct` ile belirlenir (ornegin %50 + %50)
- Kardes emir politikasi: `TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING` (kademe doldugunda kalan kardesleri yeniden boyutlandir)

### 3.2 SL Tetik Fiyat Modu (`slTriggerPriceMode`)

Bot, SL tetik fiyatini belirlemek icin farkli fiyat kaynaklari kullanabilir:

| Mod | Aciklama | Davranis |
|-----|----------|---------|
| `best_bid` | Sadece best_bid fiyatini kullanir | Hizli tetik, flash crash'e duyali |
| `composite` (default) | best_bid + last_trade fiyatlari filtreler | Dengeli |
| `composite_safe` | Iki fiyat kaynagi da SL seviyesinin altinda olmali | En guvenli, late tetik |
| `composite_fast` | best_bid ve last_trade'in minimumu | Agresif |
| `last_trade` | Sadece son trade fiyatini kullanir | Nadir kullanilir |

Kod kaynaklari: `crates/bot-runner/src/lib_parts/part_018.rs:754-772`

```rust
let sl_trigger_price_mode: Option<&str> = if sl_enabled {
    let raw = node_config_string(node, "slTriggerPriceMode");
    let mode = match raw.as_deref() {
        Some("best_bid") => "best_bid",
        Some("composite") => "composite",
        Some("composite_safe") => "composite_safe",
        Some("composite_fast") => "composite_fast",
        Some("last_trade") => "last_trade",
        Some(other) => { /* bail */ }
        None => "best_bid",   // DEFAULT
    };
    Some(mode)
} else {
    None
};
```

### 3.3 SL Slippage Problemi

Staged SL emirlerinde tetik fiyati ile gercek fill fiyati arasinda onemli bir fark var:

| SL Kademesi | SL Tetik Fiyati | Gercek Fill Fiyati | Slippage |
|-------------|-----------------|--------------------|---------|
| SL-0 (1. kademe) | $0.54 | **$0.43** | -11 cent |
| SL-1 (2. kademe) | $0.45 | **$0.34** | -11 cent |

**Neden?** Polymarket orderbook'unun likidite eksikligi. SL tetiklendigi anda piyasa fiyati SL tetik fiyatindan cok daha asagi olabilir. Ornegin:
- SL @ 50 cent tetiklenir
- Bot satim emri gonderir (market order)
- Orderbook'ta 50 cent seviyesinde yeterli likidite yok
- Emir 43-47 cent seviyelerine kadar dolarak gercekleisir

### 3.4 SL Gerceklestirme Verileri

| SL Kademesi | Islem | Ort. Alim | SL Tetik | Gercek Fill | Kayip % | Toplam Zarar |
|-------------|-------|-----------|----------|------------|---------|---------------|
| SL-0 (1. kademe) | 156 | $0.68 | $0.54 | **$0.43** | -36.1% | -$496 |
| SL-1 (2. kademe) | 114 | $0.68 | $0.45 | **$0.34** | -49.2% | -$396 |
| Hard SL (tek) | 279 | $0.80 | $0.56 | $0.59 | -26.4% | -$386 |
| **Toplam** | **549** | | | | **-33.9%** | **-$1,279** |

**2. kademe SL %49 kayip yaratiyor.** Toplam SL zararinin %45'i 2. kademeden geliyor.

### 3.5 SL Fill Fiyat Dagilimi (Ucuz Alim, <=65 cent)

| Outcome | Ort. Alim | Ort. SL Fill | Geri Kazanim | Min Fill | Max Fill |
|---------|-----------|-------------|-------------|----------|----------|
| Up | $0.59 | **$0.40** | %67.3 | $0.01 | $0.99 |
| Down | $0.58 | **$0.34** | %57.8 | $0.01 | $0.60 |

**Down token SL fill fiyati ortalamada sadece $0.34.** Bazi islemlerde $0.01'e kadar dusuyor (nearly worthless).

### 3.6 Re-Entry Mekanizmasi

SL tetiklendiginde bot otomatik olarak yeniden giris yapabilir:

```
Alim: YES @ 64 cent, $5 USDC
SL @ 50 cent -> sat -> -$1.60 zarar
Re-entry 1: YES @ 64 cent, $5 USDC (AYNI parametreler)
SL @ 50 cent -> sat -> -$1.60 zarar
Re-entry 2: YES @ 64 cent, $5 USDC (AYNI parametreler)
SL @ 50 cent -> sat -> -$1.60 zarar

Toplam: 3 SL x $1.60 = -$4.80 zarar (tek islem zinciri)
```

**Re-entry ayni parametreyle yapiyor:** ayni maxPrice (64 cent), ayni PTB threshold ($10), ayni sizeUsdc ($5). SL olmus piyasada ayni giris sartlariyla tekrar girmek, genellikle ayni sonucu veriyor.

Kod kaynaklari:
- Re-entry kontrolu: `crates/bot-runner/src/trade_builder/fill_finalize.rs:1-211`
- `reentryMaxAttempts`: 1-10 arasi, mevcut deger: 2
- Re-entry PTB guard: Ayni PTB threshold ile degerlendirilir
- Re-entry boyut: Ayni `sizeUsdc` ile girilir

### 3.7 PTB Stop Loss (Alternatif SL Mekanizmasi)

Bot, sabit fiyat SL yerine PTB bazli SL kullanabilir:

```
ptbStopLossEnabled = true
ptbStopLossGapUsd = 0 (veya 5, 10, vb.)
```

**PTB SL mantigi:** Fiyat PTB'ye donunce (gap <= threshold) satis yapilir.

Kod kaynaklari: `crates/bot-runner/src/trade_builder/ptb_stop_loss.rs`

```rust
fn trade_builder_evaluate_ptb_stop_loss(order: &TradeBuilderOrder) -> Option<TradeBuilderPtbStopLossEvaluation> {
    // directional_gap = (up) ? current_chainlink_price - ptb_reference_price : ptb_reference_price - current_chainlink_price
    // should_trigger = directional_gap <= threshold_gap_usd
}
```

Migration: `migrations/056_trade_builder_ptb_stop_loss.sql`

---

## 4. Genel P&L Tablosu

| Metrik | Deger |
|--------|-------|
| Toplam Alim (Live) | $7,149 |
| Toplam Satim (Stated) | $5,137 |
| Toplam Satim (Gercek, last_seen_price) | $4,530 |
| Net Zarar (Stated) | -$2,012 |
| Net Zarar (Gercek) | -$2,619 |
| Geri Kazanim Orani | %63.4 |

**size_usdc yaniltmaciligi:** Satim emirlerinde `size_usdc` alani alim referans fiyatini kullanir, gercek satim fiyatini degil. Bu nedenle stated geliry gercekten $607 az gorunuyor.

Kod kaynaklari:
- Satim `size_usdc` hesaplamasi: `crates/bot-runner/src/trade_builder/exit_math.rs:765-776`
- `size_usdc = target_qty * execution_price` (execution_price = alim fiyati)

---

## 5. TP vs SL Dagilimi

| Satim Tipi | Islem | Pay | Ort. Alim | Ort. Satim | Ort. Getiri | Toplam Gelir |
|-----------|-------|-----|-----------|-----------|-------------|--------------|
| TP (cross_above) | 668 | 51% | $0.80 | $0.94 | +19.9% | $2,889 |
| SL (cross_below) | 645 | 49% | $0.73 | $0.47 | -33.9% | $1,641 |

**TP islemleri kârli (+$2,889 gelir, +%19.9 getiri) ama SL islemleri cok daha buyuk zarar veriyor (-$1,641 gelir, -%33.9 getiri).**

### 5.1 Eslestirilmis Islemlerde P&L

66 eslestirilmis (alim + satim) islemde:
- Toplam alim: $6,456
- Toplam satim: $5,137
- Net zarar: -$1,319
- **Hicbir eslestirilmis islem kârl degil**
- Geri kazanim orani: %79.6

### 5.2 TP/SL Outcome Dagilimi (Ucuz Alim, <=65 cent)

| Outcome | TP Orani | TP Kâr | SL Zarar |
|---------|----------|--------|----------|
| Up (YES) | %43.2 | +49.6% | -32.7% |
| Down (NO) | %35.7 | +46.4% | **-42.2%** |

**Down taraf ucuz alisda ozellikle kotu:** hem dusuk TP orani hem yuksek SL zarari.

---

## 6. Alim Fiyatina Gore TP/SL Orani

| Alim Fiyati | Islem | TP | SL | TP Orani | TP Kâr | SL Zarar | Net EV |
|-------------|-------|----|----|----------|--------|----------|---------|
| <=55 cent | 55 | 13 | 42 | %23.6 | +68.7% | -37.8% | -12.2% |
| 56-65 cent | 196 | 85 | 111 | %43.4 | +44.8% | -38.5% | -8.3% |
| 66-75 cent | 185 | 78 | 107 | %42.2 | +27.9% | -36.4% | -9.7% |
| 76-85 cent | 392 | 212 | 180 | %54.1 | +16.4% | -28.2% | -1.4% |
| >85 cent | 326 | 217 | 109 | %66.6 | +7.9% | -34.6% | -6.2% |

**Her fiyat bracket'i negatif EV.** En az zararli: 76-85 cent (%54 TP, -%1.4 EV). En cok zararli: <=55 cent (%24 TP, -%12.2 EV).

### 6.1 Beklenen Deger (Expected Value) Hesabi

Her alım fiyatı icin beklenen deger:
```
EV = (TP_orani * TP_kâr_orani) - ((1 - TP_orani) * SL_zarar_orani)
```

| Alim Fiyati | TP Orani | TP Kâr | SL Zarar | Beklenen Kâr | Beklenen Zarar | **Net EV** |
|-------------|----------|--------|----------|-------------|---------------|-----------|
| 55 cent | %24 | +69% | -38% | +$16.5 | -$28.7 | **-$12.2** |
| 60 cent | %33 | +52% | -38% | +$17.2 | -$25.5 | **-$8.3** |
| 65 cent | %43 | +45% | -38% | +$19.4 | -$21.7 | **-$2.3** |
| 70 cent | %42 | +28% | -37% | +$11.8 | -$21.5 | **-$9.7** |
| 75 cent | %54 | +23% | -30% | +$12.4 | -$13.8 | **-$1.4** |
| 80 cent | %54 | +17% | -28% | +$9.2 | -$12.9 | **-$3.7** |
| 85 cent | %60 | +12% | -33% | +$7.2 | -$13.2 | **-$6.0** |

**Her fiyat seviyesi negatif EV.** En az zararli iki nokta: 65 cent (-%2.3) ve 75 cent (-%1.4).

---

## 7. Ayni Trade'de Up + Down Birlikte Sonucu

| Up Sonucu | Down Sonucu | Sayi | Yorum |
|-----------|-------------|------|--------|
| SL | SL | 59 | Yon belirsiz, piyasa yatay |
| TP | SL | 48 | BTC yukseldi (Up kazandi) |
| SL | TP | 42 | BTC dustu (Down kazandi) |
| TP | TP | 12 | Anomali |

**%42'sinde her iki taraf da SL yiyor.** Bu, yon belirsiz oldugunda dual-side stratejisinin cift zarar verdigini gosteriyor.

### 7.1 Dual-Side Zarar Mekanizmasi

Bot YES ve NO token'larini ayni anda aliyor:

```
YES @ 60 cent al + NO @ 40 cent al = $1.00/market payi
```

Polymarket fee yapisi (10% on profit):
- YES kazanirsa: YES $1.00 cozulur, NO $0.00 olur. Net: $1.00 - $0.40 (NO kaybi) - fee = az kâr
- NO kazanirsa: NO $1.00 cozulur, YES $0.00 olur. Net: $1.00 - $0.60 (YES kaybi) - fee = az kâr

Yon belirsiz oldugunda (%42): her iki taraf da SL yiyor, cift zarar.

---

## 8. Re-Entry Sorunu

### 8.1 Mevcut Re-Entry Davranisi

```
Islem: YES @ 64 cent al, $5 USDC
SL @ 50 cent -> sat @ ~43 cent -> -$1.60 zarar
Re-entry 1: YES @ 64 cent al, $5 USDC (AYNI parametreler)
SL @ 50 cent -> sat @ ~43 cent -> -$1.60 zarar
Re-entry 2: YES @ 64 cent al, $5 USDC (AYNI parametreler)
SL @ 50 cent -> sat @ ~43 cent -> -$1.60 zarar

Toplam: 3 SL x $1.60 = -$4.80 zarar (tek islem zinciri)
```

**Re-entry ayni parametreyle yapiyor:** ayni maxPrice (64 cent), ayni PTB threshold ($10), ayni sizeUsdc ($5). SL olmus piyasada ayni giris sartlariyla tekrar girmek, genellikle ayni sonucu veriyor.

### 8.2 Re-Entry Parametreleri

- `reenterOnSlHit = true`: SL'den sonra re-entry yap
- `reentryMaxAttempts = 2`: En fazla 2 kez tekrar gir
- `stagedSlReentryOnlyAfterAllStages = false`: Tum SL kademeleri bitmeden re-entry yapabilir

Kod kaynaklari:
- Re-entry kontrolu: `crates/bot-runner/src/trade_builder/fill_finalize.rs:1-211`
- Re-entry zamanlamasi: `crates/bot-runner/src/lib_parts/part_018.rs:773-801`
- Re-entry PTB guard: Ayni PTB threshold ile degerlendirilir

---

## 9. Saat Bazli TP Orani (Ucuz Alim, <=65 cent)

| Saat (UTC) | Islem | TP Orani | Yorum |
|-------------|-------|----------|--------|
| 00:00 | 21 | %61.9 | iyi |
| 01:00 | 28 | %50.0 | orta |
| 02:00 | 13 | %53.8 | orta |
| 03:00 | 27 | %25.9 | kotu |
| 04:00 | 14 | %35.7 | kotu |
| 05:00 | 14 | %64.3 | iyi |
| 06:00 | 3 | %0.0 | cok kotu (az veri) |
| 08:00 | 2 | %0.0 | cok kotu (az veri) |
| 09:00 | 7 | %71.4 | iyi (az veri) |
| 10:00 | 3 | %0.0 | kotu (az veri) |
| 11:00 | 4 | %25.0 | kotu |
| 13:00 | 7 | %42.9 | orta |
| 14:00 | 14 | %35.7 | kotu |
| 15:00 | 10 | %40.0 | orta |
| 16:00 | 25 | %44.0 | orta |
| 17:00 | 18 | %38.9 | kotu |
| 18:00 | 18 | %11.1 | cok kotu |
| 19:00 | 12 | %0.0 | felaket |
| 22:00 | 1 | %100.0 | (az veri) |
| 23:00 | 10 | %40.0 | orta |

**UTC 00:00-05:00 (Asya saatleri) ve UTC 09:00 iyimser.** UTC 18:00-19:00 (ABD aksami) felaket.

### 9.1 Saatlerin Anlami (Turkiye Saati)

| UTC | Turkiye (UTC+3) | TP Orani | Yorum |
|-----|-----------------|----------|--------|
| 00:00 | 03:00 (gece) | %61.9 | iyi |
| 03:00 | 06:00 (sabah) | %25.9 | kotu |
| 09:00 | 12:00 (ogle) | %71.4 | iyi |
| 18:00 | 21:00 (aksam) | %11.1 | kotu |
| 19:00 | 22:00 (gece) | %0.0 | felaket |

---

## 10. Varlik Bazli TP Orani

### 10.1 Ucuz Alim (<=65 cent)

| Varlik | Islem | TP Orani |
|--------|-------|----------|
| BTC 5m | 96 | %30.2 |
| ETH 5m | 154 | %44.8 |
| SOL 5m | 1 | %0.0 (1 islem) |

**BTC 5m ucuz alimda en kotu performans (%30.2 TP).** ETH nispeten daha iyi (%44.8).

### 10.2 Tum Fiyatlar (Up only, 5m piyasalari)

| Varlik | Islem | TP | SL | TP Orani |
|--------|-------|----|----|----------|
| BTC (Up, 5m) | <=65c: 42 / >65c: 208 | 14 | 28 | %33.3 / %56.3 |
| ETH (Up, 5m) | <- | - | - | %49.3 |
| SOL (Up, 5m) | <- | - | - | %51.9 |

### 10.3 Up vs Down Token Karsilastirmasi (Ucuz Alim, <=65 cent)

| Outcome | Islem | TP Orani | TP Kâr | SL Zarar |
|---------|-------|----------|--------|----------|
| Up (YES) | 111 | %43.2 | +49.6% | -32.7% |
| Down (NO) | 140 | %35.7 | +46.4% | **-42.2%** |

**Down token ucuz alisda hem dusuk TP orani hem yuksek SL zarari.**

---

## 11. Fee ve Buffer Kaybi

### 11.1 Polymarket Fee Yapisi

```
fee_rate_bps = 1000 (default)
fee_curve_rate = 1000 / 4000 = 0.25
fee_shares = gross_qty * fee_curve_rate * (price * (1-price))^2 / price
```

Ornek: 7.25 share @ $0.69 alindiginda:
- fee_shares ≈ 0.12 share
- net_qty = 7.25 - 0.12 = 7.13
- buffer = max(7.25 * 0.01, 0.03) = 0.073
- visible_qty = 7.13 - 0.07 = 7.05

**7.25 share al, sadece 7.05 share satilabilir.** Bu %2.8 kayip per islem.

Kod kaynaklari:
- Fee hesaplamasi: `crates/bot-runner/src/lib_parts/part_021.rs:682-698`
- Buffer hesaplamasi: `crates/bot-runner/src/lib_parts/part_000.rs:73-74`
- `TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER = 0.03`
- `TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER_RATE = 0.01`

### 11.2 exit_price_capped

22,567 kez `exit_price_capped` event tetiklenmis. Bu, TP satim fiyatinin `trigger_price - 0.05` ile sinirlandirilmasi demek.

TP fiyat 0.98 ise: floor = 0.98 - 0.05 = 0.93. Piyasa fiyati 0.80 ise, satim 0.80'den yapilir (capped degil). Ama piyasa fiyati 0.96 ise, satim 0.93'den yapilir (capped).

Kod kaynagi: `crates/bot-runner/src/trade_builder/exit_math.rs:362-383`

---

## 12. PTB Guard Verimi

| Metrik | Deger |
|--------|-------|
| Toplam PTB guard degerlendirmesi | 2,971,722 |
| PTB guard bloklama | 2,971,722 (%99.96) |
| PTB guard gecis | 11,388 (%0.04) |

**%99.96 PTB guard bloklama orani.** Bot cok secici ama girdigi islemlerin %65'i SL oluyor.

Kod kaynaklari:
- PTB guard mantigi: `crates/bot-runner/src/trade_builder/price_to_beat.rs`
- PTB modu (manual/auto_last_3_avg_excursion/auto_vol_pct): `crates/bot-runner/src/lib_parts/part_018.rs`

---

## 13. Sorunlar Hiyerarsisi

### 13.1 Ana Sorun: Giris Kalitesi

**TP orani %35 ile uzun vadeli kârlilik mumkun degil.** Her SL basina ~%34 zarar, her TP basina ~%20 kâr. Break-even icin minimum %63 TP orani gerekiyor. Mevcut TP orani bunun yarisindan az.

**Neden?** 5 dakikalik Up/Down piyasalarinda token fiyati (YES/NO) olasiligi yansitir:
- YES @ 60 cent = piyasa %60 olasilikla yuksek diyor
- Ama %60 olasilik demek %40 SL olasilik demek
- Bot maxPrice=64 ile aldiginda, token fiyati genellikle 55-65 cent arasinda olur
- Bu da %55-65 olasilik demek ve SL orani %35-45 olur

**Temel celiski:** Ucuzdan almak kâr marjini artirir ama TP olasiligini dusurur. Pahaliya almak TP olasiligini artitirr ama kâr marjini dusurur. Her iki durumda da negatif EV.

### 13.2 Ikincil Sorunlar

1. **SL zarari buyuk:** Ortalama -%34 kayip, staged SL 2. kademe -%49 kayip
2. **Re-entry ayni parametrelerle:** SL'den sonra ayni sartlarla tekrar girmek zarari katliyor
3. **Dual-side zarar:** %42'sinde her iki taraf SL yiyor
4. **Saat bazli kotu performans:** UTC 18:00-19:00'da %0 TP orani
5. **BTC kotu performans:** BTC 5m ucuz alimda %30.2 TP orani
6. **Down taraf kotu performans:** %35.7 TP orani, -%42.2 SL zarari
7. **size_usdc yaniltmaciligi:** DB'de SL gelir gercekte farkli
8. **Slippage:** Staged SL'de 11 cent slippage (tetik fiyati vs gercek fill fiyati)
9. **Fee ve buffer kaybi:** %2.8 pay kaybi per islem
10. **PTB guard %99.6 bloklama:** 2,971,722 bloklama vs 11,388 gecis