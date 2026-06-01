#[cfg(test)]
mod live_gap_collector_tests {
    use super::*;

    fn test_config() -> ActionPlaceOrderLiveGapCollectorConfig {
        ActionPlaceOrderLiveGapCollectorConfig {
            window_start_sec: 220,
            window_end_sec: 285,
            retry_ms: 150,
            hard_max_price: 0.93,
            binance_max_stale_ms: 1_500,
            low_clean_gap_usd: 22.0,
            normal_gap_usd: 32.0,
            high_gap_usd: 48.0,
            high_chop_gap_usd: 55.0,
            detailed_gap_bands: LiveGapDetailedGapBandsConfig {
                enabled: false,
                ultra_clean_gap_usd: 18.0,
                low_clean_gap_usd: 22.0,
                mild_clean_gap_usd: 26.0,
                normal_gap_usd: 32.0,
                active_gap_usd: 40.0,
                high_gap_usd: 48.0,
                high_chop_gap_usd: 55.0,
                extreme_chop_gap_usd: 55.0,
            },
            adaptive_low_gap: LiveGapAdaptiveLowGapConfig {
                enabled: false,
                trigger_count: 1,
                step_pct: 0.05,
                max_relax_pct: 0.05,
                max_shortfall_pct: 0.20,
                max_fill_price: 0.90,
                min_remaining_sec: 35,
                require_local_path_clean: true,
            },
            latency_buffer_usd: 0.0,
            strong_only_under_sec: 20,
            no_new_entry_under_sec: 15,
            strong_signal_extra_gap_usd: 8.0,
            pre_buy_collapse_guard: PreBuyCollapseGuardConfig {
                candidate_max_age_ms: 500,
                late_remaining_sec: 60,
                history_min_age_ms: 750,
                high_price: 0.85,
                very_high_price: 0.89,
                high_price_gap_drop_3s_usd: 8.0,
                mid_price_gap_drop_3s_usd: 12.0,
                mid_price_gap_drop_5s_usd: 16.0,
                bounce_buffer_usd: 3.0,
            },
            no_reversal_entry_guard: NoReversalEntryGuardConfig {
                enabled: false,
                decision_mode: NO_REVERSAL_DECISION_MODE_HISTORICAL_ADAPTIVE.to_string(),
                lookback_mode: "multi_window_adaptive".to_string(),
                precomputed_profiles_enabled: false,
                allow_cold_profile_query: true,
                baseline_floor_pct: 0.80,
                daily_fallback_floor_pct: 0.70,
                source_mismatch_buffer_usd: None,
                source_mismatch_buffer_floor_ratio: 0.15,
                late_high_extra_buffer_usd: None,
                freeze_per_market: true,
                cache_ttl_sec: 60,
                profile_query_timeout_ms: 500,
                profile_lookup_timeout_ms: 500,
                prewarm_query_timeout_ms: 30_000,
                max_relax_pct_per_window: 0.20,
                max_tighten_pct_per_window: 0.40,
                soft_pass_on_insufficient_data: true,
                use_local_path_fallback_on_missing_profile: false,
                local_path_fallback_enabled: false,
                local_path_lookback_ms: 120_000,
                local_path_lookback_source: "node_config".to_string(),
                local_path_min_history_ms: 30_000,
                local_path_gate_mode: NO_REVERSAL_LOCAL_PATH_GATE_MODE_CLEAN_FLOOR.to_string(),
                local_path_fresh_retrace_window_ms: 10_000,
                local_path_fresh_max_drop_usd: 5.0,
                local_path_fresh_min_history_ms: 1_000,
                block_if_profile_missing_and_local_path_insufficient: false,
                profile_missing_emergency_margin_enabled: false,
                profile_missing_emergency_margin_floor_ratio: 0.9,
                ptb_floor_usd: None,
            },
            notify_pre_buy_collapse_guard_decision: true,
            pre_buy_collapse_guard_notification_mode: "smart".to_string(),
            notify_on_adaptive_low_gap_change: true,
            notify_on_adaptive_low_gap_near_miss_change: true,
            live_gap_history_prewarm_enabled: true,
            live_gap_history_prewarm_sec: 35,
            live_gap_history_prewarm_start_mode: "before_trigger_window".to_string(),
            live_gap_history_prewarm_sides: "both".to_string(),
            live_gap_history_sample_ms: 250,
            live_gap_history_retention_ms: 30_000,
            notify_on_pre_buy_history_warning: true,
            pre_buy_history_warning_mode: "smart".to_string(),
            ptb_telemetry_enabled: true,
            notify_on_decision: true,
            live_gap_stop_loss_enabled: true,
            live_gap_stop_loss_entry_gap_ratio: 0.33,
            live_gap_stop_loss_gap_usd: None,
            live_gap_stop_loss_min_remaining_sec: 15,
        }
    }

