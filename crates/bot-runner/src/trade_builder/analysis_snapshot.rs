const AUTO_SCOPE_ANALYSIS_BACKFILL_LIMIT: i64 = 25;
const AUTO_SCOPE_ANALYSIS_PNL_MODEL_VERSION: i64 = 8;
const AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_DELAYS_SECS: [u64; 4] = [1, 3, 8, 20];

static AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS: LazyLock<parking_lot::Mutex<HashSet<i64>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashSet::new()));
static AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_ROOTS: LazyLock<parking_lot::Mutex<HashSet<i64>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashSet::new()));

#[derive(Debug, Clone)]
struct AutoScopeAnalysisOrderMetrics {
    qty: f64,
    notional_usdc: f64,
    fee_usdc: f64,
    first_filled_at: Option<DateTime<Utc>>,
    last_filled_at: Option<DateTime<Utc>>,
    avg_price: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct AutoScopeAnalysisPnlBreakdown {
    buy_notional_usdc: f64,
    buy_fee_usdc: f64,
    cost_basis_usdc: f64,
    net_value_usdc: f64,
    row_pnl_usdc: f64,
    pnl_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct AutoScopeAnalysisSellAllocationSummary {
    observed_sell_qty: f64,
    allocated_sold_qty: f64,
    ignored_sell_qty: f64,
}

#[derive(Debug, Clone, Copy)]
struct AutoScopeAnalysisSellFillAllocation {
    allocated_qty: f64,
    ignored_qty: f64,
    allocation_ratio: f64,
    remaining_qty_after_exit: f64,
}

#[derive(Debug, Clone)]
struct SelectedAutoScopeTriggerEvent {
    created_at: DateTime<Utc>,
    market_open_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoScopeAnalysisRefreshOutcome {
    Updated,
    Skipped(&'static str),
}

impl AutoScopeAnalysisRefreshOutcome {
    fn event_type(self) -> &'static str {
        match self {
            Self::Updated => "AUTO_SCOPE_ANALYSIS_REFRESH_UPDATED",
            Self::Skipped(_) => "AUTO_SCOPE_ANALYSIS_REFRESH_SKIPPED",
        }
    }

    fn skip_reason(self) -> Option<&'static str> {
        match self {
            Self::Updated => None,
            Self::Skipped(reason) => Some(reason),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AutoScopeAnalysisRefreshContext {
    trigger: &'static str,
    retry_attempt_no: u8,
    max_retry_attempts: u8,
}

impl AutoScopeAnalysisRefreshContext {
    fn direct(trigger: &'static str) -> Self {
        Self {
            trigger,
            retry_attempt_no: 0,
            max_retry_attempts: AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_DELAYS_SECS.len() as u8,
        }
    }

    fn retry(trigger: &'static str, retry_attempt_no: u8) -> Self {
        Self {
            trigger,
            retry_attempt_no,
            max_retry_attempts: AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_DELAYS_SECS.len() as u8,
        }
    }
}

fn auto_scope_analysis_refresh_should_retry(reason: &str) -> bool {
    matches!(reason, "missing_buy_fill_metrics" | "zero_buy_fill_qty")
}

fn trade_builder_spawn_auto_scope_analysis_refresh_log(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_type: &'static str,
    context: AutoScopeAnalysisRefreshContext,
    mark_price_override: Option<f64>,
    skip_reason: Option<&str>,
) {
    trade_builder_spawn_decision_log(
        repo,
        order,
        event_type,
        json!({
            "refresh_trigger": context.trigger,
            "retry_attempt_no": context.retry_attempt_no,
            "max_retry_attempts": context.max_retry_attempts,
            "mark_price_override": mark_price_override,
            "skip_reason": skip_reason,
        }),
        TradeBuilderDecisionLogOptions {
            idempotency_key: Some(format!(
                "{event_type}:{}:{}:{}",
                order.id, context.trigger, context.retry_attempt_no
            )),
            ..TradeBuilderDecisionLogOptions::default()
        },
    );
}

async fn skip_trade_builder_auto_scope_analysis_snapshot(
    repo: &PostgresRepository,
    root_order: Option<&TradeBuilderOrder>,
    root_builder_order_id: i64,
    context: AutoScopeAnalysisRefreshContext,
    mark_price_override: Option<f64>,
    reason: &'static str,
) -> Result<AutoScopeAnalysisRefreshOutcome> {
    repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
        .await?;
    if let Some(order) = root_order {
        let outcome = AutoScopeAnalysisRefreshOutcome::Skipped(reason);
        trade_builder_spawn_auto_scope_analysis_refresh_log(
            repo,
            order,
            outcome.event_type(),
            context,
            mark_price_override,
            outcome.skip_reason(),
        );
    }
    Ok(AutoScopeAnalysisRefreshOutcome::Skipped(reason))
}

fn trade_builder_analysis_event_priority(event_type: &str) -> i32 {
    match event_type {
        "trigger_once_fired" => 0,
        "trigger_cycle_window_condition_met" => 1,
        "trigger_ws_price_enqueued" => 2,
        _ => 99,
    }
}

fn trade_builder_analysis_payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let value = payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(value) = value {
            return Some(value.to_string());
        }
    }
    None
}

fn trade_builder_analysis_payload_number(payload: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let Some(value) = payload.get(*key) else {
            continue;
        };
        let parsed = match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse::<f64>().ok(),
            _ => None,
        };
        if parsed.is_some() {
            return parsed;
        }
    }
    None
}

fn trade_builder_analysis_pnl_breakdown(
    row_qty: f64,
    buy_notional_per_share: f64,
    buy_fee_per_share: f64,
    net_value_usdc: f64,
) -> AutoScopeAnalysisPnlBreakdown {
    let buy_notional_usdc = row_qty * buy_notional_per_share;
    let buy_fee_usdc = row_qty * buy_fee_per_share;
    let cost_basis_usdc = buy_notional_usdc + buy_fee_usdc;
    let row_pnl_usdc = net_value_usdc - cost_basis_usdc;
    let pnl_pct = (cost_basis_usdc > 0.0)
        .then_some(round_trade_builder_signed_qty(
            (row_pnl_usdc / cost_basis_usdc) * 100.0,
        ));

    AutoScopeAnalysisPnlBreakdown {
        buy_notional_usdc: round_trade_builder_signed_qty(buy_notional_usdc),
        buy_fee_usdc: round_trade_builder_signed_qty(buy_fee_usdc),
        cost_basis_usdc: round_trade_builder_signed_qty(cost_basis_usdc),
        net_value_usdc: round_trade_builder_signed_qty(net_value_usdc),
        row_pnl_usdc: round_trade_builder_signed_qty(row_pnl_usdc),
        pnl_pct,
    }
}

fn trade_builder_analysis_allocate_sell_fill(
    buy_qty: f64,
    allocated_sold_qty: f64,
    raw_sell_qty: f64,
) -> AutoScopeAnalysisSellFillAllocation {
    let sell_qty = round_trade_builder_share_qty(raw_sell_qty.max(0.0));
    let remaining_qty_before_exit =
        round_trade_builder_share_qty((buy_qty - allocated_sold_qty).max(0.0));
    let allocated_qty = round_trade_builder_share_qty(sell_qty.min(remaining_qty_before_exit));
    let ignored_qty = round_trade_builder_share_qty((sell_qty - allocated_qty).max(0.0));
    let allocation_ratio = if sell_qty > 0.0 {
        allocated_qty / sell_qty
    } else {
        0.0
    };
    let remaining_qty_after_exit =
        round_trade_builder_share_qty((remaining_qty_before_exit - allocated_qty).max(0.0));

    AutoScopeAnalysisSellFillAllocation {
        allocated_qty,
        ignored_qty,
        allocation_ratio,
        remaining_qty_after_exit,
    }
}

fn trade_builder_analysis_payload_datetime(
    payload: &Value,
    keys: &[&str],
) -> Option<DateTime<Utc>> {
    for key in keys {
        let Some(value) = payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
            return Some(parsed.with_timezone(&Utc));
        }
    }
    None
}

