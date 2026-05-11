# Geçmiş İşlem Kanıtları

Güncelleme tarihi: 2026-04-27

Bu dosya botun çalışma şeklini dış teoriyle değil, botun kendi eski işlem kayıtlarıyla anlatır. Veri kaynağı canlı PostgreSQL okumasıdır; kullanılan sorgular sadece `SELECT` sorgusudur.

Kapsam:

```text
Filtre: updated_at >= now() - interval '30 days'
Diagnostic fiili kayıt aralığı: 2026-04-24 16:10:57 UTC - 2026-04-26 23:58:56 UTC
Diagnostic trade sayısı: 887
Auto-scope analysis satırı: 1966
```

Bu sayıların anlamı şudur: son 30 gün filtresi uygulanınca elde kalan diagnostic kayıtların güncel fiili aralığı 24-26 Nisan 2026'dır. Dolayısıyla aşağıdaki yorumlar "bu aralıkta bot nasıl davranmış?" sorusuna cevap verir.

## Kısa Sonuç

Son 30 gün filtresinde diagnostic özet:

| Metrik | Değer |
|---|---:|
| Trade sayısı | 887 |
| Toplam PnL | -215.72 USDC |
| Ortalama PnL | -0.2432 USDC |
| Realized PnL | -195.53 USDC |
| Open PnL | -20.19 USDC |

Ana yorum:

- Bot TP alabildiğinde iyi sonuç üretmiş: `take_profit_success` ve `clean_win` toplamı pozitif.
- Toplam sonucu bozan ana kütle SL ve açık pozisyon mark zararıdır.
- Bu yüzden "bot çalışıyor mu?" sorusu tek başına doğru soru değildir. Doğru soru şudur: giriş kalitesi, SL davranışı, açık pozisyon taşıma ve exit kalitesi birlikte ne üretmiş?

## Diagnosis Dağılımı

| Diagnosis | Trade | Toplam PnL | Ortalama PnL |
|---|---:|---:|---:|
| `unrealized_mark_loss` - Açık pozisyon mark zararı | 518 | -708.13 | -1.3670 |
| `bad_entry_price` - Kötü giriş fiyatı | 1 | -4.93 | -4.9300 |
| `unknown` - Başa baş | 13 | 0.00 | 0.0000 |
| `clean_win` - Temiz kar | 84 | +153.68 | +1.8295 |
| `take_profit_success` - TP başarılı | 271 | +343.66 | +1.2681 |

Kanıt:

- Kazanan trade sayısı az değil: 271 TP başarı + 84 clean win = 355 pozitif sınıf.
- Buna rağmen 518 açık pozisyon mark zararı toplam sonucu aşağı çekmiş.
- Tek bir "TP var, sistem iyi" veya "zarar var, sistem bozuk" cümlesi eksik kalır. Kayıtlar iki tarafı da gösteriyor.

## Exit Satırları Ne Diyor?

`trade_flow_auto_scope_analysis_rows` son 30 gün özeti:

| Row type | Exit reason | Satır | PnL | Ortalama |
|---|---|---:|---:|---:|
| `sell_exit` | `sl` | 524 | -698.63 | -1.3333 |
| `open_position` | `open_position` | 962 | -24.82 | -0.0258 |
| `sell_exit` | `other` | 24 | -5.00 | -0.2083 |
| `sell_exit` | `tp` | 456 | +456.85 | +1.0019 |

Bu tablo botun gerçek problem yerini açık gösterir:

1. TP satırları karlı.
2. SL satırları TP kazancından daha büyük zarar yazmış.
3. Açık pozisyonlar küçük ortalama zarar taşıyor ama sayıları yüksek.

Bu yüzden config tartışması önce SL ve açık pozisyon yönetiminden başlamalıdır. Entry guard'ları önemli ama toplam PnL'yi bu aralıkta esas bozan alan exit tarafıdır.

## Order Durumları

Son 30 gün `trade_builder_orders` durumları:

| Status | Side | Kind | Adet |
|---|---|---|---:|
| `expired` | buy | immediate | 1161 |
| `error` | buy | immediate | 733 |
| `completed` | buy | immediate | 556 |
| `canceled` | sell | conditional | 351 |
| `completed` | sell | conditional | 346 |
| `expired` | sell | conditional | 292 |
| `canceled` | sell | immediate | 130 |
| `canceled` | buy | immediate | 96 |
| `expired` | sell | immediate | 25 |
| `completed` | sell | immediate | 25 |

Yorum:

- Çok sayıda buy immediate order `expired` veya `error` durumuna düşmüş. Bu "bot hiç denemiyor" değil, order lifecycle içinde çok sayıda girişin terminal olmayan iyi fill'e dönemediği anlamına gelir.
- Sell conditional tarafında hem completed hem canceled var. TP/SL çocuk emirleri OCO, inventory, window ve sibling policy ile birlikte okunmalıdır.
- Buy completed sayısı tek başına başarı değildir; buy sonrası exit sonucu ayrı ölçülür.

