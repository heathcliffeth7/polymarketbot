use super::*;

fn second_snapshot(
    market_slug: &str,
    second_offset: i64,
    best_ask: f64,
    ask_depth_usdc: f64,
    chainlink_price: f64,
    ptb_ref_price: f64,
    outcome_side: &str,
) -> bot_infra::db::TradeBuilderMarketSecondSnapshot {
    let (window_start, window_end) =
        crate::trade_builder_second_snapshot_window(market_slug).expect("window");
    let second_ts = window_start + crate::ChronoDuration::seconds(second_offset);
    bot_infra::db::TradeBuilderMarketSecondSnapshot {
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
async fn shared_guard_evaluation_sets_relax_tracking_on_first_market() {
    let _guard = lock_price_to_beat_test_state();
    let now_ms = chrono::Utc::now().timestamp_millis();
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "btc",
        &[(now_ms, 100.0)],
    )
    .expect("seed btc tick");
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            BTC_MARKET_5M,
            "btc",
            "5m",
            100.0,
            None
        )
    );
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 80,
        "priceToBeatMaxDiffUnit": "cent",
    }));
    let mut context = json!({});

    let evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        None,
        &mut context,
        &node,
        1,
        BTC_MARKET_5M,
        "Up",
    )
    .await
    .expect("shared evaluation");

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "price_to_beat_gap_below_threshold");
    assert_eq!(
        context.pointer("/nodeState/action_1/ptb_max_price_relax_tracking_start_market_slug"),
        Some(&json!(BTC_MARKET_5M))
    );
}

#[tokio::test]
async fn shared_guard_evaluation_relaxes_after_tracked_fill_less_markets() {
    let _guard = lock_price_to_beat_test_state();
    let current_market_slug = "eth-updown-5m-1774117900";
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "eth",
        &[(chrono::Utc::now().timestamp_millis(), 2_001.6)],
    )
    .expect("seed eth tick");
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            current_market_slug,
            "eth",
            "5m",
            2_000.0,
            None
        )
    );
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 1.8,
        "priceToBeatMaxDiffUnit": "usd",
        "maxPriceCent": 80,
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpUnit": "cent",
    }));
    let mut context = json!({
        "nodeState": {
            "action_1": {
                "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
            }
        }
    });
    let current_start = crate::MarketCycleId(current_market_slug.to_string())
        .start_time()
        .expect("current start");
    let mut snapshots = std::collections::HashMap::new();
    for (offset, gap) in [(1, 1.25), (2, 1.05), (3, 1.30), (4, 1.10), (5, 1.40), (6, 1.35)] {
        let market_start = current_start - crate::ChronoDuration::milliseconds(300_000 * offset);
        let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
        snapshots.insert(
            market_slug.clone(),
            vec![
                second_snapshot(&market_slug, 120, 0.79, 10.0, 2_000.0 + gap, 2_000.0, "yes"),
                second_snapshot(
                    &market_slug,
                    121,
                    0.79,
                    10.0,
                    2_000.01 + gap,
                    2_000.0,
                    "yes",
                ),
            ],
        );
    }

    let mut evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        None,
        &mut context,
        &node,
        1,
        current_market_slug,
        "Up",
    )
    .await
    .expect("shared evaluation");
    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "price_to_beat_gap_below_threshold");

    super::super::max_price_relax::preview_action_place_order_max_price_relaxation_with_snapshots(
        &mut context,
        &node,
        1,
        current_market_slug,
        "Up",
        &mut evaluation,
        snapshots,
        std::collections::HashMap::new(),
    )
    .await
    .expect("preview relax");

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "price_to_beat_gap_below_threshold");
    assert_close(evaluation.threshold_usd, 1.8, 1e-9);
    assert_eq!(
        evaluation
            .max_price_relax
            .as_ref()
            .and_then(|value| value.get("max_price_relax_applied"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(
        context
            .pointer("/nodeState/action_1/ptb_current_effective_threshold_usd")
            .is_none()
    );
}

#[tokio::test]
async fn shared_guard_evaluation_skips_relax_preview_for_non_gap_reason() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 1.8,
        "priceToBeatMaxDiffUnit": "usd",
        "maxPriceCent": 80,
    }));
    let mut context = json!({});
    let mut evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        None,
        &mut context,
        &node,
        1,
        "unsupported-market",
        "Up",
    )
    .await
    .expect("shared evaluation");

    super::super::max_price_relax::preview_action_place_order_max_price_relaxation_with_snapshots(
        &mut context,
        &node,
        1,
        "unsupported-market",
        "Up",
        &mut evaluation,
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    )
    .await
    .expect("preview relax");

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "unsupported_market");
    assert!(evaluation.max_price_relax.is_none());
}

#[tokio::test]
async fn shared_guard_evaluation_preview_persists_relaxed_effective_threshold() {
    let _guard = lock_price_to_beat_test_state();
    let current_market_slug = "eth-updown-5m-1774118200";
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "eth",
        &[(chrono::Utc::now().timestamp_millis(), 2_001.05)],
    )
    .expect("seed eth tick");
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            current_market_slug,
            "eth",
            "5m",
            2_000.0,
            None
        )
    );
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 100,
        "priceToBeatMaxDiffUnit": "cent",
        "maxPriceCent": 80,
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 25,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global",
        "priceToBeatMaxPriceRelaxMissCount": 3,
        "priceToBeatMaxPriceRelaxHistoryCount": 3,
        "priceToBeatMaxPriceRelaxMinValue": 90,
        "priceToBeatMaxPriceRelaxMinUnit": "cent"
    }));
    let mut context = json!({
        "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100",
                    "ptb_stop_loss_bump_count": 12,
                    "ptb_stop_loss_bump_accumulated_usd": 3.0,
                    "ptb_stop_loss_bump_last_increment_usd": 0.25,
                    "ptb_stop_loss_bump_last_market_slug": "eth-updown-5m-1774117600"
                }
            }
        });
    let current_start = crate::MarketCycleId(current_market_slug.to_string())
        .start_time()
        .expect("current start");
    let mut snapshots = std::collections::HashMap::new();
    for offset in 1..=6 {
        let market_start = current_start - crate::ChronoDuration::milliseconds(300_000 * offset);
        let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
        snapshots.insert(
            market_slug.clone(),
            vec![
                second_snapshot(&market_slug, 120, 0.79, 10.0, 2_600.0, 2_600.0, "yes"),
                second_snapshot(&market_slug, 121, 0.79, 10.0, 2_600.01, 2_600.0, "yes"),
            ],
        );
    }

    let mut evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        None,
        &mut context,
        &node,
        1,
        current_market_slug,
        "Up",
    )
    .await
    .expect("shared evaluation");
    assert!(!evaluation.passed);

    super::super::max_price_relax::preview_action_place_order_max_price_relaxation_with_snapshots(
        &mut context,
        &node,
        1,
        current_market_slug,
        "Up",
        &mut evaluation,
        snapshots,
        std::collections::HashMap::new(),
    )
    .await
    .expect("preview relax");

    assert!(evaluation.passed);
    assert_close(evaluation.threshold_usd, 1.0, 1e-9);
    assert_eq!(
        context.pointer("/nodeState/action_1/ptb_current_effective_threshold_usd"),
        Some(&json!(1.0))
    );
    assert_eq!(
        context.pointer("/nodeState/action_1/ptb_current_effective_bump_usd"),
        Some(&json!(3.0))
    );
}
