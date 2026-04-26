# Referans - Terimler, Eventler ve Telemetry

Bu dosya dokümantasyonda geçen ortak terimleri, event ailelerini ve telemetry alanlarını açıklar.

## Temel Terimler

| Terim | Anlam |
|---|---|
| Outcome | Up/Down veya YES/NO sonucu |
| Token | Belirli outcome için CTF token |
| Market slug | Polymarket market kimliği |
| Scope | Auto-scope market ailesi |
| Window | 5 dakikalık çözüm aralığı |
| Source trade | Buy/sell lifecycle bağlayan trade kaydı |
| Builder order | Botun CLOB'a göndermek üzere oluşturduğu order modeli |
| Inline submit | Builder order'ın oluşur oluşmaz CLOB'a gönderilmesi |
| Guard | Order üretimini engelleyebilen karar katmanı |
| Retry | Guard block sonrası tekrar deneme planı |
| Pair session | Pair lock lifecycle bağlamı |
| Orphan leg | Pair planında tek başına fill olmuş bacak |

## Event Aileleri

| Aile | Örnek anlam |
|---|---|
| Trigger event | Fiyat koşulu geçti/geçmedi, market seçildi |
| Action event | Order oluşturuldu, reuse edildi, block edildi |
| Guard event | PTB, max price, execution floor, risk kararı |
| Submit event | CLOB submit denendi veya başarısız oldu |
| Fill event | Order fill veya partial fill oldu |
| Exit event | TP, SL, PTB SL, time exit çalıştı |
| Pair event | Pair locked, no decision, unwind |
| Analytics event | No-order, relax, bump, fillability özetleri |

## Payload Okuma Sırası

Bir event payload'ını okurken şu sırayı izle:

1. `node_key` ve `node_type`.
2. `market_slug`, `token_id`, `outcome_label`.
3. `side`, `execution_mode`, `kind`.
4. Karar alanı: pass, block, skip, retry, submit.
5. Guard detayları.
6. Order id ve source trade id.
7. Pair session veya re-entry bağlamı.
8. Notification flag'leri.

## Price-to-Beat Telemetry

Önemli alanlar:

- `threshold`
- `effective_threshold`
- `gap_usd`
- `gap_cent`
- `threshold_mode`
- `stop_loss_bump_count`
- `stop_loss_bump_usd`
- `max_price_relax_relax_credit_usd`
- `max_price_relax_miss_reason`
- `iv_mismatch_edge`

`iv_mismatch_edge` içinde:

- `q`
- `q_final`
- `edge`
- `cost`
- `dynamic_threshold`
- `gap_strength`
- `required_gap_strength`
- `binance_staleness_ms`
- `binance_same_direction`
- `depth_guard_result`
- `adaptive_regime`
- `hourly_volume_ratio`

## Max Price Relax Telemetry

| Alan | Anlam |
|---|---|
| `relax_credit_usd` | Threshold'a uygulanan gevşeme |
| `relax_miss_reason` | Relax neden oluştu/oluşmadı |
| `first_tradable_second` | İlk tradeable saniye |
| `first_tradable_gap_usd` | İlk tradeable gap |
| `tradable_seconds_count` | Geçmiş markette tradeable saniye sayısı |
| `price_ok_depth_fail_count` | Fiyat uygun ama depth yetersiz sayısı |
| `max_fillability_score` | En iyi fillability skoru |
| `quality_score` | Relax kalite skoru |

## Pair Lock Telemetry

| Alan | Anlam |
|---|---|
| `pair_session_id` | Pair lifecycle kimliği |
| `pair_lock_strategy` | `legacy` veya `edge_pairlock_v1` |
| `pair_lock_edge_decision` | `position_counter_lock`, `fresh_equal_pair`, `single_edge`, no decision |
| `pair_lock_edge` | Edge skoru |
| `pair_total` | YES+NO toplam maliyet |
| `target_qty` | Hedef share miktarı |
| `counter_builder_order_id` | Counter leg order id |

## Bildirim ve Event Ayrımı

Telegram bildirimi event'in kullanıcıya gösterilmiş halidir. Bildirim kapalıysa event yine yazılmış olabilir.

Örnek:

- `notifyOnPriceToBeatGapBlocked=false` ise PTB block Telegram'a düşmeyebilir.
- Analytics/event payload yine block nedenini içerebilir.

## Sık Yanlış Okumalar

| Yanlış yorum | Doğru yorum |
|---|---|
| Submit bildirimi fill demektir | Submit sadece CLOB'a gönderme denemesidir |
| Trigger pass order garantisidir | Action guard'ları ayrıca block edebilir |
| Max price block PTB block'tur | İki guard ayrıdır |
| Relax açık demek node gevşer | Global toggle, miss count ve depth şartı da gerekir |
| Pair lock no decision hata demektir | Maliyet/edge şartı yoksa beklenen skip olabilir |

## Minimum Teşhis Seti

Bir sorunu raporlarken şu alanları birlikte al:

- Market slug ve window zamanı.
- Trigger node key ve action node key.
- Token/outcome.
- Guard decision payload.
- Builder order id varsa status.
- Telegram mesajı veya event id.
- Analytics zaman aralığı.
