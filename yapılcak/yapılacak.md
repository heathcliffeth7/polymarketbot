# IV-Mismatch Edge Mode + Pairlock Exit

## Karar

Bu sistem ayrı bir `priceToBeatMode`/strateji modu olarak entegre edilecek.

Mod adı: `iv_mismatch_edge`

`pairlock`, bu modun giriş sinyali değil; pozisyon kâra fırsat verdiğinde mevcut `action.place_order mode=pair_lock` akışı üstünden çalışan çıkış/koruma davranışıdır.

Bu doküman yalnızca entegrasyon taslağıdır. Bu turda Rust, frontend veya config değişikliği yapılmaz.

## Amaç

Ham PTB farkı büyük diye trade açma. CLOB fiyatı, RTDS Chainlink fiyatından hesaplanan gerçek kazanma olasılığına göre ucuzsa trade aç.

Kısa ifade:

```text
q_up = Φ((C - P) / (σ√t))
q_down = Φ((P - C) / (σ√t))
cost = ask + fee + buffer
edge = q_side - cost
trade if edge >= time_adjusted_threshold
pairlock if avg_cost_first_leg + counter_eff <= max_pair_total
```

V1 hedefi, 5-15 dakikalık crypto Up/Down marketlerinde şu soruyu cevaplamaktır:

```text
Bu outcome share'i, kalan süre ve gerçekleşen volatiliteye göre ucuz mu?
```

## Projeye Entegrasyon Noktaları

### RTDS Chainlink Fiyatı

Kaynak fiyat değişkeni:

```text
C = anlık Chainlink fiyatı
P = Price to Beat
g = C - P
t = kalan saniye
```

RTDS tarafında Chainlink akışı `crypto_prices_chainlink` topic'i ve slash formatlı sembollerle takip edilir:

```text
symbol = btc/usd
payload.value -> C
payload.timestamp -> fiyat zamanı
```

Mod, `C` için son 30-60 saniyelik rolling pencere tutar. Bu pencere şunları üretir:

- `sigma`: kısa dönem gerçekleşen volatilite.
- `zero_cross_count`: `g = C - P` işaretinin son pencerede kaç kez değiştiği.
- `chainlink_staleness_ms`: fiyatın karar anına göre ne kadar eski olduğu.

Stream eksik veya stale ise mod trade üretmemeli; `decision_reason` bunu açık yazmalı.

### CLOB Top-of-Book

Gerçek işlem maliyeti midpoint değildir:

```text
Alışta: best_ask
Satışta: best_bid
spread = best_ask - best_bid
```

Up/Down adayları için V1 inputları:

```text
A_u = Up best_ask
A_d = Down best_ask
B_u = Up best_bid
B_d = Down best_bid
spread_u = A_u - B_u
spread_d = A_d - B_d
```

`iv_mismatch_edge`, alım adayı seçtiği için edge hesabında `best_ask` kullanır. Pairlock veya ileride satış/stop davranışı değerlendirilirse çıkış fiyatında `best_bid` esas alınır.

### PTB Guard Karşılığı

Mevcut PTB guard mantığı yön ve fark kontrolü yapıyor. Bu modda guard, sabit fark eşiği yerine olasılık/maliyet farkını ölçer:

```text
q_up = Φ(z_up)
q_down = Φ(z_down)
effective_cost = best_ask + taker_fee(best_ask) + buffer
edge = q_side - effective_cost
```

Bu yüzden `iv_mismatch_edge`, `manual`, `auto_last_3_avg_excursion`, `auto_vol_pct` ve `signal_formula` benzeri bir PTB mode/strateji varyantı olarak düşünülmeli.

### Pairlock Karşılığı

V1 giriş kararı ayrı, pairlock ayrı tutulur:

```text
iv_mismatch_edge -> aday Up/Down seçer
pair_lock_only -> action.place_order mode=pair_lock
```

Pairlock mevcut karşı bacak kilitleme davranışını kullanır. Modun görevi, pairlock için uygun birincil bacağı seçmek ve karar telemetrisine nedenini yazmaktır.