## Event Kanıtı

Son 30 gün en yoğun eventler:

| Event | Adet | Anlam |
|---|---:|---|
| `guard_evaluated` | 32925 | Bot sürekli guard kararı üretmiş |
| `notification_sent` | 5911 | Operatöre karar/sonuç taşınmış |
| `exit_inventory_source_selected` | 4395 | Exit inventory kaynağı seçilmiş |
| `exit_price_capped` | 4035 | TP/exit fiyatı floor/cap ile sınırlandırılmış |
| `max_price_waiting` | 2603 | Pahalı girişler beklemeye alınmış |
| `flow_created` | 2546 | Flow order lifecycle üretmiş |
| `expired` | 1496 | Emirler zaman/fill koşulunda kapanmış |
| `submitted` | 1010 | CLOB submit denenmiş |
| `filled` | 938 | Fill oluşmuş |
| `execution_floor_waiting` | 716 | Çok düşük/kötü book yüzeyi beklemeye alınmış |
| `sl_sell_created` | 532 | SL çocuk satışları yaratılmış |
| `tp_sell_created` | 379 | TP çocuk satışları yaratılmış |
| `pair_lock_session_state_changed` | 202 | Pair lock session state değişmiş |
| `pair_lock_unwind_scheduled` | 181 | Pair/orphan unwind planı oluşmuş |

Bu event dağılımı botun çalışma şeklini ispatlar:

- Bot sadece fiyat görünce almıyor; önce guard çalıştırıyor.
- Max price ve execution floor beklemeleri canlı davranışın parçası.
- Fill sonrası TP/SL çocuk emirleri gerçekten kuruluyor.
- Pair lock sadece config satırı değil, session state ve unwind eventleriyle runtime'da çalışmış.

## Kötü Örnekler

En kötü diagnostic örnekleri:

| Root order | Market | Outcome | PnL | Diagnosis |
|---:|---|---|---:|---|
| 10778 | `btc-updown-5m-1774713600` | Down | -7.95 | Açık pozisyon mark zararı |
| 10685 | `sol-updown-5m-1774705500` | Up | -7.70 | Açık pozisyon mark zararı |
| 10722 | `btc-updown-5m-1774708200` | Down | -5.96 | Açık pozisyon mark zararı |
| 10870 | `btc-updown-5m-1774719900` | Up | -5.50 | Açık pozisyon mark zararı |
| 11648 | `sol-updown-5m-1774870200` | Up | -5.43 | Açık pozisyon mark zararı |
| 1068 | `sol-updown-5m-1772982900` | No | -4.93 | Kötü giriş fiyatı |

Bu örnekler şunu anlatır:

- Kayıpların çoğu "bot hiç çalışmadı" değil, pozisyon açık kalıp mark/canlı fiyatla zarar yazdığı durumlar.
- `bad_entry_price` az sayıda ama keskin bir örnek: fill fiyatı referansa göre pahalı kalmış.
- Açık pozisyon zararları, window end sell/time exit/SL/pair unwind davranışıyla birlikte incelenmelidir.

## İyi Örnekler

En iyi diagnostic örnekleri:

| Root order | Market | Outcome | PnL | Diagnosis |
|---:|---|---|---:|---|
| 10811 | `btc-updown-5m-1774716300` | Up | +6.11 | Temiz kar |
| 10759 | `btc-updown-5m-1774711800` | Up | +5.96 | TP başarılı |
| 10834 | `btc-updown-5m-1774717200` | Up | +4.74 | TP başarılı |
| 10853 | `btc-updown-5m-1774719000` | Down | +4.73 | Temiz kar |
| 14421 | `btc-updown-5m-1777112400` | Up | +4.70 | Temiz kar |
| 14509 | `btc-updown-5m-1777146300` | Up | +4.61 | Temiz kar |
| 14364 | `btc-updown-5m-1777082700` | Up | +4.61 | Temiz kar |
| 14594 | `btc-updown-5m-1777173900` | Down | +4.61 | Temiz kar |

Bu örnekler sistemin pozitif tarafını gösterir:

- TP çalıştığında ve pozisyon temiz kapandığında trade başına +4 ila +6 USDC arası iyi sonuçlar var.
- Hem Up hem Down tarafında temiz kar örneği var; problem tek yön seçimiyle sınırlı değil.
- Kârlı örnekler, giriş + exit birlikte düzgün olduğunda botun beklenen lifecycle'ı tamamladığını gösterir.

## Botun Çalışma Şeklini Eski İşlemlerden Okuma

