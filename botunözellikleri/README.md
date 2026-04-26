# Botun Özellikleri

Güncelleme tarihi: 2026-04-26

Bu klasör Polymarket botunun trade flow, guard, pair lock, çıkış, analiz ve operasyon özelliklerini senaryo kitabı formatında anlatır. Amaç tek bir uzun dosyada kaybolmadan, her özelliği kendi bağlamında okumaktır.

## Okuma Sırası

1. [node-ozellikleri.md](./node-ozellikleri.md) - kısa node index'i.
2. [yeni-ozellikler.md](./yeni-ozellikler.md) - son eklenen özelliklerin özet changelog'u.
3. [senaryolar/01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) - 5 dakikalık market ve auto-scope temeli.
4. [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) - trigger, entry timing ve fiyat tetikleri.
5. [senaryolar/03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) - order üretimi, sizing, fill lock.
6. [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) - PTB guard ve `iv_mismatch_edge`.
7. [senaryolar/05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) - stop-loss sonrası bump ve relax.
8. [senaryolar/06-tp-sl-time-exit-ve-reentry.md](./senaryolar/06-tp-sl-time-exit-ve-reentry.md) - TP, SL, time exit, re-entry.
9. [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) - pair lock ve `edge_pairlock_v1`.
10. [senaryolar/08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) - risk, retry ve block akışları.
11. [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) - bildirim, event ve analiz ekranları.
12. [senaryolar/10-volatility-capture-stratejileri.md](./senaryolar/10-volatility-capture-stratejileri.md) - strateji riskleri ve EV senaryoları.

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
| Telegram mesajında hangi alan ne demek? | [09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) |
| Config örneği lazım | [config-receteleri.md](./ornekler/config-receteleri.md) |

## Bakım Notu

- Bu klasördeki dosyalar el yazımı dokümandır; her dosya 1500 satır altında kalmalıdır.
- Yeni özellik eklenirse önce ilgili senaryo dosyası güncellenir, sonra gerekirse `referans/` ve `ornekler/` tamamlanır.
- Eski tek dosyalı referansın yerini artık bu index ve alt klasörler alır.
