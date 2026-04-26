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
