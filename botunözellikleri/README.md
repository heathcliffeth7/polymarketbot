# Botun Özellikleri

Güncelleme tarihi: 2026-05-01

Bu klasör Polymarket botunun trade flow, guard, pair lock, DCA, analiz, claim ve operasyon özelliklerini senaryo kitabı formatında anlatır. Amaç tek bir uzun dosyada kaybolmadan, her özelliği kendi bağlamında okumaktır.

## Okuma Sırası

1. [node-ozellikleri.md](./node-ozellikleri.md) - kısa node index'i.
2. [yeni-ozellikler.md](./yeni-ozellikler.md) - son eklenen özelliklerin özet changelog'u.
3. [gecmis-islem-kanitlari.md](./gecmis-islem-kanitlari.md) - botun kendi eski işlem kayıtlarından çalışma kanıtları.
4. [olympusx-dersleri-bot-neden-basarisiz.md](./olympusx-dersleri-bot-neden-basarisiz.md) - OlympusX/JetFadil analizinden botun başarısızlık nedenleri.
5. [senaryolar/01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) - 5 dakikalık market ve auto-scope temeli.
6. [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) - trigger, entry timing ve fiyat tetikleri.
7. [senaryolar/03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) - order üretimi, sizing, fill lock.
8. [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) - PTB guard ve `iv_mismatch_edge`.
9. [senaryolar/05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) - stop-loss sonrası bump ve relax.
10. [senaryolar/06-tp-sl-time-exit-ve-reentry.md](./senaryolar/06-tp-sl-time-exit-ve-reentry.md) - TP, SL, time exit, re-entry.
11. [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) - pair lock ve `edge_pairlock_v1`.
12. [senaryolar/08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) - risk, retry ve block akışları.
13. [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) - bildirim, event ve analiz ekranları.
14. [senaryolar/10-volatility-capture-stratejileri.md](./senaryolar/10-volatility-capture-stratejileri.md) - strateji riskleri ve EV senaryoları.
15. [senaryolar/11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) - generic DCA live action ve trigger binding.
16. [senaryolar/12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) - adaptive max price, manual adaptive risk ve biased hedge.
17. [senaryolar/13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) - decision logs, node snapshots ve cash PnL.
18. [senaryolar/14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) - claim sweep, relayer ve funds activation.

## Klasörler

- [senaryolar/](./senaryolar/) - operatörün karar anında okuyacağı akışlar.
- [referans/](./referans/) - node alanları, output/event isimleri ve config referansı.
- [ornekler/](./ornekler/) - kopyalanabilir config reçeteleri ve troubleshooting checklist.

## Hızlı Harita

| İhtiyaç | Okunacak dosya |
|---|---|
| Auto-scope neden yanlış markete baktı? | [01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) |
| Trigger geçiyor ama order gelmiyor | [02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md), [08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) |
| PTB iyi mi, pahalı mı, IV edge ne diyor? | [04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) |
| Çok SL sonrası bot niye daha seçici oldu? | [05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) |
| Pair lock neden tek bacak açtı veya hiç açmadı? | [07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) |
| DCA live action neden çalışmadı? | [11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) |
| Adaptive pair lock neden gevşemedi veya sıkılaştı? | [12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) |
| Geçmiş kararın hangi config ile alındığı lazım | [13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) |
| Claim, redeem veya funds activation hatası var | [14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) |
| Botun eski işlemlerden kanıtı lazım | [gecmis-islem-kanitlari.md](./gecmis-islem-kanitlari.md) |
| Bot neden başarısız oluyor? | [olympusx-dersleri-bot-neden-basarisiz.md](./olympusx-dersleri-bot-neden-basarisiz.md) |
| Telegram mesajında hangi alan ne demek? | [09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) |
| Config örneği lazım | [config-receteleri.md](./ornekler/config-receteleri.md) |

## Bakım Notu

- Bu klasördeki dosyalar el yazımı dokümandır; her dosya 1500 satır altında kalmalıdır.
- Yeni özellik eklenirse önce ilgili senaryo dosyası güncellenir, sonra gerekirse `referans/` ve `ornekler/` tamamlanır.
- Eski tek dosyalı referansın yerini artık bu index ve alt klasörler alır.

## Kullanıcı Tipine Göre Okuma Yolu

