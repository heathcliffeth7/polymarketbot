# 5 Dakikalik Marketler Nasıl Çalışır

Polymarket updown marketlerinde bot, her 5 veya 15 dakikada bir yenilenen rolling pencerelerde işlem yapar. Bu döküman, sistem mimarisini uçtan uca açıklar.

---

## 1. Genel Bakış

Updown marketleri, belirli bir varlık (BTC, ETH, SOL, XRP) için "5 dakika içinde fiyat X'in üstünde mi / altında mı" sorusuna dayalı binary option marketleridir. Her 5/15 dakikalık pencere bağımsız bir market slug ile temsil edilir ve her pencere kapandığında otomatik olarak yeni markete geçiş yapılır.

**Temel akış:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Gamma API (market discovery)                                    │
│       ↓                                                          │
│  Auto-Scope Market Resolution (candidate slug üretimi → seçim)  │
│       ↓                                                          │
│  WS Fast-Path (gerçek zamanlı fiyat akışı + market rotasyonu)   │
│       ↓                                                          │
│  Cycle Window Timer (alt-pencere tetikleme)                     │
│       ↓                                                          │
│  Trade Flow Execution (order placement + pair lock)              │
│       ↓                                                          │
│  Fill & Pair Lock Lifecycle (lock → unwind/complete)            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Market Slug Formatı

Her market, `{asset}-updown-{timeframe}-{unix_timestamp}` formatında bir slug ile tanımlanır.

**Örnekler:**

| Slug | Açıklama | Başlangıç | Bitiş |
|------|----------|-----------|-------|
| `btc-updown-5m-1776522900` | BTC 5dk market, 09:15 UTC başlangıçlı | 1776522900 | 1776523200 |
| `eth-updown-15m-1774052400` | ETH 15dk market | 1774052400 | 1774053300 |
| `sol-updown-5m-1776522600` | SOL 5dk market, 09:10 UTC başlangıçlı | 1776522600 | 1776522900 |

**Timestamp alignment kuralı:**

- **5m** marketler: timestamp her zaman 300'ün katı (`ts - (ts % 300)`)
- **15m** marketler: timestamp her zaman 900'ün katı (`ts - (ts % 900)`)

**Implementasyon:** `bot-core/src/market_cycle.rs`

```rust
pub fn parse_unix_start(&self) -> Option<i64> {
    self.0.rsplit('-').next()?.parse::<i64>().ok()
}

pub fn start_time(&self) -> Option<DateTime<Utc>> {
    let ts = self.parse_unix_start()?;
    Utc.timestamp_opt(ts, 0).single()
}
```

Slug'daki son `-` den sonraki kısım Unix timestamp olarak ayrıştırılır. `start_time()` bunu `DateTime<Utc>`'e çevirir.

---

## 3. Desteklenen Scope Tanımları

8 scope tanımı mevcuttur:

| Scope | Asset | Timeframe | Slug Prefix | Yalkışma |
|-------|-------|-----------|-------------|----------|
| `btc_5m_updown` | btc | 5m | `btc-updown-5m-` | 300s |
| `btc_15m_updown` | btc | 15m | `btc-updown-15m-` | 900s |
| `eth_5m_updown` | eth | 5m | `eth-updown-5m-` | 300s |
| `eth_15m_updown` | eth | 15m | `eth-updown-15m-` | 900s |
| `sol_5m_updown` | sol | 5m | `sol-updown-5m-` | 300s |
| `sol_15m_updown` | sol | 15m | `sol-updown-15m-` | 900s |
| `xrp_5m_updown` | xrp | 5m | `xrp-updown-5m-` | 300s |
| `xrp_15m_updown` | xrp | 15m | `xrp-updown-15m-` | 900s |

**Kaynak:** `bot-runner/src/lib_parts/part_000.rs`

```rust
#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdownScopeDef {
    scope: &'static str,
    asset: &'static str,
    timeframe: &'static str,
    slug_prefix: &'static str,
}
```

