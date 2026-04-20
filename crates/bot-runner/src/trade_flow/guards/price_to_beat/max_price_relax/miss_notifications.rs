use super::*;

const NODE_STATE_LAST_NOTIFIED_MISS_STREAK: &str = "ptb_max_price_relax_last_notified_miss_streak";
const NODE_STATE_LAST_NOTIFIED_MISS_MARKET_SLUG: &str =
    "ptb_max_price_relax_last_notified_miss_market_slug";

fn previous_notified_relax_miss_streak(context: &Value, node_key: &str) -> Option<i64> {
    context
        .get("nodeState")
        .and_then(|value| value.get(node_key))
        .and_then(|value| value.get(NODE_STATE_LAST_NOTIFIED_MISS_STREAK))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(raw) => raw.parse::<i64>().ok(),
            _ => None,
        })
}

fn set_last_notified_relax_miss_streak(
    context: &mut Value,
    node_key: &str,
    missed_market_slug: &str,
    miss_streak: i64,
) {
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_LAST_NOTIFIED_MISS_STREAK,
        json!(miss_streak),
    );
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_LAST_NOTIFIED_MISS_MARKET_SLUG,
        json!(missed_market_slug),
    );
}

#[cfg(test)]
pub(super) fn should_notify_relax_miss_streak_change(
    previous_missed_market_slug: Option<&str>,
    previous_miss_streak: Option<i64>,
    missed_market_slug: Option<&str>,
    miss_streak: i64,
) -> bool {
    relax_miss_notification_skip_reason(
        previous_missed_market_slug,
        previous_miss_streak,
        missed_market_slug,
        miss_streak,
    )
    .is_none()
}

pub(super) fn relax_miss_notification_skip_reason(
    previous_missed_market_slug: Option<&str>,
    previous_miss_streak: Option<i64>,
    missed_market_slug: Option<&str>,
    miss_streak: i64,
) -> Option<&'static str> {
    let Some(missed_market_slug) = missed_market_slug else {
        return Some("missing_missed_market_slug");
    };
    if miss_streak <= 0 {
        return Some("zero_miss_streak");
    }
    if previous_missed_market_slug == Some(missed_market_slug)
        && previous_miss_streak == Some(miss_streak)
    {
        return Some("dedup_same_market_same_streak");
    }
    None
}

