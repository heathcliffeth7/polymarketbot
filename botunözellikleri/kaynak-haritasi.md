# Kaynak Haritası

Güncelleme tarihi: 2026-04-26

Bu dosya `botunözellikleri/` altındaki yeni dokümantasyonun hangi repo içi notlardan ve kod alanlarından derlendiğini gösterir.

## Doküman Kaynakları

| Kaynak | Bu sette kullanıldığı yer |
|---|---|
| [problemler/5dakikalik-marketler-nasil-calisir.md](../problemler/5dakikalik-marketler-nasil-calisir.md) | 5 dakikalık market, auto-scope, WS fast path, cycle window, pair lock etkileşimi |
| [problemler/bot-ozellikleri-ve-senaryolar.md](../problemler/bot-ozellikleri-ve-senaryolar.md) | Guard, TP/SL, re-entry, Telegram ve exit price cap senaryoları |
| [problemler/ptb-threshold-problemi.md](../problemler/ptb-threshold-problemi.md) | PTB modları, `auto_vol_pct`, threshold mantığı, örnek karşılaştırmalar |
| [problemler/sl-ve-giris-kalite-analizi.md](../problemler/sl-ve-giris-kalite-analizi.md) | TP/SL sonucu, re-entry riski, giriş kalitesi, fee/buffer etkisi |
| [problemler/vol-capture-sorunlar.md](../problemler/vol-capture-sorunlar.md) | Volatility capture stratejisi, tek taraf dolma, likidite, pencere riski |
| [yapılcak/yapılacak.md](../yapılcak/yapılacak.md) | `iv_mismatch_edge`, pairlock exit ve V1 karar akışı |

## Kod Referansları

| Alan | Kod bölgesi |
|---|---|
| `trigger.market_price` | `crates/bot-runner/src/trade_flow/triggers/market_price.rs` |
| Entry timing | `crates/bot-runner/src/trade_flow/triggers/market_price_entry_timing.rs` |
| PTB guard | `crates/bot-runner/src/trade_flow/guards/price_to_beat.rs` |
| `iv_mismatch_edge` runtime config | `crates/bot-runner/src/trade_flow/guards/price_to_beat/iv_mismatch_runtime_config.rs` |
| Max price relax | `crates/bot-runner/src/trade_flow/guards/price_to_beat/max_price_relax/` |
| PTB stop-loss bump | `crates/bot-runner/src/trade_builder/ptb_stop_loss_bump.rs` |
| PTB stop-loss | `crates/bot-runner/src/trade_builder/ptb_stop_loss.rs` |
| Pair lock validation | `frontend/src/lib/queries/trade-flow/validation-action-place-order-pair.ts` |
| PTB bump/relax validation | `frontend/src/lib/queries/trade-flow/validation-action-place-order-ptb-bump.ts` |
| PTB V2 validation | `frontend/src/lib/queries/trade-flow/validation-action-place-order-ptb-v2.ts` |
| Analytics extraction | `frontend/src/lib/queries/trade-flow/analytics.ts` |
| Relax API | `frontend/src/app/api/relax/route.ts` |

## Bakım Kuralları

- Kod alanı değiştiğinde önce ilgili `referans/` dosyası, sonra onu anlatan `senaryolar/` dosyası güncellenmelidir.
- Sadece strateji yorumu değiştiyse `senaryolar/10-volatility-capture-stratejileri.md` yeterli olabilir.
- Yeni config alanı UI validation'da görünüyorsa [referans/action-place-order.md](./referans/action-place-order.md) veya [referans/trigger-market-price.md](./referans/trigger-market-price.md) içine eklenmelidir.
- Yeni telemetry alanı analytics'e çıktıysa [referans/terimler-eventler-ve-telemetri.md](./referans/terimler-eventler-ve-telemetri.md) ve [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) güncellenmelidir.

## Değişiklik Yaparken İzlenecek Yol

1. Önce kod değişikliğinin hangi runtime davranışı etkilediğini belirle.
2. Etkilenen config alanları varsa `referans/` dosyasına ekle.
3. Operatörün canlıda göreceği sonuç değişiyorsa ilgili `senaryolar/` dosyasına örnek akış ekle.
4. Kopyalanabilir ayar gerekiyorsa `ornekler/config-receteleri.md` içine küçük ama çalışan örnek koy.
5. Yeni hata veya block nedeni oluştuysa `ornekler/troubleshooting-checklist.md` içine teşhis adımı ekle.
6. Yeni telemetry alanı varsa `referans/terimler-eventler-ve-telemetri.md` içinde alanın ne zaman dolu olduğunu açıkla.

Bu sıra dokümanın iki farklı probleme düşmesini engeller: sadece alan listesi olup davranışı anlatmamak veya sadece senaryo anlatıp gerçek config adını vermemek.

## Koddan Dokümana Eşleme Örnekleri

| Kodda görülen değişiklik | Dokümanda güncellenecek yer |
|---|---|
| Yeni `priceToBeatIv*` alanı | `referans/action-place-order.md`, `senaryolar/04-ptb-guard-ve-iv-mismatch.md` |
| Yeni `notifyOn*` alanı | `referans/action-place-order.md`, `senaryolar/09-telegram-telemetri-ve-analiz.md` |
| Pair lock validation değişikliği | `senaryolar/07-pair-lock-ve-edge-pairlock.md`, `ornekler/troubleshooting-checklist.md` |
| Yeni analytics column | `referans/terimler-eventler-ve-telemetri.md`, `senaryolar/09-telegram-telemetri-ve-analiz.md` |
| Yeni risk guard | `senaryolar/08-risk-guardlari-ve-hata-durumlari.md`, `ornekler/config-receteleri.md` |

## Güncellik Kontrolü

Bu dokümanlar kaynak kodun birebir otomatik çıktısı değildir. Bu yüzden yeni bir alan eklenince üç kontrol yapılmalıdır:

- Frontend validation alanı kabul ediyor mu?
- Runtime bu alanı gerçekten okuyor mu?
- Event veya analytics payload operatöre bu kararın sonucunu gösteriyor mu?

Üçünden biri eksikse dokümanda "var" diye anlatmak yanıltıcı olur. Özellikle UI'da görünen ama runtime tarafından kullanılmayan alanlar net olarak ayrıştırılmalıdır.
