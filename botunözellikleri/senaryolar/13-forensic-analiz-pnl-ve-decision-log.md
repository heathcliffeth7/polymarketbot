# 13 - Forensic Analiz, PnL ve Decision Log

Güncelleme tarihi: 2026-05-01

## Amaç

Trade analysis artık sadece order/event listesi değildir. Bot kararlarının neden verildiğini açıklamak için decision log, node snapshot, official/activity cash PnL ve missed-market timing diagnostics birlikte okunur.

Bu bölüm "bot niye almadı?", "aldı ama neden zarar yazdı?", "analytics Polymarket profilinden niye farklıydı?" sorularını aynı kanıt setiyle cevaplamak içindir.

## Ana Veri Katmanları

| Katman | Ne gösterir |
|---|---|
| `trade_builder_orders` | Order lifecycle, status, side, price, qty |
| `trade_builder_order_events` | Guard, submit, fill, TP/SL, notification eventleri |
| `bot_decision_logs` | Kararın alındığı anda normalleştirilmiş forensic kayıt |
| `trade_builder_order_node_snapshots` | Order üretildiği sıradaki action node ve upstream config snapshot'ı |
| `trade_flow_auto_scope_analysis_rows` | Market/window bazlı analiz satırları |
| Polymarket activity/Data API | Official cash PnL ve redeem/settlement uyumu |

Tek kaynak yeterli değildir. Decision log karar sebebini, order table lifecycle'ı, activity cash ise gerçekleşen nakit etkisini gösterir.

## Decision Log

Decision log şu durumlarda özellikle değerlidir:

- Entry evaluate edildi ama order yok.
- Guard block mu, max price mı, risk gate mi ayırmak gerekiyor.
- SL/TP sonrası child order davranışı inceleniyor.
- Pair lock veya adaptive strategy neden farklı karar verdi anlaşılmıyor.

Beklenen alanlar:

- `event_type`
- `decision_id`
- `root_order_id`
- `order_id`
- `market_slug`
- `node_key`
- `payload_json`
- `created_at`

İyi teşhis cümlesi:

```text
Decision log entry_evaluated aşamasında node snapshot'ı gösteriyor; action config manual_adaptive_risk_v1, in_window=false olduğu için primary buy açılmamış.
```

## Node Snapshot

Node snapshot order oluşturulurken kullanılan config'i dondurur.

Faydası:

- UI'da sonradan değişen config ile geçmiş order kararı karıştırılmaz.
- Action node ve direct upstream node'lar birlikte okunur.
- Adaptive strategy payload'ı hangi config ile üretildi görülebilir.

Özellikle şu pointer'lar aranır:

- `/node_snapshot/action_node/config`
- `/node_snapshot/action_node/config/adaptiveMaxPrice`
- `/node_snapshot/action_node/config/manualAdaptiveRisk`
- `/node_snapshot/upstream_nodes`

## Official / Activity Cash PnL

Analiz PnL'i artık sadece botun lokal fill/exit hesabı değildir. Polymarket activity ve wallet PnL ile reconcile edilen cash metrikleri kullanılır.

Önemli ayrım:

| Alan | Anlamı |
|---|---|
| Diagnostic PnL | Botun kendi trade lifecycle hesabı |
| Activity cash PnL | Polymarket activity/redeem etkisine göre nakit sonuç |
| Wallet/reference PnL | Harici profil veya wallet görünümüyle karşılaştırma |
| `cashStatus` | Pending, redeemed, unclaimed/lost gibi nakit durum sınıfı |

`lost_unclaimed_or_unredeemed` görüldüğünde bu her zaman trading hatası değildir; claim/redeem lifecycle tamamlanmamış olabilir.

## 48h ve Zaman Aralığı

Analiz ekranında 48h aralığı kısa süreli strateji değişikliği sonrası daha hızlı geri bildirim verir.

Kullanım:

- Son deploy sonrası davranış değişti mi?
- Son SL serisi sadece bugünün sorunu mu?
- Activity cash ile diagnostic PnL ayrımı son 48h içinde mi oluştu?

Yanlış kullanım:

- 48h aralığını uzun vadeli EV sonucu gibi yorumlamak.
- Market settlement/redeem gecikmesi varken cash PnL'i nihai sonuç saymak.

## Missed Market Timing Diagnostics

No-order bildirimi artık sadece "order yok" demez; market timeline çıkarmaya çalışır.

Bakılacak alanlar:

- `market_timeline_status`
- `market_start_at`
- `rotation_detected_at`
- `rotation_lag_ms`
- `first_trigger_at`
- `first_action_at`
- `first_ptb_guard_at`
- `ptb_cache_lag_ms`

Yorum örneği:

```text
Market rotation 4 saniye geç yakalanmış, trigger hiç action'a ulaşmamış. Bu PTB threshold sorunu değil, market/timing sorunu.
```

## Analiz Akışı

```text
1. Market slug ve zaman aralığını sabitle
2. Analysis row ile cashStatus ve PnL sınıfını oku
3. Order lifecycle var mı bak
4. Decision log ile son karar noktasını bul
5. Node snapshot ile o andaki config'i doğrula
6. Telegram/no-order payload varsa timeline ile karşılaştır
```

Bu sıra ters yapılırsa son config'e bakıp geçmiş kararı yanlış yorumlama riski oluşur.

## Sık Teşhisler

| Belirti | İlk bakılacak kanıt |
|---|---|
| Trigger var, order yok | Decision log + action event |
| Order var, fill yok | Order status + depth/limit payload |
| PnL UI ile profil farklı | Activity cash PnL + `cashStatus` |
| Claim sonrası PnL değişti | Redeem/claim events + official ledger |
| Adaptive strateji çalışmadı | Node snapshot strategy config |
| Missed market bildirimi geldi | Timeline diagnostics |

## Operatör Checklist

1. Analiz aralığı doğru mu?
2. Cash PnL mi diagnostic PnL mi okunuyor?
3. Aynı root order için decision log var mı?
4. Node snapshot config'i bugünkü UI config'inden farklı mı?
5. No-order için trigger/action/guard zamanları ayrılmış mı?
6. Claim/redeem pending ise trading PnL ile cash PnL ayrı raporlanmalı.

## Kaynak Notu

Kod ve migration referansları:

- `migrations/070_bot_decision_logs.sql`
- `migrations/071_trade_builder_order_node_snapshots.sql`
- `migrations/072_trade_flow_auto_scope_settled_payout.sql`
- `crates/bot-runner/src/trade_builder/decision_logs.rs`
- `crates/bot-runner/src/trade_builder/node_snapshot.rs`
- `crates/bot-runner/src/trade_flow/missed_market_timeline.rs`
- `frontend/src/lib/queries/trade-flow/analytics.ts`
- `frontend/src/lib/queries/trade-flow/auto-scope-analysis-extras.ts`
