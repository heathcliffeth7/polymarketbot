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
                lookback_mode: "multi_window_adaptive".to_string(),
                baseline_floor_pct: 0.80,
                daily_fallback_floor_pct: 0.70,
                source_mismatch_buffer_usd: None,
                source_mismatch_buffer_floor_ratio: 0.15,
                late_high_extra_buffer_usd: None,
                freeze_per_market: true,
                cache_ttl_sec: 60,
                profile_query_timeout_ms: 500,
                max_relax_pct_per_window: 0.20,
                max_tighten_pct_per_window: 0.40,
                soft_pass_on_insufficient_data: true,
                ptb_floor_usd: None,
            },
            notify_pre_buy_collapse_guard_decision: true,
            pre_buy_collapse_guard_notification_mode: "smart".to_string(),
            live_gap_history_prewarm_enabled: true,
            live_gap_history_prewarm_sec: 20,
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
            live_gap_collector_required_gap(
                &cfg,
                LiveGapCollectorRegime::LowClean,
                42,
                Some(0.914)
            ),
            26.0
        );
        assert_eq!(
            live_gap_collector_required_gap(&cfg, LiveGapCollectorRegime::HighChop, 40, Some(0.91)),
            59.0
        );
        assert_eq!(
            live_gap_collector_required_gap(&cfg, LiveGapCollectorRegime::LowClean, 18, Some(0.84)),
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
        assert!(message.contains("Regime: low_clean"));
        assert!(message.contains("Remaining: 42s"));
        assert!(message.contains("Reason: passed"));
        assert!(message.contains("PTB: telemetry only"));
        assert!(message.contains("Lag: 1900ms"));
    }
}