**Pencere süresi hesabı:** `updown_scope_window_seconds()`

```rust
fn updown_scope_window_seconds(scope_def: UpdownScopeDef) -> i64 {
    match scope_def.timeframe {
        "15m" => 900,
        _ => 300,  // 5m ve bilinmeyen timeframe'ler
    }
}
```

**Slug eşleştirme:** `find_updown_scope_by_slug()`

```rust
fn find_updown_scope_by_slug(slug: &str) -> Option<UpdownScopeDef> {
    let normalized = slug.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS.iter().copied()
        .find(|def| normalized.starts_with(def.slug_prefix))
}
```

Prefix match yapar. `btc-updown-5m-1776522900` → `btc_5m_updown` scope tanımını döner.

---

## 4. Auto-Scope Döngüsü

### 4.1 Market Mode: `auto_scope` vs `fixed`

`trigger.market_price` düğümünde iki mod vardır:

| Mod | Açıklama |
|-----|----------|
| `fixed` | `marketSlug` config'den sabit okunur. Pencere bitince manuel değişim gerekir. |
| `auto_scope` | Pencere each cycle'da otomatik olarak yeni aktif markete geçer. |

**Config örneği (auto_scope):**

```json
{
  "nodeType": "trigger.market_price",
  "marketMode": "auto_scope",
  "marketScope": "btc_5m_updown",
  "marketSelection": "latest_by_slug"
}
```

**Implementasyon:** `bot-runner/src/lib_parts/part_010.rs`

```rust
fn node_market_mode(node: &TradeFlowNode) -> &str {
    match node.config.get("marketMode")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("auto_scope") => "auto_scope",
        _ => "fixed",
    }
}
```

### 4.2 Candidate Slug Üretimi

Her pencere geçişinde bot, 4 aday slug üretir:

```rust
fn updown_scope_candidate_slugs(scope_def: UpdownScopeDef, now: DateTime<Utc>) -> Vec<String> {
    let window = updown_scope_window_seconds(scope_def);
    let now_ts = now.timestamp();
    let base = now_ts - now_ts.rem_euclid(window);
    [base - window, base, base + window, base + (2 * window)]
        .into_iter()
        .filter(|ts| *ts > 0)
        .map(|ts| format!("{}{}", scope_def.slug_prefix, ts))
        .collect()
}
```

**Örnek:** `now = 09:17:30 UTC` (1776523050), scope `btc_5m_updown`:
- `base = 1776522900` (09:15:00 — şu anki pencere başlangıcı)
- Adaylar: `1776522600` (09:10), `1776522900` (09:15), `1776523200` (09:20), `1776523500` (09:25)

**Kaynak:** `bot-runner/src/lib_parts/part_006.rs`

### 4.3 Market Seçim Önceliği

`select_market_from_candidates()` fonksiyonu, Gamma API'den gelen marketler arasından seçim yapar:

1. **InWindow** — Şu an aktif olan (start ≤ now < end) market. En yüksek öncelik.
2. **NearestFuture** — Henüz başlamamış, en yakın gelecek market.
3. **LatestBySlugFallback** — Hiçbir candidate aktif değilse, en son slug'a göre fallback.

**Bitmiş market filtrelemesi:** `scope_candidate_window_markets()`

```rust
fn scope_candidate_window_markets(
    scope_def: UpdownScopeDef,
    markets: &[GammaMarket],
    now: DateTime<Utc>,
) -> Vec<GammaMarket> {
    let candidate_slugs: HashSet<String> = updown_scope_candidate_slugs(scope_def, now)
        .into_iter().collect();
    markets.iter().filter(|market| {
        if !candidate_slugs.contains(&market.slug) { return false; }
        let (_, ends_at) = infer_updown_market_window(market);
        ends_at.map(|e| e > now).unwrap_or(true)
    }).cloned().collect()
}
```

Gamma API gecikmesi nedeniyle bitmiş market dönebilir — bu filtre onları hariç tutar.

### 4.4 Auto-Scope Context State Keys

