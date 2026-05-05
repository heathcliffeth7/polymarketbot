use crate::trade_flow::guards::price_to_beat::{
    normalize_price_to_beat_threshold_usd, resolve_price_to_beat_current_price_snapshot,
    PriceToBeatCurrentPriceSource, PriceToBeatDiffUnit,
};
use crate::trade_flow::guards::polymarket_price_to_beat::PriceToBeatSource;
use bot_infra::db::TradeBuilderPtbStopLossRule;

#[derive(Debug, Clone, PartialEq)]
struct ActionPlaceOrderPtbStopLossConfig {
    hard_gap_usd: Option<f64>,
    staged_rules: Vec<TradeBuilderPtbStopLossRule>,
    reference_price: Option<f64>,
    time_decay_mode: Option<String>,
    current_price_source: PriceToBeatCurrentPriceSource,
}

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderPtbStopLossEvaluation {
    asset: Option<String>,
    direction: Option<String>,
    threshold_gap_usd: f64,
    ptb_reference_price: Option<f64>,
    current_price: Option<f64>,
    current_price_source: &'static str,
    current_chainlink_price: Option<f64>,
    directional_gap: Option<f64>,
    reason_code: &'static str,
    should_trigger: bool,
}

fn trade_builder_market_supports_ptb_stop_loss(market_slug: &str) -> bool {
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return false;
    };
    matches!(scope.timeframe, "5m" | "15m")
}