fn trade_builder_analysis_market_open_at_from_slug(
    market_slug: &str,
) -> Option<DateTime<Utc>> {
    let suffix = market_slug.trim().rsplit('-').next()?;
    if suffix.len() < 9 || suffix.len() > 13 || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let raw_ts = suffix.parse::<i64>().ok()?;
    if raw_ts <= 0 {
        return None;
    }
    if raw_ts > 10_000_000_000 {
        return DateTime::<Utc>::from_timestamp_millis(raw_ts);
    }
    DateTime::<Utc>::from_timestamp(raw_ts, 0)
}

fn trade_builder_analysis_has_upstream_auto_scope_trigger(
    graph: &TradeFlowGraphRuntime,
    action_node_key: &str,
) -> bool {
    let mut incoming_by_target: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        incoming_by_target
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }

    let mut queue = VecDeque::from([action_node_key.to_string()]);
    if let Some(base_node_key) = action_node_key.strip_suffix("__counter") {
        queue.push_back(base_node_key.to_string());
    }
    let mut visited = HashSet::new();
    while let Some(current) = queue.pop_front() {
        if !visited.insert(current.clone()) {
            continue;
        }

        for source_key in incoming_by_target
            .get(current.as_str())
            .into_iter()
            .flat_map(|items| items.iter())
        {
            let Some(source_node) = flow_node(graph, source_key) else {
                continue;
            };
            if source_node.node_type == "trigger.market_price"
                && node_market_mode(source_node) == "auto_scope"
            {
                return true;
            }
            queue.push_back((*source_key).to_string());
        }
    }

    false
}

fn trade_builder_analysis_extract_exchange_order_ids(
    events: &[TradeBuilderOrderEventRecord],
) -> Vec<String> {
    let mut ids = HashSet::new();
    for event in events {
        for key_group in [
            ["exchange_order_id", "exchangeOrderId"].as_slice(),
            ["new_exchange_order_id", "newExchangeOrderId"].as_slice(),
            ["prev_exchange_order_id", "prevExchangeOrderId"].as_slice(),
        ] {
            if let Some(value) =
                trade_builder_analysis_payload_string(&event.payload_json, key_group)
            {
                ids.insert(value);
            }
        }
    }
    ids.into_iter().collect()
}