Market seçimi sonucunda şu state key'leri yazılır:

| Key | Açıklama |
|-----|----------|
| `auto_scope_market_slug` | Seçilen market slug |
| `auto_scope_market_scope` | Scope adı (ör. `btc_5m_updown`) |
| `auto_scope_market_asset` | Varlık (ör. `btc`) |
| `auto_scope_market_timeframe` | Zowluk (ör. `5m`) |
| `auto_scope_yes_token_id` | YES token ID |
| `auto_scope_no_token_id` | NO token ID |
| `auto_scope_resolved_token_id` | Seçilen outcome token ID |
| `auto_scope_resolved_outcome_label` | Seçilen outcome etiketi |
| `auto_scope_selection_reason` | Seçim nedeni (InWindow, NearestFuture, vb.) |

**Kaynak:** `bot-runner/src/lib_parts/part_020.rs`

---

## 5. WS Fast-Path Market Rotasyonu

### 5.1 Rotasyon Tespiti Akışı

```
┌──────────────────────────────────────────────────────────────┐
│ 1. WS fiyat güncellemesi gelir                                │
│    ↓                                                          │
│ 2. trade_flow_ws_fast_path_cache_requires_refresh_now()       │
│    → Her auto_scope node için staleness kontrolü              │
│    ↓                                                          │
│ 3. Eğer stale ise: build_trade_flow_ws_fast_path_cache()      │
│    → sync_trigger_market_auto_scope_context()                 │
│    → Gamma API'den yeni market seçimi                         │
│    ↓                                                          │
│ 4. sync_trade_flow_auto_scope_market_rollover_state()         │
│    → Eski slug vs yeni slug karşılaştırması                   │
│    → Fark varsa: tüm runtime state temizlenir                  │
│    → AutoScopeMarketRotation event kaydedilir                 │
└──────────────────────────────────────────────────────────────┘
```

### 5.2 Staleness (Eskilik) Kontrolü

**Kaynak:** `bot-runner/src/lib_parts/part_010.rs`

```rust
fn is_auto_scope_market_stale_for_current_window(
    scope_def: UpdownScopeDef,
    market_slug: &str,
    now: DateTime<Utc>,
) -> bool {
    let Some(current_market_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return false;
    };
    current_market_start < current_updown_scope_window_start(scope_def, now)
}
```

**Mantık:** Market slug'ındaki Unix timestamp, mevcut pencere başlangıcından daha eski ise market stale kabul edilir.

`current_updown_scope_window_start()` fonksiyonu:

```rust
fn current_updown_scope_window_start(scope_def: UpdownScopeDef, now: DateTime<Utc>) -> DateTime<Utc> {
    let window_secs = updown_scope_window_seconds(scope_def);
    let now_ts = now.timestamp();
    let base_ts = now_ts - now_ts.rem_euclid(window_secs);
    DateTime::<Utc>::from_timestamp(base_ts, 0).unwrap_or(now)
}
```

**Örnek:** `now = 09:20:05 UTC` (1776523205), `btc_5m_updown`:
- `base = 1776523200` (09:20:00)
- Eski slug: `btc-updown-5m-1776522900` (09:15:00) → `1776522900 < 1776523200` → **STALE**

### 5.3 Market Rotasyonu State Temizleme

Rotasyon tespit edildiğinde, `clear_trade_flow_market_price_ws_runtime_state()` çağrılır ve şu state'ler temizlenir:

- `last_price`, `previous_price` (fiyat geçmişi)
- Cross-pending confirmation state
- Cycle window boundary markers
- Cycle window last-eval state
- `once_fired`, `once_fired_at` (trigger geçmişi)

**Ayrıca:** Yeni market için Chainlink price-to-beat seed değeri çekilir ve ilgili cache'e yazılır.

**Kaynak:** `bot-runner/src/trade_flow/ws_fast_path.rs:155-225`

### 5.4 Rotation Lag Ölçümü

Her rotasyon event'inde `rotation_lag_ms` hesaplanır:

