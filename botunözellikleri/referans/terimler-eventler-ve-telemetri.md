# Referans - Terimler, Eventler ve Telemetry

Güncelleme tarihi: 2026-05-01

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
| DCA live | Generic slug/outcome üstünde DCA ladder action'ı |
| Decision log | Karar anındaki forensic kayıt |
| Node snapshot | Order anındaki action/upstream config kopyası |
| Activity cash PnL | Polymarket activity/redeem etkisine göre nakit sonuç |
| Funds activation | Safe üzerindeki USDC.e bakiyeyi pUSD'ye activate etme işlemi |

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
| Forensic event | Decision log ve node snapshot kayıtları |
| Claim event | Redeem, receipt confirmation, funds activation |

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

Adaptive pair alanları:

| Alan | Anlam |
|---|---|
| `adaptiveMaxPrice` | Adaptive max price karar payload'ı |
| `manualAdaptiveRisk` | Manual adaptive risk karar payload'ı |
| `manualAdaptiveRiskCounterCapCent` | Counter bacak için uygulanmış risk tavanı |
| `biasedHedge` | Biased hedge primary/hedge karar payload'ı |
| `timing` | Strategy window, elapsed/remaining saniye ve in-window bilgisi |

## DCA Live Telemetry

| Alan | Anlam |
|---|---|
| `mode="dca_live_v1"` | Action DCA live yolunda çalıştı |
| `selectedOutcomes` | DCA için çözülen outcome listesi |
| `sideMode` | `one_sided`, `two_sided_pair` veya `multi_outcome_basket` |
| `dcaLevels` | Üretilmesi beklenen ladder seviyesi |
| `blocked_reason` | Window, budget veya outcome block nedeni |

## Forensic Telemetry

| Alan | Anlam |
|---|---|
| `decision_id` | Karar zinciri kimliği |
| `root_order_id` | Ana order bağlamı |
| `node_snapshot` | Action ve direct upstream config snapshot'ı |
| `cashStatus` | Activity cash PnL durum sınıfı |
| `market_timeline_status` | No-order timeline hazır mı |
| `rotation_lag_ms` | Auto-scope market rotasyon gecikmesi |
| `first_trigger_lag_ms` | Market başlangıcından ilk trigger'a gecikme |
| `first_action_lag_ms` | Market başlangıcından ilk action'a gecikme |

## Claim ve Funds Activation Eventleri

| Event | Anlam |
|---|---|
| `receipt_confirmed` | Redeem tx receipt confirmed |
| `funds_activated` | USDC.e -> pUSD activation transaction submitted |
| `funds_activation_skipped` | Activation gerekli değil veya threshold altında |
| `funds_activation_failed` | Relayer/config/onchain activation hatası |

Funds activation payload'ında `owner_address`, `activated_amount_usdc`, `approve_tx_hash`, `wrap_tx_hash`, `usdce_balance`, `pusd_balance` ve `message` birlikte okunmalıdır.

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
| Decision log bugünkü config'i gösterir | Order anındaki snapshot ayrı okunmalıdır |
| Activity cash PnL diagnostic PnL ile aynı | Redeem/settlement durumuna göre ayrışabilir |
| Funds activation CLOB order hatasıdır | Bu CTF/relayer token lifecycle operasyonudur |

## Minimum Teşhis Seti

Bir sorunu raporlarken şu alanları birlikte al:

- Market slug ve window zamanı.
- Trigger node key ve action node key.
- Token/outcome.
- Guard decision payload.
- Builder order id varsa status.
- Telegram mesajı veya event id.
- Analytics zaman aralığı.

## Örnek Payload Yorumlama

Örnek PTB block:

```json
{
  "node_type": "action.place_order",
  "market_slug": "btc-updown-5m-1800000000",
  "side": "buy",
  "price_to_beat_guard": {
    "threshold_mode": "iv_mismatch_edge",
    "edge": 0.04,
    "dynamic_threshold": 0.08,
    "adaptive_regime": "orange",
    "binance_same_direction": false
  }
}
```

Yorum:

- Action çalışmış.
- Buy order PTB guard aşamasında durmuş.
- Edge 0.04, gereken threshold 0.08.
- Orange rejim ve Binance ters yön kararı sıkılaştırmış.
- Max price veya risk gate değil, IV edge kaynaklı block okunmalıdır.

Örnek max price block:

```json
{
  "node_type": "action.place_order",
  "selected_entry_max_price": 0.62,
  "best_ask": 0.65,
  "max_price_guard": {
    "decision": "blocked"
  }
}
```

Yorum:

- Trigger profile 0.62 tavan üretmiş.
- Best ask 0.65.
- PTB iyi olsa bile order gönderilmemesi beklenir.

## Event Zamanı ve Market Zamanı

Event timestamp ile market window timestamp aynı şey değildir.

- Event timestamp: Botun olayı kaydettiği an.
- Market window: Polymarket 5m marketinin başlangıç/bitiş aralığı.
- Provider timestamp: Chainlink/Binance gibi external verinin kendi zamanı.

Staleness sorunlarında bu üç zaman ayrıştırılmalıdır. Event yeni olabilir ama provider timestamp eski olabilir. Bu durumda bot çalışıyor, fakat karar verisi taze değildir.

## Telemetry Alanı Boşsa Ne Anlama Gelir?

Boş alan her zaman hata değildir.

| Boş alan | Muhtemel sebep |
|---|---|
| `iv_mismatch_edge` yok | PTB mode IV edge değildir veya guard çalışmamıştır |
| `builder_order_id` yok | Order creation aşamasına gelinmemiştir |
| `pair_session_id` yok | Pair lock flow'u değildir veya pair başlamamıştır |
| `relax_credit_usd` yok | Relax config kapalı veya PTB guard çalışmamıştır |
| Fill alanları yok | Submit/fill aşamasına gelinmemiştir |

Boş alan yorumlanırken önce ilgili feature'ın açık olup olmadığı kontrol edilmelidir.

## Teşhis Cümlesi Kurma

İyi teşhis:

```text
BTC 5m Up 12:00 window'unda trigger pass olmuş, action çalışmış, max price guard best ask 0.65 > selected max 0.62 nedeniyle block etmiş. Submit yok, fill yok beklenen.
```

Zayıf teşhis:

```text
Bot almadı.
```

İyi teşhis hem son başarılı aşamayı hem de durduran kararı söyler.
