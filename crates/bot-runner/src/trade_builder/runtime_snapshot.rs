const TRADE_BUILDER_RUNTIME_SNAPSHOT_TTL_MS: i64 = 500;
const TRADE_BUILDER_FRESH_SUBMIT_LEASE_MS: i64 = 500;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TradeBuilderRuntimeSnapshotMarketSpec {
    neg_risk: bool,
    order_price_min_tick_size: Option<f64>,
    order_min_size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TradeBuilderRuntimeSnapshot {
    captured_at: DateTime<Utc>,
    source: String,
    current_price: Option<f64>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    trigger_reference_price: Option<f64>,
    guard_reference_price: Option<f64>,
    fee_rate_bps: Option<u64>,
    market_spec: Option<TradeBuilderRuntimeSnapshotMarketSpec>,
}

fn trade_builder_market_spec_to_runtime_snapshot(
    market_spec: Option<TradeBuilderMarketSpec>,
) -> Option<TradeBuilderRuntimeSnapshotMarketSpec> {
    market_spec.map(|spec| TradeBuilderRuntimeSnapshotMarketSpec {
        neg_risk: spec.neg_risk,
        order_price_min_tick_size: spec.order_price_min_tick_size,
        order_min_size: spec.order_min_size,
    })
}

fn trade_builder_market_spec_from_runtime_snapshot(
    snapshot: &TradeBuilderRuntimeSnapshot,
) -> Option<TradeBuilderMarketSpec> {
    snapshot.market_spec.as_ref().map(|spec| TradeBuilderMarketSpec {
        neg_risk: spec.neg_risk,
        order_price_min_tick_size: spec.order_price_min_tick_size,
        order_min_size: spec.order_min_size,
    })
}

fn trade_builder_runtime_snapshot_price_source(step: &TradeFlowRunStep) -> String {
    step_input_string(
        step,
        &[
            "wsPriceSource",
            "ws_price_source",
            "triggerSource",
            "trigger_source",
        ],
    )
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| "flow_step".to_string())
}

fn trade_builder_runtime_snapshot_to_json(
    snapshot: &TradeBuilderRuntimeSnapshot,
) -> Option<Value> {
    serde_json::to_value(snapshot).ok()
}

fn trade_builder_runtime_snapshot_from_order(
    order: &TradeBuilderOrder,
) -> Option<TradeBuilderRuntimeSnapshot> {
    let value = order.runtime_snapshot_json.as_ref()?.clone();
    serde_json::from_value(value).ok()
}

fn trade_builder_runtime_snapshot_age_ms(
    snapshot: &TradeBuilderRuntimeSnapshot,
    now: DateTime<Utc>,
) -> i64 {
    now.signed_duration_since(snapshot.captured_at)
        .num_milliseconds()
        .max(0)
}

fn trade_builder_runtime_snapshot_is_fresh(
    snapshot: &TradeBuilderRuntimeSnapshot,
    now: DateTime<Utc>,
) -> bool {
    trade_builder_runtime_snapshot_age_ms(snapshot, now) <= TRADE_BUILDER_RUNTIME_SNAPSHOT_TTL_MS
}

fn trade_builder_runtime_snapshot_lease_until(
    snapshot: &TradeBuilderRuntimeSnapshot,
) -> DateTime<Utc> {
    snapshot.captured_at + ChronoDuration::milliseconds(TRADE_BUILDER_FRESH_SUBMIT_LEASE_MS)
}

fn trade_builder_runtime_price_from_snapshot(
    snapshot: &TradeBuilderRuntimeSnapshot,
) -> Option<TradeBuilderRuntimePrice> {
    let price = snapshot
        .current_price
        .or(snapshot.best_bid)
        .or(snapshot.last_trade_price)
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)?;
    Some(TradeBuilderRuntimePrice {
        price,
        source: "runtime_snapshot",
        runtime_warning: None,
        best_bid: snapshot.best_bid.map(clamp_probability),
        best_ask: snapshot.best_ask.map(clamp_probability),
        last_trade_price: snapshot.last_trade_price.map(clamp_probability),
    })
}

