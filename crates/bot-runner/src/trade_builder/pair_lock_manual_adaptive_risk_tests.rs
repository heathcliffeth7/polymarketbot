#[cfg(test)]
mod pair_lock_manual_adaptive_risk_tests {
    use super::*;

    fn default_manual_config() -> PairLockManualAdaptiveRiskConfig {
        PairLockManualAdaptiveRiskConfig {
            window_start_sec: Some(120),
            window_end_sec: Some(290),
            volume_normal_lt: 1.5,
            volume_elevated_lt: 2.5,
            volume_high_lt: 4.0,
            trend_delta_usd: 0.05,
            normal_flat_max_price_sub_cent: 2.0,
            normal_flat_size_multiplier: 0.8,
            normal_flat_ptb_gap_add_cent: 5.0,
            normal_collapsing_max_price_cent: 62.0,
            normal_collapsing_size_multiplier: 0.4,
            normal_collapsing_ptb_gap_add_cent: 15.0,
            elevated_max_price_cent: 66.0,
            elevated_size_multiplier: 0.6,
            elevated_ptb_gap_add_cent: 10.0,
            high_max_price_cent: 58.0,
            high_size_multiplier: 0.3,
            high_ptb_gap_add_cent: 25.0,
            after_sl_max_price_sub_cent: 5.0,
            after_sl_ptb_gap_add_cent: 15.0,
            sl_cooldown_markets: 3,
            pair_buffer_cent: 1.0,
            self_tune: PairLockManualSelfTuneConfig::default(),
        }
    }

    fn timing() -> PairLockManualAdaptiveTiming {
        PairLockManualAdaptiveTiming {
            configured_window_start_sec: 120,
            configured_window_end_sec: 290,
            cycle_window_start_sec: Some(120),
            cycle_window_end_sec: Some(290),
            effective_window_start_sec: 120,
            effective_window_end_sec: 290,
            market_elapsed_s: Some(150),
            in_window: true,
        }
    }

    fn volume(regime: &'static str) -> PairLockManualAdaptiveVolumeContext {
        PairLockManualAdaptiveVolumeContext {
            regime,
            ratio: Some(match regime {
                "normal" => 1.0,
                "elevated" => 2.0,
                "high" => 3.0,
                "extreme" => 4.5,
                _ => 0.0,
            }),
            recent_notional_30s: Some(10.0),
            baseline_notional_30s: Some(10.0),
        }
    }

    fn input(
        regime: &'static str,
        trend: &'static str,
    ) -> PairLockManualAdaptiveDecisionInput {
        PairLockManualAdaptiveDecisionInput {
            config: default_manual_config(),
            base_max_price: Some(0.70),
            base_size_usdc: 5.0,
            base_reenter_on_sl_hit: true,
            base_counter_max_price: Some(0.70),
            counter_floor_price: Some(0.10),
            ask: Some(0.55),
            primary_estimated_avg_fill: Some(0.55),
            counter_estimated_avg_fill: Some(0.25),
            pair_max_total_price: 0.96,
            base_decision_passed: true,
            base_reason_code: "passed".to_string(),
            ptb_passed: true,
            ptb_directional_gap: Some(0.80),
            ptb_threshold_usd: Some(0.20),
            ptb_threshold_value: Some(20.0),
            ptb_threshold_unit: Some("cent".to_string()),
            volume: volume(regime),
            ptb_trend: trend,
            timing: timing(),
            cooldown: PairLockManualAdaptiveCooldown {
                active: false,
                remaining_before: 0,
                remaining_after: 0,
            },
            scope_side: "eth_5m_updown:UP".to_string(),
            self_tune: PairLockManualSelfTuneRuntime {
                state: PairLockManualSelfTuneState::default(),
                miss_relax_applies: false,
                cooldown_active: false,
                lockdown_active: false,
            },
        }
    }

    fn enable_self_tune(
        mut input: PairLockManualAdaptiveDecisionInput,
        state: PairLockManualSelfTuneState,
        miss_relax_applies: bool,
    ) -> PairLockManualAdaptiveDecisionInput {
        input.config.self_tune.enabled = true;
        input.config.self_tune.max_price_relax_hard_cap_cent = 90.0;
        input.self_tune = PairLockManualSelfTuneRuntime {
            cooldown_active: state.cooldown_markets_left > 0,
            lockdown_active: state.lockdown_markets_left > 0,
            state,
            miss_relax_applies,
        };
        input
    }

    #[test]
    fn manual_adaptive_normal_expanding_keeps_base() {
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input("normal", "expanding"));

