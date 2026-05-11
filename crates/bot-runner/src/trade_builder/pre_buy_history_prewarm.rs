#[derive(Debug, Clone, PartialEq)]
struct LiveGapHistoryPrewarmTarget {
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    node_key: String,
    trigger_node_key: String,
    market_slug: String,
    token_id: String,
    outcome_label: String,
    asset: String,
    direction: String,
    enabled: bool,
    prewarm_start_elapsed_sec: i64,
    trigger_window_start_sec: i64,
    action_window_start_sec: i64,
    action_window_end_sec: i64,
    sample_ms: i64,
    retention_ms: i64,
    trigger_condition: String,
    trigger_price: f64,
}

static LIVE_GAP_HISTORY_PREWARM_LAST_SAMPLE_MS: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn live_gap_history_prewarm_start_elapsed_sec(
    trigger_window_start_sec: Option<i64>,
    live_gap_window_start_sec: i64,
    prewarm_sec: i64,
    mode: &str,
) -> i64 {
    let base = if mode.trim().eq_ignore_ascii_case("before_trigger_window") {
        trigger_window_start_sec.unwrap_or(live_gap_window_start_sec)
    } else {
        live_gap_window_start_sec
    };
    (base - prewarm_sec.max(0)).max(0)
}

fn live_gap_history_prewarm_target_context(
    target: &LiveGapHistoryPrewarmTarget,
    now_ms: i64,
) -> PreBuyHistoryContext {
    PreBuyHistoryContext {
        run_id: target.run_id,
        definition_id: target.definition_id,
        version_id: target.version_id,
        node_key: target.node_key.clone(),
        trigger_node_key: Some(target.trigger_node_key.clone()),
        market_slug: target.market_slug.clone(),
        token_id: target.token_id.clone(),
        outcome_label: target.outcome_label.clone(),
        asset: target.asset.clone(),
        direction: target.direction.clone(),
        prewarm_enabled: target.enabled,
        prewarm_start_elapsed_sec: Some(target.prewarm_start_elapsed_sec),
        trigger_window_start_sec: Some(target.trigger_window_start_sec),
        action_window_start_sec: Some(target.action_window_start_sec),
        action_window_end_sec: Some(target.action_window_end_sec),
        sample_ms: target.sample_ms,
        retention_ms: target.retention_ms,
        trigger_condition: Some(target.trigger_condition.clone()),
        trigger_price: Some(target.trigger_price),
        triggered_price: None,
        trigger_source: Some("trigger.market_price".to_string()),
        registered_at_ms: now_ms,
    }
}

fn live_gap_history_register_prewarm_context(target: &LiveGapHistoryPrewarmTarget, now_ms: i64) {
    register_pre_buy_history_context(live_gap_history_prewarm_target_context(target, now_ms));
}

fn live_gap_history_prewarm_outcome_tokens(
    spec: &WsOpenPositionPriceNodeSpec,
    context: &Value,
    sides: &str,
) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    fn push_prewarm_outcome(
        targets: &mut Vec<(String, String)>,
        seen: &mut HashSet<(String, String)>,
        outcome_label: &str,
        token_id: Option<String>,
    ) {
        let token_id = token_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let Some(token_id) = token_id else {
            return;
        };
        if seen.insert((outcome_label.to_ascii_lowercase(), token_id.clone())) {
            targets.push((outcome_label.to_string(), token_id));
        }
    }

    match sides.trim().to_ascii_lowercase().as_str() {
        "both" => {
            push_prewarm_outcome(
                &mut targets,
                &mut seen,
                "Up",
                resolve_token_id_for_outcome_label_for_node(&spec.node_key, "Up", context)
                    .or_else(|| resolve_token_id_for_outcome_label("Up", context)),
            );
            push_prewarm_outcome(
                &mut targets,
                &mut seen,
                "Down",
                resolve_token_id_for_outcome_label_for_node(&spec.node_key, "Down", context)
                    .or_else(|| resolve_token_id_for_outcome_label("Down", context)),
            );
        }
        _ => {}
    }

    if targets.is_empty() || sides.trim().eq_ignore_ascii_case("triggered") {
        push_prewarm_outcome(
            &mut targets,
            &mut seen,
            &spec.outcome_label,
            Some(spec.token_id.clone()),
        );
    }
    targets
}

