#[derive(Debug, Clone, Copy, PartialEq)]
struct PtbOracleLagBookLeadStopConfig {
    enabled: bool,
    min_age_after_fill_ms: i64,
    max_token_spread: f64,
}

impl Default for PtbOracleLagBookLeadStopConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_age_after_fill_ms: 1_500,
            max_token_spread: 0.08,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PtbOracleLagBookLeadStopEvaluation {
    enabled: bool,
    should_trigger: bool,
    reason_code: &'static str,
    payload: Value,
}

fn append_action_place_order_ptb_oracle_lag_stop_payload(
    payload: &mut serde_json::Map<String, Value>,
    node: &TradeFlowNode,
) {
    payload.insert(
        "ptb_stop_loss_oracle_lag_book_lead_enabled".to_string(),
        json!(node_config_bool(node, "ptbStopLossOracleLagBookLeadEnabled").unwrap_or(false)),
    );
    payload.insert(
        "ptb_stop_loss_oracle_lag_book_lead_min_age_after_fill_ms".to_string(),
        json!(
            node_config_i64(node, "ptbStopLossOracleLagBookLeadMinAgeAfterFillMs")
                .unwrap_or(1_500)
                .max(0)
        ),
    );
    payload.insert(
        "ptb_stop_loss_oracle_lag_max_token_spread_cent".to_string(),
        json!(node_config_f64(node, "ptbStopLossOracleLagMaxTokenSpreadCent").unwrap_or(8.0)),
    );
}

