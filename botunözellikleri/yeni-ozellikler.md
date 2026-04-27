# Yeni Özellikler

Güncelleme tarihi: 2026-04-26

Bu dosya son eklenen trade flow, guard, pair lock ve analiz özelliklerinin kısa haritasıdır. Ayrıntılı senaryolar ilgili dosyalara taşındı.

## Hızlı Özet

- `trigger.market_price` kalan süreye göre `entryTimingProfiles` seçebilir.
- `action.place_order` PTB tarafında `iv_mismatch_edge`, adaptive rejimler, Binance teyidi, depth guard ve participation credit kullanabilir.
- PTB stop-loss sonrası `priceToBeatStopLossBump*` alanları sonraki giriş threshold'unu artırabilir.
- Art arda kaçan marketlerden sonra `priceToBeatMaxPriceRelax*` alanları max price/PTB şartını kontrollü gevşetebilir.
- `pairLockStrategy="edge_pairlock_v1"` açık pozisyonu counter ile kilitleme, yeni eşit pair açma veya tek taraflı edge alma kararını verebilir.
- `buyFillLockEnabled` aynı market cycle içinde aynı gruptan ikinci buy girişini engelleyebilir.
- `notifyOnOrderSubmitted`, no-order tanısı, auto-scope analiz zaman aralıkları ve `/api/relax` kontrolü eklidir.

## Yeni Özellik Dosyaları

| Özellik | Detay |
|---|---|
| Entry timing profiles | [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) |
| Buy sizing ve fill lock | [senaryolar/03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) |
| `iv_mismatch_edge` | [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) |
| PTB bump ve max price relax | [senaryolar/05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) |
| `edge_pairlock_v1` | [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) |
| Telegram, submit ve no-order analytics | [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) |
| Kopyalanabilir örnekler | [ornekler/config-receteleri.md](./ornekler/config-receteleri.md) |

## Operatör İçin Kısa Karar Rehberi

| Durum | Kullanılacak özellik |
|---|---|
| Market başında ucuz giriş, sonlara doğru daha pahalı ama güçlü giriş istiyorsan | `entryTimingProfiles` |
| PTB gap iyi görünse bile Binance/book ters diyorsa girmesin istiyorsan | `priceToBeatMode="iv_mismatch_edge"` |
| Stop-loss serisinden sonra botun daha seçici olmasını istiyorsan | `priceToBeatStopLossBumpEnabled=true` |
| Bot çok uzun süre hiç trade alamıyorsa ama fırsatlar sonradan ucuzlamışsa | `priceToBeatMaxPriceRelaxEnabled=true` ve global `max_price_relax_enabled=true` |
| Up/Down birlikte maliyet kilidi kurmak istiyorsan | `mode="pair_lock"` |
| Pair lock maliyet/edge kararını otomatik yapsın istiyorsan | `pairLockStrategy="edge_pairlock_v1"` |
| Aynı markette iki ayrı buy fill istemiyorsan | `buyFillLockEnabled=true` |
| "Neden order yok?" sorusuna UI'dan cevap arıyorsan | no-order analytics ve [troubleshooting checklist](./ornekler/troubleshooting-checklist.md) |

## Kaynak

Bu özet şu yerel kaynaklarla uyumludur:

- [problemler/5dakikalik-marketler-nasil-calisir.md](../problemler/5dakikalik-marketler-nasil-calisir.md)
- [problemler/bot-ozellikleri-ve-senaryolar.md](../problemler/bot-ozellikleri-ve-senaryolar.md)
- [problemler/ptb-threshold-problemi.md](../problemler/ptb-threshold-problemi.md)
- [problemler/sl-ve-giris-kalite-analizi.md](../problemler/sl-ve-giris-kalite-analizi.md)
- [problemler/vol-capture-sorunlar.md](../problemler/vol-capture-sorunlar.md)
- [yapılcak/yapılacak.md](../yapılcak/yapılacak.md)

## Özelliklerin Pratik Etkisi

### `entryTimingProfiles`

Bu özellik tek bir market içinde tek bir giriş kuralı kullanma zorunluluğunu kaldırır. 5 dakikalık marketin ilk 2 dakikası ile son 45 saniyesi aynı risk profiline sahip değildir. Erken bölümde fiyatın ucuz olması daha önemlidir; geç bölümde ise hareketin güçlü ve doğrulanmış olması daha önemlidir.

Pratik sonuç:

