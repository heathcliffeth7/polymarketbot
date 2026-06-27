use super::current_price::CURRENT_PRICE_SOURCE_CHAINLINK;
use super::notification::{
    build_price_to_beat_bump_increased_notification_message,
    build_price_to_beat_bump_max_reached_notification_message,
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_recovered_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
    build_price_to_beat_relax_changed_notification_message,
    build_price_to_beat_relax_miss_notification_message,
};
use super::*;

fn default_guard_evaluation() -> PriceToBeatGuardEvaluation {
    PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: String::new(),
        reason_detail: None,
        normalized_outcome_label: None,
        direction: None,
        market_slug: String::new(),
        event_url: String::new(),
        timeframe: None,
        asset: None,
        price_to_beat: None,
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_mode: "manual".to_string(),
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: None,
        threshold_value: 0.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.0,
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
    }
}

fn entry_quality_iv(reason: &str) -> Value {
    entry_quality_iv_with_decision("skip", reason, "stale", true, 2_600)
}

fn cex_entry_source_debug(
    reason_code: &str,
    bybit_current: Option<f64>,
    bybit_gap: Option<f64>,
    bybit_hit: bool,
    binance_hit: bool,
    coinbase_hit: bool,
    bybit_error: Option<&str>,
) -> Value {
    let confirming_venues = [("binance", binance_hit), ("coinbase", coinbase_hit)]
        .into_iter()
        .filter_map(|(venue, hit)| hit.then_some(venue))
        .collect::<Vec<_>>();
    json!({
        "selected_entry_current_source": "chainlink_cex_consensus_or_hybrid",
        "hybrid_mode": "or",
        "entry_current_source_evaluations": [
            {
                "source": "chainlink",
                "passed": false,
                "reason_code": "blocked_insufficient_vol_samples",
                "current_price": 65.475
            },
            {
                "source": "cex_consensus_bybit_plus_one",
                "passed": false,
                "confirmed": false,
                "confirming_venues": confirming_venues,
                "reason_code": reason_code,
                "current_price": bybit_current,
                "bybit": {
                    "venue": "bybit",
                    "current_price": bybit_current,
                    "bid": bybit_current.map(|value| value - 0.01),
                    "ask": bybit_current.map(|value| value + 0.01),
                    "directional_gap": bybit_gap,
                    "threshold_hit": bybit_hit,
                    "book_staleness_ms": bybit_current.map(|_| 220),
                    "ticker_staleness_ms": bybit_current.map(|_| 180),
                    "error": bybit_error
                },
                "binance": {
                    "venue": "binance",
                    "current_price": 65.41,
                    "bid": 65.40,
                    "ask": 65.42,
                    "directional_gap": 0.035,
                    "threshold_hit": binance_hit,
                    "book_staleness_ms": 340,
                    "ticker_staleness_ms": 300
                },
                "coinbase": {
                    "venue": "coinbase",
                    "current_price": 65.39,
                    "bid": 65.38,
                    "ask": 65.40,
                    "directional_gap": 0.015,
                    "threshold_hit": coinbase_hit,
                    "book_staleness_ms": 410,
                    "ticker_staleness_ms": 390
                }
            }
        ]
    })
}

fn remove_eq77_risk_cap_debug_fields(mut value: Value) -> Value {
    if let Some(debug) = value
        .pointer_mut("/entry_quality_debug")
        .and_then(Value::as_object_mut)
    {
        for key in [
            "allowed",
            "primary_reason",
            "entry_action",
            "hard_block",
            "deferred",
            "signal_recheck_required",
            "risk_cap_price_cent",
            "ask_over_cap_cent",
            "risk_score",
            "cap_haircut_cent",
            "risk_level",
            "lane",
            "size_multiplier",
            "effective_max_buy_price",
            "fair_probability",
            "fee_buffer",
            "min_edge",
            "premium_ev_margin_cent",
            "risk_components",
            "cap_components",
        ] {
            debug.remove(key);
        }
    }
    value
}