### 1. Guard katmanı gerçekten aktif

`guard_evaluated=32925`, `max_price_waiting=2603`, `execution_floor_waiting=716`.

Bu, botun her sinyali doğrudan order'a çevirmediğini gösterir. Max price ve execution floor beklemeleri "fırsat kaçırma" gibi görünebilir, ama eski işlem kayıtları bu beklemelerin runtime'da çok sık gerçekleştiğini söylüyor.

Operatör yorumu:

- Trigger geçti diye order bekleme.
- Önce guard eventlerini oku.
- Guard bekliyorsa sorun CLOB submit değil, giriş kalitesi veya fiyat koşuludur.

### 2. Submit ve fill ayrı aşamalar

Son 30 günde `submitted=1010`, `filled=938`.

Bu ikisi yakın ama aynı değildir. Submit oluşup fill gelmeyen veya kısmi lifecycle'a giden durumlar hâlâ mümkündür.

Operatör yorumu:

- Submit yoksa guard/action tarafına bak.
- Submit var ama fill yoksa fiyat, liquidity, min size ve orderbook tarafına bak.
- Fill var ama PnL kötü ise exit ve window yönetimine bak.

### 3. TP tek başına sistemi kurtarmıyor

TP satırları +456.85 USDC üretmiş. Buna rağmen SL satırları -698.63 USDC yazmış.

Bu eski işlemlerden çıkan en net ders:

```text
TP başarılı olabilir ama SL zararı daha büyükse toplam strateji negatif kalır.
```

Bu yüzden sadece TP hit rate değil, `avg_win / avg_loss`, SL slippage ve açık pozisyon oranı birlikte izlenmelidir.

### 4. Açık pozisyon mark zararı ana sınıf

`unrealized_mark_loss` 518 trade ile -708.13 USDC.

Bu sınıf şunu söyler:

- Pozisyon kapalı realized zarar değil, açık/mark zararı olarak okunmuş.
- Window sonu, auto-sell, time exit, protective unwind ve pair lock çıkışları kontrol edilmeden "entry kötüydü" demek eksiktir.
- En kötü örneklerin çoğu açık pozisyon mark zararıdır.

### 5. Pair lock runtime'da iz bırakıyor

`pair_lock_session_state_changed=202`, `pair_lock_unwind_scheduled=181`, `pair_lock_counter_remaining_budget_rebased=1640`.

Bu kayıtlar pair lock'un sadece UI/config değil, runtime'da session, counter budget ve unwind olarak çalıştığını gösterir.

Operatör yorumu:

- Pair lock hatası ararken sadece "iki bacak açıldı mı?" diye bakma.
- Session state, counter budget rebased ve unwind eventlerini birlikte oku.
- Unwind schedule varsa bot orphan/bozuk lock riskini yönetmeye çalışmıştır.

## Config İçin Çıkan Dersler

1. SL tarafı yeniden incelenmeli.
   SL satırları TP kazancını fazlasıyla siliyor. `slTriggerPriceMode`, staged SL, PTB stop-loss ve SL sonrası re-entry birlikte değerlendirilmelidir.

2. Açık pozisyon taşıma azaltılmalı.
   `autoSellOnWindowEnd`, time exit, pair protective unwind ve open position diagnostics ana kontrol alanlarıdır.

3. TP çalışıyor ama oran yetmiyor.
   TP ayarı kaldırılacak alan değil; SL ve açık pozisyon zararıyla dengelenmesi gereken pozitif motor.

4. Guard beklemeleri korunmalı ama sebebi okunmalı.
   `max_price_waiting` ve `execution_floor_waiting` yüksek. Bu seçicilik bazen iyi, bazen fırsat kaçırma olabilir. No-order analytics ile hangi beklemenin doğru olduğu ayrılmalıdır.

5. Error/expired buy immediate sayısı takip edilmeli.
   Son 30 günde buy immediate tarafında `expired=1161`, `error=733`. Bu, lifecycle ve CLOB/fill koşullarının entry kalitesi kadar önemli olduğunu gösterir.

## Operatör İçin Kanıt Formatı

Bir işlem tartışılırken şu bilgi yeterli kanıt sayılır:

```text
root_builder_order_id:
market_slug:
outcome:
diagnosis:
total_pnl_usdc:
row_type / exit_reason:
submitted event var mı:
filled event var mı:
SL/TP child yaratıldı mı:
guard_waiting eventleri:
pair_lock eventleri:
```

Bu format kullanılmadan yapılan yorum eksik kalır. Eski işlemlerden çıkan ana gerçek şu: bot çalışıyor, guard ve exit lifecycle üretiyor; problem bu lifecycle'ın bazı rejimlerde SL ve açık pozisyon zararıyla toplam PnL'yi negatife çevirmesi.