fn trade_builder_cached_ptb_reference_price(market_slug: &str) -> Option<f64> {
    trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(market_slug)
        .map(|snapshot| snapshot.price_to_beat)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn resolve_action_place_order_ptb_stop_loss_config(
    node: &TradeFlowNode,
    side: &str,
    market_slug: &str,
) -> Result<Option<ActionPlaceOrderPtbStopLossConfig>> {
    let hard_stop_loss_enabled = node_config_bool(node, "ptbStopLossEnabled").unwrap_or(false);
    let gap_unit = resolve_action_place_order_ptb_stop_loss_gap_unit(node)?;
    let staged_rules = parse_action_place_order_ptb_stop_loss_rules(
        node.config.get("ptbStopLossRules"),
        gap_unit,
    )?;
    if side != "buy" || (!hard_stop_loss_enabled && staged_rules.is_empty()) {
        return Ok(None);
    }

    anyhow::ensure!(
        trade_builder_market_supports_ptb_stop_loss(market_slug),
        "action.place_order ptbStopLossEnabled only supports 5m/15m updown market slugs"
    );

    let hard_gap_usd = node_config_f64(node, "ptbStopLossGapUsd")
        .map(|value| normalize_price_to_beat_threshold_usd(value, gap_unit));
    if hard_stop_loss_enabled && hard_gap_usd.is_none() && staged_rules.is_empty() {
        anyhow::bail!("action.place_order ptbStopLossGapUsd must be set");
    }
    if let Some(gap_usd) = hard_gap_usd {
        anyhow::ensure!(
            gap_usd.is_finite(),
            "action.place_order ptbStopLossGapUsd must be finite"
        );
    }

    let time_decay_mode = node_config_string(node, "ptbStopLossTimeDecayMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "none" || value == "tighten" || value == "relax")
        .or_else(|| Some("tighten".to_string()));
    let stop_loss_current_price_source = node_config_string(node, "ptbStopLossCurrentPriceSource");
    let entry_current_price_source = node_config_string(node, "priceToBeatCurrentPriceSource");
    let current_price_source = PriceToBeatCurrentPriceSource::parse(
        stop_loss_current_price_source
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or(entry_current_price_source.as_deref()),
    );

    Ok(Some(ActionPlaceOrderPtbStopLossConfig {
        hard_gap_usd,
        staged_rules,
        reference_price: trade_builder_cached_ptb_reference_price(market_slug),
        time_decay_mode,
        current_price_source,
    }))
}

fn resolve_action_place_order_ptb_stop_loss_gap_unit(
    node: &TradeFlowNode,
) -> Result<PriceToBeatDiffUnit> {
    let raw = node_config_string(node, "ptbStopLossGapUnit");
    PriceToBeatDiffUnit::parse(raw.as_deref())
        .ok_or_else(|| anyhow::anyhow!("action.place_order ptbStopLossGapUnit must be usd or cent"))
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActionPlaceOrderPtbStopLossRuleConfig {
    gap_usd: f64,
    size_pct: f64,
}

fn parse_action_place_order_ptb_stop_loss_rules(
    raw_value: Option<&Value>,
    gap_unit: PriceToBeatDiffUnit,
) -> Result<Vec<TradeBuilderPtbStopLossRule>> {
    let Some(raw_value) = raw_value else {
        return Ok(Vec::new());
    };
    let rules: Vec<ActionPlaceOrderPtbStopLossRuleConfig> =
        serde_json::from_value(raw_value.clone())
            .context("action.place_order ptbStopLossRules must be an array")?;
    anyhow::ensure!(
        rules.len() <= TRADE_BUILDER_EXIT_LADDER_MAX_RULES,
        "action.place_order ptbStopLossRules supports at most {} rules",
        TRADE_BUILDER_EXIT_LADDER_MAX_RULES
    );
    let mut parsed = Vec::with_capacity(rules.len());
    let mut previous_gap_usd = None;
    let mut total_size_pct = 0.0_f64;
    for (index, rule) in rules.into_iter().enumerate() {
        anyhow::ensure!(
            rule.gap_usd.is_finite(),
            "action.place_order ptbStopLossRules[{index}].gapUsd must be finite"
        );
        anyhow::ensure!(
            rule.size_pct.is_finite() && rule.size_pct > 0.0 && rule.size_pct <= 100.0,
            "action.place_order ptbStopLossRules[{index}].sizePct must be in (0, 100]"
        );
        if let Some(previous_gap_usd) = previous_gap_usd {
            anyhow::ensure!(
                rule.gap_usd < previous_gap_usd,
                "action.place_order ptbStopLossRules gapUsd values must be strictly decreasing"
            );
        }
        previous_gap_usd = Some(rule.gap_usd);
        total_size_pct += rule.size_pct;
        parsed.push(TradeBuilderPtbStopLossRule {
            gap_usd: normalize_price_to_beat_threshold_usd(rule.gap_usd, gap_unit),
            size_pct: rule.size_pct,
        });
    }
    anyhow::ensure!(
        (total_size_pct - 100.0).abs() <= 0.000001 || parsed.is_empty(),
        "action.place_order ptbStopLossRules sizePct total must equal 100"
    );
    Ok(parsed)
}

fn trade_builder_ptb_stop_loss_target_plan(
    rules: &[TradeBuilderPtbStopLossRule],
    canonical_entry_qty: f64,
    order_min_size: Option<f64>,
) -> TradeBuilderLadderTargetPlan<usize> {
    let weighted_rules = rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (index, rule.size_pct))
        .collect::<Vec<_>>();
    trade_builder_plan_weighted_ladder_targets(&weighted_rules, canonical_entry_qty, order_min_size)
}

fn trade_builder_ptb_stop_loss_gap_usd(order: &TradeBuilderOrder) -> Option<f64> {
    order
        .ptb_stop_loss_gap_usd
        .filter(|value| value.is_finite())
}

fn trade_builder_ptb_stop_loss_effective_gap_usd(
    order: &TradeBuilderOrder,
    threshold_gap_usd: f64,
) -> f64 {
    let mode = order
        .ptb_stop_loss_time_decay_mode
        .as_deref()
        .unwrap_or("tighten");
    if mode == "none" || threshold_gap_usd < 0.0 {
        return threshold_gap_usd;
    }
    let Some(scope) = find_updown_scope_by_slug(&order.market_slug) else {
        return threshold_gap_usd;
    };
    let Some(window_start) = MarketCycleId(order.market_slug.clone()).start_time() else {
        return threshold_gap_usd;
    };
    let window_seconds = updown_scope_window_seconds(scope).max(1);
    let elapsed_ratio = (Utc::now()
        .signed_duration_since(window_start)
        .num_milliseconds()
        .max(0) as f64
        / (window_seconds * 1_000) as f64)
        .clamp(0.0, 1.0);
    match mode {
        "tighten" => (threshold_gap_usd * (1.0 - elapsed_ratio)).max(0.0),
        "relax" => threshold_gap_usd * (1.0 + elapsed_ratio),
        _ => threshold_gap_usd,
    }
}

fn trade_builder_ptb_direction(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn trade_builder_evaluate_ptb_stop_loss(
    order: &TradeBuilderOrder,
) -> Option<TradeBuilderPtbStopLossEvaluation> {
    let threshold_gap_usd = trade_builder_ptb_stop_loss_gap_usd(order)?;
    let threshold_gap_usd = trade_builder_ptb_stop_loss_effective_gap_usd(order, threshold_gap_usd);
    let current_price_source =
        PriceToBeatCurrentPriceSource::parse(Some(order.ptb_current_price_source.as_str()));
    let current_price_source_label = current_price_source.current_price_source_label();
    if !trade_builder_is_stop_loss_child(order) {
        return None;
    }

    let Some(scope) = find_updown_scope_by_slug(&order.market_slug) else {
        return Some(TradeBuilderPtbStopLossEvaluation {
            asset: None,
            direction: None,
            threshold_gap_usd,
            ptb_reference_price: order.ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_market",
            should_trigger: false,
        });
    };
    if !matches!(scope.timeframe, "5m" | "15m") {
        return Some(TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: None,
            threshold_gap_usd,
            ptb_reference_price: order.ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_market",
            should_trigger: false,
        });
    }

    let Some(direction) = trade_builder_ptb_direction(&order.outcome_label) else {
        return Some(TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: None,
            threshold_gap_usd,
            ptb_reference_price: order.ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_outcome_label",
            should_trigger: false,
        });
    };

    let ptb_reference_price = order
        .ptb_reference_price
        .filter(|value| value.is_finite() && *value > 0.0)
        .or_else(|| trade_builder_cached_ptb_reference_price(&order.market_slug));
    if ptb_reference_price.is_none() {
        return Some(TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: Some(direction.to_string()),
            threshold_gap_usd,
            ptb_reference_price: None,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "ptb_reference_pending",
            should_trigger: false,
        });
    }

    let current_price = resolve_price_to_beat_current_price_snapshot(
        current_price_source,
        PriceToBeatSource::Polymarket,
        &order.market_slug,
        scope.asset,
        None,
    )
    .ok()
    .map(|(price, _)| price);
    let Some(current_price) = current_price else {
        return Some(TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: Some(direction.to_string()),
            threshold_gap_usd,
            ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "ptb_current_price_unavailable",
            should_trigger: false,
        });
    };

    let directional_gap = if direction == "up" {
        current_price - ptb_reference_price.unwrap_or_default()
    } else {
        ptb_reference_price.unwrap_or_default() - current_price
    };
    // Negative thresholds intentionally mean "wait for parity, then overshoot against the position".
    let should_trigger = directional_gap <= threshold_gap_usd;
    Some(TradeBuilderPtbStopLossEvaluation {
        asset: Some(scope.asset.to_string()),
        direction: Some(direction.to_string()),
        threshold_gap_usd,
        ptb_reference_price,
        current_price: Some(current_price),
        current_price_source: current_price_source_label,
        current_chainlink_price: (current_price_source == PriceToBeatCurrentPriceSource::Chainlink)
            .then_some(current_price),
        directional_gap: Some(directional_gap),
        reason_code: if should_trigger {
            "ptb_gap_threshold_hit"
        } else {
            "ptb_gap_threshold_not_met"
        },
        should_trigger,
    })
}