fn entry_quality_iv_with_decision(
    decision: &str,
    reason: &str,
    cex_status: &str,
    cex_blocking: bool,
    chainlink_age_ms: i64,
) -> Value {
    let cex_reason_code = match cex_status {
        "aligned" => "cex_direction_guard_passed",
        "opposite" => "cex_direction_guard_opposite",
        "neutral" => "cex_direction_guard_neutral",
        "unconfirmed" => "cex_direction_guard_unconfirmed",
        "stale" => "cex_direction_guard_unavailable",
        _ => "cex_direction_guard_unavailable",
    };
    let mut value = json!({
        "entry_quality_debug": {
            "decision": decision,
            "reason": reason,
            "ptb_gate": {
                "passed": true,
                "gapUsd": 10.8,
                "requiredGapUsd": 10.0,
            },
            "iv_edge": {
                "passed": decision == "allow",
                "edge": 0.12,
                "requiredEdge": 0.08,
                "adjustedMargin": 0.04,
                "gapStrength": 0.91,
                "requiredGapStrength": 0.75,
                "gapStrengthMargin": 0.16,
                "matchedRule": "30-15",
            },
            "cex_direction_guard": {
                "enabled": true,
                "mode": "bybit_plus_one",
                "status": cex_status,
                "blocking": cex_blocking,
                "reasonCode": cex_reason_code,
            },
            "source": {
                "ptbCurrentPriceSource": "chainlink",
                "chainlinkAgeMs": chainlink_age_ms,
            },
            "allowed": decision == "allow",
            "primary_reason": reason,
            "entry_action": "wait_for_price",
            "hard_block": false,
            "deferred": true,
            "signal_recheck_required": true,
            "risk_cap_price_cent": 70.0,
            "ask_over_cap_cent": 5.0,
            "risk_score": 48.0,
            "cap_haircut_cent": 6.0,
            "risk_level": "high",
            "lane": "high",
            "size_multiplier": 0.5,
            "effective_max_buy_price": 70.0,
            "fair_probability": 78.2,
            "fee_buffer": 1.0,
            "min_edge": 1.0,
            "premium_ev_margin_cent": 8.2,
            "risk_components": [
                {"name": "impulse_ratio_strong", "risk_points": 18.0, "haircut_cent": 2.0},
                {"name": "cex_chainlink_conflict", "risk_points": 10.0, "haircut_cent": 2.0},
                {"name": "stop_cushion_weak", "risk_points": 20.0, "haircut_cent": 2.0},
                {"name": "borderline_gap_margin", "risk_points": 8.0, "haircut_cent": 1.0},
                {"name": "missing_same_side_history_5s", "risk_points": 8.0},
                {"name": "missing_same_side_history_10s", "risk_points": 4.0},
                {"name": "odds_spread_low_confidence", "risk_points": 8.0}
            ],
            "cap_components": [
                {"name": "lane_max", "cap_cent": 70.0},
                {"name": "risk_haircut", "haircut_cent": 6.0},
                {"name": "risk_cap", "cap_cent": 70.0},
                {"name": "ev_cap", "cap_cent": 76.2}
            ],
        },
    });
    if let Some(debug) = value
        .pointer_mut("/entry_quality_debug")
        .and_then(Value::as_object_mut)
    {
        debug.insert("eq77_lite_profile".to_string(), json!("lite_v1"));
        debug.insert("gap_strength_required".to_string(), json!(1.45));
        debug.insert("gap_strength_required_with_margin".to_string(), json!(1.48));
        debug.insert("gap_strength_hard_floor".to_string(), json!(0.75));
        debug.insert("gap_strength_deficit".to_string(), json!(0.68));
        debug.insert("gap_strength_soft_low_ratio".to_string(), json!(0.93));
        debug.insert("gap_soft_low_risk_points".to_string(), json!(30.0));
        debug.insert("gap_strength_soft_low".to_string(), json!(true));
    }
    value
}

#[test]
fn blocked_notification_includes_detailed_ptb_summary_when_metadata_present() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        direction: Some("up".to_string()),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2366.97),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        current_price: Some(2367.12),
        directional_gap: Some(0.15),
        gap_abs: Some(0.15),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: Some(80.0),
        base_threshold_unit: Some("cent".to_string()),
        base_threshold_usd: Some(0.8),
        threshold_value: 90.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 0.9,
        stop_loss_bump_count: 2,
        stop_loss_bump_applied_count: 1,
        stop_loss_bump_amount: Some(10.0),
        stop_loss_bump_max_value: Some(30.0),
        stop_loss_bump_unit: Some("cent".to_string()),
        stop_loss_bump_usd: 0.1,
        stop_loss_bump_capped: true,
        stop_loss_bump_current_market_excluded: true,
        reentry_generation: 1,
        reentry_override_active: true,
        reentry_override_value: Some(5.0),
        reentry_override_unit: Some("cent".to_string()),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 90.00000000 cent (~0.90000000 USD)"));
    assert!(message.contains("Base PTB: 80.00000000 cent (~0.80000000 USD)"));
    assert!(message.contains("Re-entry Override: 5.00000000 cent (~0.05000000 USD)"));
    assert!(message.contains("SL Bump: kademe 10.00000000 cent"));
    assert!(message.contains("uygulanan 0.10000000 USD"));
    assert!(message.contains("bu market dislandi"));
}

