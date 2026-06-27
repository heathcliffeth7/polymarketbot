use super::*;
use async_trait::async_trait;
use bot_infra::exchange::{
    FillInfo, OrderAck, OrderInfo, PlaceOrderRequest, PriceHistoryPoint, PriceSnapshot,
};

struct FakeExecutor {
    quotes: HashMap<String, (Option<f64>, Option<f64>, Option<f64>)>,
}

#[async_trait]
impl OrderExecutor for FakeExecutor {
    async fn midpoint(&self, market: &str) -> Result<PriceSnapshot> {
        Ok(PriceSnapshot {
            market: market.to_string(),
            price: 0.5,
        })
    }

    async fn best_bid_ask(&self, token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        let (best_bid, best_ask, _) = self
            .quotes
            .get(token_id)
            .cloned()
            .unwrap_or((None, None, None));
        Ok((best_bid, best_ask))
    }

    async fn last_trade_price(&self, token_id: &str) -> Result<Option<f64>> {
        Ok(self
            .quotes
            .get(token_id)
            .and_then(|(_, _, last_trade)| *last_trade))
    }

    async fn price_history(
        &self,
        _token_id: &str,
        _start_ts: i64,
        _end_ts: i64,
        _fidelity: i64,
    ) -> Result<Vec<PriceHistoryPoint>> {
        Ok(Vec::new())
    }

    async fn fee_rate_bps(&self, _token_id: &str) -> Result<Option<u64>> {
        Ok(Some(0))
    }

    async fn place(&self, _req: &PlaceOrderRequest) -> Result<OrderAck> {
        anyhow::bail!("not used in test")
    }

    async fn cancel(&self, _exchange_order_id: &str) -> Result<()> {
        anyhow::bail!("not used in test")
    }

    async fn status(&self, _exchange_order_id: &str) -> Result<OrderInfo> {
        anyhow::bail!("not used in test")
    }

    async fn list_open(&self, _market: Option<&str>) -> Result<Vec<OrderInfo>> {
        Ok(Vec::new())
    }

    async fn list_fills(&self, _next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
        Ok(Vec::new())
    }

    async fn available_token_qty(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }
}

