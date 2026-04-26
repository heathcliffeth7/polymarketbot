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