fn build_live_gap_history_prewarm_targets_for_trigger(
    run_spec: &WsOpenPositionPriceRunSpec,
    graph: &TradeFlowGraphRuntime,
    context: &Value,
    spec: &WsOpenPositionPriceNodeSpec,
) -> Vec<LiveGapHistoryPrewarmTarget> {
    if spec.node_type != "trigger.market_price" {
        return Vec::new();
    }
    let Some(market_slug) = spec
        .market_slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return Vec::new();
    };

    let mut targets = Vec::new();
    for edge in graph.edges.iter().filter(|edge| edge.source == spec.node_key) {
        let Some(action_node) = flow_node(graph, &edge.target) else {
            continue;
        };
        if action_node.node_type != "action.place_order" {
            continue;
        }
        let side = node_config_string(action_node, "side")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "buy".to_string());
        let Ok(Some(config)) = resolve_action_place_order_live_gap_collector_config(action_node, &side)
        else {
            continue;
        };
        let trigger_window_start_sec = spec
            .cycle_window_start_sec
            .unwrap_or(config.window_start_sec)
            .clamp(0, 900);
        let prewarm_start_elapsed_sec = live_gap_history_prewarm_start_elapsed_sec(
            spec.cycle_window_start_sec,
            config.window_start_sec,
            config.live_gap_history_prewarm_sec,
            &config.live_gap_history_prewarm_start_mode,
        );
        for (outcome_label, token_id) in live_gap_history_prewarm_outcome_tokens(
            spec,
            context,
            &config.live_gap_history_prewarm_sides,
        ) {
            let Some(direction) = live_gap_collector_direction(&outcome_label) else {
                continue;
            };
            targets.push(LiveGapHistoryPrewarmTarget {
                run_id: run_spec.run_id,
                definition_id: run_spec.definition_id,
                version_id: run_spec.version_id,
                node_key: action_node.key.clone(),
                trigger_node_key: spec.node_key.clone(),
                market_slug: market_slug.to_string(),
                token_id,
                outcome_label,
                asset: scope.asset.to_string(),
                direction: direction.to_string(),
                enabled: config.live_gap_history_prewarm_enabled,
                prewarm_start_elapsed_sec,
                trigger_window_start_sec,
                action_window_start_sec: config.window_start_sec,
                action_window_end_sec: config.window_end_sec,
                sample_ms: config.live_gap_history_sample_ms,
                retention_ms: config.live_gap_history_retention_ms,
                trigger_condition: spec.trigger_condition.clone(),
                trigger_price: spec.trigger_price,
            });
        }
    }
    targets
}

fn append_live_gap_history_prewarm_targets(
    targets_by_token: &mut HashMap<String, Vec<LiveGapHistoryPrewarmTarget>>,
    run_spec: &WsOpenPositionPriceRunSpec,
    graph: &TradeFlowGraphRuntime,
) {
    let now_ms = Utc::now().timestamp_millis();
    for spec in &run_spec.nodes {
        for target in build_live_gap_history_prewarm_targets_for_trigger(
            run_spec,
            graph,
            &run_spec.context,
            spec,
        ) {
            live_gap_history_register_prewarm_context(&target, now_ms);
            if target.enabled {
                targets_by_token
                    .entry(target.token_id.clone())
                    .or_default()
                    .push(target);
            }
        }
    }
}

fn live_gap_history_prewarm_should_sample(key: &str, now_ms: i64, sample_ms: i64) -> bool {
    let mut last_samples = LIVE_GAP_HISTORY_PREWARM_LAST_SAMPLE_MS
        .lock()
        .expect("live-gap history prewarm throttles");
    if let Some(last_ms) = last_samples.get(key) {
        if now_ms - *last_ms < sample_ms.max(1) {
            return false;
        }
    }
    last_samples.insert(key.to_string(), now_ms);
    if last_samples.len() > 2_048 {
        let cutoff_ms = now_ms - 120_000;
        last_samples.retain(|_, last_ms| *last_ms >= cutoff_ms);
    }
    true
}