#[test]
fn blocked_ptb_notification_includes_iv_entry_quality_debug_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_direction_stale".to_string(),
        direction: Some("up".to_string()),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(100.0),
        price_to_beat_status: Some("ok".to_string()),
        price_to_beat_source: Some("chainlink".to_string()),
        current_price: Some(110.8),
        directional_gap: Some(10.8),
        gap_abs: Some(10.8),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(entry_quality_iv("cex_direction_stale")),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("IV Entry Quality:"));
    assert!(message.contains("Decision: skip"));
    assert!(message.contains("Reason: cex_direction_stale"));
    assert!(message.contains("PTB Gate: passed=true gap=10.80000000 required=10.00000000"));
    assert!(message
        .contains("IV Edge: passed=false edge=0.12000000 required=0.08000000 margin=0.04000000"));
    assert!(message.contains(
        "Gap Strength: value=0.91000000 required=0.75000000 margin=0.16000000 rule=30-15"
    ));
    assert!(message.contains(
        "CEX Direction Guard: enabled=true mode=bybit_plus_one status=stale blocking=true reason=cex_direction_guard_unavailable"
    ));
    assert!(message.contains("Source: chainlink age_ms=2600"));
    assert!(message.contains("EQ77 Risk Cap:"));
    assert!(message.contains(
        "Action: entry_action=wait_for_price allowed=false hard_block=false deferred=true recheck=true"
    ));
    assert!(message.contains("Risk: score=48.00000000 level=high lane=high size=0.50000000"));
    assert!(message.contains(
        "Gap Lite: profile=lite_v1 required=1.45000000 with_margin=1.48000000 floor=0.75000000 deficit=0.68000000 soft_low=true ratio=0.93000000 points=30.00000000"
    ));
    assert!(message.contains(
        "Cap: risk=70.00000000 effective=70.00000000 haircut=6.00000000 ask_over=5.00000000"
    ));
    assert!(message.contains(
        "EV: fair=78.20000000 fee_buffer=1.00000000 min_edge=1.00000000 margin=8.20000000"
    ));
    assert!(message.contains(
        "Risk Components: impulse_ratio_strong(+18.00pt/-2.00c), cex_chainlink_conflict(+10.00pt/-2.00c), stop_cushion_weak(+20.00pt/-2.00c), borderline_gap_margin(+8.00pt/-1.00c), missing_same_side_history_5s(+8.00pt), missing_same_side_history_10s(+4.00pt), (+1 more)"
    ));
    assert!(message.contains(
        "Cap Components: lane_max=70.00c, risk_haircut=-6.00c, risk_cap=70.00c, ev_cap=76.20c"
    ));
}

#[test]
fn blocked_notification_explains_cex_entry_bybit_below_threshold() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_consensus_bybit_below_threshold".to_string(),
        reason_detail: Some(
            "market_slug=sol-updown-5m-1776200100; cex_entry_consensus requires bybit threshold hit plus binance or coinbase confirmation".to_string(),
        ),
        market_slug: "sol-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/sol-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 0.05,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.05,
        entry_current_source_debug: Some(cex_entry_source_debug(
            "cex_consensus_bybit_below_threshold",
            Some(65.42),
            Some(0.03),
            false,
            true,
            false,
            None,
        )),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("Entry Current Source:"));
    assert!(message.contains(
        "CEX Consensus: passed=false confirmed=false reason=cex_consensus_bybit_below_threshold confirming=binance"
    ));
    assert!(message.contains(
        "Bybit Threshold: bybit_gap=0.03000000 required_threshold_usd=0.05000000 margin=-0.02000000 hit=false"
    ));
    assert!(message.contains(
        "bybit: current=65.42000000 bid=65.41000000 ask=65.43000000 gap=0.03000000 hit=false book_age_ms=220 ticker_age_ms=180 error=N/A"
    ));
    assert!(message.contains("binance: current=65.41000000"));
    assert!(message.contains("coinbase: current=65.39000000"));
}

#[test]
fn blocked_notification_explains_cex_entry_unconfirmed() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_consensus_unconfirmed".to_string(),
        market_slug: "sol-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/sol-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 0.05,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.05,
        entry_current_source_debug: Some(cex_entry_source_debug(
            "cex_consensus_unconfirmed",
            Some(65.48),
            Some(0.08),
            true,
            false,
            false,
            None,
        )),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains(
        "CEX Consensus: passed=false confirmed=false reason=cex_consensus_unconfirmed confirming=N/A"
    ));
    assert!(message.contains(
        "Bybit Threshold: bybit_gap=0.08000000 required_threshold_usd=0.05000000 margin=0.03000000 hit=true"
    ));
    assert!(message.contains("binance: current=65.41000000"));
    assert!(message.contains("coinbase: current=65.39000000"));
}

#[test]
fn blocked_notification_explains_cex_entry_bybit_unavailable() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_consensus_bybit_unavailable".to_string(),
        market_slug: "sol-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/sol-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 0.05,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.05,
        entry_current_source_debug: Some(cex_entry_source_debug(
            "cex_consensus_bybit_unavailable",
            None,
            None,
            false,
            true,
            false,
            Some("no current CEX book for sol on bybit"),
        )),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains(
        "CEX Consensus: passed=false confirmed=false reason=cex_consensus_bybit_unavailable confirming=binance"
    ));
    assert!(message.contains(
        "bybit: current=N/A bid=N/A ask=N/A gap=N/A hit=false book_age_ms=N/A ticker_age_ms=N/A error=no current CEX book for sol on bybit"
    ));
}