    fn test_live_gap_collector_required_gap(
        config: &ActionPlaceOrderLiveGapCollectorConfig,
        regime: LiveGapCollectorRegime,
        remaining_sec: i64,
        fill_price: Option<f64>,
    ) -> f64 {
        let base = match regime {
            LiveGapCollectorRegime::LowClean => config.low_clean_gap_usd,
            LiveGapCollectorRegime::Normal => config.normal_gap_usd,
            LiveGapCollectorRegime::High => config.high_gap_usd,
            LiveGapCollectorRegime::HighChop => config.high_chop_gap_usd,
            LiveGapCollectorRegime::Red => f64::INFINITY,
        };
        let late_penalty = if remaining_sec < config.strong_only_under_sec {
            config.strong_signal_extra_gap_usd
        } else {
            0.0
        };
        (base
            + live_gap_collector_price_adjustment(config, fill_price)
            + late_penalty
            + config.latency_buffer_usd)
            .max(0.0)
    }

    #[test]
    fn live_gap_calculates_up_and_down_symmetrically() {
        assert_eq!(
            live_gap_collector_directional_gap("up", 100_000.0, 100_024.0),
            24.0
        );
        assert_eq!(
            live_gap_collector_directional_gap("down", 100_000.0, 99_976.0),
            24.0
        );
    }

    #[test]
    fn required_gap_uses_regime_and_price_bucket() {
        let cfg = test_config();
        assert_eq!(
            test_live_gap_collector_required_gap(
                &cfg,
                LiveGapCollectorRegime::LowClean,
                42,
                Some(0.914)
            ),
            26.0
        );
        assert_eq!(
            test_live_gap_collector_required_gap(
                &cfg,
                LiveGapCollectorRegime::HighChop,
                40,
                Some(0.91)
            ),
            59.0
        );
        assert_eq!(
            test_live_gap_collector_required_gap(
                &cfg,
                LiveGapCollectorRegime::LowClean,
                18,
                Some(0.84)
            ),
            26.0
        );
    }

    #[test]
    fn regime_blocks_stale_and_marks_chop() {
        assert_eq!(
            live_gap_collector_regime(2_000, 1_500, Some(1.0), Some(1.0)),
            LiveGapCollectorRegime::Red
        );
        assert_eq!(
            live_gap_collector_regime(100, 1_500, Some(4.1), Some(5.0)),
            LiveGapCollectorRegime::HighChop
        );
        assert_eq!(
            live_gap_collector_regime(100, 1_500, Some(1.0), Some(5.0)),
            LiveGapCollectorRegime::LowClean
        );
    }

    #[test]
    fn regime_staleness_preserves_chainlink_fallback_age() {
        let snapshot = LiveGapCollectorPriceSnapshot {
            price: 100_025.0,
            timestamp_ms: Some(1_000),
            staleness_ms: 2_250,
            source: "chainlink_live_cached_binance_stale_fallback".to_string(),
            fallback_reason: Some("binance_stale:1800ms".to_string()),
            binance_staleness_ms: Some(1_800),
        };

        assert_eq!(live_gap_collector_regime_staleness_ms(&snapshot), 2_250);
        assert_eq!(
            live_gap_collector_regime(
                live_gap_collector_regime_staleness_ms(&snapshot),
                1_500,
                Some(1.0),
                Some(1.0)
            ),
            LiveGapCollectorRegime::Red
        );
    }

