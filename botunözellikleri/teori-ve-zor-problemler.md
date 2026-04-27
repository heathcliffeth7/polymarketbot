# Teori ve Zor Problemler

Guncelleme tarihi: 2026-04-27

Bu dosya botun calisma seklini sadece ozellik listesi olarak degil, ispat ve karsi ornek uzerinden anlatir. Amac su soruya disiplinli cevap vermektir:

```text
5 dakikalik Up/Down markette botun order acmamasi, tek bacak acmasi
veya pair lock kurmasi ne zaman dogru davranistir?
```

Buradaki "ispat" akademik kesinlik degil, runtime karar zincirinin hangi varsayimlarla savunulabilir oldugunu gosteren operasyonel ispattir. Her iddia icin karsit bir ornek de verilir; cunku trade botunda en tehlikeli hata, tek bir sinyali yeterli kanit sanmaktir.

## Bilinen Guclu Sonuclar

Prediction market ve betting teorisinden bot tasarimina dogrudan yansiyan dort guclu fikir vardir:

1. Binary prediction market fiyati uygun varsayimlarda olay olasiligi gibi okunabilir, ama bu bir yasa degildir. Risk tercihleri, inanc dagilimi, likidite ve katilimci yapisi fiyat ile gercek olasilik arasinda sapma uretir. Wolfers ve Zitzewitz'in prediction market calismalari bu fiyatlarin genellikle faydali ama bazen bias'li olasilik tahminleri oldugunu anlatir.
2. Proper scoring ve market scoring rule fikri, piyasanin bilgi toplamasi icin tesvik mekanizmasi verir. Hanson'in LMSR sonucu, ortak tahminin nasil uretilebildigini ve logaritmik kurallarin bazi lokal inference ozelliklerini aciklar.
3. Kelly kriteri, edge varsa bankroll'un nasil buyutulecegini log-growth hedefiyle baglar. Ama edge yanlis tahmin edilirse agresif sizing zarari buyutur; bu bot icin fractional, capped ve guard'li sizing ihtiyacini destekler.
4. No-arbitrage dusuncesi, iki binary bacagin toplam maliyeti 1 USDC altinda gorunse bile bunun otomatik risksiz kar olmadigini soyler. Fee, spread, depth, partial fill, submit latency ve resolution operasyonu toplam maliyeti degistirir.

Kaynaklar:

- Wolfers/Zitzewitz, "Prediction Markets", 2004: https://users.nber.org/~jwolfers/papers/Predictionmarkets.pdf
- Wolfers/Zitzewitz, "Interpreting Prediction Market Prices as Probabilities", 2006: https://www.nber.org/papers/w12200.pdf
- Hanson, "Logarithmic Market Scoring Rules", 2002: https://hanson.gmu.edu/mktscore.pdf
- Kelly, "A New Interpretation of Information Rate", 1956: https://www.princeton.edu/~wbialek/rome/refs/kelly_56.pdf

## Formal Model

Tek market penceresi icin:

```text
S = secilen outcome: Up veya Down
p = market token ask fiyati
c = fee, buffer ve depth sonrasi effective cost
q = modelin ham kazanma olasiligi
q_final = Binance, book, stale, depth ve adaptive penalty sonrasi olasilik
e = edge = q_final - c
theta = dynamic_threshold
g = gap_strength
gamma = required_gap_strength
```

Botun buy karari kabaca su zincirde okunur:

```text
trigger pass
  -> market/token/context dogru mu?
  -> action risk ve stale kontrolleri
  -> max price / execution floor
  -> PTB veya iv_mismatch_edge
  -> depth ve external teyit
  -> builder order submit
  -> fill
  -> exit, re-entry veya pair lifecycle
```

Bu zincirden su invariant cikar:

```text
trigger pass tek basina trade acildi demek degildir.
trade acildi diyebilmek icin action guard'lari gecip builder order submit/fill akisi gorulmelidir.
```

## Lemma 1 - Trigger Pass Order Garantisi Degildir