fn ptb_oracle_lag_stop_config_from_payload(
    payload: Option<&Value>,
) -> PtbOracleLagBookLeadStopConfig {
    let Some(payload) = payload else {
        return PtbOracleLagBookLeadStopConfig::default();
    };
    PtbOracleLagBookLeadStopConfig {
        enabled: payload
            .get("ptb_stop_loss_oracle_lag_book_lead_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        min_age_after_fill_ms: payload
            .get("ptb_stop_loss_oracle_lag_book_lead_min_age_after_fill_ms")
            .and_then(Value::as_i64)
            .unwrap_or(1_500)
            .max(0),
        max_token_spread: payload
            .get("ptb_stop_loss_oracle_lag_max_token_spread_cent")
            .and_then(Value::as_f64)
            .map(|value| value / 100.0)
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
            .unwrap_or(0.08),
    }
}

fn trade_builder_ptb_oracle_lag_stop_candidate_enabled(order: &TradeBuilderOrder) -> bool {
    matches!(
        order.origin_flow_node_key.as_deref(),
        Some("action_btc_eq77_up" | "action_btc_eq77_down")
    )
}

fn trade_builder_ptb_stop_loss_cex_source_triggered(
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> bool {
    evaluation
        .source_evaluations
        .iter()
        .any(|source| source.config_source == "cex_consensus" && source.should_trigger)
}

fn trade_builder_ptb_stop_loss_cex_directional_gap(
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> Option<f64> {
    evaluation
        .source_evaluations
        .iter()
        .find(|source| source.config_source == "cex_consensus")
        .and_then(|source| source.directional_gap)
}

fn trade_builder_ptb_stop_loss_chainlink_gap(
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> Option<f64> {
    evaluation
        .source_evaluations
        .iter()
        .find(|source| source.config_source == "chainlink")
        .and_then(|source| source.directional_gap)
}

fn trade_builder_ptb_oracle_lag_stop_dirty_candidate(
    order: &TradeBuilderOrder,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> bool {
    trade_builder_ptb_oracle_lag_stop_candidate_enabled(order)
        && trade_builder_ptb_stop_loss_cex_source_triggered(evaluation)
}

fn ptb_oracle_lag_best_bid(book: &OrderBookSnapshot) -> Option<f64> {
    book.bids
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.price < 1.0)
        .map(|level| level.price)
        .max_by(f64::total_cmp)
}

fn ptb_oracle_lag_visible_bid_qty(book: &OrderBookSnapshot) -> Option<f64> {
    let qty: f64 = book
        .bids
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.size.is_finite())
        .map(|level| level.size.max(0.0))
        .sum();
    (qty > 0.0).then_some(qty)
}

fn ptb_oracle_lag_mid(best_bid: Option<f64>, best_ask: Option<f64>) -> Option<f64> {
    match (best_bid, best_ask) {
        (Some(bid), Some(ask)) if ask >= bid => Some((bid + ask) / 2.0),
        (Some(bid), None) => Some(bid),
        (None, Some(ask)) => Some(ask),
        _ => None,
    }
}

fn ptb_oracle_lag_selected_token_drop_threshold(seconds_left: Option<f64>) -> f64 {
    match seconds_left.unwrap_or(120.0) {
        seconds if seconds > 60.0 => 0.10,
        seconds if seconds > 25.0 => 0.08,
        _ => 0.05,
    }
}

fn ptb_oracle_lag_confirm_elapsed_ms(seconds_left: Option<f64>) -> i64 {
    match seconds_left.unwrap_or(120.0) {
        seconds if seconds > 60.0 => 750,
        seconds if seconds > 25.0 => 500,
        _ => 250,
    }
}

fn ptb_oracle_lag_seconds_left(market_slug: &str) -> Option<f64> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let start = MarketCycleId(market_slug.to_string()).start_time()?;
    let end = start + ChronoDuration::seconds(updown_scope_window_seconds(scope));
    Some(
        end.signed_duration_since(Utc::now())
            .num_milliseconds()
            .max(0) as f64
            / 1_000.0,
    )
}

async fn trade_builder_ptb_oracle_lag_opposite_book(
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
) -> Option<OrderBookSnapshot> {
    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let market = gamma
        .get_market_spec_by_slug(&order.market_slug)
        .await
        .ok()
        .flatten()?;
    let outcome = order.outcome_label.trim().to_ascii_lowercase();
    let opposite_token = if outcome == "up"
        || outcome == "yes"
        || market.yes_token_id.as_deref() == Some(order.token_id.as_str())
    {
        market.no_token_id
    } else {
        market.yes_token_id
    }?;
    client.order_book(&opposite_token).await.ok().flatten()
}

async fn evaluate_trade_builder_ptb_oracle_lag_book_lead_stop(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
    ptb_evaluation: Option<&TradeBuilderPtbStopLossEvaluation>,
) -> Result<Option<PtbOracleLagBookLeadStopEvaluation>> {
    let Some(parent_order_id) = order.parent_order_id else {
        return Ok(None);
    };
    let parent_payload = repo
        .load_trade_builder_order_flow_created_payload(parent_order_id)
        .await?;
    let config = ptb_oracle_lag_stop_config_from_payload(parent_payload.as_ref());
    if !config.enabled {
        return Ok(None);
    }
    let Some(ptb_evaluation) = ptb_evaluation else {
        return Ok(Some(ptb_oracle_lag_stop_no_trigger(
            "ptb_evaluation_unavailable",
            json!({ "book_lead_stop_confidence": "unavailable" }),
        )));
    };
    let order_age_ms = Utc::now()
        .signed_duration_since(order.created_at)
        .num_milliseconds()
        .max(0);
    let cooldown_active = order_age_ms < config.min_age_after_fill_ms;
    if cooldown_active {
        return Ok(Some(ptb_oracle_lag_stop_no_trigger(
            "book_lead_stop_cooldown_active",
            json!({
                "book_lead_stop_cooldown_active": true,
                "book_lead_stop_baseline_ready": order.last_seen_price.is_some(),
                "book_lead_stop_confidence": "cooldown",
            }),
        )));
    }
    let Some(cex_gap) = trade_builder_ptb_stop_loss_cex_directional_gap(ptb_evaluation) else {
        return Ok(Some(ptb_oracle_lag_stop_no_trigger(
            "cex_only_unconfirmed_no_stop",
            json!({ "book_lead_stop_confidence": "unavailable" }),
        )));
    };
    let chainlink_gap = trade_builder_ptb_stop_loss_chainlink_gap(ptb_evaluation);
    let chainlink_fresh_positive = chainlink_gap.map(|gap| gap > 0.0).unwrap_or(false);
    let seconds_left = ptb_oracle_lag_seconds_left(&order.market_slug);
    let strict_multiplier = if chainlink_fresh_positive { 1.5 } else { 1.0 };
    let required_cex_gap = if chainlink_fresh_positive { -8.0 } else { -5.0 };
    if cex_gap > required_cex_gap {
        return Ok(Some(ptb_oracle_lag_stop_no_trigger(
            "cex_only_unconfirmed_no_stop",
            json!({
                "cex_gap": cex_gap,
                "required_cex_gap": required_cex_gap,
                "book_lead_stop_confidence": "low",
            }),
        )));
    }
    let selected_best_bid = runtime_price.best_bid;
    let selected_best_ask = runtime_price.best_ask;
    let selected_mid = ptb_oracle_lag_mid(selected_best_bid, selected_best_ask);
    let selected_spread = selected_best_bid
        .zip(selected_best_ask)
        .map(|(bid, ask)| ask - bid);
    let selected_token_depth_ok = selected_best_bid.is_some();
    let baseline = order.last_seen_price;
    let selected_bid_drop = baseline.zip(selected_mid).map(|(base, mid)| base - mid);
    let selected_drop_threshold =
        ptb_oracle_lag_selected_token_drop_threshold(seconds_left) * strict_multiplier;
    let confirm_elapsed_required =
        (ptb_oracle_lag_confirm_elapsed_ms(seconds_left) as f64 * strict_multiplier).round() as i64;
    let opposite_book = trade_builder_ptb_oracle_lag_opposite_book(cfg, client, order).await;
    let opposite_best_bid = opposite_book.as_ref().and_then(ptb_oracle_lag_best_bid);
    let opposite_bid_depth = opposite_book
        .as_ref()
        .and_then(ptb_oracle_lag_visible_bid_qty);
    let spread_ok = selected_spread
        .map(|spread| spread <= config.max_token_spread + 0.000000001)
        .unwrap_or(false);
    let drop_ok = selected_bid_drop
        .map(|drop| drop >= selected_drop_threshold)
        .unwrap_or(false);
    let opposite_ok = opposite_best_bid.is_some() && opposite_bid_depth.unwrap_or(0.0) > 0.0;
    let elapsed_ok = order_age_ms >= confirm_elapsed_required;
    let should_trigger =
        selected_token_depth_ok && spread_ok && drop_ok && opposite_ok && elapsed_ok;
    let reason_code = if should_trigger {
        "oracle_lag_book_lead_stop"
    } else {
        "cex_only_unconfirmed_no_stop"
    };
    let payload = json!({
        "book_lead_stop_cooldown_active": false,
        "book_lead_stop_baseline_ready": baseline.is_some(),
        "selected_token_mid_baseline_cent": baseline.map(|value| value * 100.0),
        "selected_token_spread_cent": selected_spread.map(|value| value * 100.0),
        "selected_token_book_age_ms": 0,
        "selected_token_depth_ok": selected_token_depth_ok,
        "selected_token_bid_drop_cent": selected_bid_drop.map(|value| value * 100.0),
        "selected_token_bid_depth_delta": Value::Null,
        "opposite_bid_depth_delta": Value::Null,
        "book_lead_stop_confidence": if should_trigger { "high" } else { "low" },
        "cex_gap": cex_gap,
        "chainlink_gap": chainlink_gap,
        "chainlink_fresh_positive": chainlink_fresh_positive,
        "selected_drop_threshold_cent": selected_drop_threshold * 100.0,
        "confirm_elapsed_ms_required": confirm_elapsed_required,
        "order_age_ms": order_age_ms,
        "selected_best_bid": selected_best_bid,
        "selected_best_ask": selected_best_ask,
        "opposite_best_bid": opposite_best_bid,
        "opposite_bid_depth": opposite_bid_depth,
    });
    Ok(Some(PtbOracleLagBookLeadStopEvaluation {
        enabled: true,
        should_trigger,
        reason_code,
        payload,
    }))
}

fn ptb_oracle_lag_stop_no_trigger(
    reason_code: &'static str,
    mut payload: Value,
) -> PtbOracleLagBookLeadStopEvaluation {
    if let Some(object) = payload.as_object_mut() {
        object.insert("reason_code".to_string(), json!(reason_code));
    }
    PtbOracleLagBookLeadStopEvaluation {
        enabled: true,
        should_trigger: false,
        reason_code,
        payload,
    }
}

fn append_trade_builder_ptb_oracle_lag_stop_payload(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &PtbOracleLagBookLeadStopEvaluation,
) {
    payload.insert(
        "ptb_oracle_lag_book_lead_stop".to_string(),
        json!({
            "enabled": evaluation.enabled,
            "should_trigger": evaluation.should_trigger,
            "reason_code": evaluation.reason_code,
            "details": evaluation.payload,
        }),
    );
}

#[cfg(test)]
mod ptb_stop_loss_oracle_lag_book_lead_tests {
    use super::*;

    fn source_eval(
        config_source: &'static str,
        source: &'static str,
        should_trigger: bool,
    ) -> TradeBuilderPtbStopLossSourceEvaluation {
        TradeBuilderPtbStopLossSourceEvaluation {
            config_source,
            current_price_source: source,
            current_price: Some(60_000.0),
            directional_gap: Some(-6.0),
            reason_code: "ptb_gap_threshold_hit",
            should_trigger,
            error_code: None,
            error_detail: None,
            metadata: None,
        }
    }

    #[test]
    fn ptb_stop_loss_oracle_lag_config_parses_payload_defaults() {
        let config = ptb_oracle_lag_stop_config_from_payload(Some(&json!({
            "ptb_stop_loss_oracle_lag_book_lead_enabled": true,
            "ptb_stop_loss_oracle_lag_book_lead_min_age_after_fill_ms": 900,
            "ptb_stop_loss_oracle_lag_max_token_spread_cent": 6.0,
        })));

        assert!(config.enabled);
        assert_eq!(config.min_age_after_fill_ms, 900);
        assert_eq!(config.max_token_spread, 0.06);
    }

    #[test]
    fn ptb_stop_loss_oracle_lag_drop_thresholds_follow_time_bucket() {
        assert_eq!(
            ptb_oracle_lag_selected_token_drop_threshold(Some(90.0)),
            0.10
        );
        assert_eq!(
            ptb_oracle_lag_selected_token_drop_threshold(Some(40.0)),
            0.08
        );
        assert_eq!(
            ptb_oracle_lag_selected_token_drop_threshold(Some(12.0)),
            0.05
        );
        assert_eq!(ptb_oracle_lag_confirm_elapsed_ms(Some(90.0)), 750);
        assert_eq!(ptb_oracle_lag_confirm_elapsed_ms(Some(40.0)), 500);
        assert_eq!(ptb_oracle_lag_confirm_elapsed_ms(Some(12.0)), 250);
    }

    #[test]
    fn ptb_stop_loss_oracle_lag_dirty_candidate_requires_cex_trigger() {
        let mut evaluation = TradeBuilderPtbStopLossEvaluation {
            asset: Some("btc".to_string()),
            direction: Some("up".to_string()),
            threshold_gap_usd: -3.0,
            ptb_reference_price: Some(60_000.0),
            current_price: Some(59_994.0),
            current_price_source: "chainlink_cex_consensus_confirmed",
            current_chainlink_price: Some(60_002.0),
            directional_gap: Some(-6.0),
            reason_code: "ptb_gap_threshold_hit",
            should_trigger: true,
            source_evaluations: vec![source_eval("chainlink", "chainlink_live_data_ws", true)],
        };

        assert!(!trade_builder_ptb_stop_loss_cex_source_triggered(&evaluation));
        evaluation
            .source_evaluations
            .push(source_eval("cex_consensus", "binance_btc_usdt", true));
        assert!(trade_builder_ptb_stop_loss_cex_source_triggered(&evaluation));
    }
}
