# OlympusX Dersleri: Bot Neden Başarısız Oluyor?

Güncelleme tarihi: 2026-04-27

Bu dosya `/home/heathcliff/olympusx_analysis` altındaki JetFadil/OlympusX reverse-engineering çıktılarından çıkarılan dersleri, bu botun kendi işlem kanıtlarıyla yan yana koyar. Amaç trade talimatı vermek değil; botun neden negatif sonuç ürettiğini net teşhis etmektir.

## Kısa Cevap

Botun başarısızlığı tek bir bug değil. Ana problem şu:

```text
Bot karlı TP üretebiliyor, ama strateji davranışı kazanan pattern'e benzemiyor.
Kazanan pattern erken ana taraf + sınırlı hedge + settlement/redeem ağırlığı.
Botun gerçek kayıtlarında ise SL zararı, açık pozisyon mark zararı,
expired/error order lifecycle ve eşit/kararsız pair davranışı baskın.
```

Son işlem kanıtı:

- TP satırları: `+456.85 USDC`
- SL satırları: `-698.63 USDC`
- Açık pozisyon mark zararı diagnosis: `518 trade`, `-708.13 USDC`
- Toplam diagnostic PnL: `-215.72 USDC`

OlympusX dersi:

- `biased_hedge`: `+67921.63 PnL`, `7.92% ROI`
- `one_sided_conviction`: `+26792.30 PnL`, `26.27% ROI`
- `heavy_two_sided_scalp`: `-8754.54 PnL`, `-1.95% ROI`
- `50-60% dominant share`: `-18477.23 PnL`, `-1.3% ROI`
- `90-100% dominant share`: `+26792.30 PnL`, `26.27% ROI`

Yani sorun "iki tarafı da alalım" fikrinde değil; hangi tarafın ana taraf, hangi tarafın hedge olduğunu koruyamamakta.

## 1. Bot Kazanan Pattern'i Yanlış Kopyalıyor

OlympusX raporunda pozitif davranış şudur:

```text
erken bias
-> ana taraf ağırlığı yüksek
-> karşı taraf küçük hedge
-> geç chase sınırlı
-> çoğu pozisyon settlement/redeem ile kapanıyor
```

Senin botta görülen davranış:

```text
guard pass / submit / fill
-> TP ve SL çocuk emirleri
-> SL satırları TP kazancını siliyor
-> açık pozisyon mark zararı büyüyor
-> pair lock/unwind runtime'da var ama PnL'yi düzeltmiyor
```

Bu yüzden bot "JetFadil gibi iki taraflı oynuyor" sanılsa bile asıl çekirdeği kaçırıyor. JetFadil'in edge'i eşit pair açmak değil, dominant tarafı doğru tutup hedge'i küçük bırakmak.

## 2. Eşit Pair ve Kararsız İki Taraflılık Negatif Alan

OlympusX verisinde dominant share sonucu çok açık:

| Dominant share | PnL | ROI |
|---|---:|---:|
| 50-60% | -18477.23 | -1.3% |
| 60-75% | +14768.64 | 0.81% |
| 75-90% | +68403.51 | 9.31% |
| 90-100% | +26792.30 | 26.27% |

Ders:

- Ana taraf yoksa edge zayıflıyor.
- Eşit veya eşite yakın iki taraflılık maliyeti büyütüyor.
- Güçlü sonuç, `75%+` dominant taraflarda geliyor.

Bot açısından teşhis:

- Pair lock iki bacağı "maliyet kilidi" diye eşitlemeye çalışırsa kazanan pattern'i değil, düşük ROI/negatif alanı taklit eder.
- `edge_pairlock_v1` tek taraf edge kararını verebilse bile bunu ana strateji haline getirmeden pair davranışı PnL'yi taşımaz.
- Botun başarısızlık nedeni pair lock'un var olmaması değil; pair lock'un dominant bias ile hizalanmaması.

## 3. SL Sistemi Kazanan Playbook'a Ters Çalışıyor

OlympusX raporunda SELL sayısı `0`, REDEEM sayısı `5907`. Bu veri, çıkışın satışla değil çoğunlukla settlement/redeem ile okunduğunu gösteriyor.

Senin botta:

| Exit reason | Satır | PnL |
|---|---:|---:|
| `tp` | 456 | +456.85 |
| `sl` | 524 | -698.63 |
| `open_position` | 962 | -24.82 |

Teşhis:

- Bot çok erken zarar kesiyor veya kötü liquidity anında SL satıyor.
- TP pozitif ama SL daha büyük negatif üretiyor.
- JetFadil tarzı settlement taşıma ile botun TP/SL tabanlı intrawindow çıkışı aynı sistem değil.

Sonuç:

```text
Botun başarısızlığının ana matematiği:
TP motoru çalışıyor, ama SL motoru daha büyük zarar yazıyor.
```

## 4. Geç Kovalama ve Side Switch Bot İçin Zehirli

OlympusX dersi:

| Late spend | PnL | ROI |
|---|---:|---:|
| 0% | +67504.20 | 10.87% |
| 0-10% | +19265.27 | 5.24% |
| 35-50% | -3909.74 | -0.47% |
| 50%+ | -20710.95 | -1.97% |

En kötü subpattern:

```text
Karşı taraf ladder + Geç kovalama + Yüksek fiyat kovalama
PnL = -29275.91
ROI = -2.93%
```

Bot açısından:

- Geç markette re-entry açmak, max price relax ile pahalı girişe izin vermek veya counter leg'i kapanışa yakın kovalamak bu kötü kümeye benzer.
- SL sonrası aynı window içinde yeniden giriş, geç chase'e dönüşebilir.
- Counter leg geç kaldığında hedge değil zarar büyütme aracı olur.