Iddia:

```text
trigger.market_price pass=true uretirse action.place_order'in order uretmesi zorunlu degildir.
```

Ispat fikri:

`trigger.market_price` market slug, token, fiyat kosulu, binding ve entry timing context'i uretir. `action.place_order` ise ayrica stale market, max price, execution floor, PTB/IV edge, risk, sizing ve pair lock kararlarini calistirir. Zincirde sonraki herhangi bir guard block ederse order uretilmez.

Karsi ornek:

```text
Trigger:
  Up fiyati 0.62
  guardTriggerPrice = 0.60
  pass=true

Action:
  maxPriceCent = 61
  best ask = 0.62

Sonuc:
  trigger pass var
  action max price block eder
  submit yok, fill yok
```

Bot kaniti:

- Trigger output'ta `pass=true`, `marketSlug`, `tokenId`, `outcomeLabel` aranir.
- Action telemetry'de block sebebi aranir.
- Submit event yoksa "fill olmadi" degil, "order uretilmedi veya submit edilmedi" denmelidir.

## Lemma 2 - Ucuz Token Edge Degildir

Iddia:

```text
Token ask fiyati dusuk diye trade pozitif EV sayilamaz.
```

Ispat fikri:

Binary token kazanirsa 1, kaybederse 0 oder. Share basina yaklasik beklenen deger:

```text
EV = q_final - cost
```

Bot sadece `EV > 0` aramaz; model hatasi, latency, spread ve rejim riski icin `dynamic_threshold` ister. Bu yuzden pratik pass kosulu:

```text
q_final - cost >= dynamic_threshold
gap_strength >= required_gap_strength
```

Karsi ornek:

```text
Ask = 0.57
Cost = 0.59
q_final = 0.63
edge = 0.04
dynamic_threshold = 0.08

0.04 < 0.08 -> block
```

Yuzeyden bakinca 57 cent ucuz gorunur. Ama fee/buffer/depth sonrasi cost 59 cent'e cikmis, model edge'i de gerekli margin'i karsilamamistir.

Bot kaniti:

- `price_to_beat_guard.iv_mismatch_edge.q_final`
- `cost`
- `edge`
- `dynamic_threshold`
- `gap_strength`
- `required_gap_strength`
- `depth_guard_result`

Bu alanlar birlikte okunmadan "bot iyi fiyati kacirdi" denemez.

## Lemma 3 - Manual PTB Pass IV Edge Pass Demek Degildir

Iddia:

```text
Manual PTB gap yeterli olsa bile iv_mismatch_edge block edebilir ve bu dogru olabilir.
```

Ispat fikri:

Manual PTB temelde underlying ile price-to-beat arasindaki gap'e bakar. `iv_mismatch_edge` ise token implied probability, Binance teyidi, orderbook mid, depth, volume rejimi ve zaman kuralini birlikte degerlendirir. Daha fazla kanit isteyen model daha az trade alir ama yanlis pozitifleri azaltir.

Karsi ornek:

```text
Manual PTB:
  gap = 28 USD
  min gap = 20 USD
  sonuc = pass

IV edge:
  Binance same direction = false
  opposite book mid guclu
  estimated_avg_fill = 0.66
  q_final = 0.69
  cost = 0.66
  dynamic_threshold = 0.08
  edge = 0.03

0.03 < 0.08 -> block
```

Burada manual gap dogru olabilir, ama piyasa ve external teyit sinyal kalitesini dusurmustur.

Bot kaniti:

- `threshold_mode="iv_mismatch_edge"`
- `binance_same_direction=false`
- `estimated_avg_fill`
- `adaptive_regime`
- `edge < dynamic_threshold`

## Lemma 4 - Pair Lock Risksiz Arbitraj Degildir

Iddia:

```text
YES ask + NO ask < 1.00 gormek tek basina risksiz kar ispati degildir.
```

Ispat fikri:

Binary resolution'da YES ve NO toplam odemesi 1 USDC'dir. Teoride iki bacagi toplam 0.97'ye almak buffer verir. Ama bot market buy veya limit submit ettiginde gercek maliyet best ask toplamindan farkli olabilir:

```text
effective_total = yes_vwap + no_vwap + fee + buffer + orphan_risk_cost
```

Pass icin ham toplam degil, effective toplam ve fill edilebilirlik gerekir.

Karsi ornek:

```text
YES best ask = 0.47, depth = 2 USDC
NO best ask = 0.48, depth = 2 USDC
Target = 20 USDC

Ham toplam = 0.95
VWAP sonrasi:
  YES effective = 0.54
  NO effective = 0.53
Effective toplam = 1.07

Sonuc:
  ham pair ucuz
  gercek hedef buyuklugunde pair zararli
```

Bot kaniti:

- `pair_total`
- `target_qty`
- counter leg price/depth
- `pair_lock_edge_decision`
- orphan veya protective unwind event'leri

## Lemma 5 - No Decision Hata Olmayabilir

Iddia:

```text
edge_pairlock_v1 icin pair_lock_edge_no_decision beklenen ve dogru bir karar olabilir.
```

Ispat fikri:

`edge_pairlock_v1` karar sirasi:

```text
1. Mevcut tek bacagi counter ile kilitle
2. Yeni esit pair ac
3. Pair yoksa guclu tek taraf edge al
4. Hicbiri yoksa no decision
```

Bu sirada hicbir kosul saglanmiyorsa order uretmemek risk azaltan karardir.

Karsi ornek:

```text
Open leg yok
YES + NO effective toplam = 1.02
single edge = 0.03
pairLockSingleEdgeThreshold = 0.08
depth yetersiz

Sonuc:
  fresh pair yok
  single edge yok
  no decision dogru
```

Bot kaniti:

- `pair_lock_strategy="edge_pairlock_v1"`
- `pair_lock_edge_decision` yoksa veya no decision reason varsa
- `priceToBeatMode="iv_mismatch_edge"` acik mi?
- `pairLockSingleEdgeThreshold` ve effective pair cost birlikte okunur.

## Lemma 6 - Bump ve Relax Alpha Degil, Feedback Kontroludur

Iddia:

```text
PTB bump ve max price relax stratejiye kendi basina edge eklemez.
```

Ispat fikri:

Bump, zarar sonrasi giris threshold'unu artirir. Relax, cok sayida miss sonrasi gecmiste tradeable firsat olduguna dair kanit varsa max price/PTB kosulunu kontrollu gevsetir. Ikisi de sinyal uretmez; yalnizca mevcut sinyalin kabul esigini degistirir.

Karsi ornek:

```text
SL serisi var
Relax da acik
Depth zayif
Binance ters
IV edge red

Relax max price'i gevsetse bile IV edge ve depth block dogru kalabilir.
```

Bot kaniti:

- `stop_loss_bump_count`
- `stop_loss_bump_usd`
- `stop_loss_bump_capped`
- relax credit veya `relax_miss_reason`
- final effective threshold

Sadece "trade az" diye relax acmak ispat degildir. Ispat icin gecmis marketlerde gercekten fill edilebilir firsat oldugu gosterilmelidir.

## Lemma 7 - Re-entry Negatif EV'yi Buyutebilir

Iddia:

```text
Ayni markette ayni kosullarla re-entry yapmak, ilk giris hatasini tekrar edebilir.
```

Ispat fikri:

SL olmasi tek basina stratejinin kotu oldugunu ispatlamaz. Ama re-entry ayni `maxPrice`, ayni PTB threshold, ayni zayif depth ve ayni chop rejiminde calisiyorsa bagimsiz yeni firsat degil, ayni hatanin tekrari olabilir.

Karsi ornek:

```text
Entry 1:
  buy 0.64
  SL fill 0.43
  loss = 0.21/share

Re-entry:
  ayni window
  ayni q_final/cost kosulu
  ayni zayif book
  tekrar buy 0.64
  tekrar SL 0.43

Toplam kayip yaklasik iki katina cikar.
```