    #[test]
    fn detailed_gap_bands_select_ultra_clean_only_with_clean_path() {
        let mut cfg = test_config();
        cfg.detailed_gap_bands = LiveGapDetailedGapBandsConfig {
            enabled: true,
            ultra_clean_gap_usd: 18.0,
            low_clean_gap_usd: 20.0,
            mild_clean_gap_usd: 23.0,
            normal_gap_usd: 27.0,
            active_gap_usd: 31.0,
            high_gap_usd: 38.0,
            high_chop_gap_usd: 46.0,
            extreme_chop_gap_usd: 55.0,
        };
        let volume = LiveGapVolumeContext {
            volume_ratio_30s: Some(0.40),
            trade_count_60s: Some(10),
            ..LiveGapVolumeContext::unavailable()
        };
        let path = LiveGapBandPathContext {
            history_age_ms: 5_000,
            sample_count: 8,
            gap_drop_3s_usd: Some(1.0),
            gap_drop_5s_usd: Some(2.0),
            gap_slope_3s_usd_per_sec: Some(0.2),
        };
        let selection = live_gap_select_detailed_gap_band(
            &cfg,
            LiveGapCollectorRegime::LowClean,
            &volume,
            Some(2.0),
            &path,
            Some(0.84),
        );
        assert_eq!(selection.band, LiveGapDetailedGapBand::UltraClean);
        let required = live_gap_required_gap_evaluation(&cfg, &selection, 42, Some(0.84));
        assert_eq!(required.required_gap_usd, 14.0);
    }

    #[test]
    fn detailed_gap_bands_mark_unstable_path_as_chop() {
        let mut cfg = test_config();
        cfg.detailed_gap_bands.enabled = true;
        cfg.detailed_gap_bands.high_chop_gap_usd = 46.0;
        let volume = LiveGapVolumeContext {
            volume_ratio_30s: Some(0.70),
            trade_count_60s: Some(12),
            ..LiveGapVolumeContext::unavailable()
        };
        let path = LiveGapBandPathContext {
            history_age_ms: 6_000,
            sample_count: 9,
            gap_drop_3s_usd: Some(30.0),
            gap_drop_5s_usd: Some(35.0),
            gap_slope_3s_usd_per_sec: Some(-8.0),
        };
        let selection = live_gap_select_detailed_gap_band(
            &cfg,
            LiveGapCollectorRegime::Normal,
            &volume,
            Some(5.0),
            &path,
            Some(0.86),
        );
        assert_eq!(selection.band, LiveGapDetailedGapBand::HighChop);
    }

    #[test]
    fn volume_dead_activity_requires_ratio_and_trade_count() {
        let sparse = LiveGapVolumeContext {
            volume_120s: Some(1.0),
            trade_count_60s: Some(1),
            trade_count_120s: Some(2),
            volume_ratio_30s: Some(0.11),
            ..LiveGapVolumeContext::unavailable()
        };
        assert_eq!(
            sparse.dead_activity_block_reason(),
            Some("volume_dead_insufficient_activity")
        );
        let one_large_print = LiveGapVolumeContext {
            volume_60s: Some(0.0),
            volume_120s: Some(200.0),
            trade_count_60s: Some(1),
            trade_count_120s: Some(5),
            volume_ratio_30s: Some(0.30),
            ..LiveGapVolumeContext::unavailable()
        };
        assert_eq!(one_large_print.dead_activity_block_reason(), None);
    }

    fn adaptive_test_config() -> LiveGapAdaptiveLowGapConfig {
        LiveGapAdaptiveLowGapConfig {
            enabled: true,
            trigger_count: 1,
            step_pct: 0.05,
            max_relax_pct: 0.05,
            max_shortfall_pct: 0.20,
            max_fill_price: 0.90,
            min_remaining_sec: 35,
            require_local_path_clean: true,
        }
    }