```rust
expected_market_start: MarketCycleId(new_market_slug.to_string()).start_time(),
rotation_detected_at,  // Utc::now()
```

Bu, rotasyonun ne kadar gecikmeli tespit edildiğini gösterir. İdeal olarak market başlangıcı ile aynı anda (lag ≈ 0ms) tespit edilmelidir.

### 5.5 Boundary Timer Sistemi

`trade_flow_next_auto_scope_boundary_delay()` fonksiyonu, mevcut marketin bitişine kadar olan süreyi hesaplar:

```rust
fn auto_scope_market_boundary_delay(node_spec, now) -> Option<Duration> {
    let market_slug = node_spec.market_slug.as_deref()?;
    let scope_def = find_updown_scope_by_slug(market_slug)?;
    let window_secs = updown_scope_window_seconds(scope_def);
    let market_start = MarketCycleId(market_slug.to_string()).start_time()?;
    let market_end = market_start + ChronoDuration::seconds(window_secs);
    // now >= market_end → Duration::ZERO (hemen tetikle)
    // else → delay = market_end - now
}
```

Timer sıfırlağında → cache rebuild → staleness tespiti → yeni markete geçiş.

---

## 6. Cycle Window Timer'lar

### 6.1 Alt-Pencere (Sub-Window) Kavramı

Her 5m/15m market penceresi içinde daha dar alt-pencereler tanımlanabilir. Bu, tetikleyicinin ne zaman aktif olacağını sınırlar.

**Üç mod:**

| Mod | Açıklama | Formül |
|-----|----------|--------|
| `first` | Pencere başından itibaren N saniye | `[start, start + N)` |
| `last` | Pencere sonundan N saniye | `[end - N, end)` |
| `custom_range` | Pencere içinde belirli aralık | `[start + s, start + e)` |

**Kaynak:** `bot-runner/src/trade_flow/cycle_window_timers.rs`

```rust
fn cycle_window_bounds(node_spec: &WsOpenPositionPriceNodeSpec) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let start_at = MarketCycleId(market_slug.to_string()).start_time()?;
    let duration_secs = updown_scope_window_seconds(scope_def);
    let end_at = start_at + ChronoDuration::seconds(duration_secs);

    match cycle_window_mode {
        "first" => {
            let effective = node_spec.cycle_window_secs?.clamp(1, duration_secs);
            Some((start_at, start_at + ChronoDuration::seconds(effective)))
        }
        "last" => {
            let effective = node_spec.cycle_window_secs?.clamp(1, duration_secs);
            Some((end_at - ChronoDuration::seconds(effective), end_at))
        }
        "custom_range" => {
            let s = node_spec.cycle_window_start_sec?;
            let e = node_spec.cycle_window_end_sec?;
            if s >= e || e > duration_secs { return None; }
            Some((start_at + ChronoDuration::seconds(s), start_at + ChronoDuration::seconds(e)))
        }
        _ => None,
    }
}
```

### 6.2 Sub-Window Örnekleri

**Market:** `btc-updown-5m-1776522900` (09:15:00 - 09:20:00 UTC)

| Mod | Config | Aktif Pencere | Açıklama |
|-----|--------|---------------|----------|
| `first` | `cycleWindowSecs: 180` | 09:15:00 - 09:18:00 | İlk 3 dakika |
| `last` | `cycleWindowSecs: 120` | 09:18:00 - 09:20:00 | Son 2 dakika |
| `custom_range` | `startSec: 30, endSec: 180` | 09:15:30 - 09:18:00 | 30sn sonrasından 3. dakikaya kadar |
| `custom_range` | `startSec: 60, endSec: 240` | 09:16:00 - 09:19:00 | 1. dakika sonrasından 4. dakikaya kadar |

### 6.3 Boundary Marker Idempotency

Her alt-pencere tetiklemesi bir boundary marker ile idempotent hale getirilir:

```
first:btc-updown-5m-1776522900:180
custom_range:btc-updown-5m-1776522900:30:180
```

