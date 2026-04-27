# 04 - PTB Guard ve IV Mismatch

Bu dosya Price-to-Beat guard'ın klasik modlarını ve yeni `iv_mismatch_edge` karar modelini anlatır.

## Amaç

PTB guard, "bu token fiyatı alım için yeterince iyi mi?" sorusunu cevaplar. Sadece token fiyatına değil, underlying fiyat hareketine, orderbook'a, Binance teyidine ve kalan süreye de bakabilir.

## PTB Modları

| Mod | Kullanım |
|---|---|
| `manual` | Sabit min/max gap ile giriş kalitesi kontrolü |
| `auto_last_3_avg_excursion` | Son market hareketlerinden otomatik threshold üretimi |
| `auto_vol_pct` | Volatiliteye göre yüzde bazlı threshold |
| `signal_formula` | Sinyal formülüne dayalı karar |
| `iv_mismatch_edge` | Implied probability ile underlying hareketi arasındaki edge'i hesaplar |

Klasik modlar daha basit ve açıklaması kolaydır. `iv_mismatch_edge` daha fazla veri ister ama chop, ters book ve pahalı girişleri daha iyi filtreler.

## `iv_mismatch_edge` Ne Yapar?

Model kabaca şu kararları birlikte değerlendirir:

- Token ask fiyatı implied probability olarak ne söylüyor?
- Chainlink/RTDS underlying hareketi seçili yönü destekliyor mu?
- Binance fiyatı taze mi ve aynı yönde mi?
- YES/NO orderbook mid ve depth seçili yönü doğruluyor mu?
- Kalan süreye göre beklenen hareket yeterli mi?
- Fiyat çok iyi görünüyor ama book/model ayrışması şüpheli mi?
- Son hacim normal, düşük, yüksek veya extreme mi?

Sonuçta `edge`, `gap_strength`, `threshold` ve `adaptive_regime` üretilir.

## Temel Config

```json
{
  "side": "buy",
  "executionMode": "market",
  "priceToBeatGuardEnabled": true,
  "priceToBeatMode": "iv_mismatch_edge",
  "priceToBeatIvProtectionMode": "adaptive",
  "priceToBeatIvDepthGuardEnabled": true,
  "priceToBeatIvRequireBinanceFreshUnderSec": 60,
  "priceToBeatIvBinanceMaxStaleMs": 2000
}
```

## Zaman Kuralları

`priceToBeatIvTimeRules` kalan süreye göre threshold seçer.

```json
{
  "priceToBeatIvTimeRules": [
    {
      "startRemainingSec": 180,
      "endRemainingSec": 90,
      "maxPriceCent": 62,
      "minEdge": 0.08,
      "minGapStrength": 0.85,
      "minExpectedMoveUsd": 10
    },
    {
      "startRemainingSec": 90,
      "endRemainingSec": 30,
      "maxPriceCent": 68,
      "minEdge": 0.12,
      "minGapStrength": 1.10,
      "minExpectedMoveUsd": 18
    }
  ]
}
```

Yorum:

- Erken bölümde daha ucuz fiyat aranır.
- Geç bölümde daha pahalı fiyat kabul edilebilir ama edge/gap şartı sıkılaşır.
- `maxPriceCent` action max price ile birlikte düşünülmelidir.

## Adaptive Rejimler

| Rejim | Tipik durum | Davranış |
|---|---|---|
| `green` | Book seçili yönü destekler, Binance aynı yönde, hacim normal | Edge ve gap şartları sınırlı gevşeyebilir |
| `orange` | Book karşı sinyal veriyor veya hacim/chop riski yükseliyor | Şartlar sıkılaşır |
| `red` | Güvenilir karşı book, extreme hacim veya sert uyumsuzluk | Block edebilir |

Adaptive ayarlar:

- `priceToBeatIvAdaptiveGreenEdgeDelta`
- `priceToBeatIvAdaptiveGreenGapStrengthDelta`
- `priceToBeatIvAdaptiveOrangeEdgeDelta`
- `priceToBeatIvAdaptiveOrangeGapStrengthDelta`
- `priceToBeatIvAdaptiveOrangeGapUsdMarginDelta`
- `priceToBeatIvAdaptiveRedBlock`