    fn adaptive_test_input<'a>(
        config: &'a LiveGapAdaptiveLowGapConfig,
        market_slug: &'a str,
        now_ms: i64,
    ) -> LiveGapAdaptiveLowGapInput<'a> {
        LiveGapAdaptiveLowGapInput {
            config,
            run_id: Some(807),
            node_key: "action_qontiv",
            market_slug,
            outcome_label: "Up",
            asset: "btc",
            direction: "up",
            band: LiveGapDetailedGapBand::LowClean,
            local_path_decision: "clean",
            dead_activity_reason: None,
            effective_fill: Some(0.84),
            remaining_sec: 60,
            live_gap_usd: 19.0,
            pre_required_gap_usd: 20.0,
            now_ms,
        }
    }

    #[test]
    fn adaptive_low_gap_records_one_near_miss_per_market_key() {
        live_gap_adaptive_low_gap_reset_state();
        let config = adaptive_test_config();
        let first = live_gap_adaptive_low_gap_evaluate(adaptive_test_input(
            &config,
            "btc-updown-5m-1",
            1_000,
        ));
        assert_eq!(first.market_near_miss_count, 0);
        assert_eq!(first.adaptive_required_gap_usd, 20.0);

        let mut payload = json!({});
        live_gap_record_adaptive_low_gap_near_miss(
            &mut payload,
            &first,
            "live_gap_below_required",
            1_000,
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_recorded")
                .and_then(Value::as_bool),
            Some(true)
        );
        live_gap_record_adaptive_low_gap_near_miss(
            &mut payload,
            &first,
            "live_gap_below_required",
            1_100,
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_deduped")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_market_near_miss_count")
                .and_then(Value::as_i64),
            Some(1)
        );

        let second = live_gap_adaptive_low_gap_evaluate(adaptive_test_input(
            &config,
            "btc-updown-5m-1",
            1_200,
        ));
        assert_eq!(second.market_near_miss_count, 1);
        assert_eq!(second.status, "applied");
        assert_eq!(second.adaptive_required_gap_usd, 19.0);
        assert!(second.saved_from_block);

        let different_market = live_gap_adaptive_low_gap_evaluate(adaptive_test_input(
            &config,
            "btc-updown-5m-2",
            1_300,
        ));
        assert_eq!(different_market.market_near_miss_count, 0);
        assert_eq!(different_market.adaptive_required_gap_usd, 20.0);
    }

    #[test]
    fn adaptive_low_gap_guard_miss_reasons_record_market_near_miss() {
        live_gap_adaptive_low_gap_reset_state();
        let config = adaptive_test_config();
        let above_max = LiveGapAdaptiveLowGapInput {
            effective_fill: Some(0.99),
            ..adaptive_test_input(&config, "btc-updown-5m-3", 1_000)
        };
        let evaluation = live_gap_adaptive_low_gap_evaluate(above_max);
        assert!(!evaluation.can_record_near_miss);
        assert!(evaluation.can_record_guard_miss_near_miss);

        let mut payload_obj = serde_json::Map::new();
        live_gap_append_adaptive_low_gap_evaluation(&mut payload_obj, &evaluation);
        let mut payload = Value::Object(payload_obj);
        live_gap_record_adaptive_low_gap_near_miss_from_payload(
            &mut payload,
            "above_max_price",
            1_000,
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_recorded")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_reason")
                .and_then(Value::as_str),
            Some("above_max_price")
        );

        let unavailable = LiveGapAdaptiveLowGapInput {
            effective_fill: None,
            ..adaptive_test_input(&config, "btc-updown-5m-4", 1_100)
        };
        let evaluation = live_gap_adaptive_low_gap_evaluate(unavailable);
        assert!(!evaluation.can_record_near_miss);
        assert!(evaluation.can_record_guard_miss_near_miss);

        let mut payload_obj = serde_json::Map::new();
        live_gap_append_adaptive_low_gap_evaluation(&mut payload_obj, &evaluation);
        let mut payload = Value::Object(payload_obj);
        live_gap_record_adaptive_low_gap_near_miss_from_payload(
            &mut payload,
            "best_ask_unavailable",
            1_100,
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_recorded")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_reason")
                .and_then(Value::as_str),
            Some("best_ask_unavailable")
        );
    }

    #[test]
    fn adaptive_low_gap_does_not_record_non_gap_or_ineligible_blocks() {
        live_gap_adaptive_low_gap_reset_state();
        let config = adaptive_test_config();
        let input = LiveGapAdaptiveLowGapInput {
            config: &config,
            run_id: Some(807),
            node_key: "action_qontiv",
            market_slug: "btc-updown-5m-1",
            outcome_label: "Up",
            asset: "btc",
            direction: "up",
            band: LiveGapDetailedGapBand::Active,
            local_path_decision: "clean",
            dead_activity_reason: None,
            effective_fill: Some(0.84),
            remaining_sec: 60,
            live_gap_usd: 19.0,
            pre_required_gap_usd: 20.0,
            now_ms: 2_000,
        };
        let evaluation = live_gap_adaptive_low_gap_evaluate(input);
        assert_eq!(evaluation.reason, "band_not_eligible");

        let mut payload = json!({});
        live_gap_record_adaptive_low_gap_near_miss(
            &mut payload,
            &evaluation,
            "effective_fill_above_hard_max",
            2_000,
        );
        assert_eq!(
            payload
                .get("adaptive_low_gap_near_miss_recorded")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn adaptive_low_gap_change_notification_dedupes_same_relax_level() {
        live_gap_adaptive_low_gap_reset_state();
        let payload = json!({
            "adaptive_low_gap_status": "applied",
            "adaptive_low_gap_key": "run=1|node=n|market=m|outcome=Up|asset=btc|direction=up|band=low_clean|price_bucket=80_84",
            "adaptive_low_gap_relax_pct": 0.05,
            "adaptive_required_gap_usd": 19.0
        });
        let first = live_gap_mark_adaptive_low_gap_change_notified(&payload, 1_000)
            .expect("first relax change should notify");
        assert_eq!(first.previous_relax_pct, None);
        assert_eq!(first.new_relax_pct, 0.05);
        assert!(live_gap_mark_adaptive_low_gap_change_notified(&payload, 1_100).is_none());

        let changed = json!({
            "adaptive_low_gap_status": "applied",
            "adaptive_low_gap_key": "run=1|node=n|market=m|outcome=Up|asset=btc|direction=up|band=low_clean|price_bucket=80_84",
            "adaptive_low_gap_relax_pct": 0.10,
            "adaptive_required_gap_usd": 18.0
        });
        let second = live_gap_mark_adaptive_low_gap_change_notified(&changed, 1_200)
            .expect("new relax level should notify");
        assert_eq!(second.previous_relax_pct, Some(0.05));
        assert_eq!(second.new_relax_pct, 0.10);

        let disabled = json!({
            "adaptive_low_gap_status": "not_applied",
            "adaptive_low_gap_key": "different",
            "adaptive_low_gap_relax_pct": 0.05,
            "adaptive_required_gap_usd": 19.0
        });
        assert!(live_gap_mark_adaptive_low_gap_change_notified(&disabled, 1_300).is_none());
    }

    #[test]
    fn adaptive_low_gap_guard_miss_change_notification_projects_relax() {
        live_gap_adaptive_low_gap_reset_state();
        let payload = json!({
            "adaptive_low_gap_near_miss_recorded": true,
            "adaptive_low_gap_key": "run=1|node=n|market=m|outcome=Up|asset=btc|direction=up|band=low_clean|price_bucket=90_plus",
            "adaptive_low_gap_market_near_miss_count": 1,
            "pre_adaptive_required_gap_usd": 20.0,
            "resolved_guard_config": {
                "adaptiveLowGap": {
                    "triggerCount": 1,
                    "stepPct": 0.05,
                    "maxRelaxPct": 0.05
                }
            }
        });
        let first = live_gap_mark_adaptive_low_gap_near_miss_change_notified(&payload, 1_000)
            .expect("projected guard-miss relax should notify");
        assert_eq!(first.previous_relax_pct, None);
        assert_eq!(first.new_relax_pct, 0.05);
        assert_eq!(first.new_adaptive_required_gap_usd, 19.0);
        assert!(
            live_gap_mark_adaptive_low_gap_near_miss_change_notified(&payload, 1_100).is_none()
        );

        let normal_same_change = json!({
            "adaptive_low_gap_status": "applied",
            "adaptive_low_gap_key": "run=1|node=n|market=m|outcome=Up|asset=btc|direction=up|band=low_clean|price_bucket=90_plus",
            "adaptive_low_gap_relax_pct": 0.05,
            "adaptive_required_gap_usd": 19.0
        });
        assert!(
            live_gap_mark_adaptive_low_gap_change_notified(&normal_same_change, 1_200).is_none()
        );
    }

    #[test]
    fn adaptive_low_gap_guard_miss_change_waits_for_trigger_count() {
        live_gap_adaptive_low_gap_reset_state();
        let payload = json!({
            "adaptive_low_gap_near_miss_recorded": true,
            "adaptive_low_gap_key": "run=2|node=n|market=m|outcome=Up|asset=btc|direction=up|band=low_clean|price_bucket=90_plus",
            "adaptive_low_gap_market_near_miss_count": 1,
            "pre_adaptive_required_gap_usd": 20.0,
            "resolved_guard_config": {
                "adaptiveLowGap": {
                    "triggerCount": 2,
                    "stepPct": 0.05,
                    "maxRelaxPct": 0.05
                }
            }
        });
        assert!(
            live_gap_mark_adaptive_low_gap_near_miss_change_notified(&payload, 1_000).is_none()
        );
    }

    #[test]
    fn best_ask_unavailable_relax_overrides_live_gap_cap_only_when_applied() {
        let cfg = test_config();
        assert_eq!(
            live_gap_collector_effective_max_price(Some(0.98), Some(&cfg), None),
            Some(0.93)
        );

        let context = json!({
            "flowContext": {
                "liveGapCollector": {
                    "mode": ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1,
                    "best_ask_unavailable_relax": {
                        "applied": true
                    }
                }
            }
        });
        assert_eq!(
            live_gap_collector_effective_max_price(Some(0.93), Some(&cfg), Some(&context)),
            Some(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE)
        );
    }

    #[test]
    fn best_ask_unavailable_relax_payload_records_fallback_and_skipped_guards() {
        let mut payload = json!({
            "best_ask_source": "best_bid_ask_fallback"
        });
        live_gap_collector_append_best_ask_unavailable_relax(
            &mut payload,
            Some(0.98),
            Some(0.99),
            "best_bid_ask_fallback",
        );

        assert_eq!(
            payload
                .pointer("/best_ask_unavailable_relax/applied")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload.get("fallback_best_ask").and_then(Value::as_f64),
            Some(0.99)
        );
        assert_eq!(
            payload
                .get("relaxed_effective_max_price")
                .and_then(Value::as_f64),
            Some(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE)
        );
        assert_eq!(
            payload.get("reason_code").and_then(Value::as_str),
            Some(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_REASON)
        );
        assert_eq!(
            payload.get("depth_guard_result").and_then(Value::as_str),
            Some(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP)
        );
        assert!(payload
            .get("price_dependent_guards_skipped")
            .and_then(Value::as_array)
            .is_some_and(|guards| guards.len() == 4));
    }

    #[test]
    fn decision_notification_eligibility_skips_retry_blocks() {
        let cfg = test_config();
        let pass = LiveGapCollectorDecision {
            passed: true,
            terminal: false,
            reason_code: "passed",
            payload: json!({}),
        };
        let retry_block = LiveGapCollectorDecision {
            passed: false,
            terminal: false,
            reason_code: "live_gap_below_required",
            payload: json!({}),
        };
        let terminal_block = LiveGapCollectorDecision {
            passed: false,
            terminal: true,
            reason_code: "after_live_gap_window",
            payload: json!({}),
        };

        assert!(should_send_live_gap_collector_decision_notification(
            &cfg,
            &pass,
            &json!({})
        ));
        assert!(!should_send_live_gap_collector_decision_notification(
            &cfg,
            &retry_block,
            &json!({})
        ));
        assert!(should_send_live_gap_collector_decision_notification(
            &cfg,
            &terminal_block,
            &json!({})
        ));
    }

    #[test]
    fn decision_notification_respects_toggle() {
        let mut cfg = test_config();
        cfg.notify_on_decision = false;
        let decision = LiveGapCollectorDecision {
            passed: true,
            terminal: false,
            reason_code: "passed",
            payload: json!({}),
        };

        assert!(!should_send_live_gap_collector_decision_notification(
            &cfg,
            &decision,
            &json!({})
        ));
    }

    #[test]
    fn decision_notification_message_includes_core_fields() {
        let decision = LiveGapCollectorDecision {
            passed: true,
            terminal: false,
            reason_code: "passed",
            payload: json!({}),
        };
        let payload = json!({
            "market_slug": "btc-updown-5m-1773319200",
            "outcome_label": "Up",
            "side": "buy",
            "best_ask": 0.91,
            "effective_fill_price": 0.914,
            "live_gap_usd": 23.8,
            "required_gap_usd": 22.0,
            "base_required_gap_usd": 20.0,
            "final_required_gap_usd": 22.0,
            "gap_band": "low_clean",
            "old_4_band_equivalent": "low_clean",
            "volume_ratio_30s": 0.73,
            "volume_bucket": "low",
            "volume_10s": 12.0,
            "volume_30s": 31.0,
            "volume_60s": 44.0,
            "volume_90s": 56.0,
            "volume_120s": 71.0,
            "trade_count_10s": 2,
            "trade_count_30s": 5,
            "trade_count_60s": 8,
            "trade_count_90s": 10,
            "trade_count_120s": 13,
            "volatility_usd_15s": 4.2,
            "local_path_decision": "clean",
            "band_reason": "clean_path",
            "regime": "low_clean",
            "remaining_sec": 42,
            "ptb_telemetry": {
                "price_to_beat": 30.0,
                "source": "cache",
                "source_latency_ms": 1900
            }
        });

        let message = build_live_gap_collector_decision_notification_message(&decision, &payload);

        assert!(message.contains("Live Gap Collector BUY"));
        assert!(message.contains("Market: btc-updown-5m-1773319200"));
        assert!(message.contains("Outcome: Up"));
        assert!(message.contains("Best Ask: 0.9100"));
        assert!(message.contains("Effective Fill: 0.9140"));
        assert!(message.contains("Live Gap: 23.80 USD"));
        assert!(message.contains("Required Gap: 22.00 USD"));
        assert!(message.contains("Gap Band: low_clean"));
        assert!(message.contains("Base Required Gap: 20.00 USD"));
        assert!(message.contains("Final Required Gap: 22.00 USD"));
        assert!(message.contains("Old 4-Band Equivalent: low_clean"));
        assert!(message.contains("Volume: ratio=0.730, bucket=low"));
        assert!(message.contains("10/30/60/90/120=12.00/31.00/44.00/56.00/71.00"));
        assert!(message.contains("trades10/30/60/90/120=2/5/8/10/13"));
        assert!(message.contains("Local Path Decision: clean"));
        assert!(message.contains("Band Reason: clean_path"));
        assert!(message.contains("Regime: low_clean"));
        assert!(message.contains("Remaining: 42s"));
        assert!(message.contains("Reason: passed"));
        assert!(message.contains("PTB: telemetry only"));
        assert!(message.contains("Lag: 1900ms"));
    }
}
