#[cfg(test)]
mod confidence_ladder_tests {
    use super::*;

    fn empty_state() -> TradeBuilderConfidenceLadderState {
        TradeBuilderConfidenceLadderState::default()
    }

    fn quote(side: &'static str, bid: f64, ask: f64) -> ConfidenceLadderQuote {
        ConfidenceLadderQuote {
            ladder_side: side,
            token_id: side.to_string(),
            outcome_label: if side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(bid),
            best_ask: Some(ask),
            best_bid_size: None,
            best_ask_size: None,
            last_trade_price: Some(ask),
            current_price: ask,
            snapshot: Value::Null,
        }
    }

    #[test]
    fn confidence_ladder_fee_and_all_in_cost_match_crypto_fee_formula() {
        let config = ConfidenceLadderConfig::default();
        let fee = confidence_ladder_fee(0.90, config.taker_fee_rate);
        assert!((fee - 0.0063).abs() < 0.000001);
        let all_in = confidence_ladder_all_in_cost(0.90, &config, true);
        assert!((all_in - 0.9113).abs() < 0.000001);
    }

    #[test]
    fn confidence_ladder_hard_no_chase_blocks_93c_and_above() {
        let config = ConfidenceLadderConfig::default();
        assert!(confidence_ladder_band_for_price(&config, 0.92).is_none());
        assert!(confidence_ladder_band_for_price(&config, 0.93).is_none());
        assert!(confidence_ladder_band_for_price(&config, 0.80).is_some());
    }

    #[test]
    fn confidence_ladder_worst_case_pnl_blocks_over_limit() {
        let config = ConfidenceLadderConfig::default();
        let state = TradeBuilderConfidenceLadderState {
            up_qty: 8.0,
            down_qty: 0.0,
            total_cost_usdc: 4.0,
            ..empty_state()
        };
        assert!(!confidence_ladder_risk_allows_buy(
            &config, &state, "up", 8.0, 0.80
        ));
        assert!(confidence_ladder_risk_allows_buy(
            &config, &state, "down", 4.0, 0.15
        ));
    }

    #[test]
    fn confidence_ladder_hedge_pair_cost_requires_profit_lock_margin() {
        let config = ConfidenceLadderConfig::default();
        let state = TradeBuilderConfidenceLadderState {
            up_qty: 12.0,
            down_qty: 0.0,
            total_cost_usdc: 8.28,
            up_cost_usdc: 8.28,
            up_avg_cost: Some(0.69),
            ..empty_state()
        };
        let model = ConfidenceLadderModel {
            p_up: 0.82,
            p_down: 0.18,
            chop: 0.2,
            score_up: 0.7,
            score_down: 0.3,
            dominant_side: Some("up"),
            diagnostics: Value::Null,
        };
        let decision = confidence_ladder_hedge_decision(
            &config,
            &state,
            &model,
            &quote("up", 0.86, 0.88),
            &quote("down", 0.11, 0.12),
        )
        .expect("hedge decision");
        assert_eq!(decision.ladder_side, "down");
        assert_eq!(decision.intent, "profit_lock_strong");
        assert_eq!(decision.quantity, 12.0);
    }

    #[test]
    fn confidence_ladder_late_reversal_uses_damage_control_opposite_buy() {
        let config = ConfidenceLadderConfig::default();
        let state = TradeBuilderConfidenceLadderState {
            up_qty: 12.0,
            down_qty: 0.0,
            total_cost_usdc: 8.28,
            up_cost_usdc: 8.28,
            up_avg_cost: Some(0.69),
            worst_case_pnl: -8.28,
            ..empty_state()
        };
        let model = ConfidenceLadderModel {
            p_up: 0.3,
            p_down: 0.7,
            chop: 0.2,
            score_up: 0.3,
            score_down: 0.7,
            dominant_side: Some("down"),
            diagnostics: Value::Null,
        };
        let decision = confidence_ladder_hedge_decision(
            &config,
            &state,
            &model,
            &quote("up", 0.47, 0.49),
            &quote("down", 0.51, 0.52),
        )
        .expect("damage control decision");
        assert_eq!(decision.ladder_side, "down");
        assert_eq!(decision.intent, "damage_control_hedge");
        assert!(decision.quantity <= state.up_qty);
        assert!(
            confidence_ladder_projected_worst_case_pnl(
                &state,
                decision.ladder_side,
                decision.quantity,
                decision.all_in_cost
            ) >= -config.max_loss_per_market_usdc
        );
    }

    #[test]
    fn confidence_ladder_side_flip_disable_uses_configured_cap() {
        let config = ConfidenceLadderConfig::default();
        let state = TradeBuilderConfidenceLadderState {
            side_switch_count: 2,
            ..empty_state()
        };
        assert!(confidence_ladder_side_flip_disabled(&config, &state));
    }
}
