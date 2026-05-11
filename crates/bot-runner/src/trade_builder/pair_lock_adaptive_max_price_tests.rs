#[cfg(test)]
mod pair_lock_adaptive_max_price_tests {
    use super::*;

    fn default_config() -> PairLockAdaptiveMaxPriceConfig {
        PairLockAdaptiveMaxPriceConfig {
            miss_count: 3,
            required_good_miss_count: 2,
            relax_credit_cent: 2.0,
            max_relax_credit_cent: 5.0,
            hard_cap_cent: 76.0,
            extra_buffer_cent: 1.0,
            pair_buffer_cent: 1.0,
            size_multiplier: 0.5,
            window_start_sec: None,
            window_end_sec: None,
            legacy_late_relax_cutoff_s: None,
            late_risk_enabled: true,
            late_risk_after_sec: 210,
            late_extra_buffer_cent: 1.0,
            late_size_multiplier: 0.35,
            sl_cooldown_markets: 3,
        }
    }

    fn good_history() -> PairLockAdaptiveMaxPriceHistory {
        PairLockAdaptiveMaxPriceHistory {
            resolved_good_miss_count: 2,
            resolved_good_block_count: 1,
            resolved_miss_count: 3,
            ..Default::default()
        }
    }

    fn default_input() -> PairLockAdaptiveMaxPriceDecisionInput<'static> {
        PairLockAdaptiveMaxPriceDecisionInput {
            config: default_config(),
            base_max_price: Some(0.70),
            ask: Some(0.72),
            estimated_avg_fill: Some(0.72),
            counter_estimated_avg_fill: Some(0.23),
            q_final: Some(0.84),
            dynamic_threshold: Some(0.07),
            pair_max_total_price: 0.96,
            base_size_usdc: 5.0,
            ptb_passed: true,
            base_max_price_blocked: true,
            depth_guard_pass: true,
            counter_depth_ok: true,
            book_reliability_ok: true,
            volume_regime: "normal",
            ptb_trend: "expanding",
            market_elapsed_s: Some(144),
            cycle_window_start_sec: None,
            cycle_window_end_sec: None,
            already_relaxed_current_market: false,
            history: good_history(),
        }
    }

    fn adaptive_summary(
        outcome_label: &str,
        classification: &str,
        sl_hit: bool,
    ) -> bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord {
        bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord {
            id: 1,
            definition_id: 1,
            version_id: 1,
            flow_run_id: Some(1),
            node_key: "pair_buy".to_string(),
            market_scope: "eth_5m_updown".to_string(),
            market_slug: "eth-updown-5m-1".to_string(),
            window_start: None,
            window_end: None,
            completed_at: Utc::now(),
            trigger_passed: true,
            action_started: true,
            builder_order_created: false,
            order_submitted: false,
            order_filled: false,
            first_terminal_guard_scope: None,
            first_terminal_guard_code: None,
            first_terminal_guard_node: None,
            first_terminal_guard_at: None,
            last_guard_scope: None,
            last_guard_code: None,
            max_price_block: true,
            execution_floor_block: false,
            ptb_block: false,
            pair_total_block: false,
            counter_max_block: false,
            counter_floor_block: false,
            risk_block: false,
            data_problem_block: false,
            best_ask_at_block: Some(0.72),
            max_price_effective: Some(0.70),
            execution_floor_effective: None,
            pair_total_effective: Some(0.96),
            counter_price_effective: Some(0.23),
            iv_edge_margin: Some(0.07),
            binance_stale_ms: None,
            binance_same_direction: None,
            depth_ok: Some(true),
            floor_recovered_once: false,
            max_best_ask_after_block: None,
            tradable_seconds_count: Some(180),
            pair_session_id: None,
            pair_locked: false,
            locked_qty: None,
            unpaired_qty: None,
            locked_profit_per_share: None,
            orphan_detected: false,
            protective_unwind_triggered: false,
            sl_hit,
            tp_hit: false,
            realized_pnl_usdc: None,
            metrics_json: json!({
                "adaptive_max_price": {
                    "outcome_label": outcome_label,
                    "ptb_pass": true,
                    "resolved_classification": classification,
                }
            }),
        }
    }

    #[test]
    fn adaptive_max_price_allows_resolved_good_miss_normal_expanding() {
        let decision = evaluate_pair_lock_adaptive_max_price_decision(default_input());
        assert!(decision.relax_applied);
        assert_eq!(decision.decision, "RELAX_ALLOW");
        assert_eq!(decision.effective_max_price, Some(0.72));
        assert_eq!(decision.effective_size_usdc, Some(2.5));
    }

    #[test]
    fn adaptive_max_price_ignores_pending_and_unknown_misses() {
        let mut input = default_input();
        input.history = PairLockAdaptiveMaxPriceHistory {
            resolved_good_miss_count: 1,
            resolved_good_block_count: 0,
            pending_miss_count: 2,
            unknown_miss_count: 2,
            resolved_miss_count: 1,
            ..Default::default()
        };
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "resolved_miss_history_insufficient");
    }

    #[test]
    fn adaptive_max_price_allows_configured_window_at_elapsed_250() {
        let mut input = default_input();
        input.config.window_start_sec = Some(120);
        input.config.window_end_sec = Some(290);
        input.market_elapsed_s = Some(250);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(decision.relax_applied);
    }

    #[test]
    fn adaptive_max_price_blocks_outside_adaptive_window() {
        let mut input = default_input();
        input.config.window_start_sec = Some(120);
        input.config.window_end_sec = Some(210);
        input.market_elapsed_s = Some(250);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "outside_adaptive_window");
    }

    #[test]
    fn adaptive_max_price_intersects_configured_and_cycle_windows() {
        let mut input = default_input();
        input.config.window_start_sec = Some(0);
        input.config.window_end_sec = Some(210);
        input.cycle_window_start_sec = Some(120);
        input.cycle_window_end_sec = Some(290);
        input.market_elapsed_s = Some(100);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert_eq!(decision.reason, "outside_adaptive_window");
        let timing = decision
            .diagnostics
            .get("timing")
            .expect("timing payload");
        assert_eq!(
            timing
                .pointer("/effective_window/start_sec")
                .and_then(Value::as_i64),
            Some(120)
        );
        assert_eq!(
            timing
                .pointer("/effective_window/end_sec")
                .and_then(Value::as_i64),
            Some(210)
        );
    }

    #[test]
    fn adaptive_max_price_late_risk_tightens_buffer_and_size() {
        let mut input = default_input();
        input.config.window_start_sec = Some(120);
        input.config.window_end_sec = Some(290);
        input.market_elapsed_s = Some(250);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(decision.relax_applied);
        assert_eq!(decision.reason, "late_risk_size_reduced");
        assert_eq!(decision.effective_size_usdc, Some(1.75));
        let payload = decision.diagnostics;
        assert_eq!(
            payload
                .get("applied_extra_buffer_cent")
                .and_then(Value::as_f64),
            Some(2.0)
        );
        assert_eq!(
            payload
                .get("applied_size_multiplier")
                .and_then(Value::as_f64),
            Some(0.35)
        );
    }

    #[test]
    fn adaptive_max_price_late_risk_can_be_disabled() {
        let mut input = default_input();
        input.config.window_start_sec = Some(120);
        input.config.window_end_sec = Some(290);
        input.config.late_risk_enabled = false;
        input.market_elapsed_s = Some(250);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(decision.relax_applied);
        assert_eq!(decision.reason, "resolved_good_miss_history_normal_expanding");
        assert_eq!(decision.effective_size_usdc, Some(2.5));
    }

    #[test]
    fn adaptive_max_price_late_risk_block_when_tightening_breaks_edge() {
        let mut input = default_input();
        input.config.window_start_sec = Some(120);
        input.config.window_end_sec = Some(290);
        input.market_elapsed_s = Some(250);
        input.q_final = Some(0.79);
        input.dynamic_threshold = Some(0.06);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "late_risk_block");
    }

    #[test]
    fn adaptive_max_price_legacy_cutoff_is_fallback_window_end() {
        let mut input = default_input();
        input.config.legacy_late_relax_cutoff_s = Some(210);
        input.market_elapsed_s = Some(250);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "outside_adaptive_window");
    }

    #[test]
    fn adaptive_max_price_blocks_estimated_fill_above_effective_cap() {
        let mut input = default_input();
        input.estimated_avg_fill = Some(0.731);
        input.counter_estimated_avg_fill = Some(0.20);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "estimated_avg_fill_above_effective_max_price");
    }

    #[test]
    fn adaptive_max_price_uses_counter_vwap_for_pair_cap() {
        let mut input = default_input();
        input.counter_estimated_avg_fill = Some(0.26);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "pair_cap_below_estimated_fill");
    }

    #[test]
    fn adaptive_max_price_blocks_after_recent_sl_cooldown() {
        let mut input = default_input();
        input.history.recent_sl = true;
        input.history.cooldown_active = true;
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "recent_sl_cooldown");
    }

    #[test]
    fn adaptive_max_price_normalizes_probability_units_to_cent_payload() {
        let decision = evaluate_pair_lock_adaptive_max_price_decision(default_input());
        let payload = decision.diagnostics;
        assert_eq!(payload.get("q_final_cent").and_then(Value::as_f64), Some(84.0));
        let threshold = payload
            .get("dynamic_threshold_cent")
            .and_then(Value::as_f64)
            .expect("dynamic threshold");
        assert!((threshold - 7.0).abs() < 0.000001);
    }

    #[test]
    fn adaptive_max_price_blocks_collapsing_or_high_volume() {
        let mut collapsing = default_input();
        collapsing.ptb_trend = "collapsing";
        let collapsing_decision = evaluate_pair_lock_adaptive_max_price_decision(collapsing);
        assert_eq!(collapsing_decision.reason, "ptb_not_expanding");

        let mut high_volume = default_input();
        high_volume.volume_regime = "high";
        let high_volume_decision = evaluate_pair_lock_adaptive_max_price_decision(high_volume);
        assert_eq!(high_volume_decision.reason, "high_volume");
    }

    #[test]
    fn adaptive_max_price_skips_when_ask_does_not_need_override() {
        let mut input = default_input();
        input.ask = Some(0.70);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "ask_not_above_base_max_price");
    }

    #[test]
    fn adaptive_max_price_history_is_side_specific() {
        let summaries = vec![
            adaptive_summary("Down", "good_block", false),
            adaptive_summary("Up", "good_miss", false),
            adaptive_summary("Up", "good_miss", false),
            adaptive_summary("Down", "good_block", false),
            adaptive_summary("Up", "good_block", false),
        ];

        let up_history = build_pair_lock_adaptive_history(&summaries, "UP", default_config());
        assert_eq!(up_history.resolved_good_miss_count, 2);
        assert_eq!(up_history.resolved_good_block_count, 1);

        let down_history = build_pair_lock_adaptive_history(&summaries, "DOWN", default_config());
        assert_eq!(down_history.resolved_good_miss_count, 0);
        assert_eq!(down_history.resolved_good_block_count, 2);
    }
}