        assert!(!decision.applied);
        assert_eq!(decision.decision, "BASE");
        assert_eq!(decision.reason, "base_normal_expanding");
        assert_eq!(decision.effective_max_price, Some(0.70));
    }

    #[test]
    fn manual_adaptive_elevated_expanding_tightens_price_size_and_ptb() {
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input("elevated", "expanding"));

        assert!(decision.applied);
        assert_eq!(decision.decision, "ALLOW_STRICT");
        assert_eq!(decision.reason, "elevated_volume_strict");
        assert_eq!(decision.effective_max_price, Some(0.66));
        assert_eq!(decision.effective_size_usdc, Some(3.0));
        assert_eq!(decision.effective_ptb_threshold_value, Some(30.0));
    }

    #[test]
    fn manual_adaptive_high_expanding_allows_small_when_fill_is_cheap() {
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input("high", "expanding"));

        assert!(decision.applied);
        assert_eq!(decision.decision, "ALLOW_STRICT");
        assert_eq!(decision.reason, "high_volume_strict");
        assert_eq!(decision.effective_max_price, Some(0.58));
        assert_eq!(decision.effective_size_usdc, Some(1.5));
    }

    #[test]
    fn manual_adaptive_high_collapsing_blocks() {
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input("high", "collapsing"));

        assert_eq!(decision.decision, "BLOCK");
        assert_eq!(decision.reason, "high_volume_gap_collapsing");
    }

    #[test]
    fn manual_adaptive_extreme_volume_blocks() {
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input("extreme", "expanding"));

        assert_eq!(decision.decision, "BLOCK");
        assert_eq!(decision.reason, "extreme_volume");
    }

    #[test]
    fn manual_adaptive_recent_sl_applies_strict_cooldown() {
        let mut input = input("normal", "expanding");
        input.cooldown = PairLockManualAdaptiveCooldown {
            active: true,
            remaining_before: 3,
            remaining_after: 2,
        };
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);

        assert_eq!(decision.decision, "ALLOW_STRICT");
        assert_eq!(decision.reason, "recent_sl_strict_mode");
        assert_eq!(decision.effective_max_price, Some(0.65));
        assert_eq!(decision.effective_ptb_threshold_value, Some(35.0));
    }

    #[test]
    fn manual_self_tune_safe_miss_relaxes_ptb_and_max_price() {
        let mut state = PairLockManualSelfTuneState::default();
        state.ptb_relax_credit_cent = 10.0;
        state.max_price_relax_credit_cent = 2.0;
        let mut input = input("normal", "expanding");
        input.base_max_price = Some(0.85);
        input.ptb_threshold_value = Some(125.0);
        input.ptb_threshold_usd = Some(1.25);
        input.ptb_directional_gap = Some(1.5);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(
            enable_self_tune(input, state, true),
        );

        assert!(decision.applied);
        assert_eq!(decision.reason, "manual_self_tune_safe_miss_relax");
        assert_eq!(decision.effective_ptb_threshold_value, Some(115.0));
        assert_eq!(decision.effective_max_price, Some(0.87));
    }

    #[test]
    fn manual_self_tune_sl_bump_dominates_existing_miss_relax() {
        let mut state = PairLockManualSelfTuneState::default();
        state.ptb_relax_credit_cent = 10.0;
        state.max_price_relax_credit_cent = 2.0;
        state.ptb_sl_bump_cent = 15.0;
        state.max_price_sl_penalty_cent = 5.0;
        let mut input = input("normal", "expanding");
        input.base_max_price = Some(0.85);
        input.ptb_threshold_value = Some(125.0);
        input.ptb_threshold_usd = Some(1.25);
        input.ptb_directional_gap = Some(1.5);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(
            enable_self_tune(input, state, true),
        );

        assert_eq!(decision.reason, "manual_self_tune_sl_strict");
        assert_eq!(decision.effective_ptb_threshold_value, Some(130.0));
        assert_eq!(decision.effective_max_price, Some(0.82));
    }

    #[test]
    fn manual_self_tune_high_volume_cap_beats_miss_relax() {
        let mut state = PairLockManualSelfTuneState::default();
        state.ptb_relax_credit_cent = 20.0;
        state.max_price_relax_credit_cent = 10.0;
        let mut input = input("high", "expanding");
        input.base_max_price = Some(0.85);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(
            enable_self_tune(input, state, false),
        );

        assert_eq!(decision.reason, "high_volume_strict");
        assert_eq!(decision.effective_max_price, Some(0.58));
        assert_eq!(decision.effective_ptb_threshold_value, Some(45.0));
    }

    #[test]
    fn manual_self_tune_lockdown_blocks_scope_side() {
        let mut state = PairLockManualSelfTuneState::default();
        state.lockdown_markets_left = 3;
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(
            enable_self_tune(input("normal", "expanding"), state, false),
        );

        assert_eq!(decision.decision, "BLOCK");
        assert_eq!(decision.reason, "manual_self_tune_lockdown");
    }

    #[test]
    fn manual_adaptive_strict_ptb_can_block_otherwise_passed_candidate() {
        let mut input = input("elevated", "expanding");
        input.ptb_directional_gap = Some(0.25);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);

        assert_eq!(decision.decision, "BLOCK");
        assert_eq!(decision.reason, "manual_adaptive_ptb_gap_below_strict_threshold");
    }

    #[test]
    fn manual_adaptive_counter_pair_cap_blocks_when_vwap_breaks_pair_total() {
        let mut input = input("normal", "expanding");
        input.primary_estimated_avg_fill = Some(0.70);
        input.counter_estimated_avg_fill = Some(0.27);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);

        assert_eq!(decision.decision, "BLOCK");
        assert_eq!(decision.reason, "manual_adaptive_pair_cap_block");
    }

    #[test]
    fn manual_adaptive_runtime_gap_delta_is_side_specific() {
        let mut context = json!({});
        let first = pair_lock_manual_adaptive_ptb_trend(
            &mut context,
            "node",
            "eth-updown-5m-1",
            "Up",
            Some(0.20),
            0.05,
        );
        let second = pair_lock_manual_adaptive_ptb_trend(
            &mut context,
            "node",
            "eth-updown-5m-1",
            "Up",
            Some(0.28),
            0.05,
        );
        let down = pair_lock_manual_adaptive_ptb_trend(
            &mut context,
            "node",
            "eth-updown-5m-1",
            "Down",
            Some(0.10),
            0.05,
        );

        assert_eq!(first, "flat");
        assert_eq!(second, "expanding");
        assert_eq!(down, "flat");
    }

    fn notify_config() -> PairLockManualAdaptiveNotifyConfig {
        PairLockManualAdaptiveNotifyConfig {
            notify_block: true,
            notify_strict: true,
            notify_sl_bump: true,
            notify_summary: true,
            notify_counter_cap: true,
            min_interval_sec: 30,
            summary_every_markets: 5,
            counter_cap_notify_min_delta_cent: 3.0,
            include_payload: false,
        }
    }

    #[test]
    fn manual_adaptive_counter_cap_notify_payload_requires_material_clamp() {
        let mut input = input("high", "expanding");
        input.primary_estimated_avg_fill = Some(0.56);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);
        let (payload, reason, force) =
            pair_lock_manual_counter_cap_event_payload(&decision.diagnostics, notify_config())
                .expect("material counter clamp should notify");

        assert_eq!(reason, "pair_cap_protection");
        assert!(!force);
        assert_eq!(
            payload
                .pointer("/counter_dynamic_cap/base_counter_max_cent")
                .and_then(value_as_f64),
            Some(70.0)
        );
        assert_eq!(
            payload
                .pointer("/counter_dynamic_cap/effective_counter_max_cent")
                .and_then(value_as_f64),
            Some(39.0)
        );
    }

    #[test]
    fn manual_adaptive_counter_cap_notify_payload_skips_small_clamp() {
        let mut input = input("high", "expanding");
        input.primary_estimated_avg_fill = Some(0.27);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);

        assert!(
            pair_lock_manual_counter_cap_event_payload(&decision.diagnostics, notify_config())
                .is_none()
        );
    }

    #[test]
    fn manual_adaptive_counter_cap_notify_payload_respects_disabled_flag() {
        let mut input = input("high", "expanding");
        input.primary_estimated_avg_fill = Some(0.56);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);
        let mut cfg = notify_config();
        cfg.notify_counter_cap = false;

        assert!(pair_lock_manual_counter_cap_notify_payload(&decision.diagnostics, cfg).is_none());
    }

    #[test]
    fn manual_adaptive_counter_cap_notify_payload_marks_floor_block() {
        let mut input = input("high", "expanding");
        input.primary_estimated_avg_fill = Some(0.90);
        let decision = evaluate_pair_lock_manual_adaptive_risk_decision(input);
        let (_payload, reason, force) =
            pair_lock_manual_counter_cap_event_payload(&decision.diagnostics, notify_config())
                .expect("below-floor counter cap should notify");

        assert_eq!(reason, "counter_cap_below_floor");
        assert!(force);
    }
}