fn trade_builder_ptb_reference_price_persist_candidate(
    order: &TradeBuilderOrder,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> Option<f64> {
    if order
        .ptb_reference_price
        .is_some_and(|value| value.is_finite() && value > 0.0)
    {
        return None;
    }
    evaluation
        .ptb_reference_price
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn append_trade_builder_ptb_stop_loss_payload(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) {
    payload.insert(
        "ptb_stop_loss".to_string(),
        json!({
            "reason_code": evaluation.reason_code,
            "asset": evaluation.asset,
            "direction": evaluation.direction,
            "threshold_gap_usd": evaluation.threshold_gap_usd,
            "ptb_reference_price": evaluation.ptb_reference_price,
            "current_price": evaluation.current_price,
            "current_price_source": evaluation.current_price_source,
            "current_chainlink_price": evaluation.current_chainlink_price,
            "directional_gap": evaluation.directional_gap,
            "should_trigger": evaluation.should_trigger,
        }),
    );
}

#[cfg(test)]
mod trade_builder_ptb_stop_loss_tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, seed_cex_book_test_sample, CexBookSample, CexVenue,
    };
    use chrono::Utc;
    use std::sync::{Mutex, MutexGuard};

    static PTB_STOP_LOSS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_ptb_stop_loss_test_state() -> MutexGuard<'static, ()> {
        PTB_STOP_LOSS_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn ptb_stop_loss_requires_supported_market_scope() {
        assert!(!trade_builder_market_supports_ptb_stop_loss(
            "nba-lal-orl-2026-03-21"
        ));
        assert!(trade_builder_market_supports_ptb_stop_loss(
            "eth-updown-5m-1774013100"
        ));
    }

    fn test_ptb_stop_loss_order(
        market_slug: &str,
        outcome_label: &str,
        gap_usd: f64,
        ptb_reference_price: Option<f64>,
    ) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "conditional".to_string(),
            status: "armed".to_string(),
            market_slug: market_slug.to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: outcome_label.to_string(),
            side: "sell".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_below".to_string()),
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES.to_string(),
            size_usdc: 5.0,
            target_qty: Some(5.0),
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: Some(5.0),
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: Some(9),
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: Some(gap_usd),
            ptb_reference_price,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: Some("tighten".to_string()),
            ptb_current_price_source: "chainlink".to_string(),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    fn seed_ptb_stop_loss_current_price(asset: &str, current_chainlink_price: f64) {
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            asset,
            &[
                (now_ms - 250, current_chainlink_price),
                (now_ms, current_chainlink_price),
            ],
        )
        .expect("seed chainlink ticks");
    }

    fn seed_ptb_stop_loss_cex_current_price(asset: &str, venue: CexVenue, current_price: f64) {
        let now_ms = Utc::now().timestamp_millis();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: now_ms,
            bid: current_price - 0.5,
            ask: current_price + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        });
    }

    fn evaluate_test_ptb_stop_loss(
        market_slug: &str,
        asset: &str,
        outcome_label: &str,
        gap_usd: f64,
        ptb_reference_price: f64,
        current_chainlink_price: f64,
    ) -> TradeBuilderPtbStopLossEvaluation {
        let _guard = lock_ptb_stop_loss_test_state();
        seed_ptb_stop_loss_current_price(asset, current_chainlink_price);
        let order = test_ptb_stop_loss_order(
            market_slug,
            outcome_label,
            gap_usd,
            Some(ptb_reference_price),
        );

        trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval")
    }

    fn assert_option_f64_close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("expected numeric value");
        assert!(
            (actual - expected).abs() <= 0.000001,
            "expected {expected}, got {actual}"
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_triggers_when_up_gap_reverts_to_zero() {
        let _guard = lock_ptb_stop_loss_test_state();
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "sol",
            &[(now_ms - 250, 101.0), (now_ms, 99.75)],
        )
        .expect("seed sol ticks");
        let order = test_ptb_stop_loss_order("sol-updown-5m-1774013100", "Up", 0.0, Some(100.0));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
        assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
        assert_eq!(evaluation.directional_gap, Some(-0.25));
        assert!(evaluation.should_trigger);
    }

    #[tokio::test]
    async fn ptb_stop_loss_waits_for_negative_overshoot_gap() {
        let _guard = lock_ptb_stop_loss_test_state();
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "eth",
            &[(now_ms - 250, 70.0), (now_ms, 79.0)],
        )
        .expect("seed eth ticks");
        let order = test_ptb_stop_loss_order("eth-updown-5m-1774013100", "Up", -20.0, Some(100.0));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
        assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
        assert_eq!(evaluation.directional_gap, Some(-21.0));
        assert!(evaluation.should_trigger);
    }

    #[test]
    fn ptb_stop_loss_negative_gap_for_up_triggers_only_after_price_moves_10_below_reference() {
        let market_slug = "eth-updown-5m-1774013100";
        let blocked = evaluate_test_ptb_stop_loss(market_slug, "eth", "Up", -10.0, 100.0, 90.01);
        assert_eq!(blocked.reason_code, "ptb_gap_threshold_not_met");
        assert_option_f64_close(blocked.directional_gap, -9.99);
        assert!(!blocked.should_trigger);

        let triggered = evaluate_test_ptb_stop_loss(market_slug, "eth", "Up", -10.0, 100.0, 90.0);
        assert_eq!(triggered.reason_code, "ptb_gap_threshold_hit");
        assert_option_f64_close(triggered.directional_gap, -10.0);
        assert!(triggered.should_trigger);
    }

    #[test]
    fn ptb_stop_loss_negative_gap_for_down_triggers_only_after_price_moves_10_above_reference() {
        let market_slug = "btc-updown-5m-1774013100";
        let blocked = evaluate_test_ptb_stop_loss(market_slug, "btc", "Down", -10.0, 100.0, 109.99);
        assert_eq!(blocked.reason_code, "ptb_gap_threshold_not_met");
        assert_option_f64_close(blocked.directional_gap, -9.99);
        assert!(!blocked.should_trigger);

        let triggered =
            evaluate_test_ptb_stop_loss(market_slug, "btc", "Down", -10.0, 100.0, 110.0);
        assert_eq!(triggered.reason_code, "ptb_gap_threshold_hit");
        assert_option_f64_close(triggered.directional_gap, -10.0);
        assert!(triggered.should_trigger);
    }

    #[tokio::test]
    async fn ptb_stop_loss_blocks_when_down_gap_stays_above_threshold() {
        let _guard = lock_ptb_stop_loss_test_state();
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, 99.0), (now_ms, 98.5)],
        )
        .expect("seed btc ticks");
        let order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Down", 1.0, Some(100.0));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
        assert_eq!(evaluation.reason_code, "ptb_gap_threshold_not_met");
        assert_eq!(evaluation.directional_gap, Some(1.5));
        assert!(!evaluation.should_trigger);
    }

    #[test]
    fn ptb_stop_loss_uses_selected_binance_current_price() {
        let _guard = lock_ptb_stop_loss_test_state();
        seed_ptb_stop_loss_current_price("btc", 140.0);
        seed_ptb_stop_loss_cex_current_price("btc", CexVenue::Binance, 90.0);
        let mut order =
            test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
        order.ptb_current_price_source = "binance".to_string();

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

        assert_eq!(evaluation.current_price_source, "binance_cex_ws_mid");
        assert_eq!(evaluation.current_price, Some(90.0));
        assert_eq!(evaluation.current_chainlink_price, None);
        assert_eq!(evaluation.directional_gap, Some(-10.0));
        assert!(evaluation.should_trigger);
    }

    #[test]
    fn ptb_stop_loss_rules_parse_and_validate_descending_weighted_rows() {
        let raw = json!([
            { "gapUsd": 12.5, "sizePct": 25.0 },
            { "gapUsd": 3.0, "sizePct": 75.0 }
        ]);
        let parsed =
            parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
                .expect("ptb staged rules");

        assert_eq!(
            parsed,
            vec![
                TradeBuilderPtbStopLossRule {
                    gap_usd: 12.5,
                    size_pct: 25.0,
                },
                TradeBuilderPtbStopLossRule {
                    gap_usd: 3.0,
                    size_pct: 75.0,
                },
            ]
        );
    }

    #[test]
    fn ptb_stop_loss_rules_accept_negative_descending_rows() {
        let raw = json!([
            { "gapUsd": 20.0, "sizePct": 25.0 },
            { "gapUsd": 0.0, "sizePct": 25.0 },
            { "gapUsd": -20.0, "sizePct": 50.0 }
        ]);
        let parsed =
            parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
                .expect("negative ptb staged rules");

        assert_eq!(
            parsed,
            vec![
                TradeBuilderPtbStopLossRule {
                    gap_usd: 20.0,
                    size_pct: 25.0,
                },
                TradeBuilderPtbStopLossRule {
                    gap_usd: 0.0,
                    size_pct: 25.0,
                },
                TradeBuilderPtbStopLossRule {
                    gap_usd: -20.0,
                    size_pct: 50.0,
                },
            ]
        );
    }

    #[test]
    fn ptb_stop_loss_rules_normalize_cent_rows_to_usd() {
        let raw = json!([
            { "gapUsd": 20.0, "sizePct": 60.0 },
            { "gapUsd": 0.0, "sizePct": 40.0 }
        ]);
        let parsed =
            parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Cent)
                .expect("cent ptb staged rules");

        assert_eq!(
            parsed,
            vec![
                TradeBuilderPtbStopLossRule {
                    gap_usd: 0.20,
                    size_pct: 60.0,
                },
                TradeBuilderPtbStopLossRule {
                    gap_usd: 0.0,
                    size_pct: 40.0,
                },
            ]
        );
    }

    fn test_place_order_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "action_test".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn ptb_stop_loss_config_accepts_staged_rules_without_legacy_gap() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossRules": [{ "gapUsd": 0.0, "sizePct": 100.0 }]
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("staged ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, None);
        assert_eq!(
            config.staged_rules,
            vec![TradeBuilderPtbStopLossRule {
                gap_usd: 0.0,
                size_pct: 100.0,
            }]
        );
    }

    #[test]
    fn ptb_stop_loss_config_preserves_legacy_hard_gap() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": 1.25
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("legacy ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, Some(1.25));
        assert!(config.staged_rules.is_empty());
    }

    #[test]
    fn ptb_stop_loss_config_inherits_entry_current_price_source() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": 1.25,
            "priceToBeatCurrentPriceSource": "binance"
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "btc-updown-5m-1774013100",
        )
        .expect("ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(
            config.current_price_source,
            PriceToBeatCurrentPriceSource::Binance
        );
    }

    #[test]
    fn ptb_stop_loss_config_override_wins_over_entry_current_price_source() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": 1.25,
            "priceToBeatCurrentPriceSource": "binance",
            "ptbStopLossCurrentPriceSource": "coinbase"
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "btc-updown-5m-1774013100",
        )
        .expect("ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(
            config.current_price_source,
            PriceToBeatCurrentPriceSource::Coinbase
        );
    }

    #[test]
    fn ptb_stop_loss_config_normalizes_cent_hard_gap_to_usd() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": 20.0,
            "ptbStopLossGapUnit": "cent"
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("cent hard ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, Some(0.2));
        assert!(config.staged_rules.is_empty());
    }

    #[test]
    fn ptb_stop_loss_config_preserves_hard_gap_with_staged_rules() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": 2.0,
            "ptbStopLossRules": [{ "gapUsd": 1.0, "sizePct": 100.0 }]
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("combined ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, Some(2.0));
        assert_eq!(
            config.staged_rules,
            vec![TradeBuilderPtbStopLossRule {
                gap_usd: 1.0,
                size_pct: 100.0,
            }]
        );
    }

    #[test]
    fn ptb_stop_loss_config_normalizes_cent_staged_rules_to_usd() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUnit": "cent",
            "ptbStopLossRules": [{ "gapUsd": 20.0, "sizePct": 100.0 }]
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("cent staged ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, None);
        assert_eq!(
            config.staged_rules,
            vec![TradeBuilderPtbStopLossRule {
                gap_usd: 0.2,
                size_pct: 100.0,
            }]
        );
    }

    #[test]
    fn ptb_stop_loss_config_preserves_negative_hard_gap() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true,
            "ptbStopLossGapUsd": -20.0
        }));

        let config = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect("negative hard ptb config should resolve")
        .expect("ptb config should be enabled");

        assert_eq!(config.hard_gap_usd, Some(-20.0));
        assert!(config.staged_rules.is_empty());
    }

    #[test]
    fn negative_ptb_gap_disables_time_decay() {
        let mut order =
            test_ptb_stop_loss_order("eth-updown-5m-1774013100", "Up", -20.0, Some(100.0));
        order.ptb_stop_loss_time_decay_mode = Some("tighten".to_string());
        let tighten_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

        order.ptb_stop_loss_time_decay_mode = Some("relax".to_string());
        let relax_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

        order.ptb_stop_loss_time_decay_mode = Some("none".to_string());
        let none_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

        assert_eq!(tighten_value, -20.0);
        assert_eq!(relax_value, -20.0);
        assert_eq!(none_value, -20.0);
    }

    #[test]
    fn ptb_stop_loss_config_still_requires_gap_or_rules_when_enabled() {
        let node = test_place_order_node(json!({
            "ptbStopLossEnabled": true
        }));

        let error = resolve_action_place_order_ptb_stop_loss_config(
            &node,
            "buy",
            "eth-updown-5m-1774013100",
        )
        .expect_err("empty ptb config should fail");

        assert!(error.to_string().contains("ptbStopLossGapUsd must be set"));
    }

    #[test]
    fn ptb_stop_loss_rules_reject_non_decreasing_gap_sequence() {
        let raw = json!([
            { "gapUsd": 3.0, "sizePct": 50.0 },
            { "gapUsd": 3.0, "sizePct": 50.0 }
        ]);

        let error =
            parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
                .expect_err("non-decreasing staged ptb rules should fail");
        assert!(error
            .to_string()
            .contains("ptbStopLossRules gapUsd values must be strictly decreasing"));
    }
}