## Senaryo A: Green Pass

Durum:

- Ask 0.58.
- Underlying Up yönünde hareket ediyor.
- Binance taze ve aynı yönde.
- Book selected side lehine.
- Depth hedef notional'ı taşıyor.

Beklenen:

- `adaptive_regime=green`.
- `edge >= threshold`.
- `gap_strength >= required_gap_strength`.
- Guard pass eder ve order üretimi devam eder.

## Senaryo B: Orange Penalty

Durum:

- Underlying hareket olumlu.
- Token fiyatı iyi görünüyor.
- Ama opposite book mid seçili yöne karşı güçlü.

Beklenen:

- Model soft penalty uygular.
- `dynamic_threshold` yükselir veya `required_gap_strength` artar.
- Edge hâlâ yeterliyse pass, değilse block.

Operatör yorumu:

- Orange block tek başına hata değildir. Book güvenilirliği zayıfsa bot pahalı/yanlış yöne atlamayı engeller.

## Senaryo C: Red Block

Durum:

- Extreme hourly volume.
- Binance ters yönde veya stale.
- Book karşı tarafı açıkça destekliyor.

Beklenen:

- `adaptive_regime=red`.
- `priceToBeatIvAdaptiveRedBlock=true` ise guard block eder.
- Telegram/no-order analytics içinde PTB block nedeni görünür.

## Depth Guard

`priceToBeatIvDepthGuardEnabled=true` ise bot sadece best ask fiyatına değil, hedef miktarın VWAP/slippage etkisine bakar.

Örnek:

- Best ask 0.57 ama sadece 2 USDC derinlik var.
- Hedef order 20 USDC.
- VWAP 0.66'ya çıkıyorsa guard block edebilir.

İlgili alan:

- `priceToBeatIvDepthMaxSlippage`

## Binance ve Momentum Koruması

Önemli alanlar:

- `priceToBeatIvRequireBinanceFreshUnderSec`
- `priceToBeatIvBinanceMaxStaleMs`
- `priceToBeatIvRequireBinanceSameDirection`
- `priceToBeatIvMomentumProtectionEnabled`
- `priceToBeatIvDropZBlockThreshold`

Kullanım:

- Binance çok eskiyse edge'e güvenme.
- Binance aynı yönde değilse penalty veya block uygula.
- Ani drop/z-score riski varsa geç girişleri sıkılaştır.

## Telemetry Alanları

`price_to_beat_guard.iv_mismatch_edge` altında şu alanlar aranır:

- `q`, `q_final`
- `edge`
- `cost`
- `threshold`
- `dynamic_threshold`
- `gap_strength`
- `required_gap_strength`
- `gap_usd_margin`
- `binance_price`
- `binance_staleness_ms`
- `binance_same_direction`
- `depth_guard_result`
- `estimated_avg_fill`
- `vwap_slippage`
- `adaptive_regime`
- `hourly_volume_ratio`
- `book_reliability`

## Operatör Checklist

- `priceToBeatGuardEnabled=true` mi?
- `priceToBeatMode="iv_mismatch_edge"` mi?
- Binance/RTDS veri tazeliği makul mü?
- `priceToBeatIvTimeRules` kalan süreyi kapsıyor mu?
- Block sebebi edge yetersizliği mi, depth mi, Binance mı, book mu?
- Max price ve PTB ayrı guard'lar olduğu için hangisi block etmiş ayrıştırıldı mı?

## Klasik PTB ile IV Edge Arasındaki Fark

Klasik PTB genellikle şunu sorar:

```text
Underlying fiyat, marketin price-to-beat seviyesine göre yeterince avantajlı mı?
```

`iv_mismatch_edge` ise daha geniş sorar:

```text
Token'ın implied probability fiyatı, underlying hareketi ve orderbook sinyaliyle kıyaslandığında pozitif edge veriyor mu?
```

Bu fark pratikte şuna yol açar:

- Manual PTB pass ederken IV edge block edebilir.
- Çünkü manual gap yeterli olsa bile book karşı yönü destekliyor olabilir.
- Ya da depth yetersiz olduğu için best ask seviyesi gerçek alım maliyetini temsil etmiyor olabilir.

