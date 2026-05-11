# 01 - Market Döngüsü ve Auto-Scope

Bu dosya 5 dakikalık Up/Down marketlerin bot içinde nasıl seçildiğini, market değişimlerinde state'in nasıl taşındığını ve auto-scope kullanırken hangi edge case'lere bakılacağını anlatır.

## Amaç

5 dakikalık marketlerde her pencere kısa yaşar. Botun doğru market slug'ını seçmesi, eski markete order göndermemesi ve pencere sonundaki pozisyonu doğru yönetmesi gerekir.

Bu özellik grubu şu soruları cevaplar:

- Şu anda hangi BTC/ETH/SOL 5m marketindeyiz?
- Auto-scope yeni pencereye geçti mi?
- WebSocket yeni marketi REST/Gamma'dan önce gördü mü?
- Eski marketten kalan context yeni order'ı bozuyor mu?
- Pair lock açıkken counter leg eski markette mi kalmalı, yeni markete mi geçmeli?

## Temel Kavramlar

| Kavram | Anlam |
|---|---|
| Market window | 5 dakikalık Up/Down çözüm penceresi |
| Scope | `btc_5m_updown`, `eth_5m_updown`, `sol_5m_updown` gibi otomatik market ailesi |
| Slug | Polymarket market kimliği; zaman damgası veya dönem parçası içerir |
| Auto-scope | Botun scope'tan canlı market slug'ını üretmesi/seçmesi |
| Boundary | 5 dakikalık pencere değişim anı |
| Stale market | Artık aktif olmayan ama context'te kalmış market |

## Normal Akış

```text
1. Bot scope'u okur.
2. Geçerli zaman için candidate slug'lar üretir.
3. Mevcut marketi REST/Gamma/CLOB/WS sinyaliyle doğrular.
4. YES/NO token ID'leri context'e yazar.
5. Trigger veya pair binding downstream node'a geçer.
6. Boundary yaklaştığında yeni pencere için state temizliği planlanır.
```

Auto-scope kullanılan flow'larda `trigger.market_price` genellikle market slug'ını config'te sabit tutmaz. Bunun yerine `marketMode="auto_scope"` ve `marketScope` ile çalışır.

## Senaryo A: Normal Market Rotasyonu

Durum:

- `marketMode="auto_scope"`
- Scope `btc_5m_updown`
- Önceki market 12:00-12:05 penceresiydi
- Saat 12:05 oldu

Beklenen:

1. Bot yeni candidate slug üretir.
2. WS fast path veya polling yeni market token'larını görür.
3. Context'teki `marketSlug`, `tokenId`, `outcomeLabel` güncellenir.
4. `action.place_order` eski slug'a order göndermez.

Operatör kontrolü:

- Analytics'te market slug değişim zamanı.
- Trigger output'ta seçilen `market_slug`.
- Stale market block veya skip event'i var mı?

## Senaryo B: Gamma API Gecikiyor

Durum:

- Boundary geçildi.
- Gamma yeni marketi geç yayınladı.
- WS tarafı yeni market fiyatını daha erken gördü.

Beklenen:

- Bot WS fast path ile yeni slug/token bilgisini yakalayabiliyorsa geçişi hızlandırır.
- WS de yeni marketi görmüyorsa polling tekrar dener.
- Eski market context'iyle buy order üretmek riskli olduğundan stale market kontrolü devreye girer.

Operatör notu:

- Boundary sonrası birkaç saniye no-order normal olabilir.
- Sürekli tekrar ediyorsa scope slug üretimi, Gamma gecikmesi ve WS staleness birlikte incelenmelidir.

## Senaryo C: Cycle Window ile Alt Pencere

`cycleWindowMode` açık olduğunda bot 5 dakikalık marketin tamamında değil, belirli alt pencerelerde tetik arar.

Örnek:

```json
{
  "marketMode": "auto_scope",
  "marketScope": "eth_5m_updown",
  "cycleWindowMode": "on",
  "eligibleAfterSec": 45,
  "eligibleBeforeSec": 240
}
```

Bu kurulumda:

- İlk 45 saniye giriş aranmaz.
- 45-240 saniye arası tetik değerlendirilebilir.
- Son 60 saniye yeni giriş için riskli kabul edilir.

`entryTimingProfiles` ile `cycleWindowMode` aynı anda tasarlanırken dikkatli olunmalıdır. Entry profile seçimi kalan süreye göre çalışır; cycle window ise botun hangi aralıkta eligible olduğunu belirler.

## Senaryo D: Window End Auto Sell

Durum:

- 5 dakikalık market bitmek üzere.
- Pozisyon hâlâ açık.
- `autoSellOnWindowEnd=true`.

Beklenen:

1. Bot pencere sonu yaklaşırken pozisyonu tespit eder.
2. Kalan pozisyon için sell order üretir.
3. Resolution'a pozisyon bırakma riski azaltılır.

Risk:

- Orderbook derinliği çok düşükse sell fill olmayabilir.
- Çok geç auto-sell agresif slippage yaratabilir.
- Çok erken auto-sell potansiyel TP'yi kaçırabilir.

## Pair Lock Etkileşimi

