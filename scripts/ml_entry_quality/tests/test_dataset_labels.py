from common import (
    SHADOW_DECISION_POLICY_VERSION,
    compute_labels,
    extract_payload_features,
    score_to_shadow_decision,
)


def test_compute_labels_marks_winning_low_risk_candidate():
    quality, risk = compute_labels(
        entry_ask=0.72,
        terminal_directional_gap=4.2,
        directional_gap=12.0,
        future_min_gap=11.4,
        stop_loss_gap_usd=1.0,
    )

    assert quality == 1
    assert risk == 0


def test_compute_labels_marks_future_adverse_stop_loss_risk():
    quality, risk = compute_labels(
        entry_ask=0.82,
        terminal_directional_gap=5.0,
        directional_gap=12.0,
        future_min_gap=9.5,
        stop_loss_gap_usd=1.0,
    )

    assert quality == 1
    assert risk == 1


def test_extract_payload_features_reads_existing_entry_debug_shape():
    payload = {
        "market_slug": "btc-updown-5m-1780443300",
        "outcome_norm": "UP",
        "market": {"asset": "BTC", "remaining_s": 44, "market_elapsed_s": 256},
        "price": {"best_ask": 0.81, "spread_cent": 1.5},
        "ptb": {"gap_now": 13.2, "slope_3s": 2.1, "slope_10s": 4.4},
        "volume": {"polymarket": {"recent_notional_10s": 12, "recent_notional_30s": 25, "recent_trade_count_30s": 3, "regime": "normal"}},
        "guard_breakdown": {
            "ptb": {
                "iv_mismatch_edge": {
                    "entry_quality_debug": {
                        "iv_edge": {
                            "edge": 0.09,
                            "adjustedMargin": 0.04,
                            "gapStrength": 0.8,
                            "gapStrengthMargin": 0.05,
                            "qFinal": 0.91,
                            "expectedMoveEff": 7.3,
                            "matchedRule": "60-30",
                        },
                        "cex_direction_guard": {"status": "neutral", "blocking": False},
                        "source": {"ptbCurrentPriceSource": "chainlink_cex_consensus", "chainlinkAgeMs": 900},
                    }
                }
            }
        },
    }

    features = extract_payload_features(payload)

    assert features["direction"] == "up"
    assert features["asset"] == "btc"
    assert features["timeframe"] == "5m"
    assert features["entry_ask"] == 0.81
    assert features["iv_edge"] == 0.09
    assert features["gap_strength"] == 0.8
    assert features["cex_status"] == "neutral"
    assert features["selected_entry_current_source"] == "chainlink_cex_consensus"


def test_shadow_decision_thresholds_are_report_only_buckets():
    assert SHADOW_DECISION_POLICY_VERSION == "static_balanced_v3"
    assert score_to_shadow_decision(0.60, 0.90) == "allow_like"
    assert score_to_shadow_decision(0.59, 0.90) == "caution"
    assert score_to_shadow_decision(0.60, 0.91) == "caution"
    assert score_to_shadow_decision(0.29, 0.40) == "avoid_like"
    assert score_to_shadow_decision(0.70, 0.95) == "avoid_like"
    assert score_to_shadow_decision(None, 0.20) == "caution"