fn pair_lock_test_run() -> TradeFlowRun {
    TradeFlowRun {
        id: 77,
        definition_id: 88,
        version_id: 99,
        user_id: 1,
        status: "running".to_string(),
        trigger_source: Some("test".to_string()),
        context_json: json!({}),
        started_at: Some(Utc::now()),
        ended_at: None,
        last_error: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn pair_lock_auto_primary_selects_up_when_only_up_passes_max_price() {
    let executor = FakeExecutor {
        quotes: HashMap::from([
            ("yes".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
            ("no".to_string(), (Some(0.71), Some(0.72), Some(0.72))),
        ]),
    };
    let ws = ClobWsClient::new("wss://example.com/ws".to_string());
    let node = TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config: json!({
            "executionMode": "market",
            "maxPriceCent": 70,
            "minPriceDistanceCent": 1,
        }),
    };
    let step = TradeFlowRunStep {
        id: 1,
        run_id: 77,
        node_key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        status: "queued".to_string(),
        attempt: 1,
        input_json: Some(json!({})),
        output_json: None,
        error_text: None,
        started_at: None,
        ended_at: None,
        available_at: Utc::now(),
        parent_step_id: None,
        idempotency_key: None,
        created_at: Utc::now(),
    };
    let mut context = json!({});

    let selection = resolve_action_place_order_pair_lock_primary_selection(
        None,
        None,
        &ws,
        &executor,
        &pair_lock_test_run(),
        &step,
        &node,
        &mut context,
        "btc-updown-5m-1",
        Some("yes".to_string()),
        Some("no".to_string()),
    )
    .await
    .expect("selection");

    assert_eq!(selection.yes_candidate.quote.best_ask, Some(0.69));
    assert_eq!(selection.no_candidate.quote.best_ask, Some(0.72));
    let selection = selection.selection.expect("primary selection");
    assert_eq!(selection.token_id, "yes");
    assert_eq!(selection.outcome_label, "Up");
    assert_eq!(selection.selection_mode, "auto_guarded");
}

#[tokio::test]
async fn pair_lock_auto_primary_selects_down_when_only_down_passes_max_price() {
    let executor = FakeExecutor {
        quotes: HashMap::from([
            ("yes".to_string(), (Some(0.73), Some(0.74), Some(0.74))),
            ("no".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
        ]),
    };
    let ws = ClobWsClient::new("wss://example.com/ws".to_string());
    let node = TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config: json!({
            "executionMode": "market",
            "maxPriceCent": 70,
            "minPriceDistanceCent": 1,
        }),
    };
    let step = TradeFlowRunStep {
        id: 1,
        run_id: 77,
        node_key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        status: "queued".to_string(),
        attempt: 1,
        input_json: Some(json!({})),
        output_json: None,
        error_text: None,
        started_at: None,
        ended_at: None,
        available_at: Utc::now(),
        parent_step_id: None,
        idempotency_key: None,
        created_at: Utc::now(),
    };
    let mut context = json!({});

    let selection = resolve_action_place_order_pair_lock_primary_selection(
        None,
        None,
        &ws,
        &executor,
        &pair_lock_test_run(),
        &step,
        &node,
        &mut context,
        "btc-updown-5m-1",
        Some("yes".to_string()),
        Some("no".to_string()),
    )
    .await
    .expect("selection");

    assert_eq!(selection.yes_candidate.quote.best_ask, Some(0.74));
    assert_eq!(selection.no_candidate.quote.best_ask, Some(0.69));
    let selection = selection.selection.expect("primary selection");
    assert_eq!(selection.token_id, "no");
    assert_eq!(selection.outcome_label, "Down");
}

#[tokio::test]
async fn pair_lock_auto_primary_returns_waiting_when_retryable_guards_block_all_candidates() {
    let executor = FakeExecutor {
        quotes: HashMap::from([
            ("yes".to_string(), (Some(0.84), Some(0.85), Some(0.85))),
            ("no".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
        ]),
    };
    let ws = ClobWsClient::new("wss://example.com/ws".to_string());
    let node = TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config: json!({
            "executionMode": "market",
            "maxPriceCent": 70,
            "minPriceDistanceCent": 1,
            "retryOnMaxPriceBlock": true,
            "executionFloorGuardEnabled": true,
            "executionFloorPriceCent": 80
        }),
    };
    let step = TradeFlowRunStep {
        id: 1,
        run_id: 77,
        node_key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        status: "queued".to_string(),
        attempt: 1,
        input_json: Some(json!({})),
        output_json: None,
        error_text: None,
        started_at: None,
        ended_at: None,
        available_at: Utc::now(),
        parent_step_id: None,
        idempotency_key: None,
        created_at: Utc::now(),
    };
    let mut context = json!({});

    let selection = resolve_action_place_order_pair_lock_primary_selection(
        None,
        None,
        &ws,
        &executor,
        &pair_lock_test_run(),
        &step,
        &node,
        &mut context,
        "btc-updown-5m-1",
        Some("yes".to_string()),
        Some("no".to_string()),
    )
    .await
    .expect("selection");

    assert!(selection.selection.is_none());
    assert!(selection.waiting);
    assert_eq!(
        selection.failure_reason,
        Some("pair_lock_primary_guard_waiting")
    );
}

#[test]
fn build_pair_lock_primary_waiting_execution_repeats_same_node() {
    let execution = build_pair_lock_primary_waiting_execution(
        "pair_buy",
        "btc-updown-5m-1",
        &json!({
            "selection_mode": "auto_guarded",
            "resolved_yes_token_id": "yes-token",
            "resolved_no_token_id": "no-token",
            "trigger_node_market_slug": "btc-updown-5m-1",
            "yes_candidate_guard": {"decision": "waiting", "reason_code": "above_max_price"},
            "no_candidate_guard": {"decision": "blocked", "reason_code": "below_best_ask_floor"}
        }),
    );

    assert_eq!(
        execution.output.get("reason").and_then(Value::as_str),
        Some("pair_lock_primary_guard_waiting")
    );
    assert_eq!(
        execution
            .output
            .get("resolved_yes_token_id")
            .and_then(Value::as_str),
        Some("yes-token")
    );
    assert!(execution.repeat_at.is_some());
    assert!(execution.routes.is_empty());
}

#[test]
fn pair_lock_primary_ptb_guard_decision_maps_retryable_failures_to_waiting() {
    assert_eq!(pair_lock_primary_ptb_guard_decision(true, true), "passed");
    assert_eq!(pair_lock_primary_ptb_guard_decision(false, true), "waiting");
    assert_eq!(
        pair_lock_primary_ptb_guard_decision(false, false),
        "blocked"
    );
}

#[test]
fn pair_lock_primary_logs_ptb_skip_only_when_pre_guard_blocks_with_ptb_enabled() {
    let node = TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config: json!({
            "priceToBeatGuardEnabled": true
        }),
    };
    let node_without_ptb = TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config: json!({}),
    };

    assert!(pair_lock_primary_should_log_ptb_skip(&node, "waiting"));
    assert!(pair_lock_primary_should_log_ptb_skip(&node, "blocked"));
    assert!(!pair_lock_primary_should_log_ptb_skip(&node, "passed"));
    assert!(!pair_lock_primary_should_log_ptb_skip(
        &node_without_ptb,
        "waiting"
    ));
}