Pair lock flow'larında upstream trigger genellikle `bindingMode="pair_lock_only"` kullanır. Bu mod fiyat tetiklemez; sadece YES/NO tokenlarını context'e bağlar.

Önemli davranışlar:

- Counter leg aynı market window içinde olmalıdır.
- Market bittiyse counter leg açmaya çalışmak yerine session temizliği beklenir.
- Orphan grace süresi varsa tek bacak kısa süre korunabilir; süre dolunca unwind veya skip kararı gerekir.

## Sık Hatalar

| Belirti | Muhtemel neden | Bakılacak yer |
|---|---|---|
| Order eski markete gidiyor | Context stale kaldı | `marketSlug`, stale market event'i |
| Yeni market birkaç saniye yok | Gamma/WS gecikmesi | market rotation event, WS staleness |
| Pair lock counter açmıyor | Counter market bitti veya token çözülemedi | pair session telemetry |
| Window end sell çalışmıyor | `autoSellOnWindowEnd` kapalı veya pozisyon yok | order lifecycle, position snapshot |

## Operatör Checklist

- Scope doğru mu: `btc_5m_updown`, `eth_5m_updown`, `sol_5m_updown`?
- Trigger output'taki `marketSlug` aktif pencereyle uyumlu mu?
- Boundary sonrası state temizliği olmuş mu?
- Auto-scope flow'unda sabit `marketSlug` eski markete kilitlemiyor mu?
- Pair lock kullanılıyorsa upstream trigger `bindingMode="pair_lock_only"` mi?
- Pencere sonu pozisyon taşıma riskine karşı `autoSellOnWindowEnd` ve exit kuralları açık mı?

## Detaylı Zaman Çizelgesi

Örnek 5 dakikalık BTC marketi:

| Zaman | Kalan süre | Botun beklenen davranışı |
|---|---:|---|
| 12:00:00 | 300 sn | Yeni window başlar, auto-scope candidate slug üretir |
| 12:00:05 | 295 sn | Gamma/WS tokenları doğrulanır, context güncellenir |
| 12:00:30 | 270 sn | Erken entry profile aktif olabilir |
| 12:02:30 | 150 sn | Orta bölümde PTB/IV edge daha anlamlı hale gelir |
| 12:04:00 | 60 sn | Geç entry profile veya time exit kararları öne çıkar |
| 12:04:30 | 30 sn | Yeni buy genellikle risklidir; window end sell hazırlanabilir |
| 12:05:00 | 0 sn | Market kapanır, yeni window candidate aranmaya başlar |

Bu tablo kesin config değildir; market döngüsünü okumak için referans ritimdir. Bazı flow'lar ilk 30 saniyeyi tamamen pas geçerken bazı momentum flow'ları sadece son 90 saniyeyi izleyebilir.

## Auto-Scope Seçimi Nasıl Düşünülmeli?

Auto-scope üç ayrı işi birleştirir:

1. Geçerli zamanı market window'una hizalamak.
2. Bu window için olası slug'ları üretmek.
3. Üretilen slug'ın gerçekten Polymarket tarafında var olduğunu doğrulamak.

Bu üç adımın herhangi biri gecikirse trigger geç çalışır. Özellikle boundary anlarında "bot marketi kaçırdı" demeden önce marketin gerçekten publish edilip edilmediği kontrol edilmelidir.

## Sayısal Örnek: Stale Market Engeli

Durum:

- Eski market: `btc-updown-5m-1800000000`.
- Yeni market: `btc-updown-5m-1800000300`.
- Trigger eski marketten `pass=true` üretmiş.
- Action çalıştığında sistem zamanı yeni window içinde.

Beklenen davranış:

```text
action.place_order
  -> context marketSlug eski
  -> auto-scope current market yeni
  -> stale market detected
  -> buy order üretilmez
  -> context refresh veya retry beklenir
```

Bu koruma olmasaydı bot kapanmış veya kapanmak üzere olan markete buy gönderebilirdi. Bu yüzden stale market skip trade kaçırma gibi görünse de güvenlik davranışıdır.

## Yanlış Yorumlar

| Yanlış yorum | Daha doğru okuma |
|---|---|
| Boundary'de 3 saniye order yoksa bot bozuk | Yeni market henüz doğrulanmamış olabilir |
| Auto-scope varken `marketSlug` önemsiz | Output'taki slug canlı teşhis için hâlâ ana kanıttır |
| Pair lock her markette counter bulmalı | Counter leg aynı window ve uygun maliyette olmalı |
| Window end sell garanti çıkış sağlar | Sell order üretilir, ama fill orderbook'a bağlıdır |

## Canlı Debug Akışı

1. Sorun yaşanan timestamp'i market boundary'ye göre konumlandır.
2. `marketSlug` eski window mu yeni window mu kontrol et.
3. Trigger event içinde `remainingSec` veya window bilgisi varsa not al.
4. Action event içinde stale skip var mı bak.
5. Pair lock ise `pair_session_id` aynı market window'una mı bağlı kontrol et.
6. Window end sell bekleniyorsa açık pozisyon ve sell order status bilgisini birlikte oku.

Bu akış özellikle "bot bazen ilk saniyelerde almıyor" şikayetinde gereklidir. İlk saniyelerde veri geçişinin güvenli doğrulanması, hatalı hızlı girişten daha değerlidir.
