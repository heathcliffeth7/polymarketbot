mod revenge_flip_mid_chop_tests {
    use super::*;

    fn revenge_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "rf".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn quote_with_mid(side: &str, mid: f64) -> RevengeFlipSideQuote {
        RevengeFlipSideQuote {
            market_slug: "btc-updown-5m-1774013100".to_string(),
            revenge_side: side.to_string(),
            token_id: format!("{side}-token"),
            outcome_label: if side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(mid - 0.005),
            best_ask: Some(mid + 0.005),
            current_price: mid,
            snapshot: json!({}),
        }
    }

    fn quote_without_book(side: &str) -> RevengeFlipSideQuote {
        RevengeFlipSideQuote {
            best_bid: None,
            best_ask: Some(0.50),
            ..quote_with_mid(side, 0.50)
        }
    }

    fn config_with_revenge_flip(revenge_flip: Value) -> RevengeFlipConfig {
        resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "priceToBeatGuardEnabled": false,
            "revengeFlip": revenge_flip
        })))
        .expect("valid revenge flip config")
    }

    fn mid_reason(
        config: &RevengeFlipConfig,
        is_flip: bool,
        up_mid: f64,
        down_mid: f64,
    ) -> Option<&'static str> {
        let quotes = vec![
            quote_with_mid("up", up_mid),
            quote_with_mid("down", down_mid),
        ];
        let evaluation = revenge_flip_post_stop_loss_mid_evaluation(config, &quotes);
        revenge_flip_post_stop_loss_mid_chop_skip_reason(config, is_flip, &evaluation)
    }

    #[test]
    fn post_stop_loss_mid_chop_guard_defaults_and_validation() {
        let defaults = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1"
        })))
        .expect("valid defaults");

        assert!(!defaults.post_stop_loss_mid_chop_guard.enabled);
        assert_eq!(defaults.post_stop_loss_mid_chop_guard.chop_mid_low, 0.45);
        assert_eq!(defaults.post_stop_loss_mid_chop_guard.chop_mid_high, 0.56);
        assert!(defaults.post_stop_loss_mid_chop_guard.fail_closed);
        assert!(!defaults.post_stop_loss_strong_trend_flip.enabled);
        assert_eq!(
            defaults.post_stop_loss_strong_trend_flip.strong_trend_mid,
            0.68
        );
        assert_eq!(
            defaults.post_stop_loss_strong_trend_flip.max_price_cent,
            85.0
        );

        assert!(
            resolve_revenge_flip_config(&revenge_node(json!({
                "mode": "revenge_flip_v1",
                "revengeFlip": {
                    "postStopLossMidChopGuard": {
                        "enabled": true,
                        "chopMidLow": 0.56,
                        "chopMidHigh": 0.45
                    }
                }
            })))
            .is_err()
        );
        assert!(
            resolve_revenge_flip_config(&revenge_node(json!({
                "mode": "revenge_flip_v1",
                "revengeFlip": {
                    "postStopLossStrongTrendFlip": {
                        "enabled": true,
                        "maxPriceCent": 101
                    }
                }
            })))
            .is_err()
        );
    }

    #[test]
    fn post_stop_loss_mid_chop_guard_blocks_only_flip_chop_zone() {
        let config = config_with_revenge_flip(json!({
            "postStopLossMidChopGuard": { "enabled": true }
        }));

        assert_eq!(
            mid_reason(&config, true, 0.52, 0.48),
            Some("blocked_mid_chop")
        );
        assert_eq!(
            mid_reason(&config, true, 0.55, 0.45),
            Some("blocked_mid_chop")
        );
        assert_eq!(mid_reason(&config, true, 0.58, 0.42), None);
        assert_eq!(mid_reason(&config, false, 0.52, 0.48), None);

        let disabled = config_with_revenge_flip(json!({
            "postStopLossMidChopGuard": { "enabled": false }
        }));
        assert_eq!(mid_reason(&disabled, true, 0.52, 0.48), None);
    }

    #[test]
    fn post_stop_loss_mid_chop_guard_handles_missing_quotes_by_fail_closed() {
        let fail_closed = config_with_revenge_flip(json!({
            "postStopLossMidChopGuard": {
                "enabled": true,
                "failClosed": true
            }
        }));
        let quotes = vec![quote_without_book("up"), quote_with_mid("down", 0.50)];
        let evaluation = revenge_flip_post_stop_loss_mid_evaluation(&fail_closed, &quotes);
        assert_eq!(
            revenge_flip_post_stop_loss_mid_chop_skip_reason(&fail_closed, true, &evaluation),
            Some("mid_chop_quote_unavailable")
        );

        let fail_open = config_with_revenge_flip(json!({
            "postStopLossMidChopGuard": {
                "enabled": true,
                "failClosed": false
            }
        }));
        let evaluation = revenge_flip_post_stop_loss_mid_evaluation(&fail_open, &quotes);
        assert_eq!(
            revenge_flip_post_stop_loss_mid_chop_skip_reason(&fail_open, true, &evaluation),
            None
        );
    }

    #[test]
    fn post_stop_loss_strong_trend_flip_overrides_rule_match_max_price() {
        let config = config_with_revenge_flip(json!({
            "reentrySideMode": "rule_match",
            "postStopLossStrongTrendFlip": {
                "enabled": true,
                "strongTrendMid": 0.68,
                "maxPriceCent": 85
            },
            "entryPtbRules": [
                {
                    "minFlip": 1,
                    "sideMode": "down",
                    "priceToBeatMinDiff": 0,
                    "maxPriceCent": 58
                }
            ]
        }));
        let state = TradeBuilderRevengeFlipState {
            flip_count: 1,
            ..TradeBuilderRevengeFlipState::default()
        };
        let quotes = vec![quote_with_mid("up", 0.30), quote_with_mid("down", 0.70)];
        let evaluation = revenge_flip_post_stop_loss_mid_evaluation(&config, &quotes);
        let max_price_override =
            revenge_flip_post_stop_loss_strong_trend_max_price_override(&config, true, &evaluation);

        assert_eq!(max_price_override, Some(85.0));
        assert!(revenge_flip_select_entry_candidate(&config, &state, &quotes, None).is_none());

        let candidate = revenge_flip_select_entry_candidate_with_max_price_override(
            &config,
            &state,
            &quotes,
            None,
            max_price_override,
        )
        .expect("strong trend override should allow down candidate");
        assert_eq!(candidate.quote.revenge_side, "down");
        assert_eq!(candidate.effective_entry_price.max_cent, Some(85.0));
        assert_eq!(
            candidate.effective_entry_price.max_source,
            "post_stop_loss_strong_trend_flip"
        );
    }
}
