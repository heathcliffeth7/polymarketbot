# 09 - Telegram, Telemetry ve Analiz

Bu dosya botun operatöre hangi olayları nasıl gösterdiğini, no-order analizinin nasıl okunacağını ve telemetry alanlarının pratik anlamını açıklar.

## Amaç

Kısa marketlerde işlem sonrası değil, işlem anında teşhis gerekir. Telegram bildirimleri hızlı sinyal verir; analytics ve event payload ise kök nedeni bulmak için kullanılır.

## Bildirim Grupları

| Alan | Anlam |
|---|---|
| `notifyOnOrderSubmitted` | Builder order CLOB submit edildiğinde bildir |
| `notifyOnOrderPlaced` | Order oluşturuldu/yerleştirildiğinde bildir |
| `notifyOnOrderNotFilled` | Submit sonrası fill olmazsa bildir |
| `notifyOnTriggerPriceBlocked` | Trigger price guard block ederse bildir |
| `notifyOnExecutionFloorBlocked` | Execution floor block ederse bildir |
| `notifyOnPriceToBeatGapBlocked` | PTB guard block ederse bildir |
| `notifyOnMaxPriceBlocked` | Max price guard block ederse bildir |
| `notifyOnTpHit` | TP gerçekleşince bildir |
| `notifyOnSlHit` | SL gerçekleşince bildir |
| `notifyOnPairLocked` | Pair lock kurulduğunda bildir |
| `notifyOnPairUnwind` | Pair unwind olduğunda bildir |

Counter leg için ayrıca `counterLegNotifyOnTpHit`, `counterLegNotifyOnSlHit` gibi alanlar kullanılabilir.

## Senaryo A: Submit Var, Fill Yok

Belirti:

- Telegram submit geldi.
- Fill bildirimi gelmedi.
- Pozisyon açılmadı.

Bakılacaklar:

1. `notifyOnOrderSubmitted` mesajındaki market/token.
2. CLOB order status.
3. Best ask/orderbook fiyatı.
4. Limit price çok düşük mü?
5. `notifyOnOrderNotFilled` geldi mi?

Yorum:

- Submit gelmesi guard'ların geçtiğini gösterir.
- Fill olmaması orderbook/limit/latency problemidir.

## Senaryo B: No-Order

Belirti:

- Trigger beklenen zamanda geçmiş gibi görünüyor.
- Submit bildirimi yok.

Bakılacaklar:

1. Trigger gerçekten `pass=true` oldu mu?
2. Action çalıştı mı?
3. Action event payload içinde block reason var mı?
4. No-order analytics hangi guard'ı işaret ediyor?

Tipik nedenler:

- Max price block.
- PTB gap block.
- Execution floor block.
- Stale market skip.
- Fill lock.
- Risk gate block.

## No-Order Analytics

Analytics tarafında PTB bump/relax ve no-order detayları şu tarz alanlarla okunur:

- `bump_usd`
- `bump_increment_usd`
- `relax_credit_usd`
- `relax_miss_reason`
- `first_tradable_second`
- `first_tradable_gap_usd`
- `tradable_seconds_count`
- `price_ok_depth_fail_count`
- `max_fillability_score`
- `quality_score`

Bu alanlar "bot hiç trade almıyor" sorusunu fiyat, depth ve guard davranışı olarak ayrıştırır.

## Senaryo C: Relax Paneli

`/api/relax` global `strategy.max_price_relax_enabled` anahtarını okur/yazar.

Operatör akışı:

1. UI veya API'dan relax durumu okunur.
2. `max_price_relax_enabled=false` ise node config açık olsa bile relax davranışı beklenmez.
3. Toggle sonrası servis restart gerekip gerekmediği kontrol edilir.
4. Analytics'te relax credit oluşup oluşmadığı izlenir.

## Event ve Payload Okuma

Bir olay payload'ında önce şu hiyerarşi okunur:

1. `node_key` ve `node_type`.
2. `market_slug`, `token_id`, `outcome_label`.
3. `side`, `execution_mode`, `kind`.
4. Guard adı ve decision.
5. Price/orderbook snapshot.
6. Retry planı.
7. Notification flags.

PTB özelinde:

- `price_to_beat_guard.threshold`
- `price_to_beat_guard.gap_usd`
- `price_to_beat_guard.iv_mismatch_edge`
- `price_to_beat_guard.max_price_relax`

Pair lock özelinde:

- `pair_lock_strategy`
- `pair_lock_edge_decision`
- `pair_session_id`
- `counter_builder_order_id`

## Telegram Mesajını Yorumlama

| Mesaj tipi | Ne anlama gelir |
|---|---|
| Order submitted | Guard geçti, CLOB submit denendi |
| Order not filled | Submit oldu ama fill gerçekleşmedi |
| PTB blocked | Giriş kalitesi yetersiz veya IV edge block |
| Max price blocked | Fiyat tavan üstünde |
| Execution floor blocked | Orderbook yeterli değil |
| TP hit | Pozisyon kârla kapandı/kısmen kapandı |
| SL hit | Pozisyon zarar kesti |
| Pair locked | YES/NO maliyet kilidi kuruldu |
| Pair unwind | Pair lock bozuldu veya koruyucu çıkış yapıldı |

## Analiz Zaman Aralığı

Auto-scope marketlerde analiz aralığı doğru seçilmezse eski marketler yeni marketlerle karışır.

Pratik:

- 5m market için son 30-60 dakikalık pencereyle başla.
- Bir sorun tek marketteyse slug bazlı filtrele.
- Boundary problemlerinde birkaç market öncesi ve sonrası birlikte incelenir.
- Pair lock için session id ile filtrelemek daha net sonuç verir.

## Operatör Checklist

- Bildirim flag'i kapalı olduğu için olay görünmüyor olabilir mi?
- Submit ile fill aynı şey sanılıyor mu?
- No-order durumunda trigger pass ile action block ayrıştırıldı mı?
- Analytics zaman aralığı doğru marketleri kapsıyor mu?
- Relax açık sanılıyor ama global toggle kapalı mı?
- Pair lock mesajında decision ve session id takip edildi mi?
