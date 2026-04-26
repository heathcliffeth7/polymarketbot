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