#[test]
fn pair_lock_primary_ptb_evaluation_log_snapshot_captures_key_fields() {
    let evaluation = crate::trade_flow::guards::price_to_beat::PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        reason_detail: None,
        normalized_outcome_label: Some("down".to_string()),
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1".to_string(),
        event_url: String::new(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2305.9),
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: Some(2304.55),
        current_price_source: "chainlink_live_data_ws",
        directional_gap: Some(1.35),
        gap_abs: Some(1.35),
        threshold_mode: "manual".to_string(),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: Some(2.5),
        threshold_value: 250.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 2.5,
        stop_loss_bump_count: 0,
        stop_loss_bump_applied_count: 0,
        stop_loss_bump_amount: None,
        stop_loss_bump_max_value: None,
        stop_loss_bump_unit: None,
        stop_loss_bump_raw_usd: 0.0,
        stop_loss_bump_usd: 0.0,
        stop_loss_bump_capped: false,
        stop_loss_bump_max_reached: false,
        stop_loss_bump_current_market_excluded: false,
        stop_loss_bump_increment_usd: 0.0,
        reentry_generation: 0,
        reentry_override_active: false,
        reentry_override_value: None,
        reentry_override_unit: None,
        max_price_relax: None,
        auto_threshold_usd: None,
        lookback_windows_used: None,
        current_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
        lookback_window_snapshots: None,
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        threshold_pct: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
        threshold_was_clamped: None,
        signal_formula: None,
        iv_mismatch_edge: None,
        early_stale_side: None,
        cex_direction_guard: None,
        entry_current_source_debug: None,
    };

    let snapshot = pair_lock_primary_ptb_evaluation_log_snapshot(
        715,
        "action_xy5g02",
        "eth-updown-5m-1776677100",
        "Down",
        &evaluation,
    );

    assert_eq!(snapshot.flow_run_id, 715);
    assert_eq!(snapshot.node_key, "action_xy5g02");
    assert_eq!(snapshot.market_slug, "eth-updown-5m-1776677100");
    assert_eq!(snapshot.outcome_label, "Down");
    assert!(!snapshot.ptb_passed);
    assert_eq!(
        snapshot.ptb_reason_code,
        "price_to_beat_gap_below_threshold"
    );
    assert_eq!(snapshot.directional_gap, Some(1.35));
    assert_eq!(snapshot.threshold_usd, 2.5);
    assert_eq!(snapshot.current_price, Some(2304.55));
    assert_eq!(snapshot.price_to_beat, Some(2305.9));
}

