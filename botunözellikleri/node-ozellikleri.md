# Trade Flow Node Özellikleri

Güncelleme tarihi: 2026-05-01

Bu dosya artık kısa node index'idir. Ayrıntılı anlatımlar özellik bazlı dosyalara bölündü.

## Ana Node'lar

| Node | Görev | Detay |
|---|---|---|
| `trigger.market_price` | Market fiyatını, auto-scope seçimini, entry timing profilini, firing mode'u ve binding modlarını yönetir | [referans/trigger-market-price.md](./referans/trigger-market-price.md) |
| `action.place_order` | Alım/satım builder order üretir, guard'ları çalıştırır, TP/SL/pair lock/DCA live kurar | [referans/action-place-order.md](./referans/action-place-order.md) |

## Uçtan Uca Akış

```text
trigger.market_price
  -> market slug ve token çözümü
  -> fiyat koşulu / binding / entry timing / firing mode
  -> context output

action.place_order
  -> stale market ve risk kontrolleri
  -> PTB / max price / execution floor guard
  -> builder order, pair lock veya DCA live kararı
  -> TP/SL/re-entry/telemetry kurulumu
```

## Özellik Bazlı Okuma

| Konu | Dosya |
|---|---|
| 5 dakikalık market döngüsü, slug, auto-scope | [senaryolar/01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) |
| Trigger koşulları, `entryTimingProfiles`, once/repeat | [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) |
| Buy/sell order, sizing, fill lock | [senaryolar/03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) |
| PTB guard modları ve `iv_mismatch_edge` | [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) |
| PTB stop-loss bump ve max price relax | [senaryolar/05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) |
| TP, SL, PTB SL, time exit, re-entry | [senaryolar/06-tp-sl-time-exit-ve-reentry.md](./senaryolar/06-tp-sl-time-exit-ve-reentry.md) |
| Pair lock ve `edge_pairlock_v1` | [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) |
| Guard block, retry, stale market, risk | [senaryolar/08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) |
| Telegram, event, analytics | [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) |
| Volatility capture strateji değerlendirmesi | [senaryolar/10-volatility-capture-stratejileri.md](./senaryolar/10-volatility-capture-stratejileri.md) |
| DCA live, generic slug/outcome DCA ve `dca_live_only` binding | [senaryolar/11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) |
| Adaptive pair lock, manual adaptive risk ve biased hedge | [senaryolar/12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) |
| Decision log, node snapshot ve official/activity cash PnL | [senaryolar/13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) |
| Claim sweep, redeem ve funds activation | [senaryolar/14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) |

## Referans Dosyaları

- [referans/trigger-market-price.md](./referans/trigger-market-price.md) - trigger config, output ve context alanları.
- [referans/action-place-order.md](./referans/action-place-order.md) - action config, guard ve lifecycle alanları.
- [referans/terimler-eventler-ve-telemetri.md](./referans/terimler-eventler-ve-telemetri.md) - ortak terimler, event isimleri, telemetry okumaları.

## Operasyon Dosyaları

- [ornekler/config-receteleri.md](./ornekler/config-receteleri.md) - yaygın kurulumlar için JSON örnekleri.
- [ornekler/troubleshooting-checklist.md](./ornekler/troubleshooting-checklist.md) - order yok, fill yok, pair lock yok, relax yok durumlarında kontrol listesi.

## Pratik Başlangıç

Yeni bir flow tasarlarken sırayla şu kararları ver:

1. Market otomatik mi seçilecek, sabit slug mı kullanılacak?
2. Trigger fiyat koşulu mu bekleyecek, pair binding mi yoksa DCA binding mi yapacak?
3. Giriş zamanı sabit mi, kalan süreye göre profile mı bağlanacak?
4. Order market mi limit mi, immediate mı conditional mı?
5. Buy tarafında PTB, max price, execution floor ve underlying guard açık mı?
6. Çıkış hard TP/SL mi, staged ladder mı, PTB stop-loss mı?
7. SL sonrası re-entry, bump veya max price relax kullanılacak mı?
8. Pair lock gerekiyorsa legacy, edge, adaptive, manual adaptive veya biased hedge mi?
9. DCA live gerekiyorsa selected outcome, ladder ve budget guard hazır mı?
10. Telegram, decision log ve analytics alanları operatörün ihtiyacına göre açık mı?

## Node'lar Arası Sorumluluk Ayrımı

`trigger.market_price` ve `action.place_order` birbirine benzeyen fiyat alanları taşıyabilir, ama sorumlulukları aynı değildir.

`trigger.market_price` kararları:

- Hangi market izlenecek?
- Hangi token/outcome bağlama yazılacak?
- Fiyat koşulu geçti mi?
- Kalan süreye göre hangi entry profile seçilecek?
- Pair lock için YES/NO tokenları çözüldü mü?
- DCA live için market/window binding'i downstream'e taşınacak mı?

`action.place_order` kararları:

- Bu sinyale gerçekten order üretilecek mi?
- Order buy mı sell mi?
- Büyüklük nereden hesaplanacak?
- Fiyat, depth, PTB, risk ve re-entry guard'ları geçiyor mu?
- TP/SL/time exit/pair lock çocuk emirleri nasıl kurulacak?
- DCA live ise selected outcome, ladder, budget ve window guard nasıl uygulanacak?

Bu ayrım canlı debug sırasında çok önemlidir. Trigger'ın başarılı olması "trade açıldı" anlamına gelmez. Trigger sadece downstream için bağlam ve izin üretir; action ise bu bağlamı order lifecycle'a dönüştürür.

## Uçtan Uca Örnek Okuma

Bir BTC 5m Up flow'u şu şekilde düşünülmelidir:

1. Auto-scope aktif BTC 5m marketini çözer.
2. Trigger Up fiyatını takip eder.
3. Entry timing kalan süreye göre profil seçer.
4. Trigger `pass=true` üretir ve `marketSlug`, `tokenId`, `outcomeLabel`, seçili max price ve size bilgilerini context'e yazar.
5. Action bu context'i alır.
6. Action önce stale market kontrolü yapar.
7. Sonra max price, execution floor ve PTB guard çalışır.
8. Risk gate izin verirse builder order oluşur.
9. `kind="immediate"` ise submit denenir.
10. Fill gelirse TP/SL/re-entry/pair logic devreye girer.

Bu zincirde sorun ararken "son başarılı halka" bulunmalıdır. Eğer trigger output var ama action event yoksa routing veya downstream aktivasyonuna bakılır. Action event var ama builder order yoksa guard block'a bakılır. Builder order var ama fill yoksa CLOB submit, limit price ve liquidity incelenir.

## Dokümanların Birbirine Bağlanma Şekli

- `senaryolar/` davranışı anlatır.
- `referans/` alan isimlerini ve config/output ilişkisini netleştirir.
- `ornekler/` kopyalanabilir reçete ve debug akışı verir.

Bir dosyada kavram anlatılıp başka dosyada config örneği veriliyorsa linkler bilinçli olarak ayrıdır. Amaç aynı uzun dosyada her şeyi tekrarlamak değil, operatörün sorun tipine göre doğru derinliğe inmesini sağlamaktır.