Bu yüzden botun başarısız olduğu yerlerden biri "kaçan trade'i geri alma" refleksidir. Kapanışa yakın ek spend, JetFadil analizinde belirgin şekilde edge'i bozuyor.

## 5. Bot Order Lifecycle'da Çok Fire Veriyor

Senin botun son 30 gün order durumları:

| Status | Side | Kind | Adet |
|---|---|---|---:|
| `expired` | buy | immediate | 1161 |
| `error` | buy | immediate | 733 |
| `completed` | buy | immediate | 556 |

Bu tablo şunu gösterir:

- Bot çok sayıda giriş deniyor ama bunların önemli kısmı iyi lifecycle'a dönmüyor.
- Başarılı sistem sadece sinyal bulmaz; fill, taşıma ve kapanışı da tutarlı yönetir.
- `completed buy` sayısı, strateji başarısı demek değildir. Buy sonrası SL/open-position sonucu kötü olabilir.

OlympusX tarafında edge, tek bir order'dan değil, market boyunca çok sayıda küçük buy davranışının ana taraf ağırlığını korumasından geliyor. Senin botta order lifecycle fire verdiğinde bu dağılım kurulmadan pozisyon ya SL'ye gidiyor ya açık zarar yazıyor.

## 6. Guard'lar Çalışıyor Ama Strateji Sorununu Çözmüyor

Bot kayıtları:

- `guard_evaluated=32925`
- `max_price_waiting=2603`
- `execution_floor_waiting=716`
- `submitted=1010`
- `filled=938`

Bu, botun kör şekilde alım yapmadığını kanıtlıyor. Ama başarısızlık guard eksikliğinden çok, guard'ların doğru oyun planını temsil etmemesinden geliyor.

Yanlış güven:

```text
PTB/IV edge pass etti -> trade iyi
```

Doğru okuma:

```text
PTB/IV edge pass etti
ama dominant taraf korunuyor mu?
hedge küçük mü?
late chase sınırlı mı?
SL toplam EV'yi siliyor mu?
pozisyon settlement'a taşınmalı mıydı?
```

Guard'lar kötü girişi azaltır. Ama kazanan pattern'i otomatik inşa etmez.

## 7. Mevcut Botun En Net Başarısızlık Nedenleri

1. **SL zararı TP kazancından büyük.**
   TP `+456.85`, SL `-698.63`. Bu tek başına toplam PnL'nin neden bozulduğunu açıklar.

2. **Açık pozisyon mark zararı ana negatif sınıf.**
   `unrealized_mark_loss` 518 trade ve `-708.13`. Bot ya pozisyonu doğru kapatamıyor ya da mark-to-market riski yönetilmiyor.

3. **Dominant taraf mantığı zayıf.**
   OlympusX pozitif edge'i 75%+ dominant tarafta. Bot pair/hedge kararlarında ana tarafı açıkça üstün tutmazsa düşük ROI alanına kayıyor.

4. **Geç chase ve re-entry riski yüksek.**
   JetFadil analizinde 50%+ late spend negatif. Botta SL sonrası re-entry, max price relax veya counter chase bu davranışı üretebilir.

5. **Order lifecycle fire veriyor.**
   Buy immediate `expired/error` toplamı `1894`, completed buy `556`. Bu kadar fire, sinyal kalitesi kadar execution kalitesinin de problem olduğunu gösterir.

6. **Hedge ile scalp karıştırılıyor.**
   `biased_hedge` pozitif; `heavy_two_sided_scalp` negatif. Bot küçük hedge yerine iki tarafı aktif trade etmeye dönerse edge kayboluyor.

7. **Exit felsefesi uyumsuz.**
   OlympusX settlement/redeem ağırlıklı okunuyor. Bot ise TP/SL ile window içinde satmaya çalışıyor. Bu farklı felsefe, özellikle SL slippage ve açık pozisyon yönetiminde zarar üretiyor.

## Ne Değişmeli?

Bu bölüm kod talimatı değil, teşhis önceliğidir.

1. Pair lock hedefi "eşit iki bacak" olmamalı.
   Hedef `dominant_side + small hedge` olmalı. Dominant taraf oranı telemetry'de açık görünmeli.

2. Re-entry ve relax kapanışa yakın çok sınırlanmalı.
   Late spend yüzdesi ayrı metric olmalı; 210. saniye sonrası yeni risk almak özel izin istemeli.

3. SL sistemi yeniden ele alınmalı.
   SL, TP kazancını siliyorsa koruma değil zarar motorudur. SL fill fiyatı, slippage ve staged SL katkısı ayrı raporlanmalı.

4. Açık pozisyon politikası netleşmeli.
   Pozisyon settlement'a mı taşınacak, time exit mi yapılacak, pair unwind mı çalışacak? Şu an negatif açık mark sınıfı bu kararın net olmadığını gösteriyor.

5. Heavy two-sided scalp engellenmeli.
   Çok fazla side switch, yüksek trade sayısı ve yüksek spend aynı markette birleşirse bot yeni alımı durdurmalı.

6. Başarı metriği `win rate` değil, pattern bazlı PnL olmalı.
   `take_profit_success` pozitif olsa bile strateji negatif kalabiliyor. `dominant_share`, `late_spend`, `SL PnL`, `open mark PnL` birlikte izlenmeli.

## Son Cümle

Botun başarısız olmasının nedeni "hiç edge yok" değil. Eski kayıtlar TP ve temiz kâr üretebildiğini gösteriyor. Problem, bu pozitif anların SL, açık pozisyon, geç chase ve kararsız iki taraflı davranış tarafından silinmesi. OlympusX analizinden çıkan ders, iki taraflı olmanın değil; erken ana tarafı koruyup hedge'i küçük, geç spend'i sınırlı tutmanın kazandırdığıdır.