fn maybe_record_live_gap_history_prewarm_sample(
    target: &LiveGapHistoryPrewarmTarget,
    snapshot: &MarketDataSnapshot,
    now: DateTime<Utc>,
) {
    let Some(elapsed_sec) = live_gap_collector_elapsed_sec(&target.market_slug, now) else {
        return;
    };
    if elapsed_sec < target.prewarm_start_elapsed_sec || elapsed_sec > target.action_window_end_sec {
        return;
    }
    let now_ms = now.timestamp_millis();
    let key = pre_buy_collapse_guard_key(
        &target.market_slug,
        &target.token_id,
        &target.outcome_label,
    );
    if !live_gap_history_prewarm_should_sample(&key, now_ms, target.sample_ms) {
        return;
    }
    let Some(best_ask) = snapshot.best_ask.filter(|value| value.is_finite() && *value > 0.0)
    else {
        return;
    };
    let Some(window_start) = MarketCycleId(target.market_slug.clone()).start_time() else {
        return;
    };
    let open_tick = match trade_flow::guards::chainlink_price::get_chainlink_price_start_tick(
        &target.asset,
        window_start.timestamp_millis(),
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => return,
    };
    let binance = match trade_flow::guards::binance_price::get_binance_price_snapshot(
        &target.asset,
        now_ms,
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => return,
    };
    let live_gap =
        live_gap_collector_directional_gap(&target.direction, open_tick.price, binance.price);
    live_gap_history_register_prewarm_context(target, now_ms);
    record_pre_buy_collapse_sample_with_retention(
        &target.market_slug,
        &target.token_id,
        &target.outcome_label,
        PreBuyCollapseSample {
            ts_ms: now_ms,
            live_gap,
            effective_fill: best_ask,
            best_ask,
            sample_source: "prewarm_ws_light",
        },
        target.retention_ms,
    );
}

fn build_live_gap_history_prewarm_callback() -> MarketTickCallback {
    Arc::new(move |token_id, snapshot| {
        let targets = if let Ok(cache) = TRADE_FLOW_WS_FAST_PATH_CACHE.try_read() {
            cache
                .live_gap_prewarm_targets
                .get(token_id)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if targets.is_empty() {
            return;
        }
        let now = Utc::now();
        for target in targets {
            maybe_record_live_gap_history_prewarm_sample(&target, snapshot, now);
        }
    })
}

#[cfg(test)]
mod live_gap_history_prewarm_tests {
    use super::*;

    fn spec() -> WsOpenPositionPriceNodeSpec {
        WsOpenPositionPriceNodeSpec {
            node_key: "trigger".to_string(),
            node_type: "trigger.market_price".to_string(),
            once_mode: false,
            once_scope_market: false,
            pair_lock_only_monitor: false,
            auto_scope: true,
            price_mode: WsPriceMode::BestAsk,
            market_slug: Some("btc-updown-5m-1777900500".to_string()),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            trigger_condition: "cross_above".to_string(),
            trigger_price: 0.80,
            max_price: None,
            price_to_beat_trigger_enabled: false,
            price_to_beat_mode: crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual,
            price_to_beat_trigger_min_gap: None,
            price_to_beat_trigger_max_gap: None,
            price_to_beat_trigger_unit:
                crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd,
            protection_mode: "off".to_string(),
            protection_asset: None,
            confirmation_ms: None,
            cycle_window_mode: Some("elapsed".to_string()),
            cycle_window_secs: Some(300),
            cycle_window_start_sec: Some(240),
            cycle_window_end_sec: Some(285),
            auto_sell_on_window_end: false,
        }
    }

    #[test]
    fn prewarm_window_uses_trigger_start_minus_configured_seconds() {
        assert_eq!(
            live_gap_history_prewarm_start_elapsed_sec(Some(240), 220, 20, "before_trigger_window"),
            220
        );
    }

    #[test]
    fn prewarm_window_falls_back_to_live_gap_window_start() {
        assert_eq!(
            live_gap_history_prewarm_start_elapsed_sec(None, 220, 20, "before_trigger_window"),
            200
        );
    }

    #[test]
    fn sample_throttle_skips_duplicates_inside_sample_window() {
        let key = "throttle:test";
        LIVE_GAP_HISTORY_PREWARM_LAST_SAMPLE_MS
            .lock()
            .expect("prewarm throttle")
            .remove(key);
        assert!(live_gap_history_prewarm_should_sample(key, 10_000, 250));
        assert!(!live_gap_history_prewarm_should_sample(key, 10_100, 250));
        assert!(live_gap_history_prewarm_should_sample(key, 10_260, 250));
    }

    #[test]
    fn both_side_prewarm_resolves_up_and_down_tokens() {
        let context = json!({
            "flowContext": {
                "yesTokenId": "tok-up",
                "noTokenId": "tok-down"
            }
        });
        let tokens = live_gap_history_prewarm_outcome_tokens(&spec(), &context, "both");
        assert_eq!(
            tokens,
            vec![
                ("Up".to_string(), "tok-up".to_string()),
                ("Down".to_string(), "tok-down".to_string()),
            ]
        );
    }
}