## V1 Mod Davranışı

### Volatilite Hesabı

Son 30-60 saniyedeki Chainlink fiyatları:

```text
C_0, C_1, C_2, ... C_n
```

Update aralıkları eşitse:

```text
d_i = C_i - C_(i-1)
σ = std(d_i)
```

Update aralıkları eşit değilse:

```text
d_i = (C_i - C_(i-1)) / sqrt(Δt_i)
σ = std(d_i)
```

Beklenen kalan hareket:

```text
M = σ * sqrt(t)
```

`σ <= 0`, yetersiz örnek sayısı veya stale RTDS durumunda karar `no_trade` olmalı.

### Kazanma Olasılığı

PTB farkı:

```text
g = C - P
```

Z-score:

```text
z_up = g / (σ * sqrt(t))
z_down = -g / (σ * sqrt(t))
```

Olasılıklar:

```text
q_up = Φ(z_up)
q_down = Φ(z_down)
Φ(z) = 0.5 * (1 + erf(z / sqrt(2)))
```

Sezgi:

```text
z = 0.00 -> q ~= 0.50
z = 0.50 -> q ~= 0.69
z = 1.00 -> q ~= 0.84
z = 1.50 -> q ~= 0.93
```

### Effective Cost ve Fee

Polymarket crypto taker fee oranı için varsayılan:

```text
r = 0.072
b = buffer / slippage payı
```

Fee formülü:

```text
taker_fee(p) = r * p * (1 - p)
```

Effective alış maliyeti:

```text
c_up = A_u + 0.072 * A_u * (1 - A_u) + b
c_down = A_d + 0.072 * A_d * (1 - A_d) + b
```

V1 başlangıç buffer'ı:

```text
b = 0.005
```

Fee market bazında farklılaşabileceği için üretim entegrasyonunda market metadata/fee parametreleri okunmalı; `0.072` V1 crypto varsayılanıdır.

### Edge Hesabı

```text
E_up = q_up - c_up
E_down = q_down - c_down
E_best = max(E_up, E_down)
side = argmax(E_up, E_down)
```

Temel karar:

```text
E_best < threshold -> no_trade
E_best >= threshold -> selected side candidate
```

Başlangıç ölçeği:

```text
0.05 <= edge < 0.08 -> small candidate
0.08 <= edge < 0.12 -> normal candidate
edge >= 0.12       -> strong candidate
```

### IV-Mismatch Skoru

Market fiyatından piyasanın fiyatladığı volatilite ters çözülür. Favori taraf için:

```text
q_market = effective_cost_fav
z_market = Φ^-1(q_market)
σ_implied = |C - P| / (z_market * sqrt(t))
IV_ratio = σ_implied / σ_real
```

Yorum:

```text
IV_ratio > 1.20 -> market fazla belirsizlik fiyatlıyor, favori ucuz olabilir
IV_ratio < 0.80 -> market fazla emin fiyatlıyor, favori pahalı olabilir
```

V1 kuralı:

```text
IV_ratio > 1.20 AND edge > 0.06 -> favori alınabilir
IV_ratio < 0.80                 -> favori bloklanır
```

`q_market <= 0.50`, `z_market <= 0` veya `σ_real <= 0` durumunda IV skoru hesaplanamaz; mod edge kararını `iv_ratio=null` telemetrisiyle verir veya konfigürasyona göre bloklar.

### Spread ve Chop Filtresi

Spread:

```text
spread = best_ask - best_bid
```

V1 kuralı:

```text
spread > 0.04 -> no_trade
spread <= 0.03 -> iyi likidite
```

Chop:

```text
zero_cross_count = count(sign(g_i) != sign(g_(i-1)))
```

V1 kuralı:

```text
zero_cross_count >= 3 -> continuation trade yok
zero_cross_count >= 3 AND E_best >= 0.10 -> yalnızca küçük value candidate
```

### Zaman Eşikleri

```text
t > 90 sn       -> no_trade veya çok seçici
30 < t <= 90    -> edge >= 0.06
15 < t <= 30    -> edge >= 0.08
8 < t <= 15     -> edge >= 0.10
t <= 8          -> yeni trade yok
```