#[test]
fn blocked_notification_explains_not_evaluated_cex_direction_guard() {
    let mut iv = entry_quality_iv_with_decision(
        "skip",
        "cex_consensus_bybit_below_threshold",
        "not_evaluated",
        false,
        500,
    );
    if let Some(cex) = iv
        .pointer_mut("/entry_quality_debug/cex_direction_guard")
        .and_then(Value::as_object_mut)
    {
        cex.insert(
            "reasonCode".to_string(),
            json!("skipped_price_to_beat_not_passed"),
        );
    }
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_consensus_bybit_below_threshold".to_string(),
        market_slug: "sol-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/sol-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 0.05,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.05,
        iv_mismatch_edge: Some(iv),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains(
        "CEX Direction Guard: enabled=true mode=bybit_plus_one status=not_evaluated blocking=false reason=skipped_price_to_beat_not_passed"
    ));
}

#[test]
fn recovered_ptb_notification_includes_iv_entry_quality_debug_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: true,
        reason_code: "selected_edge_passed".to_string(),
        direction: Some("up".to_string()),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(100.0),
        price_to_beat_status: Some("ok".to_string()),
        price_to_beat_source: Some("chainlink".to_string()),
        current_price: Some(110.8),
        directional_gap: Some(10.8),
        gap_abs: Some(10.8),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(entry_quality_iv_with_decision(
            "allow",
            "selected_edge_passed",
            "aligned",
            false,
            500,
        )),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_recovered_notification_message(
        &evaluation,
        "iv_edge_below_threshold",
    );

    assert!(message.contains("Price to Beat Korumasi Gecti"));
    assert!(message.contains("IV Entry Quality:"));
    assert!(message.contains("Decision: allow"));
    assert!(message.contains("Reason: selected_edge_passed"));
    assert!(message.contains(
        "CEX Direction Guard: enabled=true mode=bybit_plus_one status=aligned blocking=false reason=cex_direction_guard_passed"
    ));
    assert!(message.contains("Source: chainlink age_ms=500"));
    assert!(message.contains("EQ77 Risk Cap:"));
    assert!(message.contains(
        "Action: entry_action=wait_for_price allowed=true hard_block=false deferred=true recheck=true"
    ));
    assert!(message.contains("Risk: score=48.00000000 level=high lane=high size=0.50000000"));
}

#[test]
fn ptb_notification_omits_iv_entry_quality_when_debug_missing() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "iv_edge_below_threshold".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "edge_adj": 0.03,
            "dynamic_threshold": 0.08,
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(!message.contains("IV Entry Quality:"));
    assert!(!message.contains("PTB Gate:"));
    assert!(!message.contains("Gap Strength:"));
}

#[test]
fn blocked_ptb_notification_includes_execution_vwap_guard_summary() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "iv_edge_below_threshold".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "ask": 0.60,
            "depth_guard_result": "pass",
            "depth_best_ask": 0.60,
            "depth_book_best_ask": 0.60,
            "estimated_avg_fill": 0.712,
            "vwap_slippage": 0.112,
            "intended_qty": 8.33,
            "available_qty_at_best_ask": 1.0,
            "depth_levels_used": 4,
            "execution_vwap_guard_enabled": true,
            "time_rule_price_blocked": false,
            "model_ask_cent": 60.0,
            "execution_best_ask_cent": 60.0,
            "execution_vwap_cent": 71.2,
            "expected_vwap_cent": 71.2,
            "submit_limit_price_cent": 73.2,
            "execution_limit_by_vwap_action": "clamp",
            "execution_vwap_qty_requested": 8.33,
            "execution_vwap_qty_available": 8.33,
            "execution_vwap_depth_coverage_ratio": 1.0,
            "execution_vwap_levels_used": 4,
            "execution_vwap_book_depth_ok": true,
            "execution_cost_source": "execution_vwap",
            "execution_vwap_edge_margin": 1.8,
            "effective_max_price": 0.73,
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("Execution VWAP Guard:"));
    assert!(
        message.contains("Model Ask: 60.00c | Execution Best Ask: 60.00c | Execution VWAP: 71.20c")
    );
    assert!(message.contains(
        "Execution vs Model Ask: 11.20c | Effective Max: 73.00c | VWAP Edge Margin: 1.80c"
    ));
    assert!(message.contains(
        "VWAP Size: 8.33000000 | VWAP Levels: 4.00000000 | VWAP Coverage: 1.00000000 | Cost Source: execution_vwap"
    ));
    assert!(message.contains("Expected VWAP: 71.20c | Submit Limit: 73.20c | Limit Action: clamp"));
}

