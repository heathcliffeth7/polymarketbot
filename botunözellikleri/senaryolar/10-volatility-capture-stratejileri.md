# 10 - Volatility Capture Stratejileri

Bu dosya 5 dakikalık Up/Down marketlerde volatility capture yaklaşımının fırsatlarını ve risklerini özetler.

## Amaç

Volatility capture stratejisi, kısa marketlerde fiyatın iki tarafa da savrulmasından faydalanmaya çalışır. Ancak tek taraf dolma, fee, likidite, timing ve exit problemleri beklenen değeri hızla bozabilir.

## Temel Fikir

Basit form:

1. Up veya Down token ucuzken gir.
2. Token hareket ederse TP ile çık.
3. Ters hareket olursa SL veya pair lock ile riski sınırla.
4. Çok geç kalmadan pencere sonu pozisyonu azalt.

Bu fikir kâğıt üzerinde basittir; pratikte orderbook, fee ve fill sırası belirleyicidir.

## Ana Riskler

| Risk | Açıklama |
|---|---|
| Tek taraf dolma | Pair planlanır ama sadece bir bacak fill olur |
| SL konamaz | Orderbook veya fiyat hareketi SL emrini fill ettirmez |
| Fiyat beklenen seviyeye inmez | "Ucuzdan alırım" varsayımı gerçekleşmez |
| Fee ve spread | Küçük edge'i yer |
| Likidite | Best ask iyi görünür ama derinlik yetmez |
| Zamanlama | Market başı/sonu davranışı farklıdır |
| Flatten zamanı | Çok erken çıkış kârı keser, çok geç çıkış fill riskini artırır |
| Çoklu pencere | Aynı anda çok market yakalamak exposure kontrolünü zorlaştırır |

## Senaryo A: Tek Taraf Dolma

Plan:

- Up 47 cent.
- Down 48 cent.
- Toplam 95 cent ile pair aç.

Gerçek:

- Up fill olur.
- Down ask 52 cent'e kaçar.
- Pair toplamı artık tavan üstüdür.

Çözüm seçenekleri:

- `pairProtectiveUnwindEnabled` ile tek bacağı azalt.
- Orphan grace süresi içinde counter fırsatı bekle.
- `edge_pairlock_v1` single edge kararı verdiyse orphan riskinin bilinçli kabul edildiğini doğrula.

## Senaryo B: Ucuz Giriş ama Kötü Exit

Plan:

- 55 cent al.
- 70 cent TP.
- 40 cent SL.

Gerçek:

- Fiyat 66 cent'e kadar çıkar ama TP fill olmaz.
- Sonra 43 cent'e düşer.
- SL de düşük depth nedeniyle kayar.

Ders:

- TP seviyesini sadece hedef kâr değil, orderbook fill olasılığı belirlemelidir.
- Staged TP erken realized profit sağlayabilir.
- Time exit ve window end sell resolution riskini azaltır.

## Senaryo C: Çok Sıkı Guard, Hiç Trade Yok

Belirti:

- PTB iyi fırsat bekliyor.
- Max price tavan düşük.
- Bot saatlerce trade almıyor.

Çözüm:

- No-order analytics ile gerçekten tradeable saniye var mı bak.
- `priceToBeatMaxPriceRelaxEnabled` ve global relax durumunu kontrol et.
- Entry timing ile geç pencere profili ayrı ayarlanabilir.
- Depth yoksa gevşeme yapmak doğru olmayabilir.

## Senaryo D: Çok Gevşek Guard, SL Serisi

Belirti:

- Çok sayıda giriş var.
- SL oranı yüksek.
- Girişler özellikle chop döneminde geliyor.

Çözüm:

- `iv_mismatch_edge` adaptive orange/red rejimlerini aktif tut.
- Binance same direction ve depth guard sıkılaştır.
- PTB stop-loss bump ile ardışık zarar sonrası threshold artır.
- Re-entry aynı window içinde kapatılabilir: `reentrySkipCurrentWindow=true`.

## Entry Kalitesi

Entry kalitesini etkileyen ana alanlar:

- `entryTimingProfiles`
- `maxPrice` / `maxPriceCent`
- `priceToBeatGuardEnabled`
- `priceToBeatMode`
- `priceToBeatIvTimeRules`
- `priceToBeatIvDepthGuardEnabled`
- `priceToBeatIvRequireBinanceSameDirection`
- `execution floor`

İyi entry sadece düşük token fiyatı değildir. Düşük fiyat bazen kötü ihtimali doğru fiyatlayan markettir.

## Exit Kalitesi

Exit kalitesi için bakılacaklar:

- TP seviyesi orderbook'ta gerçekçi mi?
- SL mode hızlı mı güvenli mi?
- PTB stop-loss token fiyatından önce underlying bozulmayı yakalıyor mu?
- Time exit pencere sonuna çok geç mi?
- Window end auto-sell açık mı?
- Pair lock sonrası normal SL iki bacağı bozuyor mu?

## EV Kontrol Soruları

Strateji kârlı görünüyorsa şu sorular cevaplanmalıdır:

1. Ortalama giriş fiyatı nedir?
2. TP fill oranı nedir?
3. SL fill slippage ne kadar?
4. Fee ve spread sonrası net edge kalıyor mu?
5. Tek taraf dolma oranı nedir?
6. Resolution'a kalan pozisyon sayısı kaç?
7. Aynı markette re-entry zarar serisi yaratıyor mu?
8. Düşük hacim ve yüksek hacim günleri ayrı mı ölçülüyor?

## Önerilen Başlangıç Profili

Temkinli başlangıç:

- Auto-scope açık.
- Entry timing ile erken/geç ayrımı.
- `iv_mismatch_edge` adaptive protection.
- Depth guard açık.
- Staged TP.
- Composite-safe SL.
- PTB stop-loss bump açık.
- Re-entry aynı window'da kapalı.
- Pair lock kullanılacaksa `edge_pairlock_v1` ve protective unwind açık.

## Operatör Checklist

- Stratejinin ana hedefi tek taraf momentum mu, çift taraf maliyet kilidi mi?
- Fee/spread sonrası hedef kâr hâlâ anlamlı mı?
- Düşük depth yüzünden backtest/teori gerçek fill'e dönüşmüyor olabilir mi?
- Çok trade almak mı, az ama kaliteli trade almak mı hedef?
- Relax açılırsa SL serisi artıyor mu?
- Bump açılırsa fırsat kaçırma artıyor mu?

## Strateji Tasarımında Ana Ayrım

Volatility capture altında iki farklı strateji ailesi karıştırılmamalıdır:

1. Tek taraf momentum: Up veya Down tarafında güçlü edge arar.
2. Çift taraf maliyet kilidi: YES+NO toplam maliyetini düşük yakalamaya çalışır.

Tek taraf momentum için:

- `iv_mismatch_edge`.
- Entry timing.
- Depth guard.
- TP/SL ve re-entry kontrolü.

Çift taraf maliyet kilidi için:

- Pair lock.
- `pairMaxTotalCent`.
- Counter leg fill kalitesi.
- Protective unwind.

İki yaklaşım aynı flow içinde bulunabilir, ama başarı ölçütleri farklıdır. Tek taraf momentum TP/SL oranıyla ölçülür; pair lock toplam maliyet ve orphan oranıyla ölçülür.

## Basit EV Örneği

Varsayım:

```text
entry fiyatı = 0.58
TP fiyatı = 0.72
SL fiyatı = 0.44
TP olasılığı = 55%
SL olasılığı = 45%
fee/spread etkisi = 0.02
```

Yaklaşık sonuç:

```text
TP kazanç = 0.72 - 0.58 - 0.02 = 0.12
SL kayıp = 0.58 - 0.44 + 0.02 = 0.16
EV = 0.55 * 0.12 - 0.45 * 0.16
EV = 0.066 - 0.072 = -0.006
```

Yorum:

- TP oranı yüzde 55 olsa bile fee/spread ve SL büyüklüğü EV'yi negatife çekebilir.
- Bu yüzden sadece win rate değil, average win/loss ve fill slippage birlikte ölçülmelidir.

## Pair Lock EV Örneği

```text
YES effective cost = 0.48
NO effective cost = 0.49
toplam = 0.97
fee/unwind riski = 0.015
net buffer = 1.00 - 0.985 = 0.015
```

Bu teoride pozitif görünür. Ancak:

- Bir bacak fill olmazsa risk tek taraflı olur.
- Exit gerekirse spread buffer'ı yiyebilir.
- Market kapanışı ve claim süreci operasyonel risk taşır.

Pair lock'ta küçük buffer'lar sadece çok iyi fill kalitesiyle anlamlıdır.

## Guard Ayarı ve Trade Frekansı Dengesi

Çok sıkı ayarlar:

- Daha az trade.
- Daha iyi ortalama entry.
- Fırsat kaçırma ve sample azlığı riski.

Çok gevşek ayarlar:

- Daha çok trade.
- Daha fazla SL.
- Fee/spread maliyeti artar.

Bump ve relax bu dengeyi dinamik hale getirir. Bump kötü seri sonrası frene basar; relax uzun süre fırsat kaçırınca freni biraz bırakır. İkisinin de doğru çalışıp çalışmadığı analytics ile ölçülmelidir.

## Strateji Gözlem Metrikleri

Canlı değerlendirme için minimum metrikler:

| Metrik | Neden önemli |
|---|---|
| Entry avg price | Fiyat kalitesi |
| Effective fill cost | Fee/depth etkisi |
| TP hit rate | Kâr alma başarısı |
| SL hit rate | Risk kontrol sonucu |
| Avg win / avg loss | EV için win rate'ten daha önemli |
| No-order count | Guard aşırı sıkı mı |
| PTB block reason | Giriş kalitesi neden reddediliyor |
| Relax credit | Fırsat kaçırma düzeltmesi |
| Bump count | Zarar serisi freni |
| Pair orphan rate | Pair lock fill riski |

## Rejim Bazlı Düşünme

Aynı config her piyasa rejiminde iyi çalışmayabilir.

Düşük volatilite:

- Ucuz token daha az görülür.
- Relax daha sık gündeme gelebilir.
- TP hedefleri daha yakın olmalıdır.

Yüksek volatilite:

- Fırsat çoktur ama fake move da artar.
- Depth ve Binance same direction daha önemli olur.
- SL slippage büyüyebilir.

Chop:

- Re-entry zarar serisine dönüşebilir.
- Adaptive orange/red block'lar daha değerli olur.
- Time exit pozisyonu gereksiz taşımayı azaltabilir.

## Değişiklik Yapmadan Önce Sorulacaklar

Config değiştirmeden önce şu sırayı uygula:

1. Sorun trade azlığı mı, kötü trade çokluğu mu?
2. Kötü trade'ler entry mi exit kaynaklı?
3. Entry kötü ise fiyat mı, depth mi, direction teyidi mi sorun?
4. Exit kötü ise TP uzak mı, SL geç mi, time exit yok mu?
5. Pair lock'ta sorun maliyet mi, counter fill mi, orphan yönetimi mi?
6. Değişiklik sonrası hangi metric iyileşmeli?

Bu sorular cevaplanmadan yapılan ayar değişikliği stratejiyi iyileştirmek yerine sadece davranışı değiştirir.
