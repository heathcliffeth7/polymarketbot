from evaluate_shadow_outcomes import (
    classify_shadow_outcome,
    classify_trade_pnl_outcome,
    optimize_shadow_pnl_thresholds,
    optimize_trade_pnl_thresholds,
    simulate_shadow_pnl_5usdc,
    summarize_outcomes,
)


def test_allow_like_good_quality_low_risk_is_correct():
    assert classify_shadow_outcome("allow_like", 1, 0) == "correct"


def test_allow_like_stop_loss_risk_is_wrong():
    assert classify_shadow_outcome("allow_like", 1, 1) == "wrong"


def test_avoid_like_bad_quality_is_correct():
    assert classify_shadow_outcome("avoid_like", 0, 0) == "correct"


def test_caution_is_neutral_and_excluded_from_accuracy():
    assert classify_shadow_outcome("caution", 1, 0) == "neutral_caution"
    metrics = summarize_outcomes(
        [
            {
                "score_source": "live_shadow",
                "counterfactual_correctness": "neutral_caution",
                "ml_shadow_decision": "caution",
                "trade_pnl_correctness": "neutral_caution",
            }
        ]
    )

    assert metrics["live_shadow"]["resolved"] == 1
    assert metrics["live_shadow"]["caution"] == 1
    assert metrics["live_shadow"]["allow_total"] == 0
    assert metrics["live_shadow"]["avoid_total"] == 0


def test_unresolved_market_is_unresolved():
    assert classify_shadow_outcome("allow_like", None, None) == "unresolved"


def test_replay_rows_are_kept_separate_from_live_shadow():
    metrics = summarize_outcomes(
        [
            {
                "score_source": "current_model_replay",
                "counterfactual_correctness": "correct",
                "ml_shadow_decision": "avoid_like",
                "trade_pnl_correctness": "no_actual_trade",
            },
            {
                "score_source": "live_shadow",
                "counterfactual_correctness": "correct",
                "ml_shadow_decision": "allow_like",
                "trade_pnl_correctness": "no_actual_trade",
            },
        ]
    )

    assert metrics["live_shadow"]["allow_correct"] == 1
    assert metrics["replay_current_model"]["avoid_correct"] == 1


def test_trade_pnl_correctness_uses_realized_result():
    assert classify_trade_pnl_outcome("allow_like", 0.25) == "correct"
    assert classify_trade_pnl_outcome("allow_like", -0.10) == "wrong"
    assert classify_trade_pnl_outcome("avoid_like", -0.10) == "correct"


def test_threshold_optimizer_improves_five_of_eleven_fixture():
    rows = []
    for _ in range(5):
        rows.append(
            {
                "trade_pnl_correctness": "correct",
                "actual_pnl_usdc": -1.0,
                "ml_shadow_decision": "avoid_like",
                "ml_entry_quality_score": 0.45,
                "ml_stop_loss_risk": 0.90,
            }
        )
    for _ in range(6):
        rows.append(
            {
                "trade_pnl_correctness": "wrong",
                "actual_pnl_usdc": 1.0,
                "ml_shadow_decision": "avoid_like",
                "ml_entry_quality_score": 0.70,
                "ml_stop_loss_risk": 0.80,
            }
        )

    result = optimize_trade_pnl_thresholds(rows)

    assert result["current_model_right"] == 5
    assert result["best_model_right"] == 11
    assert result["best_entry_quality_min"] <= 0.70
    assert result["best_stop_loss_risk_max"] >= 0.80
    assert result["allow_count"] == 6
    assert result["avoid_count"] == 5