Bu marker node state'e yazılır ve aynı pencere için tekrar tetikleme önlenir.

**Kaynak:** `bot-runner/src/trade_flow/cycle_window_timers.rs:117-136`

### 6.4 eligible_after_at / eligible_before_at

Cycle window'dan üretilen buy order'lara eligibility zaman aralığı eklenir:

```rust
let (eligible_after_at, eligible_before_at) = if side == "buy" {
    match (cycle_window_open_at, cycle_window_end_at) {
        (Some(open_at), Some(end_at)) if open_at < end_at => (Some(open_at), Some(end_at)),
        _ => (None, None),
    }
} else {
    (None, None)
};
```

**Order processing'de kontrol:** `order_processing.rs`

```rust
if let Some(eligible_after_at) = order.eligible_after_at {
    if now < eligible_after_at { /* skip: too early */ }
}
if let Some(eligible_before_at) = order.eligible_before_at {
    if now >= eligible_before_at { /* skip: too late */ }
}
```

---

## 7. Pair Lock Etkileşimi

### 7.1 bindingMode: pair_lock_only Zorunluluğu

Pair lock modunda `action.place_order` düğümünün yukarı akışındaki `trigger.market_price` düğümü `bindingMode: "pair_lock_only"` olmak zorundadır:

```rust
fn resolve_pair_lock_direct_trigger_node_key(node_key: &str, graph: &TradeFlowGraphRuntime) -> Result<String> {
    // ...
    anyhow::ensure!(
        trigger_market_price_binding_mode(trigger_node) == "pair_lock_only",
        "action.place_order pair_lock requires upstream trigger.market_price bindingMode=pair_lock_only"
    );
}
```

**Kaynak:** `bot-runner/src/trade_builder/pair_lock.rs:215-241`

### 7.2 Counter Leg Market Bitiş Beklemesi

Auto-remaining-budget sizing modunda, counter leg (NO tarafı) market bitişine kadar bekleyebilir:

```rust
fn pair_lock_counter_waits_until_market_end(
    pair_lock: &ActionPlaceOrderPairLockConfig,
    market_slug: &str,
) -> bool {
    pair_lock.sizing_mode == ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget
        && pair_lock.orphan_grace_ms == 0
        && pair_lock_counter_market_end_eligible_before_at(market_slug).is_some()
}
```

**Şartlar:**
- `pairSizingMode: "auto_remaining_budget"` olmalı
- `pairOrphanGraceMs: 0` olmalı
- Market slug geçerli bir updown scope'a ait olmalı

Bu durumda counter order'ın `eligible_before_at` alanı market bitiş zamanına ayarlanır:

```rust
let counter_eligible_before_at = if counter_wait_until_market_end {
    pair_lock_counter_market_end_eligible_before_at(&order.market_slug)
} else {
    order.eligible_before_at
};
```

**`pair_lock_counter_market_end_eligible_before_at` hesabı:**

```rust
fn pair_lock_counter_market_end_eligible_before_at(market_slug: &str) -> Option<DateTime<Utc>> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let market_start = MarketCycleId(market_slug.to_string()).start_time()?;
    Some(market_start + ChronoDuration::seconds(updown_scope_window_seconds(scope)))
}
```

Yani counter leg, market penceresi kapanana kadar bekler ve son fiyattan dolum yapmaya çalışır.

### 7.3 pairMaxTotalCent ve pairTotalBudgetUsdc Etkileşimi

Pair lock yapılandırmasının temel parametreleri:

| Parametre | Açıklama | Örnek |
|-----------|----------|-------|
| `pairMaxTotalCent` | İki bacağın toplam maksimum fiyatı (cent) | 90 → max_total_price = $0.90 |
| `sizeUsdc` | Primary bacak miktarı (USDC) | 5.0 |
| `pairSizingMode` | Counter sizing modu | `"manual"` veya `"auto_remaining_budget"` |
| `counterLegSizeUsdc` | Manual modda counter bacak miktarı | 9.0 |
| `pairTotalBudgetUsdc` | Auto modda toplam bütçe | 14.0 |
| `pairOrphanGraceMs` | Counter dolmazsa bekleme süresi (ms) | 1500 (varsayılan) |