- Erken bölümde düşük `maxPriceCent` ve küçük `sizeUsdc` kullanılabilir.
- Geç bölümde daha yüksek `maxPriceCent` verilebilir ama PTB min gap veya IV edge şartı artırılır.
- Action node explicit size vermiyorsa profile size fallback olur.

Yanlış kullanım:

- Profil aralıkları çakışırsa hangi profilin seçileceği belirsizleşir.
- Her profile aynı max price verilirse özellik sadece karmaşıklık ekler.
- Geç profile daha pahalı fiyat verilip edge şartı artırılmazsa kötü fiyattan momentum kovalanabilir.

### `iv_mismatch_edge`

Klasik PTB gap, underlying fiyat ile market fiyatı arasındaki farkı ölçer. `iv_mismatch_edge` bunu daha ileri götürür: token'ın implied probability'si ile Chainlink/Binance/orderbook sinyalini birlikte değerlendirir.

Pratik sonuç:

- Book karşı yöne işaret ediyorsa iyi görünen fiyat block edilebilir.
- Binance stale veya ters yöndeyse edge cezalandırılır.
- Depth yetersizse best ask iyi görünse bile order engellenebilir.
- Adaptive rejim green/orange/red ile risk seviyesi okunabilir.

Yanlış kullanım:

- Sadece `priceToBeatMode="iv_mismatch_edge"` yazıp veri tazeliği ve depth ayarlarını düşünmemek.
- Orange/red block'ları "bot çalışmıyor" diye yorumlamak.
- Max price block ile IV edge block'u karıştırmak.

### PTB Bump

PTB bump, zarar sonrası botun aynı kalitede giriş almaya devam etmesini engeller. SL serisi varsa sistemin daha seçici hale gelmesi istenir.

Pratik sonuç:

- Ardışık PTB stop-loss sonrası threshold artar.
- `per_scope` seçilirse sadece ilgili asset/timeframe/direction etkilenir.
- `loss_table` seçilirse büyük zarar daha büyük sıkılaşma üretir.

Yanlış kullanım:

- Global scope ile bir asset'teki kötü seri yüzünden tüm flow'ları sıkılaştırmak.
- Bump max value vermeyip botu uzun süre aşırı seçici bırakmak.
- Relax ile birlikte effective threshold'un son halini okumamak.

### Max Price Relax

Relax, "bot hiç trade almıyor" problemini çözmek için vardır. Ancak sadece çok miss oldu diye gevşemez; geçmiş marketlerde gerçekten tradeable fırsat olup olmadığını da arar.

Pratik sonuç:

- Miss count dolunca geçmiş marketler incelenir.
- Min depth sağlanmışsa relax credit üretilebilir.
- Global `strategy.max_price_relax_enabled` kapalıysa node config tek başına yetmez.

Yanlış kullanım:

- Depth yokken fiyat uygun sanmak.
- Relax'ı SL serisi yaşayan stratejide açıp risk artırmak.
- Analytics'teki `relax_miss_reason` alanını okumadan config değiştirmek.

### `edge_pairlock_v1`

Bu strateji pair lock kararını sadece toplam maliyete bırakmaz. Açık pozisyonu counter ile kilitleme, iki bacağı birlikte açma veya tek taraflı edge alma kararını sıraya koyar.

Pratik sonuç:

- Açık tek bacak varsa önce counter lock denenir.
- Yeni pair toplam maliyeti uygunsa fresh equal pair açılır.
- Pair uygun değil ama tek bacak edge güçlü ise single edge seçilebilir.
- Hiçbiri uygun değilse no decision üretir.

Yanlış kullanım:

- Single edge kararını hatalı pair lock sanmak.
- `iv_mismatch_edge` zorunluluğunu atlamak.
- Protective unwind kapalıyken orphan riskini göz ardı etmek.

### No-Order Analytics

No-order analytics, "neden trade yok" sorusunu sadece log okuyarak değil, guard ve market kalitesiyle cevaplamayı hedefler.

Pratik sonuç:

- Fiyat uygun ama depth yetersiz mi görülebilir.
- Relax credit neden oluşmadı anlaşılabilir.
- Tradeable saniye sayısı ve fillability skoru takip edilebilir.

Yanlış kullanım:

- Zaman aralığını yanlış seçip farklı marketleri birlikte değerlendirmek.
- Submit yok ile fill yok durumlarını aynı kategoriye koymak.