def test_threshold_optimizer_tie_breaks_to_more_allow_coverage():
    rows = [
        {
            "trade_pnl_correctness": "wrong",
            "actual_pnl_usdc": 1.0,
            "ml_entry_quality_score": 0.80,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "trade_pnl_correctness": "wrong",
            "actual_pnl_usdc": 1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_trade_pnl_thresholds(rows)

    assert result["best_model_right"] == 2
    assert result["allow_count"] == 2
    assert result["best_entry_quality_min"] <= 0.70


def test_threshold_optimizer_marks_low_sample():
    rows = [
        {
            "trade_pnl_correctness": "correct",
            "actual_pnl_usdc": -1.0,
            "ml_entry_quality_score": 0.40,
            "ml_stop_loss_risk": 0.90,
        }
    ]

    result = optimize_trade_pnl_thresholds(rows)

    assert result["actual_trades"] == 1
    assert result["low_sample"]


def test_threshold_optimizer_excludes_missing_scores_from_denominator():
    rows = [
        {
            "trade_pnl_correctness": "correct",
            "actual_pnl_usdc": -1.0,
            "ml_entry_quality_score": 0.40,
            "ml_stop_loss_risk": 0.90,
        },
        {
            "trade_pnl_correctness": "wrong",
            "actual_pnl_usdc": 1.0,
            "ml_entry_quality_score": None,
            "ml_stop_loss_risk": 0.40,
        },
    ]

    result = optimize_trade_pnl_thresholds(rows)

    assert result["actual_trades"] == 1


def test_shadow_pnl_expiry_win_uses_five_usdc_stake():
    result = simulate_shadow_pnl_5usdc(entry_ask=0.80, terminal_directional_gap=2.0)

    assert result["shadow_exit_kind"] == "expiry_win"
    assert round(result["shadow_pnl_5usdc"], 2) == 1.25


def test_shadow_pnl_expiry_loss_loses_stake():
    result = simulate_shadow_pnl_5usdc(entry_ask=0.80, terminal_directional_gap=-2.0)

    assert result["shadow_exit_kind"] == "expiry_loss"
    assert result["shadow_pnl_5usdc"] == -5.0


def test_shadow_pnl_stop_loss_uses_exit_bid():
    result = simulate_shadow_pnl_5usdc(
        entry_ask=0.80,
        terminal_directional_gap=2.0,
        stop_loss_hit_ts="2026-06-03T10:00:00Z",
        stop_loss_hit_bid=0.60,
    )

    assert result["shadow_exit_kind"] == "stop_loss"
    assert result["shadow_pnl_status"] == "ok"
    assert round(result["shadow_pnl_5usdc"], 2) == -1.25


def test_shadow_pnl_stop_loss_missing_bid_falls_back_to_zero():
    result = simulate_shadow_pnl_5usdc(
        entry_ask=0.80,
        terminal_directional_gap=2.0,
        stop_loss_hit_ts="2026-06-03T10:00:00Z",
    )

    assert result["shadow_exit_kind"] == "stop_loss"
    assert result["shadow_exit_price"] == 0.0
    assert result["shadow_pnl_status"] == "fallback_zero_after_stop_loss"
    assert result["shadow_pnl_5usdc"] == -5.0


def test_shadow_pnl_stop_loss_uses_later_fallback_bid():
    result = simulate_shadow_pnl_5usdc(
        entry_ask=0.80,
        terminal_directional_gap=2.0,
        stop_loss_hit_ts="2026-06-03T10:00:00Z",
        stop_loss_fallback_bid=0.60,
    )

    assert result["shadow_exit_kind"] == "stop_loss"
    assert result["shadow_pnl_status"] == "fallback_after_stop_loss"
    assert round(result["shadow_pnl_5usdc"], 2) == -1.25


def test_shadow_pnl_optimizer_sums_current_allow_only():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": 1.25,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:01:00Z",
            "market_slug": "btc-updown-5m-b",
            "direction": "up",
            "ml_shadow_decision": "avoid_like",
            "shadow_pnl_5usdc": 2.00,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["current_total_pnl_5usdc"] == 1.25
    assert result["current_allow_count"] == 1
    assert result["current_allow_rows"] == 1


def test_shadow_pnl_optimizer_counts_first_entry_once_per_market_direction():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": 1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:00:05Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": 5.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:00:10Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": -2.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["current_total_pnl_5usdc"] == 1.0
    assert result["row_level_current_total_pnl_5usdc"] == 4.0
    assert result["current_first_entry_count"] == 1
    assert result["current_allow_rows"] == 3


def test_shadow_pnl_optimizer_splits_first_entry_by_direction():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": 1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:00:01Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "down",
            "ml_shadow_decision": "allow_like",
            "shadow_pnl_5usdc": 2.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["current_total_pnl_5usdc"] == 3.0
    assert result["current_first_entry_count"] == 2


def test_shadow_pnl_optimizer_is_pnl_primary():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "shadow_pnl_5usdc": 3.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:01:00Z",
            "market_slug": "btc-updown-5m-b",
            "direction": "up",
            "shadow_pnl_5usdc": -1.0,
            "ml_entry_quality_score": 0.55,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:02:00Z",
            "market_slug": "btc-updown-5m-c",
            "direction": "up",
            "shadow_pnl_5usdc": -2.0,
            "ml_entry_quality_score": 0.40,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["best_total_pnl_5usdc"] == 3.0
    assert result["best_entry_quality_min"] <= 0.70
    assert result["best_entry_quality_min"] > 0.55
    assert result["allow_count"] == 1


def test_shadow_pnl_optimizer_tie_breaks_model_right_then_allow():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "shadow_pnl_5usdc": 1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:01:00Z",
            "market_slug": "btc-updown-5m-b",
            "direction": "up",
            "shadow_pnl_5usdc": 0.0,
            "ml_entry_quality_score": 0.65,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["best_total_pnl_5usdc"] == 1.0
    assert result["best_model_right"] == 2
    assert result["allow_count"] == 1


def test_shadow_pnl_optimizer_tie_breaks_allow_after_model_right():
    rows = [
        {
            "event_ts": "2026-06-03T10:00:00Z",
            "market_slug": "btc-updown-5m-a",
            "direction": "up",
            "shadow_pnl_5usdc": 1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
        {
            "event_ts": "2026-06-03T10:01:00Z",
            "market_slug": "btc-updown-5m-b",
            "direction": "up",
            "shadow_pnl_5usdc": -1.0,
            "ml_entry_quality_score": 0.70,
            "ml_stop_loss_risk": 0.50,
        },
    ]

    result = optimize_shadow_pnl_thresholds(rows)

    assert result["best_total_pnl_5usdc"] == 0.0
    assert result["best_model_right"] == 1
    assert result["allow_count"] == 2