#[test]
fn pair_lock_primary_selection_prefers_relaxed_ptb_candidate() {
    let quote = PairLockResolvedQuote {
        best_bid: Some(0.69),
        best_ask: Some(0.70),
        last_trade_price: Some(0.70),
        current_price: 0.70,
        quote_source_kind: "test",
        quote_ws_state: "live_ws_not_subscribed",
        quote_event_ts: None,
        quote_snapshot_age_ms: None,
        quote_source_detail: "test".to_string(),
        quote_book_missing_fields: Vec::new(),
        quote_snapshot_used: Value::Null,
    };
    let selection = resolve_action_place_order_pair_lock_primary_selection_attempt(
        ActionPlaceOrderPairLockPrimaryCandidateEval {
            token_id: "yes".to_string(),
            outcome_label: "Up".to_string(),
            decision: "passed",
            reason_code: "passed".to_string(),
            quote: quote.clone(),
            diagnostics: json!({
                "decision": "passed",
                "reason_code": "passed",
                "outcome_label": "Up",
                "price_to_beat_guard": {
                    "passed": true,
                    "reason_code": "passed",
                    "max_price_relax": { "max_price_relax_applied": true }
                }
            }),
            adaptive_max_price_override: None,
            manual_adaptive_risk_override: None,
        },
        ActionPlaceOrderPairLockPrimaryCandidateEval {
            token_id: "no".to_string(),
            outcome_label: "Down".to_string(),
            decision: "waiting",
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            quote,
            diagnostics: json!({
                "decision": "waiting",
                "reason_code": "price_to_beat_gap_below_threshold",
                "outcome_label": "Down",
                "price_to_beat_guard": {
                    "passed": false,
                    "reason_code": "price_to_beat_gap_below_threshold"
                }
            }),
            adaptive_max_price_override: None,
            manual_adaptive_risk_override: None,
        },
    );

    assert!(!selection.waiting);
    assert_eq!(
        selection
            .selection
            .as_ref()
            .map(|value| value.token_id.as_str()),
        Some("yes")
    );
    assert_eq!(selection.failure_reason, None);
}

#[test]
fn pair_lock_primary_notification_reason_prefers_ptb_then_execution_floor_then_max_price() {
    let diagnostics = json!({
        "selection_mode": "auto_guarded",
        "yes_candidate_guard": {
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "outcome_label": "Up"
        },
        "no_candidate_guard": {
            "decision": "waiting",
            "reason_code": "price_to_beat_gap_below_threshold",
            "outcome_label": "Down"
        }
    });

    let reason = notification_selection::resolve_without_node(&diagnostics).expect("reason");
    assert_eq!(reason.scope, "price_to_beat");
    assert_eq!(reason.reason_code, "price_to_beat_gap_below_threshold");
    assert_eq!(
        reason
            .secondary_candidate
            .as_ref()
            .and_then(|value| value.get("reason_code"))
            .and_then(Value::as_str),
        Some("below_best_ask_floor")
    );
}

#[test]
fn pair_lock_primary_notification_reason_maps_execution_floor_before_max_price() {
    let diagnostics = json!({
        "selection_mode": "auto_guarded",
        "yes_candidate_guard": {
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "outcome_label": "Up"
        },
        "no_candidate_guard": {
            "decision": "waiting",
            "reason_code": "above_max_price",
            "outcome_label": "Down"
        }
    });

    let reason = notification_selection::resolve_without_node(&diagnostics).expect("reason");
    assert_eq!(reason.scope, "execution_floor");
    assert_eq!(reason.reason_code, "below_best_ask_floor");
}
