use super::*;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} to be within {tolerance} of {expected}"
    );
}

fn assert_close_option(actual: Option<f64>, expected: f64, tolerance: f64) {
    let actual = actual.expect("expected Some(f64)");
    assert_close(actual, expected, tolerance);
}

#[test]
fn percent_step_credit_scales_across_extra_misses() {
    let config = MaxPriceRelaxationConfig {
        miss_count: 5,
        history_count: 5,
        floor_usd: 1.2,
        min_depth_usdc: 5.0,
        target_notional_usdc: 5.0,
        step_mode: MaxPriceRelaxationStepMode::Percent,
        step_value: 25.0,
        step_unit: None,
    };

    assert_close(relax_credit_usd(config, 1.8, 1.2, 5), 0.15, 1e-9);
    assert_close(relax_credit_usd(config, 1.8, 1.2, 6), 0.30, 1e-9);
    assert_close(relax_credit_usd(config, 1.8, 1.2, 7), 0.45, 1e-9);
    assert_close(relax_credit_usd(config, 1.8, 1.2, 8), 0.60, 1e-9);
}

#[test]
fn absolute_step_credit_supports_usd_and_cent_and_caps() {
    let usd_config = MaxPriceRelaxationConfig {
        miss_count: 5,
        history_count: 5,
        floor_usd: 1.2,
        min_depth_usdc: 5.0,
        target_notional_usdc: 5.0,
        step_mode: MaxPriceRelaxationStepMode::Absolute,
        step_value: 0.10,
        step_unit: Some(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd),
    };
    let cent_config = MaxPriceRelaxationConfig {
        step_value: 10.0,
        step_unit: Some(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Cent),
        ..usd_config
    };
    let capped_config = MaxPriceRelaxationConfig {
        step_value: 0.40,
        step_unit: Some(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd),
        ..usd_config
    };

    assert_close(relax_credit_usd(usd_config, 1.8, 1.2, 6), 0.20, 1e-9);
    assert_close(relax_credit_usd(cent_config, 1.8, 1.2, 6), 0.20, 1e-9);
    assert_close(relax_credit_usd(capped_config, 1.8, 1.2, 6), 0.60, 1e-9);
}

struct MockDataSource {
    snapshots: HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>,
    runtime_snapshots: HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>,
}

#[async_trait]
impl MaxPriceRelaxationDataSource for MockDataSource {
    async fn load_market_second_snapshots(
        &self,
        market_slugs: &[String],
    ) -> Result<HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>> {
        Ok(market_slugs
            .iter()
            .filter_map(|market_slug| {
                self.snapshots
                    .get(market_slug)
                    .cloned()
                    .map(|rows| (market_slug.clone(), rows))
            })
            .collect())
    }

    async fn load_market_runtime_snapshots(
        &self,
        _run_id: i64,
        _node_key: &str,
        market_slugs: &[String],
    ) -> Result<HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>> {
        Ok(market_slugs
            .iter()
            .filter_map(|market_slug| {
                self.runtime_snapshots
                    .get(market_slug)
                    .cloned()
                    .map(|row| (market_slug.clone(), row))
            })
            .collect())
    }
}

fn test_node(config: Value) -> crate::TradeFlowNode {
    crate::TradeFlowNode {
        key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        config,
    }
}

fn second_snapshot(
    market_slug: &str,
    second_offset: i64,
    best_ask: f64,
    ask_depth_usdc: f64,
    chainlink_price: f64,
    ptb_ref_price: f64,
    outcome_side: &str,
) -> TradeBuilderMarketSecondSnapshot {
    let (window_start, window_end) =
        crate::trade_builder_second_snapshot_window(market_slug).expect("window");
    let second_ts = window_start + crate::ChronoDuration::seconds(second_offset);
    TradeBuilderMarketSecondSnapshot {
        market_slug: market_slug.to_string(),
        asset: "eth".to_string(),
        window_start,
        window_end,
        second_ts,
        ptb_ref_price: Some(ptb_ref_price),
        chainlink_price: Some(chainlink_price),
        yes_best_bid: None,
        yes_best_ask: (outcome_side == "yes").then_some(best_ask),
        yes_ask_depth_usdc: (outcome_side == "yes").then_some(ask_depth_usdc),
        no_best_bid: None,
        no_best_ask: (outcome_side == "no").then_some(best_ask),
        no_ask_depth_usdc: (outcome_side == "no").then_some(ask_depth_usdc),
        sample_count: 1,
    }
}

#[tokio::test]
async fn quality_score_does_not_change_relax_credit() {
    let _guard = super::super::tests::lock_price_to_beat_test_state();
    let current_market_slug = "eth-updown-5m-1774117900";
    let node = test_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "auto_last_3_avg_excursion",
        "maxPriceCent": 80,
        "sizeUsdc": 20,
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpUnit": "cent",
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
            }
        }
    });

    let current_start = crate::MarketCycleId(current_market_slug.to_string())
        .start_time()
        .expect("current start");
    let window_ms = 300_000_i64;
    let build_snapshots = |ask_depth_usdc: f64| {
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(
                        &market_slug,
                        120,
                        0.79,
                        ask_depth_usdc,
                        2_001.1,
                        2_000.0,
                        "yes",
                    ),
                    second_snapshot(
                        &market_slug,
                        121,
                        0.79,
                        ask_depth_usdc,
                        2_001.11,
                        2_000.0,
                        "yes",
                    ),
                ],
            );
        }
        snapshots
    };

    let lower_quality = evaluate_relaxation_with_data_source(
        &MockDataSource {
            snapshots: build_snapshots(10.0),
            runtime_snapshots: HashMap::new(),
        },
        &node,
        &context,
        1,
        current_market_slug,
        "Up",
        1.80,
        None,
        0,
        true,
    )
    .await
    .expect("lower quality relaxation");
    let higher_quality = evaluate_relaxation_with_data_source(
        &MockDataSource {
            snapshots: build_snapshots(20.0),
            runtime_snapshots: HashMap::new(),
        },
        &node,
        &context,
        1,
        current_market_slug,
        "Up",
        1.80,
        None,
        0,
        true,
    )
    .await
    .expect("higher quality relaxation");

    assert_close_option(lower_quality.selected_gap_usd, 1.10, 1e-9);
    assert_close_option(higher_quality.selected_gap_usd, 1.10, 1e-9);
    assert_close_option(lower_quality.effective_target_threshold_usd, 1.50, 1e-9);
    assert_close_option(higher_quality.effective_target_threshold_usd, 1.50, 1e-9);
    assert!(lower_quality.quality_score.is_some());
    assert!(higher_quality.quality_score.is_some());
    assert_ne!(lower_quality.quality_score, higher_quality.quality_score);
}