### Yeni Başlayan

1. Önce bu dosyadaki hızlı haritayı oku.
2. [node-ozellikleri.md](./node-ozellikleri.md) ile iki ana node'un rolünü ayır.
3. [01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) ile 5 dakikalık market mantığını öğren.
4. [03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) ile trigger sonrası order'ın nasıl oluştuğunu takip et.

Bu rota botun "neden önce market seçiyor, sonra trigger çalışıyor, sonra order üretiyor" sırasını anlatır. Strateji kararlarına geçmeden önce bu sıralama net olmalıdır.

### Operatör

1. Canlı sorunu önce [troubleshooting-checklist.md](./ornekler/troubleshooting-checklist.md) ile sınıflandır.
2. Sorun hangi kategoriye düşüyorsa ilgili `senaryolar/` dosyasına geç.
3. Telegram mesajı varsa [09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) ile event/notification farkını oku.
4. Decision log veya node snapshot gerekiyorsa [13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) ile geçmiş config'i sabitle.
5. Config'i değiştirmeden önce [config-receteleri.md](./ornekler/config-receteleri.md) içindeki güvenli örneğe bak.

Operatör için amaç teori değil, doğru teşhistir. "Order yok" demeden önce trigger, action, guard, submit ve fill aşamaları ayrı ayrı kontrol edilmelidir.

### Strateji Tasarlayan

1. [10-volatility-capture-stratejileri.md](./senaryolar/10-volatility-capture-stratejileri.md) ile stratejinin EV risklerini oku.
2. [gecmis-islem-kanitlari.md](./gecmis-islem-kanitlari.md) ile gerçek işlem kayıtlarının ne söylediğini oku.
3. [olympusx-dersleri-bot-neden-basarisiz.md](./olympusx-dersleri-bot-neden-basarisiz.md) ile OlympusX derslerini botun kayıp nedenleriyle karşılaştır.
4. Entry kalitesi için [02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) ve [04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) dosyalarını birlikte değerlendir.
5. Exit kalitesi için [06-tp-sl-time-exit-ve-reentry.md](./senaryolar/06-tp-sl-time-exit-ve-reentry.md) dosyasını kullan.
6. Çift taraflı strateji için [07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) dosyasındaki tek bacak ve lock risklerini kontrol et.
7. Adaptive pair veya biased hedge kullanıyorsan [12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) dosyasındaki strateji/guard uyumunu kontrol et.

Strateji tasarımında en sık hata tek bir guard'ı "kâr makinesi" gibi görmektir. Guard'lar sadece kötü girişleri azaltır; fill kalitesi, fee, depth ve exit davranışı hâlâ sonucu belirler.

### Debug Yapan Geliştirici

1. [kaynak-haritasi.md](./kaynak-haritasi.md) ile hangi dokümanın hangi kod alanına dayandığını bul.
2. [referans/trigger-market-price.md](./referans/trigger-market-price.md) ve [referans/action-place-order.md](./referans/action-place-order.md) dosyalarıyla config/output alanlarını eşleştir.
3. [referans/terimler-eventler-ve-telemetri.md](./referans/terimler-eventler-ve-telemetri.md) ile event payload okuma sırasını izle.
4. Forensic/debug için decision log ve node snapshot akışını [13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) ile eşleştir.

Geliştirici için kritik ayrım şudur: trigger'ın `pass=true` üretmesi, action'ın order oluşturacağı anlamına gelmez. Runtime davranışı trigger output, action guard payload ve builder order lifecycle birlikte okunarak anlaşılır.

## Doküman Okurken Kullanılacak Ortak Mantık

Her özellik için şu sırayı takip et:

1. Bu özellik hangi riski azaltıyor?
2. Hangi config alanı özelliği açıyor?
3. Hangi alanlar birlikte zorunlu veya anlamlı?
4. Başarılı akışta hangi event/telemetry görülür?
5. Block akışında hangi alan sorunu gösterir?
6. Retry varsa block terminal mi geçici mi?
7. Bu özelliğin başka bir guard ile çakışma ihtimali var mı?

Bu sıra kullanılmazsa aynı belirti yanlış yorumlanabilir. Örneğin "PTB block" sanılan bir durum aslında max price block olabilir; "fill yok" sanılan bir durum aslında conditional pending order olabilir.
