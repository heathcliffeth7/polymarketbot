mod revenge_flip_tests {
    use super::*;

    fn revenge_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "rf".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn quote(side: &str, ask: f64, bid: f64) -> RevengeFlipSideQuote {
        quote_for_market(side, ask, bid, "btc-updown-5m-1774013100")
    }

    fn quote_for_market(side: &str, ask: f64, bid: f64, market_slug: &str) -> RevengeFlipSideQuote {
        RevengeFlipSideQuote {
            market_slug: market_slug.to_string(),
            revenge_side: side.to_string(),
            token_id: format!("{side}-token"),
            outcome_label: if side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(bid),
            best_ask: Some(ask),
            current_price: ask,
            snapshot: json!({}),
        }
    }

    fn revenge_flip_ptb_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("revenge flip ptb test lock")
    }

    fn seed_revenge_flip_entry_ptb(
        market_slug: &str,
        price_to_beat: f64,
        current_price: f64,
    ) -> std::sync::MutexGuard<'static, ()> {
        let guard = revenge_flip_ptb_test_lock();
        assert!(
            trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
                market_slug,
                "btc",
                "5m",
                price_to_beat,
                Some(0),
            )
        );
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, current_price), (now_ms, current_price)],
        )
        .expect("seed chainlink ticks");
        guard
    }

    fn unwrap_ready_sizing(decision: RevengeFlipEntrySizingDecision) -> RevengeFlipEntrySizing {
        match decision {
            RevengeFlipEntrySizingDecision::Ready(sizing) => sizing,
            other => panic!("expected ready sizing, got {other:?}"),
        }
    }

    #[test]
    fn revenge_flip_config_defaults_and_validation() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1"
        })))
        .expect("valid defaults");

        assert_eq!(cfg.initial_order_usdc, 5.0);
        assert_eq!(cfg.profit_target_usdc, 0.25);
        assert!(cfg.classic_stop_loss_enabled);
        assert_eq!(cfg.stop_loss_pct, 0.20);
        assert!(cfg.stop_loss_rules.is_empty());
        assert!(cfg.entry_ptb_rules.is_empty());
        assert_eq!(cfg.reentry_side_mode, "opposite");
        assert!(!cfg.ptb_stop_loss.enabled);
        assert_eq!(cfg.min_reentry_shares, 0.0);
        assert_eq!(cfg.lot_limit_pct, 0.98);
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "stopLossPct": 1.2 }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "entryPtbRules": [
                    { "minFlip": 0, "priceToBeatMaxDiff": 10, "maxPriceCent": 101 }
                ]
            }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "reentrySideMode": "both" }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "minReentryShares": -1 }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "entryPtbRules": [{ "minFlip": 0, "sideMode": "sideways", "priceToBeatMaxDiff": 10 }]
            }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "ptbStopLossEnabled": true }
        })))
        .is_err());
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "classicStopLossEnabled": false }
        })))
        .is_err());
        let ptb_only = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "classicStopLossEnabled": false,
                "stopLossPct": 1.2,
                "ptbStopLossEnabled": true,
                "ptbStopLossGapUsd": 1,
                "ptbStopLossCurrentPriceSource": "cex_consensus",
                "ptbStopLossTimeDecayMode": "none"
            }
        })))
        .expect("valid ptb-only config");
        assert!(!ptb_only.classic_stop_loss_enabled);
        assert!(ptb_only.ptb_stop_loss.enabled);
        assert_eq!(ptb_only.ptb_stop_loss.gap_usd, Some(1.0));
        assert_eq!(ptb_only.ptb_stop_loss.current_price_source, "cex_consensus");
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "ptbStopLossEnabled": true,
                "ptbStopLossGapUsd": 0,
                "ptbStopLossCurrentPriceSource": "kraken"
            }
        })))
        .is_err());
    }

    #[test]
    fn revenge_flip_stop_loss_rules_override_fixed_pct_by_flip_index() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "stopLossPct": 0.2,
                "stopLossRules": [
                    { "minFlip": 0, "maxFlip": 0, "stopLossPct": 0.25 },
                    { "minFlip": 1, "maxFlip": 2, "stopLossPct": 0.15 },
                    { "minFlip": 3, "stopLossPct": 0.1 }
                ]
            }
        })))
        .expect("valid config");

        assert_eq!(revenge_flip_stop_loss_pct_for_entry(&cfg, 0), 0.25);
        assert_eq!(revenge_flip_stop_loss_pct_for_entry(&cfg, 1), 0.15);
        assert_eq!(revenge_flip_stop_loss_pct_for_entry(&cfg, 2), 0.15);
        assert_eq!(revenge_flip_stop_loss_pct_for_entry(&cfg, 3), 0.1);
        assert_eq!(revenge_flip_stop_loss_pct_for_entry(&cfg, 9), 0.1);
        assert!(resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "stopLossPct": 0.2,
                "stopLossRules": [{ "minFlip": 2, "maxFlip": 1, "stopLossPct": 0.1 }]
            }
        })))
        .is_err());
    }

    #[test]
    fn revenge_flip_entry_ptb_rules_override_by_flip_and_time() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatMaxDiff": 1,
            "priceToBeatMaxDiffUnit": "cent",
            "revengeFlip": {
                "ptbStopLossBumpEnabled": true,
                "ptbStopLossBumpAmount": 0.5,
                "ptbStopLossBumpUnit": "cent",
                "entryPtbRules": [
                    {
                        "minFlip": 0,
                        "maxFlip": 0,
                        "minRemainingSec": 0,
                        "maxRemainingSec": 300,
                        "priceToBeatMinDiff": 5,
                        "priceToBeatMinDiffUnit": "cent",
                        "maxPriceCent": 80
                    },
                    {
                        "minFlip": 1,
                        "priceToBeatMinDiff": 2,
                        "priceToBeatMinDiffUnit": "cent",
                        "maxPriceCent": 90
                    }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            ptb_bump_count: 2,
            ..TradeBuilderRevengeFlipState::default()
        };

        let initial = revenge_flip_effective_ptb(&cfg, &state, 0, Some(120), None, None);
        assert_eq!(initial.base_source, "entry_ptb_rule");
        assert_eq!(initial.unit, "cent");
        assert!((initial.max_diff - 6.0).abs() < 0.000001);
        assert!(initial.matched_entry_rule.is_some());
        assert_eq!(
            revenge_flip_effective_entry_price(&cfg, 0, Some(120), None, None).max_cent,
            Some(80.0)
        );

        let flip = revenge_flip_effective_ptb(&cfg, &state, 1, None, None, None);
        assert_eq!(flip.base_source, "entry_ptb_rule");
        assert!((flip.max_diff - 3.0).abs() < 0.000001);
        assert_eq!(
            revenge_flip_effective_entry_price(&cfg, 1, None, None, None).max_cent,
            Some(90.0)
        );

        let blocked_by_time = revenge_flip_effective_ptb(&cfg, &state, 0, Some(400), None, None);
        assert_eq!(blocked_by_time.base_source, "global");
        assert!((blocked_by_time.max_diff - 2.0).abs() < 0.000001);
        assert_eq!(
            revenge_flip_effective_entry_price(&cfg, 0, Some(400), None, None).max_cent,
            None
        );
    }

    #[test]
    fn revenge_flip_time_rule_and_ptb_bump_override_threshold() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatMaxDiff": 1.0,
            "priceToBeatMaxDiffUnit": "cent",
            "revengeFlip": {
                "ptbStopLossBumpEnabled": true,
                "ptbStopLossBumpAmount": 0.5,
                "ptbStopLossBumpUnit": "cent",
                "ptbStopLossBumpMax": 2.0,
                "ptbStopLossBumpMaxUnit": "cent",
                "timeRules": [
                    {
                        "minRemainingSec": 20,
                        "maxRemainingSec": 40,
                        "priceToBeatMaxDiff": 0.02,
                        "priceToBeatMaxDiffUnit": "usd"
                    }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            ptb_bump_count: 3,
            ..TradeBuilderRevengeFlipState::default()
        };

        let effective = revenge_flip_effective_ptb(&cfg, &state, 0, Some(30), None, None);
        assert_eq!(effective.base_source, "time_rule");
        assert_eq!(effective.unit, "usd");
        assert!((effective.max_diff - 0.035).abs() < 0.000001);
    }

    #[test]
    fn revenge_flip_trigger_price_range_passes_and_blocks() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "triggerPrice": { "enabled": true, "minCent": 45, "maxCent": 55 }
        })))
        .expect("valid config");
        let effective_entry_price = revenge_flip_effective_entry_price(&cfg, 0, None, None, None);

        assert!(revenge_flip_entry_price_passes(
            &cfg,
            &effective_entry_price,
            0.50
        ));
        assert!(!revenge_flip_entry_price_passes(
            &cfg,
            &effective_entry_price,
            0.56
        ));
    }

    #[test]
    fn revenge_flip_entry_max_price_overrides_global_trigger_max() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "triggerPrice": { "enabled": true, "minCent": 40, "maxCent": 80 },
            "revengeFlip": {
                "entryPtbRules": [
                    {
                        "minFlip": 0,
                        "maxFlip": 0,
                        "priceToBeatMaxDiff": 10,
                        "priceToBeatMaxDiffUnit": "cent",
                        "maxPriceCent": 80
                    },
                    {
                        "minFlip": 1,
                        "priceToBeatMaxDiff": 2,
                        "priceToBeatMaxDiffUnit": "cent",
                        "maxPriceCent": 90
                    }
                ]
            }
        })))
        .expect("valid config");

        let initial_price = revenge_flip_effective_entry_price(&cfg, 0, None, None, None);
        assert_eq!(initial_price.max_cent, Some(80.0));
        assert_eq!(initial_price.max_source, "entry_ptb_rule");
        assert!(revenge_flip_entry_price_passes(&cfg, &initial_price, 0.80));
        assert!(!revenge_flip_entry_price_passes(&cfg, &initial_price, 0.81));

        let flip_price = revenge_flip_effective_entry_price(&cfg, 1, None, None, None);
        assert_eq!(flip_price.max_cent, Some(90.0));
        assert_eq!(flip_price.max_source, "entry_ptb_rule");
        assert!(revenge_flip_entry_price_passes(&cfg, &flip_price, 0.90));
        assert!(!revenge_flip_entry_price_passes(&cfg, &flip_price, 0.91));

        let fallback_cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "triggerPrice": { "enabled": true, "minCent": 40, "maxCent": 70 },
            "revengeFlip": {
                "entryPtbRules": [
                    { "minFlip": 1, "priceToBeatMaxDiff": 2, "priceToBeatMaxDiffUnit": "cent" }
                ]
            }
        })))
        .expect("valid fallback config");
        let fallback = revenge_flip_effective_entry_price(&fallback_cfg, 1, None, None, None);
        assert_eq!(fallback.max_cent, Some(70.0));
        assert_eq!(fallback.max_source, "trigger_price");
    }

    #[test]
    fn revenge_flip_entry_side_modes_match_by_previous_stop_loss_side() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "entryPtbRules": [
                    { "minFlip": 1, "sideMode": "same", "priceToBeatMaxDiff": 1, "priceToBeatMaxDiffUnit": "cent" },
                    { "minFlip": 1, "sideMode": "opposite", "priceToBeatMaxDiff": 2, "priceToBeatMaxDiffUnit": "cent" },
                    { "minFlip": 0, "sideMode": "up", "priceToBeatMaxDiff": 3, "priceToBeatMaxDiffUnit": "cent" },
                    { "minFlip": 0, "sideMode": "any", "priceToBeatMaxDiff": 4, "priceToBeatMaxDiffUnit": "cent" }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState::default();

        let same = revenge_flip_effective_ptb(&cfg, &state, 1, None, Some("up"), Some("up"));
        assert_eq!(same.max_diff, 1.0);
        let opposite = revenge_flip_effective_ptb(&cfg, &state, 1, None, Some("down"), Some("up"));
        assert_eq!(opposite.max_diff, 2.0);
        let initial_up = revenge_flip_effective_ptb(&cfg, &state, 0, None, Some("up"), None);
        assert_eq!(initial_up.max_diff, 3.0);
        let initial_down = revenge_flip_effective_ptb(&cfg, &state, 0, None, Some("down"), None);
        assert_eq!(initial_down.max_diff, 4.0);
    }

    #[tokio::test]
    async fn revenge_flip_rule_match_can_reenter_same_side_after_stop_loss() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 1, "sideMode": "same", "priceToBeatMaxDiff": 4, "priceToBeatMaxDiffUnit": "cent", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            flip_count: 1,
            total_loss_usdc: 3.0,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let market_slug = "btc-updown-5m-1774013100";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 101.0);
        let quotes = vec![
            quote_for_market("up", 0.55, 0.54, market_slug),
            quote_for_market("down", 0.40, 0.39, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("rule match candidate");
        assert_eq!(candidate.quote.revenge_side, "up");
        assert_eq!(candidate.effective_ptb.max_diff, 4.0);
    }

    #[tokio::test]
    async fn revenge_flip_initial_rule_match_any_uses_rule_order_not_cheapest() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "any", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState::default();
        let market_slug = "btc-updown-5m-1774013400";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 110.0);
        let quotes = vec![
            quote_for_market("up", 0.60, 0.59, market_slug),
            quote_for_market("down", 0.40, 0.39, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("initial rule-order candidate");
        assert_eq!(candidate.quote.revenge_side, "up");
        assert_eq!(candidate.selection_mode, "initial_rule_order");
        assert_eq!(candidate.effective_ptb.unit, "usd");
        assert_eq!(candidate.effective_ptb.max_diff, 10.0);
    }

    #[tokio::test]
    async fn revenge_flip_initial_rule_match_up_rule_wins_even_when_down_is_cheaper() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "up", "priceToBeatMaxDiff": 5, "priceToBeatMaxDiffUnit": "cent", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState::default();
        let market_slug = "btc-updown-5m-1774013700";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 100.1);
        let quotes = vec![
            quote_for_market("up", 0.70, 0.69, market_slug),
            quote_for_market("down", 0.30, 0.29, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("initial up-rule candidate");
        assert_eq!(candidate.quote.revenge_side, "up");
        assert_eq!(candidate.selection_mode, "initial_rule_order");
    }

    #[tokio::test]
    async fn revenge_flip_initial_rule_order_falls_through_when_first_rule_price_blocks() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "up", "priceToBeatMaxDiff": 5, "priceToBeatMaxDiffUnit": "cent", "maxPriceCent": 50 },
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "down", "priceToBeatMaxDiff": 7, "priceToBeatMaxDiffUnit": "cent", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState::default();
        let market_slug = "btc-updown-5m-1774014000";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 99.9);
        let quotes = vec![
            quote_for_market("up", 0.60, 0.59, market_slug),
            quote_for_market("down", 0.55, 0.54, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("second rule candidate");
        assert_eq!(candidate.quote.revenge_side, "down");
        assert_eq!(candidate.selection_mode, "initial_rule_order");
        assert!((candidate.effective_ptb.max_diff - 7.0).abs() < 0.000001);
    }

    #[tokio::test]
    async fn revenge_flip_rule_match_any_picks_lowest_eligible_ask() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 1, "sideMode": "any", "priceToBeatMaxDiff": 4, "priceToBeatMaxDiffUnit": "cent", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            flip_count: 1,
            total_loss_usdc: 3.0,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let market_slug = "btc-updown-5m-1774014300";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 101.0);
        let quotes = vec![
            quote_for_market("up", 0.44, 0.43, market_slug),
            quote_for_market("down", 0.60, 0.59, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("rule match candidate");
        assert_eq!(candidate.quote.revenge_side, "up");
    }

    #[tokio::test]
    async fn revenge_flip_initial_any_selects_down_when_current_below_ptb_by_min_diff() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "any", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let market_slug = "btc-updown-5m-1774014600";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 90.0);
        let quotes = vec![
            quote_for_market("up", 0.37, 0.36, market_slug),
            quote_for_market("down", 0.64, 0.63, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quotes,
            None,
        )
        .expect("down candidate");
        assert_eq!(candidate.quote.revenge_side, "down");
        assert_eq!(candidate.entry_ptb_guard.directional_gap, Some(10.0));
        assert_eq!(candidate.entry_ptb_guard.abs_gap, Some(10.0));
    }

    #[tokio::test]
    async fn revenge_flip_initial_any_selects_up_when_current_above_ptb_by_min_diff() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "any", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let market_slug = "btc-updown-5m-1774014900";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 110.0);
        let quotes = vec![
            quote_for_market("up", 0.64, 0.63, market_slug),
            quote_for_market("down", 0.37, 0.36, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quotes,
            None,
        )
        .expect("up candidate");
        assert_eq!(candidate.quote.revenge_side, "up");
        assert_eq!(candidate.entry_ptb_guard.directional_gap, Some(10.0));
    }

    #[tokio::test]
    async fn revenge_flip_initial_any_blocks_when_abs_gap_below_min_diff() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "any", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let market_slug = "btc-updown-5m-1774015200";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 105.0);
        let quotes = vec![
            quote_for_market("up", 0.55, 0.54, market_slug),
            quote_for_market("down", 0.45, 0.44, market_slug),
        ];

        assert!(revenge_flip_select_entry_candidate(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quotes,
            None
        )
        .is_none());
    }

    #[tokio::test]
    async fn revenge_flip_explicit_side_requires_directional_ptb_pass() {
        let up_cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "up", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid up config");
        let down_cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 0, "maxFlip": 0, "sideMode": "down", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid down config");
        let market_slug = "btc-updown-5m-1774015500";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 90.0);
        let quotes = vec![
            quote_for_market("up", 0.37, 0.36, market_slug),
            quote_for_market("down", 0.64, 0.63, market_slug),
        ];

        assert!(revenge_flip_select_entry_candidate(
            &up_cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quotes,
            None
        )
        .is_none());
        let down_candidate = revenge_flip_select_entry_candidate(
            &down_cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quotes,
            None,
        )
        .expect("down candidate");
        assert_eq!(down_candidate.quote.revenge_side, "down");
    }

    #[tokio::test]
    async fn revenge_flip_reentry_skips_cheaper_candidate_when_ptb_fails() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatCurrentPriceSource": "chainlink",
            "revengeFlip": {
                "reentrySideMode": "rule_match",
                "entryPtbRules": [
                    { "minFlip": 1, "sideMode": "any", "priceToBeatMinDiff": 10, "priceToBeatMinDiffUnit": "usd", "maxPriceCent": 80 }
                ]
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            flip_count: 1,
            total_loss_usdc: 3.0,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let market_slug = "btc-updown-5m-1774015800";
        let _ptb_lock = seed_revenge_flip_entry_ptb(market_slug, 100.0, 90.0);
        let quotes = vec![
            quote_for_market("up", 0.20, 0.19, market_slug),
            quote_for_market("down", 0.64, 0.63, market_slug),
        ];

        let candidate = revenge_flip_select_entry_candidate(&cfg, &state, &quotes, None)
            .expect("down candidate");
        assert_eq!(candidate.quote.revenge_side, "down");
        assert_eq!(candidate.selection_mode, "rule_match_lowest_ask");
    }

    #[test]
    fn revenge_flip_stop_loss_uses_best_bid_threshold() {
        let _cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": { "stopLossPct": 0.5 }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            position_avg_cost: 0.50,
            position_stop_loss_enabled: true,
            position_stop_loss_pct: 0.2,
            ..TradeBuilderRevengeFlipState::default()
        };

        assert!(revenge_flip_stop_loss_triggered(&state, 0.40));
        assert!(!revenge_flip_stop_loss_triggered(&state, 0.41));
        assert!(!revenge_flip_stop_loss_triggered(
            &TradeBuilderRevengeFlipState {
                position_avg_cost: 0.50,
                position_stop_loss_enabled: false,
                position_stop_loss_pct: 0.2,
                ..TradeBuilderRevengeFlipState::default()
            },
            0.30
        ));
    }

    #[tokio::test]
    async fn revenge_flip_ptb_stop_loss_can_trigger_without_price_hit() {
        let _ptb_lock = revenge_flip_ptb_test_lock();
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, 90.0), (now_ms, 90.0)],
        )
        .expect("seed chainlink ticks");
        let evaluation = trade_builder_evaluate_ptb_stop_loss_inputs(
            "btc-updown-5m-1774013100",
            "Up",
            -10.0,
            Some(100.0),
            PriceToBeatCurrentPriceSource::Chainlink,
            Some("none"),
        );

        assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
        assert_eq!(evaluation.directional_gap, Some(-10.0));
        assert!(evaluation.should_trigger);
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, 100.5), (now_ms, 100.5)],
        )
        .expect("seed chainlink ticks");
        let one_usd_hit = trade_builder_evaluate_ptb_stop_loss_inputs(
            "btc-updown-5m-1774013100",
            "Up",
            1.0,
            Some(100.0),
            PriceToBeatCurrentPriceSource::Chainlink,
            Some("none"),
        );
        assert_eq!(one_usd_hit.directional_gap, Some(0.5));
        assert!(one_usd_hit.should_trigger);
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, 102.0), (now_ms, 102.0)],
        )
        .expect("seed chainlink ticks");
        let one_usd_hold = trade_builder_evaluate_ptb_stop_loss_inputs(
            "btc-updown-5m-1774013100",
            "Up",
            1.0,
            Some(100.0),
            PriceToBeatCurrentPriceSource::Chainlink,
            Some("none"),
        );
        assert_eq!(one_usd_hold.directional_gap, Some(2.0));
        assert!(!one_usd_hold.should_trigger);
        assert_eq!(
            revenge_flip_stop_loss_trigger_source(false, true, false),
            Some("ptb")
        );
        assert_eq!(
            revenge_flip_stop_loss_trigger_source(true, true, false),
            Some("both")
        );
        assert_eq!(
            revenge_flip_stop_loss_trigger_source(false, false, true),
            Some("token")
        );
    }

    #[test]
    fn revenge_flip_buy_node_carries_entry_stop_loss_marker() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1"
        })))
        .expect("valid config");
        let effective_ptb = revenge_flip_effective_ptb(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            0,
            None,
            None,
            None,
        );
        let node = revenge_flip_buy_node(
            &revenge_node(json!({ "mode": "revenge_flip_v1" })),
            &cfg,
            &effective_ptb,
            Some(80.0),
            &quote("up", 0.50, 0.49),
            5.0,
            0.17,
            "initial_buy",
            1,
        );

        assert_eq!(
            node.config
                .get(REVENGE_FLIP_STOP_LOSS_PCT_KEY)
                .and_then(Value::as_f64),
            Some(0.17)
        );
        assert_eq!(
            node.config
                .get(REVENGE_FLIP_STOP_LOSS_ENABLED_KEY)
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            node.config.get("maxPriceCent").and_then(Value::as_f64),
            Some(80.0)
        );
    }

    #[test]
    fn revenge_flip_buy_node_carries_disabled_classic_stop_marker() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "classicStopLossEnabled": false,
                "ptbStopLossEnabled": true,
                "ptbStopLossGapUsd": 1,
                "ptbStopLossTimeDecayMode": "none"
            }
        })))
        .expect("valid config");
        let effective_ptb = revenge_flip_effective_ptb(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            0,
            None,
            None,
            None,
        );
        let node = revenge_flip_buy_node(
            &revenge_node(json!({ "mode": "revenge_flip_v1" })),
            &cfg,
            &effective_ptb,
            Some(80.0),
            &quote("up", 0.50, 0.49),
            5.0,
            0.17,
            "initial_buy",
            1,
        );

        assert_eq!(
            node.config
                .get(REVENGE_FLIP_STOP_LOSS_ENABLED_KEY)
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn revenge_flip_initial_and_flip_sizing_apply_lot_cap_and_max_flip() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "initialOrderUsdc": 7,
                "profitTargetUsdc": 1,
                "lotLimitPct": 0.5,
                "maxFlip": 2
            }
        })))
        .expect("valid config");
        let initial = revenge_flip_entry_notional(
            &cfg,
            &TradeBuilderRevengeFlipState::default(),
            &quote("up", 0.50, 0.49),
            None,
        )
        .expect("sizing");
        let initial = unwrap_ready_sizing(initial);
        assert_eq!(initial.notional_usdc, 7.0);
        assert_eq!(initial.target_shares, 0.0);

        let flip_state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 3.0,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let flip =
            revenge_flip_entry_notional(&cfg, &flip_state, &quote("down", 0.60, 0.59), Some(5.0))
                .expect("sizing");
        let flip = unwrap_ready_sizing(flip);
        assert_eq!(flip.notional_usdc, 2.5);
        assert_eq!(flip.max_notional_usdc, Some(2.5));
        assert!(revenge_flip_max_flip_allows(&cfg, &flip_state));
        assert!(!revenge_flip_max_flip_allows(
            &cfg,
            &TradeBuilderRevengeFlipState {
                flip_count: 2,
                ..flip_state
            }
        ));
    }

    #[test]
    fn revenge_flip_reentry_min_shares_floors_flip_sizing_only() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "initialOrderUsdc": 2,
                "profitTargetUsdc": 0.25,
                "minReentryShares": 5,
                "lotLimitPct": 1
            }
        })))
        .expect("valid config");
        let initial = unwrap_ready_sizing(
            revenge_flip_entry_notional(
                &cfg,
                &TradeBuilderRevengeFlipState::default(),
                &quote("up", 0.60, 0.59),
                None,
            )
            .expect("initial sizing"),
        );
        assert_eq!(initial.notional_usdc, 2.0);
        assert!(!initial.min_reentry_shares_applied);

        let floor_state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 1.626,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let floored = unwrap_ready_sizing(
            revenge_flip_entry_notional(&cfg, &floor_state, &quote("down", 0.60, 0.59), Some(10.0))
                .expect("floored sizing"),
        );
        assert_eq!(floored.formula_target_shares, 4.69);
        assert_eq!(floored.target_shares, 5.0);
        assert_eq!(floored.notional_usdc, 3.0);
        assert!(floored.min_reentry_shares_applied);

        let formula_state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 2.95,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let unchanged = unwrap_ready_sizing(
            revenge_flip_entry_notional(
                &cfg,
                &formula_state,
                &quote("down", 0.60, 0.59),
                Some(10.0),
            )
            .expect("formula sizing"),
        );
        assert_eq!(unchanged.formula_target_shares, 8.0);
        assert_eq!(unchanged.target_shares, 8.0);
        assert_eq!(unchanged.notional_usdc, 4.8);
        assert!(!unchanged.min_reentry_shares_applied);
    }

    #[test]
    fn revenge_flip_reentry_min_shares_blocks_when_lot_cap_is_too_small() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "profitTargetUsdc": 0.25,
                "minReentryShares": 5,
                "lotLimitPct": 1
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 1.626,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };
        let decision =
            revenge_flip_entry_notional(&cfg, &state, &quote("down", 0.60, 0.59), Some(2.99))
                .expect("sizing decision");

        assert_eq!(
            decision,
            RevengeFlipEntrySizingDecision::ReentryMinSharesExceedsLotLimit {
                formula_target_shares: 4.69,
                min_reentry_shares: 5.0,
                min_reentry_notional_usdc: 3.0,
                max_notional_usdc: 2.99,
            }
        );
    }

    #[test]
    fn revenge_flip_stop_loss_sell_node_skips_dust_qty() {
        let node = revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "ptbStopLossEnabled": true,
                "ptbStopLossGapUsd": 1.0
            }
        }));
        let config = resolve_revenge_flip_config(&node).expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            current_side: Some("down".to_string()),
            position_qty: 0.01,
            position_avg_cost: 0.51,
            position_source_trade_id: Some(108258),
            position_builder_order_id: Some(18380),
            ..TradeBuilderRevengeFlipState::default()
        };

        assert!(revenge_flip_stop_loss_sell_node(
            &node,
            &config,
            &quote("down", 0.45, 0.16),
            &state,
            1
        )
        .is_none());
    }

    #[test]
    fn revenge_flip_last_stopped_side_treats_dust_as_flat() {
        let state = TradeBuilderRevengeFlipState {
            current_side: Some("down".to_string()),
            position_qty: 0.01,
            ..TradeBuilderRevengeFlipState::default()
        };

        assert_eq!(revenge_flip_last_stopped_side(&state), Some("down"));
    }

    #[test]
    fn revenge_flip_stop_loss_sell_node_uses_position_qty_and_clears_usdc_sizing() {
        let node = revenge_node(json!({
            "mode": "revenge_flip_v1",
            "sizeMode": "usdc",
            "sizeUsdc": 2.0,
            "revengeFlip": {
                "ptbStopLossEnabled": true,
                "ptbStopLossGapUsd": 1.0
            }
        }));
        let config = resolve_revenge_flip_config(&node).expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            current_side: Some("up".to_string()),
            position_qty: 3.39,
            position_avg_cost: 0.59,
            position_source_trade_id: Some(108230),
            position_builder_order_id: Some(18298),
            ..TradeBuilderRevengeFlipState::default()
        };

        let sell =
            revenge_flip_stop_loss_sell_node(&node, &config, &quote("up", 0.45, 0.44), &state, 1)
                .expect("stop loss sell node");

        assert_eq!(
            sell.config.get("sizeMode").and_then(|value| value.as_str()),
            Some("shares")
        );
        assert_eq!(
            sell.config.get("targetQty").and_then(value_as_f64),
            Some(3.39)
        );
        assert!(sell.config.get("sizeUsdc").is_none());
        assert!(sell.config.get("targetNotionalUsdc").is_none());
        assert_eq!(
            sell.config
                .get("revengeFlipIntent")
                .and_then(|value| value.as_str()),
            Some("stop_loss_sell")
        );
        assert_eq!(
            sell.config
                .get("internalMode")
                .and_then(|value| value.as_str()),
            Some("revenge_flip_stop_loss_sell")
        );
        assert_eq!(
            sell.config
                .get("parentBuilderOrderId")
                .and_then(value_as_i64),
            Some(18298)
        );
        assert_eq!(
            sell.config
                .get("revengeFlipStopLossSell")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn revenge_flip_buy_node_clears_stale_source_trade_id() {
        let node = revenge_node(json!({
            "mode": "revenge_flip_v1",
            "sourceTradeId": 108233,
            "source_trade_id": 108233,
        }));
        let config = resolve_revenge_flip_config(&node).expect("valid config");
        let state = TradeBuilderRevengeFlipState::default();
        let effective_ptb = revenge_flip_effective_ptb(&config, &state, 0, None, None, None);

        let buy = revenge_flip_buy_node(
            &node,
            &config,
            &effective_ptb,
            Some(80.0),
            &quote("down", 0.64, 0.63),
            5.0,
            0.2,
            "initial_buy",
            1,
        );

        assert!(buy.config.get("sourceTradeId").is_none());
        assert!(buy.config.get("source_trade_id").is_none());
        assert_eq!(
            buy.config
                .get("revengeFlipIntent")
                .and_then(Value::as_str),
            Some("initial_buy")
        );
    }

    #[test]
    fn augment_runtime_snapshot_records_revenge_flip_intent() {
        let node = revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlipOrder": true,
            "revengeFlipIntent": "stop_loss_sell"
        }));
        let mut snapshot = json!({"source": "flow_step"});

        augment_runtime_snapshot_with_revenge_flip_intent(&mut snapshot, &node);

        assert_eq!(
            snapshot
                .get("revenge_flip_intent")
                .and_then(|value| value.as_str()),
            Some("stop_loss_sell")
        );
    }
}