Bot kaniti:

- `reentry_generation`
- `reentry_attempts_used`
- `reentrySkipCurrentWindow`
- `reentryPriceToBeatMaxDiff`
- `reentryThresholdDecay`
- yeni giriste `q_final`, `cost`, `depth_guard_result`

Re-entry icin kabul edilebilir kanit, sadece "SL oldu" degil; piyasa state'inin veya threshold'un daha iyi hale geldigidir.

## Zor Problem - Tam Kanit veya Karsi Ornek

Problem:

```text
Bir BTC 5m Up/Down penceresinde botun su karari verdigini dusun:

1. Trigger Up icin pass=true uretmis.
2. Action priceToBeatMode="iv_mismatch_edge" kullanmis.
3. Pair lock strategy edge_pairlock_v1.
4. Bot order acmamis ve pair_lock_edge_no_decision benzeri cikti vermis.

Bu davranisin dogru oldugunu ispatla.
Eger ispatlayamiyorsan, hangi telemetry alani karsi ornegi kurar?
```

Ispat icin gerekli minimum kanit:

```text
marketSlug dogru window'a ait
tokenId ve outcomeLabel dogru
priceToBeatGuardEnabled=true
priceToBeatMode="iv_mismatch_edge"
q_final - cost < dynamic_threshold
veya gap_strength < required_gap_strength
veya depth_guard_result fail
veya effective YES+NO total > pairMaxTotalCent
veya single_edge < pairLockSingleEdgeThreshold
```

Bu kosullardan biri terminal block icin yeterliyse order acmamak savunulabilir. Birden fazlasi varsa karar daha gucludur.

Karsi ornek kurmak icin:

```text
q_final - cost >= dynamic_threshold
gap_strength >= required_gap_strength
depth_guard_result = PASS
effective YES+NO total <= pairMaxTotalCent
single_edge >= pairLockSingleEdgeThreshold veya fresh pair mumkun
ama bot yine no decision uretmis
```

Bu tablo gercekten dogrulanirsa dokumana gore bot davranisi suphelidir; runtime, binding veya pair lock decision path incelenmelidir.

## Operasyonel Ispat Formati

Bir canli problemi ispatlamak icin su format yeterlidir:

```text
Market:
  slug:
  window:
  outcome:

Trigger:
  pass:
  selected profile:
  tokenId:
  maxPrice fallback:

Action:
  max price result:
  execution floor result:
  PTB/IV result:
  depth result:
  risk result:

Pair:
  strategy:
  effective pair total:
  single edge:
  decision:

Lifecycle:
  submit event:
  fill event:
  exit/re-entry/pair event:

Sonuc:
  order yok / submit var fill yok / fill var exit sorunu
```

Bu format kullanilirsa iddia kanitlanabilir veya karsi ornek kurulabilir. Sadece "bot girmedi" ciktisi teknik iddia degildir.

## Pratik Teorem

Teorem:

```text
Botun guvenli calisma sekli, tek bir sinyalin pass etmesi degil,
bagimsiz guard'larin ayni yonde yeterli kanit uretmesidir.
```

Ispat ozeti:

- Prediction market fiyati faydali bir olasilik sinyalidir ama hatasiz degildir.
- Underlying gap hareketin yonunu anlatir ama orderbook ve depth'i anlatmaz.
- Binance/RTDS teyidi external hareketi anlatir ama Polymarket fill maliyetini garanti etmez.
- Depth guard fill maliyetini yaklastirir ama submit latency ve partial fill riskini sifirlamaz.
- Pair lock toplam maliyet fikri verir ama orphan riskini yok etmez.
- TP/SL kurallari exit plani verir ama fill kalitesini garanti etmez.

Bu yuzden botun ana tasarimi "tek sinyal al, hemen trade et" degil; "sinyal, maliyet, depth, zaman, risk ve lifecycle birlikte yeterli mi?" sorusudur. Guclu davranis buradan gelir.