**Counter max fiyat hesabı:**

```
counter_max_price = pairMaxTotalCent / 100 - lead_fill_price
```

**Örnek: `pairMaxTotalCent = 90` (max_total_price = $0.90)**

| Senaryo | Primary (YES) fill | Counter (NO) max | Toplam maliyet | Guaranteed payout | Net kar/share |
|---------|-------------------|-------------------|----------------|-------------------|---------------|
| 70/20 | $0.70 | $0.20 (= 0.90 - 0.70) | $0.90 | $1.00 | $0.10 |
| 60/30 | $0.60 | $0.30 (= 0.90 - 0.60) | $0.90 | $1.00 | $0.10 |
| 50/40 | $0.50 | $0.40 (= 0.90 - 0.50) | $0.90 | $1.00 | $0.10 |

**Düşük `pairMaxTotalCent` → daha yüksek guaranteed kar**, ama counter leg'in dolması zorlaşır.

### 7.4 Auto-Remaining-Budget Rebalance

Primary leg dolduğunda, counter leg kalan bütçeyle rebalance edilir:

```rust
fn trade_builder_pair_lock_remaining_budget_usdc(
    total_budget_usdc: f64,
    session: &TradeBuilderPairSession,
) -> Option<f64> {
    let actual_primary_spend = session.primary_fill_qty? * session.primary_avg_fill_price?;
    let remaining_budget_usdc = total_budget_usdc - actual_primary_spend;
    (remaining_budget_usdc.is_finite() && remaining_budget_usdc > 0.0)
        .then_some(remaining_budget_usdc)
}
```

Kalan bütçeyle üç senaryo:

| Rebalance Modu | Koşul | Davranış |
|----------------|-------|----------|
| `lead_qty_match` | Bütçe yeterli | Primary ile aynı net qty kadar counter al |
| `partial_hedge` | Bütçe yetmez | Kalan bütçeyle ne kadar counter alınırsa |
| `full_budget_notional` | Lead henüz dolmamışsa | Kalan bütçenin tamamını USDC olarak counter'a ayır |

### 7.5 Pair Lock Yaşam Döngüsü

```
working → (her iki leg dolunca) → locked → (residue satış) → completed
         ↘ (orphan grace süresi dolunca, lead dolup counter dolmadı) → unwinding → completed
```

| Durum | Açıklama |
|-------|----------|
| `working` | Her iki order aktif, lead dolana kadar bekleme |
| `locked` | Her iki leg doldu, common qty kadar pozisyon kilitlendi |
| `unwinding` | Lead dolup counter dolmadı; orphan grace sonrası pozisyon kapatılıyor |
| `completed` | Tüm pozisyonlar kapatıldı |
| `error` | Hata durumu |

---

## 8. Bilinen Edge Case'ler ve Dikkat Noktaları

### 8.1 Market Geçişinde State Temizleme

Rotasyon anında **tüm** transient fiyat state'i temizlenir:
- `last_price`, `previous_price`, cross-pending, boundary markers, once-fired state
- Bu, eski marketin fiyat verisinin yeni markete sızmasını önler
- **Dikkat:** Eğer rotasyon gecikmeli olursa, eski marketin kapanış fiyatı yeni marketin açılış cross_above/cross_below tetiğini yanlış ateşleyebilir

### 8.2 `rem_euclid` Alignment Kritikliği

Timestamp alignment hesabında `rem_euclid` kullanılır (modulo genellikle `%` operatörü negative sayılarda farklı davranır):

```rust
let base_ts = now_ts - now_ts.rem_euclid(window_secs);
```

Negatif timestamp'ler (1 Ocak 1970 öncesi) için `rem_euclid` non-negative remainder verir. Bot yalnızca pozitif timestamp'lerle çalışır.