async fn capture_trade_builder_runtime_snapshot(
    cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    step: &TradeFlowRunStep,
    market_slug: &str,
    token_id: &str,
    current_price: Option<f64>,
    trigger_reference_price: Option<f64>,
    guard_reference_price: Option<f64>,
) -> Option<TradeBuilderRuntimeSnapshot> {
    let best_bid = step_input_f64(step, &["wsBestBid", "ws_best_bid"]);
    let best_ask = step_input_f64(step, &["wsBestAsk", "ws_best_ask"]);
    let last_trade_price =
        step_input_f64(step, &["wsLastTradePrice", "ws_last_trade_price"]);
    if current_price.is_none()
        && best_bid.is_none()
        && best_ask.is_none()
        && last_trade_price.is_none()
    {
        return None;
    }

    let market_spec_fut = resolve_trade_builder_market_spec(cfg, market_slug, token_id);
    let fee_rate_fut = async {
        let Some(client) = client else {
            return None;
        };
        client.fee_rate_bps(token_id).await.ok().flatten()
    };
    let (market_spec, fee_rate_bps) = tokio::join!(market_spec_fut, fee_rate_fut);

    Some(TradeBuilderRuntimeSnapshot {
        captured_at: Utc::now(),
        source: trade_builder_runtime_snapshot_price_source(step),
        current_price,
        best_bid,
        best_ask,
        last_trade_price,
        trigger_reference_price,
        guard_reference_price,
        fee_rate_bps,
        market_spec: trade_builder_market_spec_to_runtime_snapshot(market_spec),
    })
}

async fn prepare_trade_builder_runtime_snapshot_state(
    cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    step: &TradeFlowRunStep,
    market_slug: &str,
    token_id: &str,
    should_inline_submit: bool,
    side: &str,
    reference_price: Option<f64>,
    best_ask_floor_price: Option<f64>,
) -> (
    Option<TradeBuilderRuntimeSnapshot>,
    Option<Value>,
    Option<DateTime<Utc>>,
    Option<u64>,
) {
    let runtime_snapshot = if should_inline_submit && side == "buy" {
        capture_trade_builder_runtime_snapshot(
            cfg,
            client,
            step,
            market_slug,
            token_id,
            reference_price,
            reference_price,
            reference_price.or(best_ask_floor_price),
        )
        .await
    } else {
        None
    };
    let runtime_snapshot_json = runtime_snapshot
        .as_ref()
        .and_then(trade_builder_runtime_snapshot_to_json);
    let fresh_submit_lease_until =
        runtime_snapshot.as_ref().map(trade_builder_runtime_snapshot_lease_until);
    let prefetched_fee_rate_bps = runtime_snapshot.as_ref().and_then(|value| value.fee_rate_bps);
    (
        runtime_snapshot,
        runtime_snapshot_json,
        fresh_submit_lease_until,
        prefetched_fee_rate_bps,
    )
}

async fn persist_trade_builder_runtime_snapshot_state(
    repo: &PostgresRepository,
    builder_order_id: i64,
    prefetched_fee_rate_bps: Option<u64>,
    runtime_snapshot_json: Option<&Value>,
    fresh_submit_lease_until: Option<DateTime<Utc>>,
) -> Result<()> {
    if let Some(fee_rate_bps) = prefetched_fee_rate_bps {
        repo.set_trade_builder_order_fee_rate_bps(builder_order_id, fee_rate_bps as i64)
            .await?;
    }
    repo.set_trade_builder_order_runtime_snapshot(
        builder_order_id,
        runtime_snapshot_json,
        fresh_submit_lease_until,
    )
    .await
}

fn append_trade_builder_runtime_snapshot_payload(
    payload: &mut serde_json::Map<String, Value>,
    runtime_snapshot: Option<&TradeBuilderRuntimeSnapshot>,
    fresh_submit_lease_until: Option<DateTime<Utc>>,
) {
    payload.insert(
        "runtime_snapshot".to_string(),
        runtime_snapshot
            .and_then(trade_builder_runtime_snapshot_to_json)
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "fresh_submit_lease_until".to_string(),
        json!(fresh_submit_lease_until.map(|value| value.to_rfc3339())),
    );
}