fn trade_builder_analysis_fallback_order_metrics(
    events: &[TradeBuilderOrderEventRecord],
) -> AutoScopeAnalysisOrderMetrics {
    let mut events = events.to_vec();
    events.sort_by_key(|event| event.created_at);

    let mut qty = 0.0;
    let mut notional_usdc = 0.0;
    let mut first_filled_at = None;
    let mut last_filled_at = None;

    for event in events {
        if event.event_type != "filled" {
            continue;
        }
        let fill_qty = trade_builder_analysis_payload_number(
            &event.payload_json,
            &["actual_fill_qty", "canonical_entry_qty"],
        );
        let execution_price =
            trade_builder_analysis_payload_number(&event.payload_json, &["execution_price"]);
        let Some(fill_qty) = fill_qty else { continue };
        let Some(execution_price) = execution_price else {
            continue;
        };
        if fill_qty <= 0.0 || execution_price <= 0.0 {
            continue;
        }
        qty += fill_qty;
        notional_usdc += fill_qty * execution_price;
        if first_filled_at.is_none() {
            first_filled_at = Some(event.created_at);
        }
        last_filled_at = Some(event.created_at);
    }

    AutoScopeAnalysisOrderMetrics {
        qty,
        notional_usdc,
        fee_usdc: 0.0,
        first_filled_at,
        last_filled_at,
        avg_price: (qty > 0.0).then_some(notional_usdc / qty),
    }
}

fn trade_builder_analysis_resolve_order_metrics(
    order_id: i64,
    events_by_order_id: &HashMap<i64, Vec<TradeBuilderOrderEventRecord>>,
    fill_summaries_by_exchange_id: &HashMap<String, TradeBuilderExchangeFillSummary>,
) -> AutoScopeAnalysisOrderMetrics {
    let events = events_by_order_id
        .get(&order_id)
        .cloned()
        .unwrap_or_default();
    let exchange_order_ids = trade_builder_analysis_extract_exchange_order_ids(&events);

    let mut qty = 0.0;
    let mut notional_usdc = 0.0;
    let mut fee_usdc = 0.0;
    let mut first_filled_at = None;
    let mut last_filled_at = None;

    for exchange_order_id in exchange_order_ids {
        let Some(summary) = fill_summaries_by_exchange_id.get(&exchange_order_id) else {
            continue;
        };
        if summary.filled_qty <= 0.0 {
            continue;
        }
        qty += summary.filled_qty;
        notional_usdc += summary.filled_notional_usdc;
        fee_usdc += summary.fee_usdc;

        if let Some(first) = summary.first_filled_at {
            if first_filled_at.map(|current| first < current).unwrap_or(true) {
                first_filled_at = Some(first);
            }
        }
        if let Some(last) = summary.last_filled_at {
            if last_filled_at.map(|current| last > current).unwrap_or(true) {
                last_filled_at = Some(last);
            }
        }
    }

    if qty > 0.0 {
        return AutoScopeAnalysisOrderMetrics {
            qty,
            notional_usdc,
            fee_usdc,
            first_filled_at,
            last_filled_at,
            avg_price: Some(notional_usdc / qty),
        };
    }

    trade_builder_analysis_fallback_order_metrics(&events)
}

fn trade_builder_analysis_select_trigger_event(
    events: &[TradeFlowEventRecord],
    market_slug: &str,
    token_id: &str,
    buy_created_at: DateTime<Utc>,
) -> Option<SelectedAutoScopeTriggerEvent> {
    let mut selected: Option<&TradeFlowEventRecord> = None;

    for event in events {
        if event.created_at > buy_created_at {
            continue;
        }

        let payload_market_slug = trade_builder_analysis_payload_string(
            &event.payload_json,
            &[
                "market_slug",
                "resolved_market_slug",
                "marketSlug",
                "resolvedMarketSlug",
            ],
        );
        let payload_token_id = trade_builder_analysis_payload_string(
            &event.payload_json,
            &["triggered_token_id", "token_id", "tokenId"],
        );

        if payload_market_slug.as_deref() != Some(market_slug)
            || payload_token_id.as_deref() != Some(token_id)
        {
            continue;
        }

        let replace = match selected {
            None => true,
            Some(current) if event.created_at > current.created_at => true,
            Some(current)
                if event.created_at == current.created_at
                    && trade_builder_analysis_event_priority(&event.event_type)
                        < trade_builder_analysis_event_priority(&current.event_type) =>
            {
                true
            }
            _ => false,
        };

        if replace {
            selected = Some(event);
        }
    }

    selected.map(|event| SelectedAutoScopeTriggerEvent {
        created_at: event.created_at,
        market_open_at: trade_builder_analysis_payload_datetime(
            &event.payload_json,
            &["window_open_at", "windowOpenAt"],
        )
        .or_else(|| trade_builder_analysis_market_open_at_from_slug(market_slug)),
    })
}

fn trade_builder_analysis_exit_reason(
    order: &TradeBuilderOrder,
    order_events: &[TradeBuilderOrderEventRecord],
) -> &'static str {
    let flow_created_payload = order_events
        .iter()
        .find(|event| event.event_type == "flow_created")
        .map(|event| &event.payload_json);
    let internal_mode = flow_created_payload.and_then(|payload| {
        trade_builder_analysis_payload_string(payload, &["internal_mode", "internalMode"])
    });

    if internal_mode.as_deref() == Some("window_end_auto_sell") {
        return "window_end_auto_sell";
    }
    if order.exit_ladder_kind.as_deref() == Some(TRADE_BUILDER_EXIT_LADDER_KIND_TP) {
        return "tp";
    }
    if matches!(
        order.exit_ladder_kind.as_deref(),
        Some(TRADE_BUILDER_EXIT_LADDER_KIND_SL | TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL)
    ) {
        return "sl";
    }
    match order.trigger_condition.as_deref() {
        Some("cross_above") => "tp",
        Some("cross_below") => "sl",
        _ => "other",
    }
}

