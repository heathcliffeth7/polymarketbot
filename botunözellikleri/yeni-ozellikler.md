# Yeni Özellikler

Güncelleme tarihi: 2026-05-01

Bu dosya son eklenen trade flow, guard, pair lock, DCA, forensic analiz ve claim operasyon özelliklerinin kısa haritasıdır. Ayrıntılı senaryolar ilgili dosyalara taşındı.

## Hızlı Özet

- `trigger.market_price` kalan süreye göre `entryTimingProfiles` seçebilir.
- `action.place_order` PTB tarafında `iv_mismatch_edge`, adaptive rejimler, Binance teyidi, depth guard ve participation credit kullanabilir.
- PTB stop-loss sonrası `priceToBeatStopLossBump*` alanları sonraki giriş threshold'unu artırabilir.
- Art arda kaçan marketlerden sonra `priceToBeatMaxPriceRelax*` alanları max price/PTB şartını kontrollü gevşetebilir.
- `pairLockStrategy="edge_pairlock_v1"` açık pozisyonu counter ile kilitleme, yeni eşit pair açma veya tek taraflı edge alma kararını verebilir.
- `action.place_order mode="dca_live_v1"` generic slug/outcome DCA, `trigger.market_price bindingMode="dca_live_only"` ile çalışabilir.
- Pair lock tarafında `adaptive_max_price_v1`, `manual_adaptive_risk_v1` ve `biased_hedge_v1` stratejileri eklidir.
- `repeatMode=once` artık `onceScope=market/run` ile UI'da firing mode olarak ayrıştırılır.
- Primary pair lock re-entry için `reentryMinPriceCent`, `reentryMaxPriceCent` ve advanced PTB/max price alanları child node'a taşınabilir.
- Decision log, order node snapshot, missed-market timing diagnostics ve official/activity cash PnL analizleri eklidir.
- Claim sweep tarafında `relayer_api_key`, USDC.e collateral, pUSD funds activation ve `/api/claim/activate-funds` akışı eklidir.
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
| DCA live ve trigger binding | [senaryolar/11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) |
| Adaptive pair lock stratejileri | [senaryolar/12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) |
| Forensic analiz ve PnL | [senaryolar/13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) |
| Claim sweep ve funds activation | [senaryolar/14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) |
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
| Pair lock max price iyi miss kanıtıyla kontrollü gevşesin istiyorsan | `pairLockStrategy="adaptive_max_price_v1"` |
| Manual PTB pair lock hacim/trend/SL riskine göre ayarlansın istiyorsan | `pairLockStrategy="manual_adaptive_risk_v1"` |
| Erken dominant taraf + sınırlı hedge istiyorsan | `pairLockStrategy="biased_hedge_v1"` |
| Generic slug/outcome DCA istiyorsan | `mode="dca_live_v1"` ve `bindingMode="dca_live_only"` |
| Aynı markette iki ayrı buy fill istemiyorsan | `buyFillLockEnabled=true` |
| "Neden order yok?" sorusuna UI'dan cevap arıyorsan | no-order analytics, decision log ve [troubleshooting checklist](./ornekler/troubleshooting-checklist.md) |
| Claim sonrası USDC.e relayer'da kullanılamıyorsa | funds activation |

## Kaynak

Bu özet şu yerel kaynaklarla uyumludur:

- [problemler/5dakikalik-marketler-nasil-calisir.md](../problemler/5dakikalik-marketler-nasil-calisir.md)
- [problemler/bot-ozellikleri-ve-senaryolar.md](../problemler/bot-ozellikleri-ve-senaryolar.md)
- [problemler/ptb-threshold-problemi.md](../problemler/ptb-threshold-problemi.md)
- [problemler/sl-ve-giris-kalite-analizi.md](../problemler/sl-ve-giris-kalite-analizi.md)
- [problemler/vol-capture-sorunlar.md](../problemler/vol-capture-sorunlar.md)
- [yapılcak/yapılacak.md](../yapılcak/yapılacak.md)
- Güncel claim/funds activation ve pair-lock primary re-entry değişiklikleri.

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

### DCA Live

`dca_live_v1`, tek crypto 5m buy node'u değil generic market/outcome DCA action'ıdır. Trigger tarafında `bindingMode="dca_live_only"` kullanılabilir; bu mod outcome condition veya PTB trigger gate değil, downstream DCA action'a market/window bağlamı taşır.

Yanlış kullanım:

- Standard trigger binding arkasına DCA live koymak.
- `sideMode` ile `selectedOutcomes` sayısını uyumsuz bırakmak.
- Budget guard vermeden çok slug üzerinde DCA çalıştırmak.

### Adaptive Pair Lock

Pair lock stratejileri artık farklı risk modelleri taşır. `adaptive_max_price_v1` IV edge ve good-miss kanıtı ister; `manual_adaptive_risk_v1` manual PTB üstüne hacim/trend/SL rejimi ekler; `biased_hedge_v1` eşit pair yerine dominant primary + sınırlı hedge kurar.

Yanlış kullanım:

- Her stratejiyi `edge_pairlock_v1` gibi yorumlamak.
- Telegram sessizliğini karar yok sanmak; notification throttle açık olabilir.
- Node snapshot okumadan geçmiş kararı bugünkü config ile açıklamak.

### Forensic Analysis

Decision log ve node snapshot, geçmiş order kararını o anki config ile birlikte saklar. Activity cash PnL ise Polymarket activity/redeem etkisini diagnostic PnL'den ayırır.

Yanlış kullanım:

- `cashStatus` pending iken PnL'i nihai sonuç saymak.
- 48h aralığını uzun vadeli EV sonucu gibi yorumlamak.
- No-order timeline varken sadece PTB threshold değiştirmek.

### Claim Funds Activation

Claim sweep resolved marketlerde redeemable pozisyonları işler. Builder/relayer modlarında Safe üzerinde USDC.e bekliyorsa funds activation USDC.e -> pUSD wrap işlemini gönderir.

Yanlış kullanım:

- `direct` execution mode'da relayer funds activation beklemek.
- USDC.e ve pUSD adreslerini aynı collateral gibi yorumlamak.
- `relayer_wallet_activation_required` hatasını CLOB order hatası sanmak.
