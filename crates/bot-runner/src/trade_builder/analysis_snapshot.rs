const AUTO_SCOPE_ANALYSIS_BACKFILL_LIMIT: i64 = 25;

static AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS: LazyLock<parking_lot::Mutex<HashSet<i64>>> =
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

#[derive(Debug, Clone)]
struct SelectedAutoScopeTriggerEvent {
    created_at: DateTime<Utc>,
    market_open_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoScopeAnalysisRefreshOutcome {
    Updated,
    Skipped,
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

async fn refresh_trade_builder_auto_scope_analysis_snapshot_for_root(
    repo: &PostgresRepository,
    root_builder_order_id: i64,
    mark_price_override: Option<f64>,
) -> Result<AutoScopeAnalysisRefreshOutcome> {
    let Some(root_order) = repo.get_trade_builder_order(root_builder_order_id).await? else {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    };

    if root_order.side != "buy"
        || root_order.parent_order_id.is_some()
        || root_order.origin_flow_run_id.is_none()
    {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    }

    let Some(run) = repo
        .get_trade_flow_run(root_order.origin_flow_run_id.unwrap_or_default())
        .await?
    else {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    };
    let Some(version) = repo.get_trade_flow_version(run.version_id).await? else {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    };
    let graph = parse_trade_flow_graph(&version)?;
    let Some(action_node_key) = root_order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    };
    if !trade_builder_analysis_has_upstream_auto_scope_trigger(&graph, action_node_key) {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
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
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
    };
    if buy_metrics.qty <= 0.0 {
        repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
            .await?;
        return Ok(AutoScopeAnalysisRefreshOutcome::Skipped);
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

    let cost_basis_per_share =
        (buy_metrics.notional_usdc + buy_metrics.fee_usdc) / buy_metrics.qty.max(0.0000001);
    let mut cumulative_sold_qty = 0.0;
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
        cumulative_sold_qty = round_trade_builder_share_qty(cumulative_sold_qty + sell_qty);
        let remaining_qty_after_exit =
            round_trade_builder_share_qty((buy_metrics.qty - cumulative_sold_qty).max(0.0));
        let row_pnl_usdc =
            metrics.notional_usdc - metrics.fee_usdc - (sell_qty * cost_basis_per_share);
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
            row_qty: sell_qty,
            remaining_qty_after_exit,
            row_pnl_usdc: round_trade_builder_signed_qty(row_pnl_usdc),
        });
    }

    let remaining_qty =
        round_trade_builder_share_qty((buy_metrics.qty - cumulative_sold_qty).max(0.0));
    if remaining_qty > 0.0 {
        let (mark_price, mark_price_captured_at) = trade_builder_analysis_mark_price(
            &root_order,
            mark_price_override,
            buy_avg_price,
        );
        let row_pnl_usdc = remaining_qty * (mark_price - cost_basis_per_share);
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
            row_pnl_usdc: round_trade_builder_signed_qty(row_pnl_usdc),
        });
    }

    repo.delete_trade_flow_auto_scope_analysis_rows_for_root(root_builder_order_id)
        .await?;
    repo.upsert_trade_flow_auto_scope_analysis_rows(&rows).await?;
    Ok(AutoScopeAnalysisRefreshOutcome::Updated)
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

        let result =
            refresh_trade_builder_auto_scope_analysis_snapshot_for_root(repo, root_order_id, None)
                .await;
        match result {
            Ok(AutoScopeAnalysisRefreshOutcome::Updated)
            | Ok(AutoScopeAnalysisRefreshOutcome::Skipped) => {
                AUTO_SCOPE_ANALYSIS_BACKFILL_CHECKED_ROOTS
                    .lock()
                    .insert(root_order_id);
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
}