fn trade_builder_analysis_mark_price(
    root_order: &TradeBuilderOrder,
    override_price: Option<f64>,
    fallback_buy_price: f64,
) -> (f64, DateTime<Utc>) {
    if let Some(price) = override_price
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
    {
        return (price, Utc::now());
    }

    if let Some(price) = root_order
        .last_seen_price
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
    {
        return (price, root_order.updated_at);
    }

    if let Some(snapshot) = trade_builder_runtime_snapshot_from_order(root_order) {
        if let Some(runtime_price) = trade_builder_runtime_price_from_snapshot(&snapshot) {
            return (runtime_price.price, snapshot.captured_at);
        }
    }

    (fallback_buy_price, root_order.updated_at)
}

async fn refresh_trade_builder_auto_scope_analysis_snapshot_for_root_with_context(
    repo: &PostgresRepository,
    root_builder_order_id: i64,
    mark_price_override: Option<f64>,
    context: AutoScopeAnalysisRefreshContext,
) -> Result<AutoScopeAnalysisRefreshOutcome> {
    let Some(root_order) = repo.get_trade_builder_order(root_builder_order_id).await? else {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            None,
            root_builder_order_id,
            context,
            mark_price_override,
            "root_order_not_found",
        )
        .await;
    };
    trade_builder_spawn_auto_scope_analysis_refresh_log(
        repo,
        &root_order,
        "AUTO_SCOPE_ANALYSIS_REFRESH_ATTEMPTED",
        context,
        mark_price_override,
        None,
    );

    if root_order.side != "buy"
        || root_order.parent_order_id.is_some()
        || root_order.origin_flow_run_id.is_none()
    {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "not_root_buy_order",
        )
        .await;
    }

    let Some(run) = repo
        .get_trade_flow_run(root_order.origin_flow_run_id.unwrap_or_default())
        .await?
    else {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "missing_flow_run",
        )
        .await;
    };
    let Some(version) = repo.get_trade_flow_version(run.version_id).await? else {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "missing_flow_version",
        )
        .await;
    };
    let graph = parse_trade_flow_graph(&version)?;
    let Some(action_node_key) = root_order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "missing_action_node_key",
        )
        .await;
    };
    if !trade_builder_analysis_has_upstream_auto_scope_trigger(&graph, action_node_key) {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "missing_auto_scope_trigger",
        )
        .await;
    }

    let child_orders = repo
        .list_trade_builder_child_orders_by_parent(root_builder_order_id, None)
        .await?;
    let mut tracked_order_ids = vec![root_builder_order_id];
    tracked_order_ids.extend(child_orders.iter().map(|order| order.id));

    let order_events = repo
        .list_trade_builder_order_events_for_orders(&tracked_order_ids)
        .await?;
    let mut events_by_order_id: HashMap<i64, Vec<TradeBuilderOrderEventRecord>> = HashMap::new();
    for event in order_events {
        events_by_order_id
            .entry(event.builder_order_id)
            .or_default()
            .push(event);
    }

    let exchange_order_ids = tracked_order_ids
        .iter()
        .flat_map(|order_id| {
            trade_builder_analysis_extract_exchange_order_ids(
                events_by_order_id.get(order_id).map(Vec::as_slice).unwrap_or(&[]),
            )
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let fill_summaries = repo
        .list_trade_builder_fill_summaries_by_exchange_order_ids(&exchange_order_ids)
        .await?;
    let fill_summaries_by_exchange_id = fill_summaries
        .into_iter()
        .map(|summary| (summary.exchange_order_id.clone(), summary))
        .collect::<HashMap<_, _>>();

    let buy_metrics = trade_builder_analysis_resolve_order_metrics(
        root_builder_order_id,
        &events_by_order_id,
        &fill_summaries_by_exchange_id,
    );
    let Some(buy_avg_price) = buy_metrics.avg_price else {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "missing_buy_fill_metrics",
        )
        .await;
    };
    if buy_metrics.qty <= 0.0 {
        return skip_trade_builder_auto_scope_analysis_snapshot(
            repo,
            Some(&root_order),
            root_builder_order_id,
            context,
            mark_price_override,
            "zero_buy_fill_qty",
        )
        .await;
    }

    let trigger_events = repo
        .list_trade_flow_events_for_run_types(
            run.id,
            &[
                "trigger_once_fired",
                "trigger_cycle_window_condition_met",
                "trigger_ws_price_enqueued",
            ],
        )
        .await?;
    let selected_trigger = trade_builder_analysis_select_trigger_event(
        &trigger_events,
        &root_order.market_slug,
        &root_order.token_id,
        root_order.created_at,
    );
    let market_open_at = selected_trigger
        .as_ref()
        .and_then(|value| value.market_open_at)
        .or_else(|| trade_builder_analysis_market_open_at_from_slug(&root_order.market_slug));
    let triggered_at = selected_trigger.as_ref().map(|value| value.created_at);
    let buy_filled_at = buy_metrics.first_filled_at;
    let open_to_trigger_ms = market_open_at
        .zip(triggered_at)
        .map(|(open_at, trigger_at)| {
            trigger_at
                .signed_duration_since(open_at)
                .num_milliseconds()
                .max(0)
        });
    let trigger_to_buy_fill_ms = triggered_at
        .zip(buy_filled_at)
        .map(|(trigger_at, buy_fill_at)| {
            buy_fill_at
                .signed_duration_since(trigger_at)
                .num_milliseconds()
                .max(0)
        });

    let buy_notional_per_share = buy_metrics.notional_usdc / buy_metrics.qty.max(0.0000001);
    let buy_fee_per_share = buy_metrics.fee_usdc / buy_metrics.qty.max(0.0000001);
    let mut sell_allocation_summary = AutoScopeAnalysisSellAllocationSummary::default();
    let mut cash_sell_notional_usdc = 0.0;
    let mut rows = Vec::new();

    let mut child_sell_entries = child_orders
        .into_iter()
        .filter(|order| order.side == "sell")
        .map(|order| {
            let metrics = trade_builder_analysis_resolve_order_metrics(
                order.id,
                &events_by_order_id,
                &fill_summaries_by_exchange_id,
            );
            (order, metrics)
        })
        .filter(|(_, metrics)| metrics.qty > 0.0 && metrics.avg_price.is_some())
        .collect::<Vec<_>>();
    child_sell_entries.sort_by(|(left_order, left_metrics), (right_order, right_metrics)| {
        let left_time = left_metrics
            .last_filled_at
            .unwrap_or(left_order.created_at);
        let right_time = right_metrics
            .last_filled_at
            .unwrap_or(right_order.created_at);
        left_time
            .cmp(&right_time)
            .then_with(|| left_order.id.cmp(&right_order.id))
    });

    for (child_order, metrics) in child_sell_entries {
        let sell_qty = round_trade_builder_share_qty(metrics.qty);
        cash_sell_notional_usdc += metrics.notional_usdc.max(0.0);
        let allocation = trade_builder_analysis_allocate_sell_fill(
            buy_metrics.qty,
            sell_allocation_summary.allocated_sold_qty,
            sell_qty,
        );
        sell_allocation_summary.observed_sell_qty =
            round_trade_builder_share_qty(sell_allocation_summary.observed_sell_qty + sell_qty);
        sell_allocation_summary.ignored_sell_qty = round_trade_builder_share_qty(
            sell_allocation_summary.ignored_sell_qty + allocation.ignored_qty,
        );
        if allocation.allocated_qty <= 0.0 {
            continue;
        }
        sell_allocation_summary.allocated_sold_qty = round_trade_builder_share_qty(
            sell_allocation_summary.allocated_sold_qty + allocation.allocated_qty,
        );
        let allocated_sell_notional_usdc = metrics.notional_usdc * allocation.allocation_ratio;
        let allocated_sell_fee_usdc = metrics.fee_usdc * allocation.allocation_ratio;
        let sell_notional_usdc = round_trade_builder_signed_qty(allocated_sell_notional_usdc);
        let sell_fee_usdc = round_trade_builder_signed_qty(allocated_sell_fee_usdc);
        let breakdown = trade_builder_analysis_pnl_breakdown(
            allocation.allocated_qty,
            buy_notional_per_share,
            buy_fee_per_share,
            allocated_sell_notional_usdc - allocated_sell_fee_usdc,
        );
        let order_events = events_by_order_id
            .get(&child_order.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        rows.push(TradeFlowAutoScopeAnalysisRowInput {
            row_key: format!("sell:{}", child_order.id),
            user_id: run.user_id,
            definition_id: run.definition_id,
            run_id: run.id,
            root_builder_order_id,
            exit_builder_order_id: Some(child_order.id),
            row_type: "sell_exit".to_string(),
            market_slug: root_order.market_slug.clone(),
            token_id: root_order.token_id.clone(),
            outcome_label: root_order.outcome_label.clone(),
            exit_reason: trade_builder_analysis_exit_reason(&child_order, order_events).to_string(),
            market_open_at: market_open_at.clone(),
            triggered_at: triggered_at.clone(),
            buy_filled_at: buy_filled_at.clone(),
            sell_filled_at: metrics.last_filled_at,
            open_to_trigger_ms,
            trigger_to_buy_fill_ms,
            buy_avg_price: Some(buy_avg_price),
            mark_or_sell_price: metrics.avg_price,
            mark_price_captured_at: metrics.last_filled_at,
            row_qty: allocation.allocated_qty,
            remaining_qty_after_exit: allocation.remaining_qty_after_exit,
            row_pnl_usdc: breakdown.row_pnl_usdc,
            buy_notional_usdc: Some(breakdown.buy_notional_usdc),
            buy_fee_usdc: Some(breakdown.buy_fee_usdc),
            cost_basis_usdc: Some(breakdown.cost_basis_usdc),
            sell_notional_usdc: Some(sell_notional_usdc),
            sell_fee_usdc: Some(sell_fee_usdc),
            mark_value_usdc: None,
            net_value_usdc: Some(breakdown.net_value_usdc),
            pnl_pct: breakdown.pnl_pct,
            valuation_kind: "realized".to_string(),
        });
    }

    let remaining_qty =
        round_trade_builder_share_qty((buy_metrics.qty - sell_allocation_summary.allocated_sold_qty).max(0.0));
    if remaining_qty > 0.0 {
        if trade_builder_analysis_market_has_ended(&root_order.market_slug) {
            rows.push(trade_builder_analysis_assumed_lost_settled_row(
                &root_order,
                &run,
                AutoScopeOfficialPnlTiming {
                    market_open_at,
                    triggered_at,
                    buy_filled_at,
                    open_to_trigger_ms,
                    trigger_to_buy_fill_ms,
                },
                remaining_qty,
                buy_avg_price,
                buy_notional_per_share,
                buy_fee_per_share,
            ));
        } else {
            let (mark_price, mark_price_captured_at) = trade_builder_analysis_mark_price(
                &root_order,
                mark_price_override,
                buy_avg_price,
            );
            let mark_value_usdc = remaining_qty * mark_price;
            let breakdown = trade_builder_analysis_pnl_breakdown(
                remaining_qty,
                buy_notional_per_share,
                buy_fee_per_share,
                mark_value_usdc,
            );
            rows.push(TradeFlowAutoScopeAnalysisRowInput {
                row_key: format!("open:{root_builder_order_id}"),
                user_id: run.user_id,
                definition_id: run.definition_id,
                run_id: run.id,
                root_builder_order_id,
                exit_builder_order_id: None,
                row_type: "open_position".to_string(),
                market_slug: root_order.market_slug.clone(),
                token_id: root_order.token_id.clone(),
                outcome_label: root_order.outcome_label.clone(),
                exit_reason: "open_position".to_string(),
                market_open_at,
                triggered_at,
                buy_filled_at,
                sell_filled_at: None,
                open_to_trigger_ms,
                trigger_to_buy_fill_ms,
                buy_avg_price: Some(buy_avg_price),
                mark_or_sell_price: Some(mark_price),
                mark_price_captured_at: Some(mark_price_captured_at),
                row_qty: remaining_qty,
                remaining_qty_after_exit: remaining_qty,
                row_pnl_usdc: breakdown.row_pnl_usdc,
                buy_notional_usdc: Some(breakdown.buy_notional_usdc),
                buy_fee_usdc: Some(breakdown.buy_fee_usdc),
                cost_basis_usdc: Some(breakdown.cost_basis_usdc),
                sell_notional_usdc: None,
                sell_fee_usdc: None,
                mark_value_usdc: Some(round_trade_builder_signed_qty(mark_value_usdc)),
                net_value_usdc: Some(breakdown.net_value_usdc),
                pnl_pct: breakdown.pnl_pct,
                valuation_kind: "mark_to_market".to_string(),
            });
        }
    }

    let internal_fallback_pnl_usdc =
        round_trade_builder_signed_qty(rows.iter().map(|row| row.row_pnl_usdc).sum());
    let official_pnl_rows = trade_builder_analysis_try_build_official_pnl_rows(
        repo,
        &root_order,
        &run,
        &rows,
        AutoScopeOfficialPnlTiming {
            market_open_at,
            triggered_at,
            buy_filled_at,
            open_to_trigger_ms,
            trigger_to_buy_fill_ms,
        },
        internal_fallback_pnl_usdc,
    )
    .await;
    rows = official_pnl_rows.rows;
    sell_allocation_summary = official_pnl_rows.sell_allocation_summary;
    let pnl_reconciliation = official_pnl_rows.reconciliation;

    let second_snapshots = repo
        .list_trade_builder_market_second_snapshots(&[root_order.market_slug.clone()])
        .await?;
    let diagnostic = trade_builder_analysis_build_trade_diagnostic(
        &root_order,
        &rows,
        &events_by_order_id,
        &second_snapshots,
        &buy_metrics,
        cash_sell_notional_usdc,
        &sell_allocation_summary,
        &pnl_reconciliation,
        open_to_trigger_ms,
        trigger_to_buy_fill_ms,
    );

    repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
        .await?;
    repo.upsert_trade_flow_auto_scope_analysis_rows(&rows).await?;
    repo.upsert_trade_flow_auto_scope_trade_diagnostic(&diagnostic)
        .await?;
    trade_builder_spawn_auto_scope_analysis_refresh_log(
        repo,
        &root_order,
        AutoScopeAnalysisRefreshOutcome::Updated.event_type(),
        context,
        mark_price_override,
        None,
    );
    Ok(AutoScopeAnalysisRefreshOutcome::Updated)
}

fn schedule_trade_builder_auto_scope_analysis_refresh_retry(
    repo: &PostgresRepository,
    root_builder_order_id: i64,
    mark_price_override: Option<f64>,
    initial_reason: &'static str,
) {
    if !auto_scope_analysis_refresh_should_retry(initial_reason) {
        return;
    }

    {
        let mut active_roots = AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_ROOTS.lock();
        if !active_roots.insert(root_builder_order_id) {
            return;
        }
    }

    let repo = repo.clone();
    tokio::spawn(async move {
        let mut last_reason = initial_reason;
        for (index, delay_s) in AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_DELAYS_SECS
            .iter()
            .copied()
            .enumerate()
        {
            sleep(Duration::from_secs(delay_s)).await;
            let retry_attempt_no = (index + 1) as u8;
            let result = refresh_trade_builder_auto_scope_analysis_snapshot_for_root_with_context(
                &repo,
                root_builder_order_id,
                mark_price_override,
                AutoScopeAnalysisRefreshContext::retry(
                    "retry_after_skip",
                    retry_attempt_no,
                ),
            )
            .await;

            match result {
                Ok(AutoScopeAnalysisRefreshOutcome::Updated) => break,
                Ok(AutoScopeAnalysisRefreshOutcome::Skipped(reason)) => {
                    last_reason = reason;
                    if !auto_scope_analysis_refresh_should_retry(reason) {
                        break;
                    }
                }
                Err(err) => {
                    warn!(
                        root_builder_order_id,
                        retry_attempt_no,
                        error = %err,
                        "AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_FAILED"
                    );
                }
            }
        }

        AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_ROOTS
            .lock()
            .remove(&root_builder_order_id);
        if auto_scope_analysis_refresh_should_retry(last_reason) {
            warn!(
                root_builder_order_id,
                skip_reason = last_reason,
                "AUTO_SCOPE_ANALYSIS_REFRESH_RETRY_EXHAUSTED"
            );
        }
    });
}

async fn refresh_trade_builder_auto_scope_analysis_snapshot_after_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    parent_order: Option<&TradeBuilderOrder>,
    execution_price: f64,
) {
    let root_builder_order_id = parent_order
        .map(|parent| parent.id)
        .or(order.parent_order_id)
        .unwrap_or(order.id);
    let mark_price_override = order.last_seen_price.or(Some(execution_price));
    let result = refresh_trade_builder_auto_scope_analysis_snapshot_for_root_with_context(
        repo,
        root_builder_order_id,
        mark_price_override,
        AutoScopeAnalysisRefreshContext::direct("order_fill"),
    )
    .await;

    match result {
        Ok(AutoScopeAnalysisRefreshOutcome::Updated) => {}
        Ok(AutoScopeAnalysisRefreshOutcome::Skipped(reason)) => {
            schedule_trade_builder_auto_scope_analysis_refresh_retry(
                repo,
                root_builder_order_id,
                mark_price_override,
                reason,
            );
        }
        Err(err) => {
            warn!(
                builder_order_id = order.id,
                root_builder_order_id,
                error = %err,
                "AUTO_SCOPE_ANALYSIS_REFRESH_FAILED"
            );
            schedule_trade_builder_auto_scope_analysis_refresh_retry(
                repo,
                root_builder_order_id,
                mark_price_override,
                "missing_buy_fill_metrics",
            );
        }
    }
}

