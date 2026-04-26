# Trade Flow Node Özellikleri

Güncelleme tarihi: 2026-04-26

Bu dosya artık kısa node index'idir. Ayrıntılı anlatımlar özellik bazlı dosyalara bölündü.

## Ana Node'lar

| Node | Görev | Detay |
|---|---|---|
| `trigger.market_price` | Market fiyatını, auto-scope seçimini, entry timing profilini ve PTB trigger gate'i yönetir | [referans/trigger-market-price.md](./referans/trigger-market-price.md) |
| `action.place_order` | Alım/satım builder order üretir, guard'ları çalıştırır, TP/SL/pair lock kurar | [referans/action-place-order.md](./referans/action-place-order.md) |

## Uçtan Uca Akış

```text
trigger.market_price
  -> market slug ve token çözümü
  -> fiyat koşulu / binding / entry timing
  -> context output

action.place_order
  -> stale market ve risk kontrolleri
  -> PTB / max price / execution floor guard
  -> builder order veya pair lock kararı
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
2. Trigger fiyat koşulu mu bekleyecek, sadece pair binding mi yapacak?
3. Giriş zamanı sabit mi, kalan süreye göre profile mı bağlanacak?
4. Order market mi limit mi, immediate mı conditional mı?
5. Buy tarafında PTB, max price, execution floor ve underlying guard açık mı?
6. Çıkış hard TP/SL mi, staged ladder mı, PTB stop-loss mı?
7. SL sonrası re-entry, bump veya max price relax kullanılacak mı?
8. Pair lock gerekiyorsa legacy mi `edge_pairlock_v1` mi?
9. Telegram ve analytics alanları operatörün ihtiyacına göre açık mı?
