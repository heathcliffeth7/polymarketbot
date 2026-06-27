mod revenge_flip_target_pnl_tests {
    use super::*;

    fn revenge_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "rf".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn quote(side: &str, ask: f64, bid: f64) -> RevengeFlipSideQuote {
        RevengeFlipSideQuote {
            market_slug: "btc-updown-5m-1774013100".to_string(),
            revenge_side: side.to_string(),
            token_id: format!("{side}-token"),
            outcome_label: if side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(bid),
            best_ask: Some(ask),
            current_price: ask,
            snapshot: json!({}),
        }
    }

    fn unwrap_ready_sizing(decision: RevengeFlipEntrySizingDecision) -> RevengeFlipEntrySizing {
        match decision {
            RevengeFlipEntrySizingDecision::Ready(sizing) => sizing,
            other => panic!("expected ready sizing, got {other:?}"),
        }
    }

    #[test]
    fn revenge_flip_config_accepts_negative_target_pnl() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "profitTargetUsdc": -1
            }
        })))
        .expect("negative target pnl is valid");

        assert_eq!(cfg.profit_target_usdc, -1.0);
    }

    #[test]
    fn revenge_flip_negative_target_pnl_recovers_only_required_loss() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "profitTargetUsdc": -1,
                "lotLimitPct": 1
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 3.0,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };

        let sizing = unwrap_ready_sizing(
            revenge_flip_entry_notional(&cfg, &state, &quote("down", 0.60, 0.59), Some(10.0))
                .expect("sizing"),
        );

        assert_eq!(sizing.formula_target_shares, 5.0);
        assert_eq!(sizing.target_shares, 5.0);
        assert_eq!(sizing.notional_usdc, 3.0);
    }

    #[test]
    fn revenge_flip_negative_target_pnl_skips_when_already_satisfied() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "profitTargetUsdc": -1,
                "minReentryShares": 5,
                "lotLimitPct": 1
            }
        })))
        .expect("valid config");
        let state = TradeBuilderRevengeFlipState {
            total_loss_usdc: 0.70,
            flip_count: 1,
            next_entry_side: Some("down".to_string()),
            ..TradeBuilderRevengeFlipState::default()
        };

        let decision =
            revenge_flip_entry_notional(&cfg, &state, &quote("down", 0.60, 0.59), Some(10.0))
                .expect("sizing decision");

        match decision {
            RevengeFlipEntrySizingDecision::TargetPnlAlreadySatisfied {
                total_loss_usdc,
                target_pnl_usdc,
                required_recovery_usdc,
            } => {
                assert_eq!(total_loss_usdc, 0.70);
                assert_eq!(target_pnl_usdc, -1.0);
                assert!(required_recovery_usdc <= 0.0);
            }
            other => panic!("expected satisfied target pnl, got {other:?}"),
        }
    }

    #[test]
    fn revenge_flip_positive_target_pnl_keeps_existing_recovery_formula() {
        let cfg = resolve_revenge_flip_config(&revenge_node(json!({
            "mode": "revenge_flip_v1",
            "revengeFlip": {
                "profitTargetUsdc": 0.25,
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

        let sizing = unwrap_ready_sizing(
            revenge_flip_entry_notional(&cfg, &state, &quote("down", 0.60, 0.59), Some(10.0))
                .expect("sizing"),
        );

        assert_eq!(sizing.formula_target_shares, 4.69);
        assert_eq!(sizing.target_shares, 4.69);
        assert_eq!(sizing.notional_usdc, 2.82);
    }
}