async fn maybe_backfill_trade_builder_auto_scope_analysis_snapshots(
    repo: &PostgresRepository,
) -> Result<()> {
    let root_order_ids = repo
        .list_trade_builder_root_orders_missing_auto_scope_analysis(
            AUTO_SCOPE_ANALYSIS_BACKFILL_LIMIT,
        )
        .await?;

    for root_order_id in root_order_ids {
        {
            let checked = AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS.lock();
            if checked.contains(&root_order_id) {
                continue;
            }
        }

        let result = refresh_trade_builder_auto_scope_analysis_snapshot_for_root_with_context(
            repo,
            root_order_id,
            None,
            AutoScopeAnalysisRefreshContext::direct("backfill_cycle"),
        )
        .await;
        match result {
            Ok(AutoScopeAnalysisRefreshOutcome::Updated) => {
                AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS
                    .lock()
                    .insert(root_order_id);
            }
            Ok(AutoScopeAnalysisRefreshOutcome::Skipped(reason)) => {
                if auto_scope_analysis_refresh_should_retry(reason) {
                    schedule_trade_builder_auto_scope_analysis_refresh_retry(
                        repo,
                        root_order_id,
                        None,
                        reason,
                    );
                } else {
                    AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS
                        .lock()
                        .insert(root_order_id);
                }
            }
            Err(err) => {
                warn!(
                    root_builder_order_id = root_order_id,
                    error = %err,
                    "AUTO_SCOPE_ANALYSIS_BACKFILL_FAILED"
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod auto_scope_analysis_tests {
    use super::*;

    #[test]
    fn market_open_at_uses_slug_suffix_timestamp() {
        let parsed = trade_builder_analysis_market_open_at_from_slug("btc-updown-5m-1772296200")
            .expect("timestamp should parse");
        assert_eq!(parsed.to_rfc3339(), "2026-02-28T16:30:00+00:00");
    }

    #[test]
    fn upstream_auto_scope_trigger_detected_through_edges() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![
                TradeFlowNode {
                    key: "trigger_auto".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "marketMode": "auto_scope" }),
                },
                TradeFlowNode {
                    key: "logic".to_string(),
                    node_type: "logic.if".to_string(),
                    config: json!({}),
                },
                TradeFlowNode {
                    key: "action_buy".to_string(),
                    node_type: "action.place_order".to_string(),
                    config: json!({}),
                },
            ],
            edges: vec![
                TradeFlowEdge {
                    source: "trigger_auto".to_string(),
                    target: "logic".to_string(),
                    edge_type: "default".to_string(),
                    condition: None,
                },
                TradeFlowEdge {
                    source: "logic".to_string(),
                    target: "action_buy".to_string(),
                    edge_type: "default".to_string(),
                    condition: None,
                },
            ],
        };

        assert!(trade_builder_analysis_has_upstream_auto_scope_trigger(
            &graph,
            "action_buy"
        ));
    }

    #[test]
    fn upstream_auto_scope_trigger_accepts_pair_lock_counter_node_key() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![
                TradeFlowNode {
                    key: "trigger_auto".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "marketMode": "auto_scope" }),
                },
                TradeFlowNode {
                    key: "action_buy".to_string(),
                    node_type: "action.place_order".to_string(),
                    config: json!({}),
                },
            ],
            edges: vec![TradeFlowEdge {
                source: "trigger_auto".to_string(),
                target: "action_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            }],
        };

        assert!(trade_builder_analysis_has_upstream_auto_scope_trigger(
            &graph,
            "action_buy__counter"
        ));
    }

    #[test]
    fn pnl_breakdown_includes_buy_fee_in_cost_basis() {
        let breakdown = trade_builder_analysis_pnl_breakdown(10.0, 0.40, 0.01, 3.50);

        assert_eq!(breakdown.buy_notional_usdc, 4.0);
        assert_eq!(breakdown.buy_fee_usdc, 0.1);
        assert_eq!(breakdown.cost_basis_usdc, 4.1);
        assert_eq!(breakdown.net_value_usdc, 3.5);
        assert_eq!(breakdown.row_pnl_usdc, -0.6);
        assert_eq!(breakdown.pnl_pct, Some(-14.63));
    }

    #[test]
    fn pnl_breakdown_supports_mark_to_market_profit() {
        let breakdown = trade_builder_analysis_pnl_breakdown(5.0, 0.20, 0.0, 1.50);

        assert_eq!(breakdown.cost_basis_usdc, 1.0);
        assert_eq!(breakdown.net_value_usdc, 1.5);
        assert_eq!(breakdown.row_pnl_usdc, 0.5);
        assert_eq!(breakdown.pnl_pct, Some(50.0));
    }

    #[test]
    fn sell_allocation_keeps_normal_exit_full_size() {
        let allocation = trade_builder_analysis_allocate_sell_fill(10.0, 0.0, 5.0);

        assert_eq!(allocation.allocated_qty, 5.0);
        assert_eq!(allocation.ignored_qty, 0.0);
        assert_eq!(allocation.allocation_ratio, 1.0);
        assert_eq!(allocation.remaining_qty_after_exit, 5.0);
    }

    #[test]
    fn sell_allocation_caps_overlapping_exit_to_remaining_buy_qty() {
        let allocation = trade_builder_analysis_allocate_sell_fill(10.0, 7.0, 7.0);

        assert_eq!(allocation.allocated_qty, 3.0);
        assert_eq!(allocation.ignored_qty, 4.0);
        assert!((allocation.allocation_ratio - (3.0 / 7.0)).abs() < 0.000001);
        assert_eq!(allocation.remaining_qty_after_exit, 0.0);
    }

    #[test]
    fn sell_allocation_ignores_duplicate_exit_after_position_closed() {
        let allocation = trade_builder_analysis_allocate_sell_fill(10.0, 10.0, 2.0);

        assert_eq!(allocation.allocated_qty, 0.0);
        assert_eq!(allocation.ignored_qty, 2.0);
        assert_eq!(allocation.allocation_ratio, 0.0);
        assert_eq!(allocation.remaining_qty_after_exit, 0.0);
    }

    #[test]
    fn partial_sell_allocation_prorates_sell_notional_and_fee() {
        let allocation = trade_builder_analysis_allocate_sell_fill(10.0, 7.0, 7.0);
        let sell_notional_usdc = 7.0 * allocation.allocation_ratio;
        let sell_fee_usdc = 0.7 * allocation.allocation_ratio;
        let breakdown = trade_builder_analysis_pnl_breakdown(
            allocation.allocated_qty,
            0.40,
            0.01,
            sell_notional_usdc - sell_fee_usdc,
        );

        assert_eq!(round_trade_builder_signed_qty(sell_notional_usdc), 3.0);
        assert_eq!(round_trade_builder_signed_qty(sell_fee_usdc), 0.3);
        assert_eq!(breakdown.cost_basis_usdc, 1.23);
        assert_eq!(breakdown.net_value_usdc, 2.7);
        assert_eq!(breakdown.row_pnl_usdc, 1.47);
    }

    #[test]
    fn diagnostics_price_path_uses_outcome_bid_prices() {
        let start = DateTime::parse_from_rfc3339("2026-02-28T16:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let snapshots = vec![
            TradeBuilderMarketSecondSnapshot {
                market_slug: "btc-updown-5m-1772296200".to_string(),
                asset: "btc".to_string(),
                window_start: start,
                window_end: start + ChronoDuration::minutes(5),
                second_ts: start + ChronoDuration::seconds(1),
                ptb_ref_price: None,
                chainlink_price: None,
                yes_best_bid: Some(0.41),
                yes_best_ask: Some(0.42),
                yes_ask_depth_usdc: Some(100.0),
                no_best_bid: Some(0.58),
                no_best_ask: Some(0.59),
                no_ask_depth_usdc: Some(100.0),
                sample_count: 1,
            },
            TradeBuilderMarketSecondSnapshot {
                market_slug: "btc-updown-5m-1772296200".to_string(),
                asset: "btc".to_string(),
                window_start: start,
                window_end: start + ChronoDuration::minutes(5),
                second_ts: start + ChronoDuration::seconds(2),
                ptb_ref_price: None,
                chainlink_price: None,
                yes_best_bid: Some(0.37),
                yes_best_ask: Some(0.38),
                yes_ask_depth_usdc: Some(100.0),
                no_best_bid: Some(0.62),
                no_best_ask: Some(0.63),
                no_ask_depth_usdc: Some(100.0),
                sample_count: 1,
            },
        ];

        let path = trade_builder_analysis_price_path(
            &snapshots,
            "Up",
            Some(start),
            Some(start + ChronoDuration::seconds(3)),
        );

        assert_eq!(path.sample_count, 2);
        assert_eq!(path.best_price, Some(0.41));
        assert_eq!(path.worst_price, Some(0.37));
    }

    #[test]
    fn diagnostics_classifier_prioritizes_bad_entry_for_loss() {
        let evidence = AutoScopeDiagnosticEvidence {
            total_pnl_usdc: -1.0,
            fee_drag_usdc: 0.05,
            cost_basis_usdc: 10.0,
            entry_slippage_usdc: Some(0.25),
            exit_reason: Some("sl".to_string()),
            gave_back_usdc: Some(0.0),
            max_adverse_usdc: Some(-0.9),
            open_to_trigger_ms: Some(10_000),
            trigger_to_buy_fill_ms: Some(500),
            submit_to_fill_ms: Some(500),
            thin_liquidity_signal: false,
            is_open_position: false,
        };

        let (primary, secondary, label, _) =
            trade_builder_analysis_choose_diagnosis(&evidence);

        assert_eq!(primary, "bad_entry_price");
        assert_eq!(secondary, Some("market_reversal".to_string()));
        assert_eq!(label, "Kotu giris fiyati");
    }
}