Bu eşikler V1 default olmalı; config ile override edilebilir.

### Pairlock Aday Seçimi

V1 seçim akışı:

```text
1. Up ve Down için q, cost, edge hesapla.
2. Spread, stale data, chop ve zaman filtrelerini uygula.
3. E_best tarafını seç.
4. Seçilen tarafı `pair_lock_only` trigger çıktısında aday olarak taşı.
5. Downstream `action.place_order mode=pair_lock` mevcut pairlock akışıyla emirleri üretir.
```

Karar çıktısı minimum şu alanları taşımalı:

```text
selected_side
selected_token_id
q
cost
edge
sigma
iv_ratio
zero_cross_count
decision_reason
```

## Varsayılan Parametreler

```text
mode = iv_mismatch_edge
vol_window_sec = 45
min_vol_samples = 8
chainlink_stale_ms = 3000
buffer = 0.005
crypto_taker_fee_rate = 0.072
max_spread = 0.04
good_spread = 0.03
chop_zero_cross_limit = 3
chop_value_edge = 0.10
iv_ratio_min_for_favorite = 1.20
iv_ratio_block_favorite_below = 0.80
max_pair_total = 0.97
strong_pair_total = 0.95
```

Zaman eşikleri:

```text
edge_threshold_30_90_sec = 0.06
edge_threshold_15_30_sec = 0.08
edge_threshold_8_15_sec = 0.10
no_new_trade_under_sec = 8
```

## Karar Akışı

```text
For each market tick:
1. C = latest RTDS Chainlink price.
2. P = current Price to Beat.
3. t = seconds_left.
4. Build rolling Chainlink returns and σ.
5. g = C - P.
6. q_up = Φ(g / (σ * sqrt(t))).
7. q_down = Φ(-g / (σ * sqrt(t))).
8. Read CLOB best_ask/best_bid for Up and Down.
9. Reject stale, wide spread, insufficient σ, or invalid orderbook state.
10. c_up = A_u + fee(A_u) + buffer.
11. c_down = A_d + fee(A_d) + buffer.
12. E_up = q_up - c_up.
13. E_down = q_down - c_down.
14. Compute IV_ratio for favorite if valid.
15. Apply chop and time threshold.
16. Select argmax(E_up, E_down), or no_trade.
17. Emit telemetry with decision_reason.
18. If connected to pair_lock_only flow, pass selected side to pairlock action.
```

Decision reason örnekleri:

```text
selected_edge_passed
blocked_rtds_stale
blocked_insufficient_vol_samples
blocked_zero_sigma
blocked_spread_wide
blocked_chop
blocked_edge_below_threshold
blocked_iv_ratio_low
blocked_too_late
```

## Pairlock Çıkış Mantığı

Pozisyon mevcutsa:

```text
avg_cost_pos = mevcut tarafın ortalama effective maliyeti
shares_pos = mevcut taraf share sayısı
counter_ask = karşı bacak best_ask
counter_eff = counter_ask + 0.072 * counter_ask * (1 - counter_ask) + b
pair_total = avg_cost_pos + counter_eff
```

Pairlock kuralı:

```text
pair_total <= 0.97 -> kilitle
pair_total <= 0.95 -> çok iyi kilit
pair_total > 0.99  -> kilitleme
```

Kilitli kâr:

```text
locked_profit_per_pair = 1 - pair_total
counter_shares_to_buy = unmatched_shares_pos
locked_profit = counter_shares_to_buy * (1 - pair_total)
```

Örnek:

```text
Up avg cost = 0.62
Down ask = 0.25
b = 0.005

counter_eff = 0.25 + 0.072 * 0.25 * 0.75 + 0.005
counter_eff = 0.2685

pair_total = 0.62 + 0.2685 = 0.8885
locked_profit_per_pair = 1 - 0.8885 = 0.1115
```

Bu davranış V1'de yeni onchain merge otomasyonu anlamına gelmez. V1 sadece mevcut pairlock emir akışını kullanır; onchain merge sonraki fazdır.