pub(super) async fn maybe_notify_relax_miss_streak_change(
    repo: &crate::PostgresRepository,
    user_id: i64,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
    evaluation: &PriceToBeatGuardEvaluation,
    relaxation: &mut ActionPlaceOrderMaxPriceRelaxation,
) -> Result<()> {
    let previous_missed_market_slug = super::node_state_market_slug(
        context,
        &node.key,
        NODE_STATE_LAST_NOTIFIED_MISS_MARKET_SLUG,
    );
    let previous_miss_streak = previous_notified_relax_miss_streak(context, &node.key);
    relaxation.previous_notified_miss_streak = previous_miss_streak;

    if let Some(skip_reason) = relax_miss_notification_skip_reason(
        previous_missed_market_slug.as_deref(),
        previous_miss_streak,
        relaxation.missed_market_slug.as_deref(),
        relaxation.miss_streak,
    ) {
        tracing::debug!(
            message = "PTB_RELAX_MISS_NOTIFICATION_SKIPPED",
            node_key = %node.key,
            current_market_slug = %market_slug,
            missed_market_slug = ?relaxation.missed_market_slug,
            miss_streak = relaxation.miss_streak,
            previous_missed_market_slug = ?previous_missed_market_slug,
            previous_miss_streak = ?previous_miss_streak,
            skip_reason = %skip_reason,
        );
        return Ok(());
    }

    let message = super::super::notification::build_price_to_beat_relax_miss_notification_message(
        evaluation,
        previous_miss_streak,
        relaxation.miss_streak,
        relaxation.missed_market_slug.as_deref(),
        relaxation.tradable_seconds_count,
        relaxation.max_fillability_score,
        relaxation.config_miss_count,
        relaxation.applied,
        relaxation.effective_target_threshold_usd,
    );
    let sent = super::super::send_price_to_beat_guard_notification(repo, user_id, &message).await;
    relaxation.miss_notification_sent = sent;
    if sent {
        tracing::info!(
            message = "PTB_RELAX_MISS_NOTIFICATION_SENT",
            node_key = %node.key,
            current_market_slug = %market_slug,
            missed_market_slug = ?relaxation.missed_market_slug,
            miss_streak = relaxation.miss_streak,
            config_miss_count = relaxation.config_miss_count,
            tradable_seconds_count = relaxation.tradable_seconds_count,
            max_fillability_score = ?relaxation.max_fillability_score,
        );
        if let Some(missed_market_slug) = relaxation.missed_market_slug.as_deref() {
            set_last_notified_relax_miss_streak(
                context,
                &node.key,
                missed_market_slug,
                relaxation.miss_streak,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use crate::Utc;

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

    fn runtime_snapshot_guard_miss(market_slug: &str) -> TradeFlowNodeRuntimeSnapshotRecord {
        TradeFlowNodeRuntimeSnapshotRecord {
            run_id: 1,
            definition_id: 1,
            version_id: Some(1),
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "completed".to_string(),
            state_kind: "step".to_string(),
            market_slug: Some(market_slug.to_string()),
            token_id: Some("tok-up".to_string()),
            snapshot_json: json!({
                "output": {
                    "price_to_beat_guard": {
                        "passed": false
                    }
                }
            }),
            updated_at: Utc::now(),
        }
    }

    fn assert_close_option(actual: Option<f64>, expected: f64, tolerance: f64) {
        let actual = actual.expect("expected Some(f64)");
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {actual} to be within {tolerance} of {expected}"
        );
    }

    #[tokio::test]
    async fn max_price_relax_applies_after_five_fill_less_markets() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_last_3_avg_excursion",
            "maxPriceCent": 80,
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
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            let gap = match offset {
                1 => 1.25,
                2 => 1.05,
                3 => 1.30,
                4 => 1.10,
                _ => 1.40,
            };
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_000.0 + gap, 2_000.0, "yes"),
                    second_snapshot(
                        &market_slug,
                        121,
                        0.79,
                        10.0,
                        2_000.0 + gap + 0.01,
                        2_000.0,
                        "yes",
                    ),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.miss_streak, 5);
        assert_eq!(
            relaxation.missed_market_slug.as_deref(),
            Some("eth-updown-5m-1774117600")
        );
        assert_close_option(relaxation.min_gap_usd, 1.05, 1e-9);
        assert_close_option(relaxation.selected_gap_usd, 1.10, 1e-9);
        assert_eq!(relaxation.buffer_usd, 0.10);
        assert_close_option(relaxation.target_threshold_usd, 1.20, 1e-9);
        assert_close_option(relaxation.effective_target_threshold_usd, 1.65, 1e-9);
        assert!(
            (relaxation.relax_credit_usd - 0.15).abs() <= 1e-9,
            "expected relax credit to be 0.15, got {}",
            relaxation.relax_credit_usd
        );
        assert_eq!(relaxation.qualified_market_slugs.len(), 5);
    }

    #[tokio::test]
    async fn max_price_relax_skips_when_no_qualified_market_exists() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
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
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![second_snapshot(
                    &market_slug,
                    120,
                    0.92,
                    10.0,
                    2_101.0,
                    2_100.0,
                    "yes",
                )],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert!(!relaxation.applied);
        assert_eq!(relaxation.miss_streak, 0);
        assert_eq!(relaxation.min_gap_usd, None);
        assert_eq!(relaxation.target_threshold_usd, None);
        assert!(relaxation.qualified_market_slugs.is_empty());
    }

    #[tokio::test]
    async fn max_price_miss_streak_counts_only_consecutive_qualified_markets() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
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
        let mut snapshots = HashMap::new();
        for offset in 1..=4 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            let ask_depth_usdc = if offset == 3 { 1.0 } else { 10.0 };
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, ask_depth_usdc, 2_001.1, 2_000.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, ask_depth_usdc, 2_001.11, 2_000.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert_eq!(relaxation.miss_streak, 2);
        assert_eq!(
            relaxation.missed_market_slug.as_deref(),
            Some("eth-updown-5m-1774117600")
        );
        assert_eq!(relaxation.miss_reason.as_deref(), Some("max_price_miss"));
        assert!(!relaxation.applied);
    }

    #[tokio::test]
    async fn max_price_miss_streak_breaks_on_guard_miss_market() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
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
        let mut snapshots = HashMap::new();
        let mut runtime_snapshots = HashMap::new();
        for offset in 1..=3 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_001.1, 2_000.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_001.11, 2_000.0, "yes"),
                ],
            );
            if offset == 2 {
                runtime_snapshots.insert(
                    market_slug.clone(),
                    runtime_snapshot_guard_miss(&market_slug),
                );
            }
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots,
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
        .expect("relaxation");

        assert_eq!(relaxation.miss_streak, 1);
        assert_eq!(
            relaxation.missed_market_slug.as_deref(),
            Some("eth-updown-5m-1774117600")
        );
        assert_eq!(relaxation.miss_reason.as_deref(), Some("max_price_miss"));
    }

    #[tokio::test]
    async fn max_price_miss_streak_resets_after_buy_fill_market() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100",
                    "ptb_max_price_relax_last_fill_market_slug": "eth-updown-5m-1774117300"
                }
            }
        });
        let previous_market_slug = "eth-updown-5m-1774117600";
        let snapshots = HashMap::from([(
            previous_market_slug.to_string(),
            vec![
                second_snapshot(previous_market_slug, 120, 0.79, 10.0, 2_001.1, 2_000.0, "yes"),
                second_snapshot(previous_market_slug, 121, 0.79, 10.0, 2_001.11, 2_000.0, "yes"),
            ],
        )]);

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert_eq!(relaxation.miss_streak, 1);
        assert_eq!(
            relaxation.missed_market_slug.as_deref(),
            Some(previous_market_slug)
        );
    }

    #[tokio::test]
    async fn max_price_relax_uses_configured_miss_and_history_counts() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
            "priceToBeatMaxPriceRelaxMissCount": 3,
            "priceToBeatMaxPriceRelaxHistoryCount": 3
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
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            let gap = 1.0 + (offset as f64 * 0.1);
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_500.0 + gap, 2_500.0, "yes"),
                    second_snapshot(
                        &market_slug,
                        121,
                        0.79,
                        10.0,
                        2_500.0 + gap + 0.01,
                        2_500.0,
                        "yes",
                    ),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.config_miss_count, 3);
        assert_eq!(relaxation.config_history_count, 3);
        assert_eq!(relaxation.qualified_market_slugs.len(), 3);
    }

    #[tokio::test]
    async fn max_price_relax_allows_manual_ptb_mode() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "manual",
            "priceToBeatMaxDiff": 80,
            "priceToBeatMaxDiffUnit": "cent",
            "maxPriceCent": 80,
            "priceToBeatMaxPriceRelaxMissCount": 3,
            "priceToBeatMaxPriceRelaxHistoryCount": 3,
            "priceToBeatMaxPriceRelaxMinValue": 15,
            "priceToBeatMaxPriceRelaxMinUnit": "cent"
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
        let mut snapshots = HashMap::new();
        for offset in 1..=3 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_601.1, 2_600.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_601.11, 2_600.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            2.00,
            Some(0.8),
            0,
            true,
        )
        .await
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.config_miss_count, 3);
    }

    #[tokio::test]
    async fn max_price_relax_never_drops_below_manual_base_threshold() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "manual",
            "priceToBeatMaxDiff": 100,
            "priceToBeatMaxDiffUnit": "cent",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 25,
            "priceToBeatStopLossBumpUnit": "cent",
            "priceToBeatMaxPriceRelaxMissCount": 3,
            "priceToBeatMaxPriceRelaxHistoryCount": 3,
            "priceToBeatMaxPriceRelaxMinValue": 90,
            "priceToBeatMaxPriceRelaxMinUnit": "cent"
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
        let mut snapshots = HashMap::new();
        for offset in 1..=6 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_600.0, 2_600.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_600.01, 2_600.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            2.25,
            Some(1.0),
            0,
            true,
        )
        .await
        .expect("relaxation");

        assert_close_option(relaxation.raw_target_threshold_usd, 0.25, 1e-9);
        assert_close_option(relaxation.target_threshold_usd, 1.0, 1e-9);
        assert_close_option(relaxation.effective_target_threshold_usd, 1.0, 1e-9);
        assert_eq!(relaxation.floor_usd, 1.0);
    }

    #[tokio::test]
    async fn max_price_relax_applies_floor_to_effective_target() {
        let _guard = super::super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_last_3_avg_excursion",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
            "priceToBeatMaxPriceRelaxMinValue": 1.20,
            "priceToBeatMaxPriceRelaxMinUnit": "usd"
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
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_801.05, 2_800.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_801.06, 2_800.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
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
        .expect("relaxation");

        assert_close_option(relaxation.raw_target_threshold_usd, 1.15, 1e-9);
        assert_close_option(relaxation.target_threshold_usd, 1.20, 1e-9);
        assert_close_option(relaxation.effective_target_threshold_usd, 1.65, 1e-9);
        assert_eq!(relaxation.floor_usd, 1.20);
    }

    #[test]
    fn fill_less_miss_streak_resets_from_last_fill_market() {
        let current_market_slug = "eth-updown-5m-1774117900";
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100",
                    "ptb_max_price_relax_last_fill_market_slug": "eth-updown-5m-1774117300"
                }
            }
        });

        assert_eq!(
            resolve_fill_less_completed_market_streak(&context, "action_1", current_market_slug),
            1
        );
    }

    #[test]
    fn relax_notification_requires_meaningful_same_market_change() {
        assert!(!should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-1",
            1.009
        ));
        assert!(should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-1",
            1.01
        ));
        assert!(should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-2",
            1.001
        ));
    }

    #[test]
    fn relax_miss_notification_requires_positive_streak() {
        assert!(!should_notify_relax_miss_streak_change(
            None,
            None,
            Some("eth-updown-5m-1"),
            0
        ));
    }

    #[test]
    fn relax_miss_notification_skip_reason_reports_missing_market() {
        assert_eq!(
            relax_miss_notification_skip_reason(None, None, None, 2),
            Some("missing_missed_market_slug")
        );
    }

    #[test]
    fn relax_miss_notification_skip_reason_reports_zero_streak() {
        assert_eq!(
            relax_miss_notification_skip_reason(None, None, Some("eth-updown-5m-1"), 0),
            Some("zero_miss_streak")
        );
    }

    #[test]
    fn relax_miss_notification_dedupes_same_market_and_streak() {
        assert!(!should_notify_relax_miss_streak_change(
            Some("eth-updown-5m-1"),
            Some(3),
            Some("eth-updown-5m-1"),
            3
        ));
        assert_eq!(
            relax_miss_notification_skip_reason(
                Some("eth-updown-5m-1"),
                Some(3),
                Some("eth-updown-5m-1"),
                3,
            ),
            Some("dedup_same_market_same_streak")
        );
    }

    #[test]
    fn relax_miss_notification_sends_for_new_market_increment() {
        assert!(should_notify_relax_miss_streak_change(
            Some("eth-updown-5m-1"),
            Some(3),
            Some("eth-updown-5m-2"),
            4
        ));
    }

    #[test]
    fn relax_miss_notification_sends_after_cycle_reset() {
        assert!(should_notify_relax_miss_streak_change(
            Some("eth-updown-5m-9"),
            Some(6),
            Some("eth-updown-5m-10"),
            1
        ));
    }
}