## Sayısal Örnek: İyi Görünen Ama Block Edilen Giriş

Durum:

```text
Up ask = 0.57
Manual PTB gap = 28 USD
Manual min gap = 20 USD
Binance yönü = aşağı
Opposite book mid = 0.62
Depth ile beklenen avg fill = 0.66
```

Manual PTB yorumu:

- 28 USD > 20 USD, pass edebilir.

IV edge yorumu:

- Binance ters.
- Opposite book güçlü.
- Hedef büyüklükte avg fill pahalı.
- `adaptive_regime=orange` veya `red`.
- Edge threshold altına düşer ve block edebilir.

Bu durumda block, fiyatı kaçırmak değil kötü sinyal birleşimini filtrelemektir.

## Formula Okuma İçin Basit Model

Telemetry'deki alanları şu şekilde düşün:

| Alan | Pratik anlam |
|---|---|
| `q` | Modelin ham kazanma olasılığı tahmini |
| `q_final` | Penalty/credit sonrası son olasılık |
| `cost` | Ask, fee ve buffer sonrası efektif maliyet |
| `edge` | `q_final - cost` gibi okunabilecek net avantaj |
| `threshold` | Minimum kabul edilen edge |
| `dynamic_threshold` | Rejim ve penalty sonrası efektif threshold |
| `gap_strength` | Underlying hareketinin yeterliliği |
| `required_gap_strength` | Bu giriş için gereken gap gücü |

Bu alanlar tek başına değil birlikte okunur. `edge` yüksek ama `gap_strength` düşükse model fiyatı ucuz görse bile hareket teyidi yetersiz olabilir.

## Time Rule Tasarım Rehberi

Geç marketlerde üç risk artar:

1. Fill için zaman azalır.
2. Resolution'a yaklaştıkça fiyat daha sert sıçrayabilir.
3. Yanlış yönde kalırsa exit şansı azalır.

Bu yüzden geç time rule genellikle:

- Daha yüksek `minEdge`.
- Daha yüksek `minGapStrength`.
- Daha yüksek `minExpectedMoveUsd`.
- Daha net Binance same direction şartı.
- Daha düşük veya dikkatli `maxPriceCent`.

Eğer geç time rule sadece `maxPriceCent` değerini yükseltiyorsa ama risk şartlarını artırmıyorsa, strateji geç FOMO'ya dönüşebilir.

## Depth Guard Neden Gerekli?

Best ask küçük order için anlamlıdır, ama hedef order büyükse yanıltıcıdır.

Örnek:

```text
hedef order = 30 USDC
ask 0.55 depth 3 USDC
ask 0.60 depth 7 USDC
ask 0.69 depth 20 USDC
```

Best ask 0.55 görünür. Ancak 30 USDC order'ın ortalama maliyeti 0.65 civarına çıkabilir. `priceToBeatIvDepthMaxSlippage` bu farkı sınırlamak için vardır.

## Binance Staleness Yorumu

Binance veya external spot verisi stale ise modelin underlying hareket teyidi zayıflar.

Örnek:

- `priceToBeatIvBinanceMaxStaleMs=2000`.
- Gelen Binance tick yaşı 4800 ms.

Beklenen:

- Same direction teyidi güvenilir sayılmaz.
- Penalty uygulanabilir veya block oluşabilir.

Bu durum botun Polymarket tarafını görmediği anlamına gelmez; external teyit taze olmadığı için karar kalitesi düşmüştür.

## Block Sebebini Ayırma

PTB block içinde farklı alt sebepler vardır:

| Alt sebep | Nasıl anlaşılır |
|---|---|
| Edge düşük | `edge < threshold` veya `edge < dynamic_threshold` |
| Gap zayıf | `gap_strength < required_gap_strength` |
| Binance ters/stale | `binance_same_direction=false` veya staleness yüksek |
| Depth fail | `depth_guard_result` fail veya slippage yüksek |
| Red regime | `adaptive_regime=red` ve red block açık |
| Model/book ayrışması | model-book gap warn/hard alanları |

Operatör önce bu alt sebebi bulmalı, sonra config değiştirmelidir.