### 8.3 `-5m-` ve `-15m-` Slug Parse Edge Case'ler

`is_auto_scope_market_expired()` fonksiyonu slug içinde `-5m-` ve `-15m-` string araması yapar:

```rust
let duration = if slug.contains("-5m-") { 300 }
               else if slug.contains("-15m-") { 900 }
               else { return false };
```

**Dikkat:** `-15m-` içeren bir slug aynı zamanda `-5m-` de içerebilir ama kontrol sıralığı doğru: `-15m-` önce kontrol edilir. Ancak `find_updown_scope_by_slug()` prefix match yapar ve `btc-updown-15m-` prefix'i `btc-updown-5m-` ile karışmaz çünstdür — `15m` ve `5m` farklı prefix'lerdir.

### 8.4 Gamma API Gecikmesi

`scope_candidate_window_markets()` fonksiyonu, Gamma API'den dönen bitmiş marketleri filtreler:

```rust
let (_, ends_at) = infer_updown_market_window(market);
ends_at.map(|e| e > now).unwrap_or(true)
```

**Sorun:** Gamma API occasionally stale data dönebilir. Yeni açılan market henüz API'de görünmeyebilir. Bu durumda `NearestFuture` veya `LatestBySlugFallback` seçim modları devreye girer.

### 8.5 Orphan Grace ve Market Bitiş Etkileşimi

Pair lock modunda:
- `pairOrphanGraceMs: 0` + `auto_remaining_budget` → counter leg market sonuna kadar bekler
- `pairOrphanGraceMs: 1500` (varsayılan) → lead dolduğundan 1.5 sn sonra counter hala dolmamışsa unwind başlar
- `orphan_grace_ms > 0` → `pair_lock_counter_waits_until_market_end` **false** olur, yani market bitişi beklemez

**Tehlike:** 5m marketlerde `orphan_grace_ms = 0` ile pair lock kullanırsanız, counter leg market bitimine kadar bekleyebilir. 5 dakika çok kısa olduğu için geç orderbook sorunları yaşanabilir.

### 8.6 `from_unix_start` Hardcoded Prefix

```rust
pub fn from_unix_start(ts: i64) -> Self {
    Self(format!("btc-updown-5m-{ts}"))
}
```

Bu fonksiyon sadece test/default bağlamda kullanılır. Gerçek market slug üretimi `updown_scope_candidate_slugs()` ile yapılır.

### 8.7 Candidate Slug Üretim Sıralığı

Aday slug'lar `(base - window, base, base + window, base + 2*window)` sırasında üretilir. `select_market_from_candidates()` InWindow marketleri önceliklendirdiğinden, sıralama kritik değildir — ama Gamma API'de aynı anda iki InWindow market varsa ilk eşleşen seçilir.

### 8.8 Window End Auto-Sell

Eğer `auto_sell_on_window_end` aktifse, market penceresi kapandığında otomatik sell order oluşturulur:

```rust
fn cycle_window_end_sell_due_target(...) -> Option<DueWindowEndSellTarget> {
    if !node_spec.auto_sell_on_window_end { return None; }
    let (_, window_end_at) = cycle_window_bounds(node_spec)?;
    if now < window_end_at { return None; }
    // İdempotency ve market slug match kontrolü...
}
```

**Dikkat:** Auto-sell tetiği için `onceFired` state'inin set olması gerekir (yani once trigger en az bir kez ateşlemiş olmalı).

---

## 9. Akış Şemaları

### 9.1 Tam 5 Dakikalık Döngü Zaman Çizelgesi