#[test]
fn blocked_ptb_notification_includes_depth_unavailable_diagnostics() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "blocked_depth_guard_unavailable".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "ask": 0.57,
            "depth_guard_result": "unavailable",
            "depth_guard_reason": "blocked_depth_guard_unavailable",
            "depth_unavailable_reason": "intended_qty_unavailable",
            "depth_sizing_missing_reason": "size_usdc_unavailable",
            "depth_sizing_source": "none",
            "depth_order_book_fetch_status": "ok",
            "depth_order_book_asks_len": 8,
            "depth_order_book_bids_len": 5,
            "depth_valid_asks_len": 8,
            "book_confirmation_missing_reason": "opposite_quote_missing",
            "depth_selected_outcome_label": "Down",
            "depth_opposite_outcome_label": "Up",
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains(
        "Depth Diagnostics: reason=intended_qty_unavailable sizing=size_usdc_unavailable source=none book=ok asks=8 bids=5 valid_asks=8"
    ));
    assert!(message.contains(
        "Book Confirmation: missing reason=opposite_quote_missing selected=Down opposite=Up"
    ));
}

#[test]
fn blocked_ptb_notification_omits_not_requested_book_confirmation_diagnostic() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "blocked_depth_guard_unavailable".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "ask": 0.57,
            "depth_guard_result": "pass",
            "depth_unavailable_reason": null,
            "book_confirmation_missing_reason": "not_requested_protection_off",
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(!message.contains("Book Confirmation: missing"));
    assert!(!message.contains("not_requested_protection_off"));
}

#[test]
fn blocked_ptb_notification_includes_cex_open_gap_summary() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "blocked_chainlink_cex_book_mismatch".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "cex_open_gap_enabled": true,
            "cex_open_gap_consensus": "mixed",
            "cex_open_gap_clean_lane": false,
            "cex_consensus_q_cap_applied": true,
            "binance_5m_open": 60750.0,
            "binance_current_mid": 60768.0,
            "binance_signed_gap": 18.0,
            "binance_gap_z": 0.72,
            "binance_state": "supporting",
            "bybit_5m_open": 60750.0,
            "bybit_current_mid": 60752.4,
            "bybit_signed_gap": 2.4,
            "bybit_gap_z": 0.10,
            "bybit_state": "weak_positive",
            "chainlink_signed_gap": 30.8,
            "conservative_cex_gap": 2.4,
            "effective_consensus_gap_usd": 2.4,
            "chainlink_cex_diff_usd": 28.4,
            "chainlink_cex_diff_z": 1.18,
            "chainlink_cex_diff_bps": 4.67,
            "q_final_before_cex_consensus": 0.972,
            "q_final_after_cex_consensus": 0.824,
            "cex_magnitude_guard_enabled": true,
            "cex_magnitude_ratio": 0.43720190779014306,
            "cex_magnitude_consensus": "shallow",
            "cex_magnitude_clean_lane": false,
            "cex_magnitude_required_gap_usd": 0.1258,
            "cex_magnitude_block_reason": "blocked_gap_fail_shallow_cex_support",
            "gap_strength": 0.8635,
            "required_gap_strength": 2.0,
            "gap_fail": true,
            "eq77_gap_override_requested": true,
            "eq77_gap_override_effective": false,
            "eq77_gap_override_suppressed_by_cex_magnitude": true,
            "override_blocked_by": "shallow_cex_magnitude",
            "chainlink_cex_book_mismatch_reason": "blocked_chainlink_cex_book_mismatch",
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("CEX Open Gap:"));
    assert!(message.contains("Consensus: mixed | clean=false | cap=true"));
    assert!(message.contains(
        "Binance: open=60750.00000000 current=60768.00000000 gap=18.00000000 z=0.72000000 state=supporting"
    ));
    assert!(message.contains(
        "Bybit: open=60750.00000000 current=60752.40000000 gap=2.40000000 z=0.10000000 state=weak_positive"
    ));
    assert!(message.contains(
        "Chainlink/CEX: chainlink=30.80000000 conservative=2.40000000 effective=2.40000000 diff=28.40000000 z=1.18000000 bps=4.67000000"
    ));
    assert!(message.contains("q consensus: before=97.20c after=82.40c"));
    assert!(message.contains("CEX Magnitude:"));
    assert!(message.contains(
        "ratio=0.43720191 classification=shallow conservative=2.40000000 required=0.12580000 clean=false reason=blocked_gap_fail_shallow_cex_support"
    ));
    assert!(message.contains("Gap Gate: gap=0.86350000 required=2.00000000 fail=true"));
    assert!(message.contains(
        "EQ77 override: requested=true effective=false suppressed=true blockedBy=shallow_cex_magnitude"
    ));
    assert!(message.contains("Reason: blocked_gap_fail_shallow_cex_support"));
}

#[test]
fn ptb_notification_omits_cex_open_gap_summary_when_telemetry_missing() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "iv_edge_below_threshold".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "edge_adj": 0.03,
            "dynamic_threshold": 0.08,
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(!message.contains("CEX Open Gap:"));
}

