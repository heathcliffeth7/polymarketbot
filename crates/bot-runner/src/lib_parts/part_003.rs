#[derive(Debug, Clone)]
struct TradeFlowNode {
    key: String,
    node_type: String,
    config: Value,
}

#[derive(Debug, Clone)]
struct TradeFlowEdge {
    source: String,
    target: String,
    edge_type: String,
    condition: Option<Value>,
}

#[derive(Debug, Clone)]
struct TradeFlowGraphRuntime {
    context: Value,
    nodes: Vec<TradeFlowNode>,
    edges: Vec<TradeFlowEdge>,
}

#[derive(Debug, Clone)]
struct TradeFlowRouteDecision {
    edge_type: String,
    available_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct TradeFlowNodeExecution {
    output: Value,
    routes: Vec<TradeFlowRouteDecision>,
    repeat_at: Option<DateTime<Utc>>,
    repeat_idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
struct TradeRuntime {
    trade_id: i64,
    user_id: Option<i64>,
    market_slug: String,
    entry_price: f64,
    tp_price: f64,
    position_size: f64,
    state: TradeState,
}

#[derive(Debug, Clone)]
struct DualLegRuntime {
    side: LegSide,
    token_id: String,
    qty: f64,
    avg_entry: f64,
    levels_filled: u32,
    last_fill_price: Option<f64>,
    last_dca_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct DualBasketRuntime {
    trade_id: i64,
    user_id: Option<i64>,
    market_slug: String,
    maker_base_fee: u64,
    state: TradeState,
    yes_leg: DualLegRuntime,
    no_leg: DualLegRuntime,
    cycle_ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct OrderMeta {
    leg_side: LegSide,
    side: String,
    intent: String,
}

#[derive(Debug, Clone)]
struct WsOpenPositionPriceNodeSpec {
    node_key: String,
    node_type: String,
    once_mode: bool,
    once_scope_market: bool,
    pair_lock_only_monitor: bool,
    auto_scope: bool,
    price_mode: WsPriceMode,
    market_slug: Option<String>,
    token_id: String,
    outcome_label: String,
    trigger_condition: String,
    trigger_price: f64,
    max_price: Option<f64>,
    price_to_beat_trigger_enabled: bool,
    price_to_beat_mode: crate::trade_flow::guards::price_to_beat::PriceToBeatMode,
    price_to_beat_trigger_min_gap: Option<f64>,
    price_to_beat_trigger_max_gap: Option<f64>,
    price_to_beat_trigger_unit: crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit,
    protection_mode: String,
    protection_asset: Option<String>,
    confirmation_ms: Option<i64>,
    cycle_window_mode: Option<String>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    auto_sell_on_window_end: bool,
}

#[derive(Debug, Clone)]
struct WsOpenPositionPriceRunSpec {
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    version_no: i32,
    context: Value,
    nodes: Vec<WsOpenPositionPriceNodeSpec>,
    context_dirty: bool,
}

#[derive(Debug, Clone, Default)]
struct TradeFlowWsFastPathCache {
    run_specs: Vec<WsOpenPositionPriceRunSpec>,
    token_targets: HashMap<String, Vec<(usize, usize)>>,
    market_targets: HashMap<String, Vec<(usize, usize)>>,
    live_gap_prewarm_targets: HashMap<String, Vec<LiveGapHistoryPrewarmTarget>>,
}

#[derive(Debug, Clone)]
struct UnderlyingProtectionConfig {
    mode: String,
    preset: String,
    asset: String,
    direction: String,
    reference_symbol: String,
}

#[derive(Debug, Clone)]
struct UnderlyingProtectionEvaluation {
    mode: String,
    preset: String,
    asset: String,
    direction: String,
    reference_feed: String,
    reference_symbol: String,
    passed: bool,
    reason_code: String,
    reason_detail: Option<String>,
    cycle_open_price: Option<f64>,
    current_price: Option<f64>,
    delta_10s_pct: Option<f64>,
    delta_30s_pct: Option<f64>,
    poly_delta_10s_cent: Option<f64>,
    divergence_blocked: bool,
}

#[derive(Debug, Clone)]
struct UnderlyingTick {
    price: f64,
    ts: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct UnderlyingReferenceState {
    ticks: VecDeque<UnderlyingTick>,
    cycle_open_by_ts: HashMap<i64, f64>,
    last_refresh_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct UnderlyingReferenceSnapshot {
    cycle_open_price: f64,
    current_price: f64,
    delta_10s_pct: Option<f64>,
    delta_30s_pct: Option<f64>,
}

#[derive(Debug)]
struct UnderlyingReferenceService {
    http: reqwest::Client,
    state: StdMutex<HashMap<String, UnderlyingReferenceState>>,
}

impl UnderlyingReferenceService {
    fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            state: StdMutex::new(HashMap::new()),
        }
    }

    async fn prime(&self, asset: &str) -> Result<()> {
        let _ = self.current_tick(asset).await?;
        Ok(())
    }

    async fn snapshot(
        &self,
        asset: &str,
        market_slug: &str,
    ) -> Result<UnderlyingReferenceSnapshot> {
        let reference_symbol = underlying_reference_symbol(asset)
            .ok_or_else(|| anyhow::anyhow!("unsupported underlying asset: {asset}"))?;
        let cycle_start = MarketCycleId(market_slug.to_string())
            .start_time()
            .ok_or_else(|| {
                anyhow::anyhow!("failed to parse cycle start from market slug: {market_slug}")
            })?;
        let current_tick = self.current_tick(asset).await?;
        let cycle_open_price = self
            .cycle_open_price(asset, reference_symbol, cycle_start)
            .await?;
        let (delta_10s_pct, delta_30s_pct) =
            self.compute_deltas(asset, current_tick.ts, current_tick.price);
        Ok(UnderlyingReferenceSnapshot {
            cycle_open_price,
            current_price: current_tick.price,
            delta_10s_pct,
            delta_30s_pct,
        })
    }

    async fn current_tick(&self, asset: &str) -> Result<UnderlyingTick> {
        if let Some(cached) = self.cached_recent_tick(asset) {
            return Ok(cached);
        }
        let reference_symbol = underlying_reference_symbol(asset)
            .ok_or_else(|| anyhow::anyhow!("unsupported underlying asset: {asset}"))?;
        let tick = self.fetch_current_tick(reference_symbol).await?;
        self.store_tick(asset, tick.clone());
        Ok(tick)
    }

    async fn cycle_open_price(
        &self,
        asset: &str,
        reference_symbol: &str,
        cycle_start: DateTime<Utc>,
    ) -> Result<f64> {
        let cycle_ts = cycle_start.timestamp();
        if let Some(price) = self.cached_cycle_open(asset, cycle_ts) {
            return Ok(price);
        }
        let price = self
            .fetch_cycle_open_price(reference_symbol, cycle_start)
            .await?;
        let mut state = self
            .state
            .lock()
            .expect("underlying reference state poisoned");
        let entry = state.entry(asset.to_string()).or_default();
        entry.cycle_open_by_ts.insert(cycle_ts, price);
        Ok(price)
    }

    fn cached_recent_tick(&self, asset: &str) -> Option<UnderlyingTick> {
        let state = self.state.lock().ok()?;
        let entry = state.get(asset)?;
        let last_refresh_at = entry.last_refresh_at?;
        if last_refresh_at.elapsed().as_secs() >= UNDERLYING_REFERENCE_MIN_REFRESH_SECS {
            return None;
        }
        entry.ticks.back().cloned()
    }

    fn cached_cycle_open(&self, asset: &str, cycle_ts: i64) -> Option<f64> {
        let state = self.state.lock().ok()?;
        state
            .get(asset)
            .and_then(|entry| entry.cycle_open_by_ts.get(&cycle_ts).copied())
    }

    fn store_tick(&self, asset: &str, tick: UnderlyingTick) {
        let mut state = self
            .state
            .lock()
            .expect("underlying reference state poisoned");
        let entry = state.entry(asset.to_string()).or_default();
        entry.last_refresh_at = Some(Instant::now());
        entry.ticks.push_back(tick.clone());
        let cutoff = tick.ts - ChronoDuration::seconds(UNDERLYING_REFERENCE_TICK_RETENTION_SECS);
        while entry
            .ticks
            .front()
            .map(|sample| sample.ts < cutoff)
            .unwrap_or(false)
        {
            entry.ticks.pop_front();
        }
        if entry.ticks.len() > 600 {
            let overflow = entry.ticks.len() - 600;
            entry.ticks.drain(0..overflow);
        }
    }

    fn compute_deltas(
        &self,
        asset: &str,
        current_ts: DateTime<Utc>,
        current_price: f64,
    ) -> (Option<f64>, Option<f64>) {
        let state = match self.state.lock() {
            Ok(value) => value,
            Err(_) => return (None, None),
        };
        let Some(entry) = state.get(asset) else {
            return (None, None);
        };
        let delta_10s_pct =
            underlying_delta_pct_from_ticks(&entry.ticks, current_ts, current_price, 10);
        let delta_30s_pct =
            underlying_delta_pct_from_ticks(&entry.ticks, current_ts, current_price, 30);
        (delta_10s_pct, delta_30s_pct)
    }

    async fn fetch_current_tick(&self, reference_symbol: &str) -> Result<UnderlyingTick> {
        let response = self
            .http
            .get(format!(
                "{UNDERLYING_REFERENCE_BASE_URL}/products/{reference_symbol}/ticker"
            ))
            .header(
                reqwest::header::USER_AGENT,
                "polymarketbot/underlying-protection",
            )
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;
        let payload: Value = response.json().await?;
        let price = payload.get("price").and_then(value_as_f64).ok_or_else(|| {
            anyhow::anyhow!("ticker response missing price for {reference_symbol}")
        })?;
        let ts = payload
            .get("time")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_utc)
            .unwrap_or_else(Utc::now);
        Ok(UnderlyingTick { price, ts })
    }

    async fn fetch_cycle_open_price(
        &self,
        reference_symbol: &str,
        cycle_start: DateTime<Utc>,
    ) -> Result<f64> {
        let start = cycle_start.to_rfc3339();
        let end = (cycle_start + ChronoDuration::minutes(1)).to_rfc3339();
        let response = self
            .http
            .get(format!(
                "{UNDERLYING_REFERENCE_BASE_URL}/products/{reference_symbol}/candles"
            ))
            .query(&[
                ("granularity", "60"),
                ("start", start.as_str()),
                ("end", end.as_str()),
            ])
            .header(
                reqwest::header::USER_AGENT,
                "polymarketbot/underlying-protection",
            )
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;
        let payload: Value = response.json().await?;
        let Some(rows) = payload.as_array() else {
            return Err(anyhow::anyhow!(
                "candles response was not an array for {reference_symbol}"
            ));
        };
        let cycle_ts = cycle_start.timestamp();
        let mut fallback_open = None;
        for row in rows {
            let Some(items) = row.as_array() else {
                continue;
            };
            let row_ts = items.first().and_then(value_as_i64);
            let row_open = items.get(3).and_then(value_as_f64);
            if fallback_open.is_none() {
                fallback_open = row_open;
            }
            if row_ts == Some(cycle_ts) {
                if let Some(open) = row_open {
                    return Ok(open);
                }
            }
        }
        fallback_open.ok_or_else(|| {
            anyhow::anyhow!("failed to resolve cycle-open candle for {reference_symbol}")
        })
    }
}

#[allow(dead_code)]
async fn fetch_underlying_reference_current_price(asset: &str) -> Result<f64> {
    Ok(UNDERLYING_REFERENCE_SERVICE
        .current_tick(asset)
        .await?
        .price)
}

impl UnderlyingProtectionEvaluation {
    fn to_value(&self) -> Value {
        json!({
            "mode": self.mode,
            "preset": self.preset,
            "asset": self.asset,
            "direction": self.direction,
            "reference_feed": self.reference_feed,
            "reference_symbol": self.reference_symbol,
            "passed": self.passed,
            "reason_code": self.reason_code,
            "reason_detail": self.reason_detail,
            "cycle_open_price": self.cycle_open_price,
            "current_price": self.current_price,
            "delta_10s_pct": self.delta_10s_pct,
            "delta_30s_pct": self.delta_30s_pct,
            "poly_delta_10s_cent": self.poly_delta_10s_cent,
            "divergence_blocked": self.divergence_blocked
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct TriggerPriceSample {
    ts_ms: i64,
    price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionDrawdownDirection {
    Down,
    Up,
}

impl PositionDrawdownDirection {
    fn parse(raw: Option<&str>) -> Option<Self> {
        let normalized = raw.map(str::trim).unwrap_or_default().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "down" {
            return Some(Self::Down);
        }
        if normalized == "up" {
            return Some(Self::Up);
        }
        None
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
        }
    }

    fn metric_type(self) -> &'static str {
        match self {
            Self::Down => "loss_pct",
            Self::Up => "gain_pct",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PositionDrawdownRule {
    index: usize,
    loss_pct: f64,
    direction: PositionDrawdownDirection,
    window_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct PositionDrawdownSample {
    ts_ms: i64,
    loss_pct: f64,
    gain_pct: f64,
    price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileOutcome {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileErrorKind {
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialSource {
    Inline,
    Env,
}

impl CredentialSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Env => "env",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClobErrorClassification {
    reason_code: &'static str,
    reason_message: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedLiveMarket {
    slug: String,
    yes_token_id: Option<String>,
    no_token_id: Option<String>,
    maker_base_fee: u64,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    selection_reason: LiveMarketSelectionReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TradeBuilderStaleRollingMarket {
    detected_scope: &'static str,
    detected_asset: &'static str,
    detected_timeframe: &'static str,
    current_live_market_slug: String,
    current_live_selection_reason: LiveMarketSelectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveMarketSelectionReason {
    InWindow,
    NearestFuture,
    LatestBySlugFallback,
    OverrideSlug,
}

impl LiveMarketSelectionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InWindow => "in_window",
            Self::NearestFuture => "nearest_future",
            Self::LatestBySlugFallback => "latest_by_slug_fallback",
            Self::OverrideSlug => "override_slug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketDiscoveryState {
    Ready,
    WaitingForMarket,
    Error,
}

impl MarketDiscoveryState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::WaitingForMarket => "waiting_for_market",
            Self::Error => "error",
        }
    }
}