```
09:14:55  WS cache boundary timer → yeni market slug keşfi başlar
09:15:00  Yeni market penceresi başlar (btc-updown-5m-1776522900)
09:15:00  Auto-scope: InWindow market seçimi
09:15:01  İlk fiyat tick'i gelir
09:15:01  Cycle window boundary (first:180s) kontrolü
09:15:02  Trigger tetiği ateşler → buy order oluşturulur
09:15:02  Order: eligible_after_at=09:15:00, eligible_before_at=09:18:00
...
09:17:30  Primary leg dolar ($0.70 fill fiyatı)
09:17:30  Counter leg oluşturulur (NO, max price=$0.20)
09:17:35  Counter leg dolar ($0.20 fill fiyatı)
09:17:35  Pair lock → LOCKED durumuna geçer
09:17:35  Residue unwind sell order oluşturulur
09:19:55  Boundary timer: market bitişine 5 sn kala refresh
09:20:00  Market penceresi kapanır (btc-updown-5m-1776523200 başlar)
09:20:00  Staleness tespiti → yeni markete rotasyon
09:20:00  Tüm runtime state temizlenir
09:20:01  Yeni market slug ile cache rebuild
```

### 9.2 Auto-Scope Rotasyon Karar Akışı

```
WS tick veya timer
    ↓
trade_flow_ws_fast_path_cache_requires_refresh_now()
    ↓ (stale?)
Evet → build_trade_flow_ws_fast_path_cache()
    → sync_trigger_market_auto_scope_context()
        → scope çözümleme (btc_5m_updown)
        → updown_scope_candidate_slugs() [4 aday]
        → Gamma API'den market listesi
        → scope_candidate_window_markets() [bitmişler filtrelenir]
        → select_market_from_candidates() [InWindow > NearestFuture > Fallback]
    → sync_trade_flow_auto_scope_market_rollover_state()
        → Eski slug vs yeni slug
        → Fark varsa: state temizleme + rotation event
Hayır → mevcut cache devam
```

---

## 10. Kaynak Kod Referansları

| Bileşen | Dosya | Satır |
|---------|-------|-------|
| UpdownScopeDef struct | `bot-runner/src/lib_parts/part_001.rs` | 138-144 |
| Scope tanımları | `bot-runner/src/lib_parts/part_000.rs` | 147-196 |
| find_updown_scope_by_slug | `bot-runner/src/lib_parts/part_006.rs` | 69-75 |
| updown_scope_window_seconds | `bot-runner/src/lib_parts/part_006.rs` | 77-82 |
| Candidate slug üretimi | `bot-runner/src/lib_parts/part_006.rs` | 84-93 |
| MarketCycleId | `bot-core/src/market_cycle.rs` | 1-33 |
| Staleness kontrolü | `bot-runner/src/lib_parts/part_010.rs` | 301-338 |
| is_auto_scope_market_expired | `bot-runner/src/lib_parts/part_010.rs` | 340-357 |
| Auto-scope context sync | `bot-runner/src/lib_parts/part_010.rs` | 1011-1085 |
| WS fast-path rotasyon | `bot-runner/src/trade_flow/ws_fast_path.rs` | 155-248 |
| Cycle window bounds | `bot-runner/src/trade_flow/cycle_window_timers.rs` | 147-182 |
| Boundary marker | `bot-runner/src/trade_flow/cycle_window_timers.rs` | 117-136 |
| Window end auto-sell | `bot-runner/src/trade_flow/cycle_window_timers.rs` | 62-105 |
| eligible_after/before_at | `bot-runner/src/lib_parts/part_018.rs` | 744-751 |
| Order eligibility kont. | `bot-infra/src/db/trade_builder/order_processing.rs` | 70-76 |
| Pair lock config | `bot-runner/src/trade_builder/pair_lock.rs` | 74-129 |
| Pair lock counter wait | `bot-runner/src/trade_builder/pair_lock_market.rs` | 28-41 |
| Pair lock budget | `bot-runner/src/trade_builder/pair_lock_budget.rs` | 1-109 |
| Pair lock rebalance | `bot-runner/src/trade_builder/pair_lock_market.rs` | 43-181 |
| Pair lock lifecycle | `bot-runner/src/trade_builder/pair_lock.rs` | 818-887 |
| Auto-remaining review | `bot-runner/src/trade_builder/pair_lock_market.rs` | 183-266 |
| Context state key'ler | `bot-runner/src/lib_parts/part_020.rs` | 113-121 |