#[test]
fn blocked_ptb_notification_includes_oracle_lag_book_lead_summary() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "blocked_oracle_lag_book_lead".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(json!({
            "oracle_lag_book_lead_guard_enabled": true,
            "oracle_lag_suspicion": "HIGH",
            "oracle_lag_action": "BLOCK",
            "oracle_lag_block_reason": "blocked_oracle_lag_book_lead",
            "oracle_lag_book_reference_status": "reliable",
            "oracle_lag_book_reference_age_ms": 420,
            "oracle_lag_book_depth_coverage_ratio": 1.0,
            "q_final_cent": 99.56,
            "execution_vwap_cent": 64.0,
            "execution_ref_cent": 64.0,
            "execution_ref_source": "execution_vwap",
            "model_book_dislocation_cent": 35.56,
        })),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("Oracle/Book Lead: suspicion=HIGH action=BLOCK"));
    assert!(message
        .contains("q_final=99.56c execution_ref=64.00c source=execution_vwap dislocation=35.56c"));
    assert!(message.contains("Book ref: reliable age=420ms coverage=1.00000000"));
    assert!(message.contains("Reason=blocked_oracle_lag_book_lead"));
}

#[test]
fn ptb_notification_omits_eq77_risk_cap_when_debug_has_no_risk_fields() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "cex_direction_stale".to_string(),
        market_slug: "btc-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 10.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 10.0,
        iv_mismatch_edge: Some(remove_eq77_risk_cap_debug_fields(entry_quality_iv(
            "cex_direction_stale",
        ))),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains("IV Entry Quality:"));
    assert!(!message.contains("EQ77 Risk Cap:"));
    assert!(!message.contains("Risk Components:"));
    assert!(!message.contains("Cap Components:"));
}

#[test]
fn entry_quality_debug_reason_codes_are_visible_in_ptb_notification() {
    for reason in [
        "cex_direction_stale",
        "chainlink_provider_stale_global",
        "chainlink_provider_stale_entry_quality",
        "entry_spread_too_wide",
        "iv_edge_below_threshold",
    ] {
        let evaluation = PriceToBeatGuardEvaluation {
            reason_code: reason.to_string(),
            market_slug: "btc-updown-5m-1776200100".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1776200100".to_string(),
            threshold_mode: "iv_mismatch_edge".to_string(),
            configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
            threshold_value: 10.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 10.0,
            iv_mismatch_edge: Some(entry_quality_iv(reason)),
            ..default_guard_evaluation()
        };

        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

        assert!(message.contains(&format!("Reason: {reason}")));
    }
}

#[test]
fn insufficient_vol_samples_notification_explains_not_computed_fields() {
    let mut iv = entry_quality_iv("blocked_insufficient_vol_samples");
    if let Some(obj) = iv.as_object_mut() {
        obj.insert(
            "decision_reason".to_string(),
            json!("blocked_insufficient_vol_samples"),
        );
        obj.insert(
            "vol_sample_status".to_string(),
            json!("insufficient_samples"),
        );
        obj.insert("sample_count".to_string(), json!(3));
        obj.insert("min_vol_samples".to_string(), json!(8));
        obj.insert("delta_count".to_string(), Value::Null);
        obj.insert("depth_guard_result".to_string(), json!("off"));
        obj.insert("execution_vwap_guard_enabled".to_string(), json!(false));
    }
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "blocked_insufficient_vol_samples".to_string(),
        market_slug: "sol-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/sol-updown-5m-1776200100".to_string(),
        threshold_mode: "iv_mismatch_edge".to_string(),
        configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
        threshold_value: 0.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.0,
        iv_mismatch_edge: Some(iv),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);

    assert!(message.contains(
        "IV Model Readiness: blocked before q/edge; status=insufficient_samples samples=3.00000000 min=8.00000000 deltas=N/A"
    ));
    assert!(message.contains("q_final/cost/edge not computed"));
    assert!(message.contains("IV Edge: not computed required=0.08000000 margin=N/A"));
    assert!(message.contains(
        "Gap Summary: PTB passed=true gap_strength actual=0.91000000 required=0.75000000 margin=0.16000000 rule=30-15"
    ));
    assert!(message.contains("Execution Ref: not requested (depth/vwap guard off)"));
}

#[test]
fn blocked_notification_hides_sl_bump_when_effective_ptb_matches_base() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: Some(100.0),
        base_threshold_unit: Some("cent".to_string()),
        base_threshold_usd: Some(1.0),
        current_effective_ptb_usd: Some(1.0),
        threshold_value: 100.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 1.0,
        stop_loss_bump_count: 7,
        stop_loss_bump_applied_count: 7,
        stop_loss_bump_amount: Some(25.0),
        stop_loss_bump_max_value: Some(300.0),
        stop_loss_bump_unit: Some("cent".to_string()),
        stop_loss_bump_usd: 1.75,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Efektif PTB: 100.00000000 cent (~1.00000000 USD)"));
    assert!(message.contains("Base PTB: 100.00000000 cent (~1.00000000 USD)"));
    assert!(!message.contains("SL Bump:"));
}

