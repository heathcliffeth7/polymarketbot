    use super::*;

    fn sample(ts: i64, price: f64) -> ChainlinkPriceSample {
        ChainlinkPriceSample {
            timestamp_ms: ts,
            price,
        }
    }

    fn input<'a>(
        config: &'a IvEntryQualityConfig,
        ask: f64,
        q_final: Option<f64>,
        samples: &'a [ChainlinkPriceSample],
    ) -> IvEntryQualityInput<'a> {
        IvEntryQualityInput {
            config,
            side: "up",
            price_to_beat: 100.0,
            current_price: 130.0,
            samples,
            latest_timestamp_ms: 20_000,
            seconds_left: 30.0,
            ask,
            spread: 0.01,
            chainlink_age_ms: Some(500),
            expected_move_raw: 10.0,
            expected_move_eff: 10.0,
            q_final,
            fee: 0.002,
            buffer: 0.003,
            dynamic_threshold: 0.005,
            configured_max_price: Some(0.96),
            gap_velocity: Some(1.0),
            cex_price: Some(130.03),
            cex_fresh: true,
            cex_same_direction: Some(true),
            rule_required_gap_strength: None,
            rule_gap_strength_margin: None,
        }
    }

    #[test]
    fn bps_floor_uses_ten_thousand_denominator() {
        let config = IvEntryQualityConfig::default();
        assert_eq!(config.expected_move_floor(60_000.0), 12.0);
    }

    #[test]
    fn opposite_side_history_is_not_same_side() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(0, 90.0),
            sample(10_000, 95.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.93, Some(0.99), &samples));

        assert_eq!(decision.same_side_history_10s, Some(false));
        assert_eq!(decision.buffer_retain_10s, None);
        assert_eq!(decision.same_side_history_5s, Some(true));
    }

    #[test]
    fn premium_requires_same_side_history_and_ev() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(10_000, 95.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.95, None, &samples));

        assert!(!decision.allowed);
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::MissingSameSideHistoryForPremium));
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::PremiumEvMissing));
    }

    #[test]
    fn premium_blocks_insufficient_fair_probability() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.95, Some(0.955), &samples));

        assert!(!decision.allowed);
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::PremiumEvInsufficient));
    }

    #[test]
    fn premium_blocks_negative_velocity_even_when_retain_passes() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let mut input = input(&config, 0.95, Some(0.99), &samples);
        input.gap_velocity = Some(-0.1);
        let decision = evaluate_iv_entry_quality(input);

        assert!(!decision.allowed);
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::NegativeVelocityPremium));
    }

    #[test]
    fn price_boundaries_split_normal_premium_and_premium_max() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];

        let normal = evaluate_iv_entry_quality(input(&config, 0.94, Some(0.99), &samples));
        assert!(normal.allowed, "{normal:?}");
        assert!(normal.premium_price_allowed);

        let premium = evaluate_iv_entry_quality(input(&config, 0.9600, Some(0.99), &samples));
        assert!(premium.allowed, "{premium:?}");
        assert_eq!(premium.effective_max_buy_price, Some(96.0));

        let above_premium = evaluate_iv_entry_quality(input(&config, 0.9601, Some(0.99), &samples));
        assert!(!above_premium.allowed);
        assert!(above_premium
            .all_reasons
            .contains(&IvEntryQualityReason::PriceAbovePremiumMax));
    }

    #[test]
    fn seconds_left_boundary_blocks_only_below_configured_window() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let mut at_boundary = input(&config, 0.93, Some(0.99), &samples);
        at_boundary.seconds_left = 8.0;

        assert!(evaluate_iv_entry_quality(at_boundary).allowed);

        let mut below_boundary = input(&config, 0.93, Some(0.99), &samples);
        below_boundary.seconds_left = 7.99;
        let decision = evaluate_iv_entry_quality(below_boundary);

        assert!(!decision.allowed);
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::BelowNoNewEntryWindow));
    }

    #[test]
    fn spike_without_retrace_does_not_block() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(0, 100.0),
            sample(5_000, 101.0),
            sample(10_000, 102.0),
            sample(15_000, 115.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.93, Some(0.99), &samples));

        assert!(!decision
            .all_reasons
            .contains(&IvEntryQualityReason::SpikeFade));
    }

    #[test]
    fn spike_with_retrace_blocks() {
        let mut config = IvEntryQualityConfig::default();
        config.enabled = true;
        let samples = [
            sample(-100_000, 100.0),
            sample(-80_000, 101.0),
            sample(-60_000, 100.0),
            sample(-40_000, 101.0),
            sample(-20_000, 100.0),
            sample(0, 100.0),
            sample(5_000, 100.0),
            sample(10_000, 150.0),
            sample(15_000, 150.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.93, Some(0.99), &samples));

        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::SpikeFade));
    }

    #[test]
    fn eq77_risk_cap_waits_for_price_and_requires_recheck() {
        let mut config = IvEntryQualityConfig {
            enabled: true,
            eq77_risk_cap_enabled: true,
            normal_max_price: 0.77,
            premium_max_price: 0.78,
            ..IvEntryQualityConfig::default()
        };
        config.odds_max_spread = 0.05;
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(17_000, 132.0),
            sample(20_000, 130.0),
        ];
        let mut input = input(&config, 0.75, Some(0.99), &samples);
        input.spread = 0.06;
        input.cex_same_direction = Some(false);
        let decision = evaluate_iv_entry_quality(input);

        assert!(!decision.allowed);
        assert_eq!(decision.entry_action, "wait_for_price");
        assert!(decision.deferred);
        assert!(decision.signal_recheck_required);
        assert_eq!(decision.risk_level, Some("moderate"));
        assert_eq!(decision.effective_max_buy_price, Some(74.0));
        assert_eq!(decision.ask_over_cap_cent, Some(1.0));
    }

    #[test]
    fn eq77_lite_soft_low_gap_waits_or_submits_but_floor_still_blocks() {
        let config = IvEntryQualityConfig {
            enabled: true,
            eq77_risk_cap_enabled: true,
            normal_max_price: 0.77,
            high_risk_max_price: 0.73,
            deep_value_max_price: 0.68,
            risk_score_clean_max: 0.0,
            risk_score_moderate_max: 0.0,
            risk_score_high_max: 90.0,
            ..IvEntryQualityConfig::default()
        };
        let samples = [
            sample(10_000, 104.0),
            sample(15_000, 106.0),
            sample(20_000, 108.0),
        ];
        let mut base = input(&config, 0.75, Some(0.99), &samples);
        base.current_price = 108.0;
        base.expected_move_eff = 10.0;
        base.expected_move_raw = 10.0;
        base.rule_required_gap_strength = Some(1.45);
        base.rule_gap_strength_margin = Some(0.03);

        for (ask, allowed, action) in [
            (0.75, false, "wait_for_price"),
            (0.72, true, "submit_order"),
        ] {
            let mut case = base;
            case.ask = ask;
            let decision = evaluate_iv_entry_quality(case);
            assert_eq!(decision.allowed, allowed, "{decision:?}");
            assert_eq!(decision.entry_action, action);
            assert_eq!(decision.lane, Some("high"));
            assert_eq!(decision.risk_cap_price_cent, Some(73.0));
            assert_eq!(decision.gap_strength_soft_low, Some(true));
        }

        let mut below_floor = base;
        below_floor.ask = 0.60;
        below_floor.current_price = 107.0;
        let decision = evaluate_iv_entry_quality(below_floor);
        assert!(!decision.allowed);
        assert_eq!(decision.entry_action, "hard_block");
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::GapStrengthTooLow));
        assert_eq!(decision.gap_strength_soft_low, Some(false));
    }

    #[test]
    fn eq77_deep_value_can_pass_high_score_without_hard_red_flag() {
        let config = IvEntryQualityConfig {
            enabled: true,
            eq77_risk_cap_enabled: true,
            normal_max_price: 0.77,
            cex_unconfirmed_risk_points: 80.0,
            ..IvEntryQualityConfig::default()
        };
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let mut input = input(&config, 0.64, Some(0.99), &samples);
        input.cex_fresh = false;
        input.cex_same_direction = None;
        let decision = evaluate_iv_entry_quality(input);

        assert!(decision.allowed, "{decision:?}");
        assert_eq!(decision.lane, Some("deep_value"));
        assert_eq!(decision.size_multiplier, Some(0.5));
        assert!(decision.risk_score.is_some_and(|score| score > 70.0));
    }

    #[test]
    fn eq77_deep_value_does_not_override_stop_cushion_hard_fail() {
        let mut config = IvEntryQualityConfig {
            enabled: true,
            eq77_risk_cap_enabled: true,
            normal_max_price: 0.77,
            ..IvEntryQualityConfig::default()
        };
        config.gap_strength_min_45_to_25 = 0.01;
        let samples = [
            sample(10_000, 101.0),
            sample(15_000, 103.0),
            sample(20_000, 104.0),
        ];
        let mut input = input(&config, 0.64, Some(0.99), &samples);
        input.current_price = 104.0;
        input.expected_move_eff = 50.0;
        input.expected_move_raw = 50.0;
        let decision = evaluate_iv_entry_quality(input);

        assert!(!decision.allowed);
        assert_eq!(decision.entry_action, "hard_block");
        assert!(decision.hard_block);
        assert!(decision
            .all_reasons
            .contains(&IvEntryQualityReason::RiskCapHardBlock));
    }

    #[test]
    fn eq77_ev_cap_can_lower_effective_cap_below_risk_cap() {
        let config = IvEntryQualityConfig {
            enabled: true,
            eq77_risk_cap_enabled: true,
            normal_max_price: 0.77,
            ..IvEntryQualityConfig::default()
        };
        let samples = [
            sample(10_000, 120.0),
            sample(15_000, 125.0),
            sample(20_000, 130.0),
        ];
        let decision = evaluate_iv_entry_quality(input(&config, 0.72, Some(0.72), &samples));

        assert!(!decision.allowed);
        assert_eq!(decision.entry_action, "wait_for_price");
        assert_eq!(decision.risk_cap_price_cent, Some(77.0));
        assert_eq!(decision.effective_max_buy_price, Some(71.0));
    }