## V1 Dışı / Sonraki Fazlar

### Pullback Entry

İlk spike'ı alma; son 10 saniyedeki en yüksek ask'i izle:

```text
H_side = max(ask_side over last 10 sec)
PB = H_side - current_ask

q_side >= 0.70
E_side >= threshold
PB >= 0.03
spread <= 0.04
```

### Overpriced Favorite Fade

Favori pahalıysa ucuz karşı tarafı küçük size ile değerlendir:

```text
favorite_overpriced = cost_fav - q_fav
E_cheap = q_cheap - cost_cheap

favorite_overpriced >= 0.06
AND E_cheap >= 0.06
AND t > 20
AND spread <= 0.04
```

### Same-Side DCA

Kör DCA yok; yalnızca edge iyileşirse ekleme:

```text
E_new >= 0.08
AND q_new >= q_entry - 0.03
AND zero_cross_count < 3
```

### q-Model Stop

PTB farkı 0 oldu diye stop yok. Stop, pozisyon olasılığı çökünce değerlendirilir:

```text
q_pos > 0.45 -> tut
0.35 <= q_pos <= 0.45 -> uyarı
q_pos < 0.30 -> çıkış adayı

q_pos < 0.30
AND 2-3 update boyunca devam ediyor
AND spread <= 0.04
-> sat
```

### Kelly Sizing

V1 sabit/kademeli size ile başlayabilir. Kelly sonraki faz:

```text
f_full = (q - c) / (1 - c)
f = 0.10 * f_full
f = 0.25 * f_full
stake = bankroll * f
```

### Onchain Merge Otomasyonu

Pairlock sonrası eşit Yes/No setlerini CTF `mergePositions()` ile pUSD'ye döndürme ayrı fazdır. V1 bunu otomatik yapmaz.

## Kabul Kriterleri

- Dokümanda mod adı her yerde `iv_mismatch_edge` olarak geçer.
- `pairlock`, giriş sinyali değil çıkış/koruma davranışı olarak anlatılır.
- RTDS Chainlink fiyatı `C`, PTB değeri `P`, volatilite `σ`, zero-cross ve kalan süre `t` açık tanımlıdır.
- CLOB alış maliyeti `best_ask`, satış/çıkış değeri `best_bid` ile anlatılır.
- PTB guard karşılığı `q_up/q_down`, effective cost ve edge üzerinden tarif edilir.
- V1 kapsamı edge, IV, spread, chop ve zaman eşikleriyle sınırlıdır.
- `pair_lock_only -> action.place_order mode=pair_lock` akışı açık yazılmıştır.
- Telemetry alanları `q`, `cost`, `edge`, `sigma`, `iv_ratio`, `zero_cross_count`, `decision_reason` olarak listelenmiştir.
- Pullback, favorite fade, same-side DCA, q-model stop, Kelly sizing ve onchain merge V1 dışına alınmıştır.
- Bu doküman kod değişikliği gerektirmez; markdown dışında test/build çalıştırılmaz.

## Kaynak Notları

- Polymarket fees: https://docs.polymarket.com/trading/fees
- Polymarket RTDS: https://docs.polymarket.com/market-data/websocket/rtds
- Polymarket orderbook: https://docs.polymarket.com/trading/orderbook
- Polymarket CTF merge: https://docs.polymarket.com/trading/ctf/merge

Resmi dokümanlardan kullanılan dayanaklar:

- Fees sayfası taker fee'nin match anında uygulandığını ve crypto taker fee rate için `0.072` değerini gösterir.
- RTDS sayfası Chainlink crypto stream'inin `crypto_prices_chainlink` topic'i ve `btc/usd` sembol formatıyla izlenebildiğini gösterir.
- Orderbook sayfası alış için best ask, satış için best bid ve spread'in best ask/best bid farkı olduğunu gösterir.
- CTF merge sayfası eşit Yes/No token setlerinin `mergePositions()` ile pUSD'ye döndürülebildiğini ve eksik token durumunda işlemin revert edeceğini gösterir.