#[test]
fn blocked_notification_shows_sl_bump_when_auto_threshold_is_lifted() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        configured_threshold_mode: Some("auto_vol_pct".to_string()),
        threshold_mode: "auto_vol_pct".to_string(),
        auto_threshold_usd: Some(1.2),
        current_effective_ptb_usd: Some(1.5),
        threshold_value: 150.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 1.5,
        stop_loss_bump_count: 2,
        stop_loss_bump_applied_count: 2,
        stop_loss_bump_amount: Some(15.0),
        stop_loss_bump_max_value: Some(200.0),
        stop_loss_bump_unit: Some("cent".to_string()),
        stop_loss_bump_usd: 0.3,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Auto Threshold: 1.20000000 USD"));
    assert!(message.contains("SL Bump: kademe 15.00000000 cent"));
}

#[test]
fn blocked_notification_uses_reentry_adjusted_base_when_deciding_sl_bump_visibility() {
    let hidden_message =
        build_price_to_beat_guard_blocked_notification_message(&PriceToBeatGuardEvaluation {
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            market_slug: "eth-updown-5m-1776200100".to_string(),
            event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
            configured_threshold_mode: Some("manual".to_string()),
            base_threshold_value: Some(50.0),
            base_threshold_unit: Some("cent".to_string()),
            base_threshold_usd: Some(0.5),
            current_effective_ptb_usd: Some(0.5),
            threshold_value: 50.0,
            threshold_unit: "cent".to_string(),
            threshold_usd: 0.5,
            reentry_override_active: true,
            reentry_override_value: Some(50.0),
            reentry_override_unit: Some("cent".to_string()),
            stop_loss_bump_count: 3,
            stop_loss_bump_applied_count: 3,
            stop_loss_bump_amount: Some(10.0),
            stop_loss_bump_unit: Some("cent".to_string()),
            stop_loss_bump_usd: 0.3,
            ..default_guard_evaluation()
        });
    assert!(!hidden_message.contains("SL Bump:"));

    let visible_message =
        build_price_to_beat_guard_blocked_notification_message(&PriceToBeatGuardEvaluation {
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            market_slug: "eth-updown-5m-1776200100".to_string(),
            event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
            configured_threshold_mode: Some("manual".to_string()),
            base_threshold_value: Some(50.0),
            base_threshold_unit: Some("cent".to_string()),
            base_threshold_usd: Some(0.5),
            current_effective_ptb_usd: Some(0.75),
            threshold_value: 75.0,
            threshold_unit: "cent".to_string(),
            threshold_usd: 0.75,
            reentry_override_active: true,
            reentry_override_value: Some(50.0),
            reentry_override_unit: Some("cent".to_string()),
            stop_loss_bump_count: 3,
            stop_loss_bump_applied_count: 3,
            stop_loss_bump_amount: Some(10.0),
            stop_loss_bump_unit: Some("cent".to_string()),
            stop_loss_bump_usd: 0.25,
            ..default_guard_evaluation()
        });
    assert!(visible_message.contains("Re-entry Override: 50.00000000 cent (~0.50000000 USD)"));
    assert!(visible_message.contains("SL Bump: kademe 10.00000000 cent"));
}

#[test]
fn blocked_notification_omits_optional_ptb_summary_lines_when_metadata_missing() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_pending".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 20.00000000 USD"));
    assert!(!message.contains("Base PTB:"));
    assert!(!message.contains("Auto Threshold:"));
    assert!(!message.contains("Re-entry Override:"));
    assert!(!message.contains("SL Bump:"));
}

#[test]
fn waiting_notification_includes_auto_threshold_summary() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_pending".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        configured_threshold_mode: Some("auto_vol_pct".to_string()),
        threshold_mode: "auto_vol_pct".to_string(),
        threshold_value: 1.2,
        threshold_unit: "usd".to_string(),
        threshold_usd: 1.2,
        auto_threshold_usd: Some(1.2),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Durum: Bekleme moduna alindi"));
    assert!(message.contains("Configured Mod: auto_vol_pct"));
    assert!(message.contains("Efektif PTB: 1.20000000 USD"));
    assert!(message.contains("Auto Threshold: 1.20000000 USD"));
}

#[test]
fn recovered_notification_includes_ptb_summary_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: true,
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2366.97),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        current_price: Some(2365.50),
        directional_gap: Some(1.47),
        gap_abs: Some(1.47),
        configured_threshold_mode: Some("auto_last_3_avg_excursion".to_string()),
        threshold_mode: "auto_last_3_avg_excursion".to_string(),
        threshold_value: 1.47,
        threshold_unit: "usd".to_string(),
        threshold_usd: 1.47,
        auto_threshold_usd: Some(1.47),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_recovered_notification_message(
        &evaluation,
        "price_to_beat_gap_below_threshold",
    );
    assert!(message.contains("Price to Beat Korumasi Gecti"));
    assert!(message.contains("Configured Mod: auto_last_3_avg_excursion"));
    assert!(message.contains("Efektif PTB: 1.47000000 USD"));
    assert!(message.contains("Auto Threshold: 1.47000000 USD"));
}

#[test]
fn relax_notification_keeps_special_fields_and_adds_shared_summary_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776198300".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: Some(100.0),
        base_threshold_unit: Some("cent".to_string()),
        base_threshold_usd: Some(1.0),
        threshold_value: 1.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 1.0,
        auto_threshold_usd: Some(1.0),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_relax_changed_notification_message(
        &evaluation,
        Some(1.0),
        Some(0.67328696),
        1.0,
        Some(0.42328696),
        0.25,
        1.0,
        11,
        &["eth-updown-5m-1776198000".to_string()],
    );

    assert!(message.contains("Onceki Bildirilen Relax PTB: 1.00000000"));
    assert!(message.contains("Ham Relax PTB: 0.67328696"));
    assert!(message.contains("Bu Market Efektif Relax PTB: 1.00000000"));
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 1.00000000 USD"));
    assert!(message.contains("Base PTB: 100.00000000 cent (~1.00000000 USD)"));
    assert!(message.contains("Auto Threshold: 1.00000000 USD"));
}

#[test]
fn relax_miss_notification_reports_streak_and_status() {
    let inactive_evaluation = PriceToBeatGuardEvaluation {
        direction: Some("up".to_string()),
        market_slug: "eth-updown-5m-1776198600".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        configured_threshold_mode: Some("manual".to_string()),
        threshold_value: 100.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 1.0,
        ..default_guard_evaluation()
    };
    let inactive_message = build_price_to_beat_relax_miss_notification_message(
        &inactive_evaluation,
        None,
        1,
        Some("eth-updown-5m-1776198300"),
        3,
        Some(0.75),
        5,
        false,
        None,
    );
    assert!(inactive_message.contains("Missed Market: eth-updown-5m-1776198300"));
    assert!(inactive_message.contains("Onceki Bildirilen Miss Streak: N/A"));
    assert!(inactive_message.contains("Yeni Miss Streak: 1"));
    assert!(inactive_message.contains("Missed Tradable Seconds: 3"));
    assert!(inactive_message.contains("Missed Max Fillability: 0.75000000"));
    assert!(inactive_message.contains("Configured Miss Count: 5"));
    assert!(inactive_message.contains("Relax Durumu: threshold henuz gevsemedi"));
    assert!(inactive_message.contains("Configured Mod: manual"));

    let active_evaluation = PriceToBeatGuardEvaluation {
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776198900".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        configured_threshold_mode: Some("manual".to_string()),
        threshold_value: 1.5,
        threshold_unit: "usd".to_string(),
        threshold_usd: 1.5,
        ..default_guard_evaluation()
    };
    let active_message = build_price_to_beat_relax_miss_notification_message(
        &active_evaluation,
        Some(5),
        6,
        Some("eth-updown-5m-1776198600"),
        4,
        Some(1.0),
        5,
        true,
        Some(1.5),
    );
    assert!(active_message.contains("Missed Market: eth-updown-5m-1776198600"));
    assert!(active_message.contains("Onceki Bildirilen Miss Streak: 5"));
    assert!(active_message.contains("Yeni Miss Streak: 6"));
    assert!(active_message.contains("Missed Tradable Seconds: 4"));
    assert!(active_message.contains("Missed Max Fillability: 1.00000000"));
    assert!(active_message.contains("Relax Durumu: aktif"));
    assert!(active_message.contains("Guncel Efektif Relax PTB: 1.50000000"));
    assert!(active_message.contains("Efektif PTB: 1.50000000 USD"));
}

#[test]
fn bump_increased_notification_reports_relaxed_threshold_delta_without_reseeding_base_plus_bump() {
    let message = build_price_to_beat_bump_increased_notification_message(
        "eth-updown-5m-1776678300",
        25.0,
        "cent",
        6,
        1.25,
        1.50,
        Some(100.0),
        Some("cent"),
        Some(1.0),
        Some(125.0),
        Some("cent"),
        Some(1.25),
    );

    assert!(message.contains("Uygulanan Toplam Artis: 1.25000000 USD -> 1.50000000 USD"));
    assert!(message.contains(
        "Efektif PTB: 100.00000000 cent (~1.00000000 USD) -> 125.00000000 cent (~1.25000000 USD)"
    ));
    assert!(message.contains("Guncel PTB: 125.00000000 cent (~1.25000000 USD)"));
}

#[test]
fn bump_max_reached_notification_formats_current_ptb_and_na() {
    let cent_message = build_price_to_beat_bump_max_reached_notification_message(
        "eth-updown-5m-1776200100",
        0.75,
        0.50,
        50.0,
        "cent",
        Some(150.0),
        Some("cent"),
        Some(1.5),
    );
    assert!(cent_message.contains("Guncel PTB: 150.00000000 cent (~1.50000000 USD)"));

    let na_message = build_price_to_beat_bump_max_reached_notification_message(
        "eth-updown-5m-1776200100",
        0.75,
        0.50,
        50.0,
        "cent",
        None,
        None,
        None,
    );
    assert!(na_message.contains("Guncel PTB: Bilinmiyor"));
}
