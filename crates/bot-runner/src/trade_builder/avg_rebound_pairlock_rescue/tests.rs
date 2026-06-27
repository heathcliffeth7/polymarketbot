#[cfg(test)]
mod avg_rebound_pairlock_rescue_tests {
    use super::*;

    fn avg_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "avg_rebound".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn book(asks: &[(f64, f64)]) -> OrderBookSnapshot {
        OrderBookSnapshot {
            bids: Vec::new(),
            asks: asks
                .iter()
                .map(|(price, size)| bot_infra::exchange::OrderBookLevel {
                    price: *price,
                    size: *size,
                })
                .collect(),
        }
    }

    fn state_with_session() -> AvgReboundRuntimeState {
        AvgReboundRuntimeState {
            session_id: Some(7),
            session_status: Some(AVG_REBOUND_STATUS_BUILDING_PRIMARY.to_string()),
            primary_total_qty: rust_decimal::Decimal::ZERO,
            primary_total_cost: rust_decimal::Decimal::ZERO,
            avg_primary_cost: None,
            opposite_filled_qty: rust_decimal::Decimal::ZERO,
            opposite_total_cost: rust_decimal::Decimal::ZERO,
            open_primary_qty: rust_decimal::Decimal::ZERO,
            locked_pnl: rust_decimal::Decimal::ZERO,
            profit_started: false,
            primary_tier_ids: Vec::new(),
            opposite_leg_ids: Vec::new(),
        }
    }

    fn tokens() -> AvgReboundTokenResolution {
        AvgReboundTokenResolution {
            primary_token_id: "yes".to_string(),
            primary_outcome_label: "Yes".to_string(),
            opposite_token_id: "no".to_string(),
            opposite_outcome_label: "No".to_string(),
        }
    }

    fn resolved_pair() -> PairLockResolvedTokenPair {
        PairLockResolvedTokenPair {
            yes_token_id: "up-token".to_string(),
            no_token_id: "down-token".to_string(),
            token_resolution_source: "test",
            trigger_node_market_slug: Some("btc-updown-5m-1770000000".to_string()),
        }
    }

    fn diagnostic_decimal(value: &Value) -> rust_decimal::Decimal {
        value
            .as_str()
            .expect("diagnostic value is a decimal string")
            .parse::<rust_decimal::Decimal>()
            .expect("diagnostic decimal parses")
    }

    fn quote(current_price: f64) -> PairLockResolvedQuote {
        PairLockResolvedQuote {
            best_bid: Some(current_price),
            best_ask: Some(current_price),
            last_trade_price: Some(current_price),
            current_price,
            quote_source_kind: "test",
            quote_ws_state: "test",
            quote_event_ts: None,
            quote_snapshot_age_ms: None,
            quote_source_detail: "test".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: json!({}),
        }
    }

    #[test]
    fn auto_scope_fallback_accepts_trigger_node_state_cache_for_current_window() {
        let context = json!({
            "nodeState": {
                "trigger_avg": {
                    "auto_scope_market_slug": "btc-updown-5m-1770000000",
                    "auto_scope_yes_token_id": "cached-up",
                    "auto_scope_no_token_id": "cached-down"
                }
            }
        });
        let scope = find_updown_scope_by_scope("btc_5m_updown").expect("scope");
        let now = chrono::DateTime::<chrono::Utc>::from_timestamp(1_770_000_100, 0).unwrap();
        let selected =
            selected_live_market_from_trigger_node_state_cache(&context, "trigger_avg", scope, now)
                .expect("cache fallback");
        assert_eq!(selected.slug, "btc-updown-5m-1770000000");
        assert_eq!(selected.yes_token_id.as_deref(), Some("cached-up"));
        assert_eq!(
            selected.selection_reason.as_str(),
            "trigger_node_state_cache"
        );
    }

    #[test]
    fn auto_scope_fallback_rejects_stale_trigger_node_state_cache() {
        let context = json!({
            "nodeState": {
                "trigger_avg": {
                    "auto_scope_market_slug": "btc-updown-5m-1770000000",
                    "auto_scope_yes_token_id": "cached-up",
                    "auto_scope_no_token_id": "cached-down"
                }
            }
        });
        let scope = find_updown_scope_by_scope("btc_5m_updown").expect("scope");
        let now = chrono::DateTime::<chrono::Utc>::from_timestamp(1_770_000_400, 0).unwrap();
        assert!(selected_live_market_from_trigger_node_state_cache(
            &context,
            "trigger_avg",
            scope,
            now,
        )
        .is_none());
    }

    #[test]
    fn default_config_parses_fok_limit_defaults() {
        let node = avg_node(json!({
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1
        }));
        let config = resolve_avg_rebound_pairlock_rescue_config(&node).expect("config");
        assert_eq!(config.order_type, "FOK");
        assert_eq!(config.execution_mode, "limit");
        assert_eq!(config.session_budget_usdc, avg_rebound_dec("50"));
        assert_eq!(config.reserved_budget_buffer_usdc, avg_rebound_dec("0.75"));
        assert_eq!(config.extra_vwap_safety_buffer, avg_rebound_dec("0.005"));
        assert_eq!(config.primary_outcome_label, "auto");
        assert_eq!(
            config.primary_side_selection,
            AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE
        );
        assert!(config.allow_primary_after_partial_profit);
        assert!(!config.pre_full_giveback_guard_enabled);
        assert!(config.full_giveback_guard_enabled);
        assert_eq!(config.rescue.emergency_vwap_cap, avg_rebound_dec("0.81"));
        assert_eq!(config.rescue.hard_max_vwap_cap, avg_rebound_dec("0.81"));
        assert_eq!(config.primary_ladder.len(), 3);
        assert_eq!(config.stages.len(), 3);
    }

    #[test]
    fn explicit_primary_outcome_label_keeps_legacy_side_selection() {
        let node = avg_node(json!({
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
            "avgReboundPairlockRescue": {
                "primaryOutcomeLabel": "Up",
                "primarySideSelection": "cheapest_eligible"
            }
        }));
        let config = resolve_avg_rebound_pairlock_rescue_config(&node).expect("config");
        assert_eq!(config.primary_outcome_label, "Up");
        assert!(!avg_rebound_primary_outcome_is_auto(&config));
    }

    #[test]
    fn invalid_primary_side_selection_is_rejected() {
        let node = avg_node(json!({
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
            "avgReboundPairlockRescue": {
                "primaryOutcomeLabel": "auto",
                "primarySideSelection": "random"
            }
        }));
        let err = resolve_avg_rebound_pairlock_rescue_config(&node)
            .expect_err("invalid side selection rejected");
        assert!(err.to_string().contains("primarySideSelection"));
    }

    #[test]
    fn micro_23_auto_cheapest_config_parses_last_chance_rescue() {
        let node = avg_node(json!({
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
            "avgReboundPairlockRescue": {
                "version": "v1",
                "sessionBudgetUsdc": "23",
                "reservedBudgetBufferUsdc": "0.25",
                "primaryOutcomeLabel": "auto",
                "oppositeOutcomeLabel": "opposite",
                "primarySideSelection": "cheapest_eligible",
                "orderType": "FOK",
                "executionMode": "limit",
                "vwapSource": "rest_book",
                "extraVwapSafetyBuffer": "0.005",
                "targetProfitUsdc": "0.10",
                "allowPrimaryAfterPartialProfit": true,
                "preFullGivebackGuardEnabled": false,
                "fullGivebackGuardEnabled": true,
                "primaryLadder": [
                    { "id": "p50", "priceCap": "0.50", "qty": "4" },
                    { "id": "p30", "priceCap": "0.30", "qty": "5" },
                    { "id": "p10", "priceCap": "0.10", "qty": "10" }
                ],
                "stages": [
                    {
                        "id": "stage_50",
                        "requiredPrimaryTierIds": ["p50"],
                        "profitLegs": [
                            { "id": "s50_profit_10c", "qty": "4", "oppositeVwapCap": "0.480" }
                        ],
                        "givebackGuard": { "trigger": "0.480", "maxExecutionVwap": "0.480" }
                    },
                    {
                        "id": "stage_30",
                        "requiredPrimaryTierIds": ["p50", "p30"],
                        "profitLegs": [
                            { "id": "s30_profit_10c", "qty": "9", "oppositeVwapCap": "0.605" }
                        ],
                        "givebackGuard": { "trigger": "0.605", "maxExecutionVwap": "0.605" }
                    },
                    {
                        "id": "stage_full",
                        "requiredPrimaryTierIds": ["p50", "p30", "p10"],
                        "profitLegs": [
                            { "id": "full_profit_10c", "qty": "19", "oppositeVwapCap": "0.763" }
                        ],
                        "givebackGuard": { "trigger": "0.770", "maxExecutionVwap": "0.770" }
                    }
                ],
                "rescue": {
                    "enabledOnlyAfterFullLadder": true,
                    "normalVwapCap": "0.770",
                    "emergencyVwapCap": "0.800",
                    "hardMaxVwapCap": "0.800",
                    "lastChanceVwapCap": "0.850"
                }
            }
        }));
        let config = resolve_avg_rebound_pairlock_rescue_config(&node).expect("micro config");

        assert_eq!(config.session_budget_usdc, avg_rebound_dec("23"));
        assert_eq!(config.reserved_budget_buffer_usdc, avg_rebound_dec("0.25"));
        assert_eq!(config.target_profit_usdc, Some(avg_rebound_dec("0.10")));
        assert_eq!(config.primary_outcome_label, "auto");
        assert_eq!(
            config.primary_side_selection,
            AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE
        );
        assert_eq!(config.primary_ladder[0].qty, avg_rebound_dec("4"));
        assert_eq!(config.primary_ladder[2].qty, avg_rebound_dec("10"));
        assert_eq!(
            config.stages[2].profit_legs[0].opposite_vwap_cap,
            avg_rebound_dec("0.763")
        );
        assert_eq!(config.rescue.normal_vwap_cap, avg_rebound_dec("0.770"));
        assert_eq!(config.rescue.emergency_vwap_cap, avg_rebound_dec("0.800"));
        assert_eq!(config.rescue.hard_max_vwap_cap, avg_rebound_dec("0.800"));
        assert_eq!(
            config.rescue.last_chance_vwap_cap,
            Some(avg_rebound_dec("0.850"))
        );
    }

    #[test]
    fn gtc_order_type_is_rejected_in_v1() {
        let node = avg_node(json!({
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
            "avgReboundPairlockRescue": { "orderType": "GTC" }
        }));
        let err = resolve_avg_rebound_pairlock_rescue_config(&node).expect_err("GTC rejected");
        assert!(err.to_string().contains("orderType=FOK"));
    }

    #[test]
    fn primary_vwap_blocks_tier_when_depth_average_exceeds_cap() {
        let result = avg_rebound_vwap_for_fok_limit(
            &book(&[(0.50, 4.0), (0.54, 4.0)]),
            avg_rebound_dec("8"),
            avg_rebound_dec("0.50"),
            rust_decimal::Decimal::ZERO,
        );
        assert_eq!(
            result.expect_err("insufficient depth at limit").reason,
            "insufficient_depth_at_or_below_limit"
        );
    }

    #[test]
    fn fok_depth_rejects_when_vwap_would_need_levels_above_limit() {
        let result = avg_rebound_vwap_for_fok_limit(
            &book(&[(0.70, 5.0), (0.74, 5.0)]),
            avg_rebound_dec("10"),
            avg_rebound_dec("0.72"),
            rust_decimal::Decimal::ZERO,
        );
        assert_eq!(
            result
                .expect_err("74c level cannot execute at 72c limit")
                .reason,
            "insufficient_depth_at_or_below_limit"
        );
    }

    #[test]
    fn hard_rescue_allows_exact_boundary_and_rejects_above() {
        let allowed = avg_rebound_vwap_for_fok_limit(
            &book(&[(0.81, 47.0)]),
            avg_rebound_dec("47"),
            avg_rebound_dec("0.81"),
            rust_decimal::Decimal::ZERO,
        )
        .expect("exact 81c");
        assert_eq!(allowed.vwap, avg_rebound_dec("0.81"));

        let rejected = avg_rebound_vwap_for_fok_limit(
            &book(&[(0.810001, 47.0)]),
            avg_rebound_dec("47"),
            avg_rebound_dec("0.81"),
            rust_decimal::Decimal::ZERO,
        );
        assert_eq!(
            rejected.expect_err("above 81c").reason,
            "insufficient_depth_at_or_below_limit"
        );
    }

    #[test]
    fn budget_buffer_blocks_projected_spend() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.primary_total_cost = avg_rebound_dec("48.90");
        let allowed = avg_rebound_projected_spend_allowed(&config, &state, avg_rebound_dec("0.40"));
        assert!(!allowed);
    }

    #[test]
    fn cheapest_primary_selects_lower_executable_vwap() {
        let config = avg_rebound_default_config();
        let state = state_with_session();
        let selection = avg_rebound_select_cheapest_primary(
            &config,
            &state,
            &resolved_pair(),
            "btc-updown-5m-1770000000",
            book(&[(0.49, 8.0)]),
            book(&[(0.47, 8.0)]),
        )
        .expect("down is cheaper");
        assert_eq!(selection.tokens.primary_token_id, "down-token");
        assert_eq!(selection.tokens.primary_outcome_label, "Down");
        assert!(selection.rejections.is_empty());
    }

    #[test]
    fn cheapest_primary_uses_only_executable_side() {
        let config = avg_rebound_default_config();
        let state = state_with_session();
        let selection = avg_rebound_select_cheapest_primary(
            &config,
            &state,
            &resolved_pair(),
            "btc-updown-5m-1770000000",
            book(&[(0.51, 8.0)]),
            book(&[(0.48, 8.0)]),
        )
        .expect("down only");
        assert_eq!(selection.tokens.primary_token_id, "down-token");
        assert_eq!(selection.rejections.len(), 1);
    }

    #[test]
    fn cheapest_primary_rejects_side_below_market_min_notional() {
        let mut config = avg_rebound_default_config();
        config.primary_ladder = vec![AvgReboundPrimaryTierConfig {
            id: "p50".to_string(),
            price_cap: avg_rebound_dec("0.50"),
            qty: avg_rebound_dec("4"),
        }];
        let state = state_with_session();
        let selection = avg_rebound_select_cheapest_primary(
            &config,
            &state,
            &resolved_pair(),
            "btc-updown-5m-1770000000",
            book(&[(0.04, 4.0)]),
            book(&[(0.47, 4.0)]),
        )
        .expect("down is the only eligible side");
        assert_eq!(selection.tokens.primary_token_id, "down-token");
        assert_eq!(selection.rejections.len(), 1);
        assert_eq!(
            selection.rejections[0]["reason"],
            json!("marketable_buy_min_notional_below_strategy_qty")
        );
    }

    #[test]
    fn cheapest_primary_rejects_when_both_sides_exceed_cap() {
        let config = avg_rebound_default_config();
        let state = state_with_session();
        let rejections = avg_rebound_select_cheapest_primary(
            &config,
            &state,
            &resolved_pair(),
            "btc-updown-5m-1770000000",
            book(&[(0.51, 8.0)]),
            book(&[(0.52, 8.0)]),
        )
        .expect_err("both sides rejected");
        assert_eq!(rejections.len(), 2);
        assert!(rejections
            .iter()
            .all(|entry| entry["reason"] == json!("vwap_rejected")));
    }

    #[test]
    fn existing_session_token_resolution_keeps_original_primary_side() {
        let session = bot_infra::db::TradeBuilderAvgReboundPairlockRescueSession {
            id: 9,
            status: AVG_REBOUND_STATUS_BUILDING_PRIMARY.to_string(),
            primary_token_id: "up-token".to_string(),
            primary_outcome_label: "Up".to_string(),
            opposite_token_id: "down-token".to_string(),
            opposite_outcome_label: "Down".to_string(),
        };
        let tokens = avg_rebound_token_resolution_from_session(&session);
        assert_eq!(tokens.primary_token_id, "up-token");
        assert_eq!(tokens.primary_outcome_label, "Up");
        assert_eq!(tokens.opposite_token_id, "down-token");
    }

    #[test]
    fn target_profit_keeps_stage_50_cap_when_primary_fills_at_50c() {
        let mut config = avg_rebound_default_config();
        config.target_profit_usdc = Some(avg_rebound_dec("0.10"));
        config.primary_ladder = vec![AvgReboundPrimaryTierConfig {
            id: "p50".to_string(),
            price_cap: avg_rebound_dec("0.50"),
            qty: avg_rebound_dec("4"),
        }];
        config.stages = vec![AvgReboundStageConfig {
            id: "stage_50".to_string(),
            required_primary_tier_ids: vec!["p50".to_string()],
            profit_legs: vec![AvgReboundProfitLegConfig {
                id: "s50_profit_10c".to_string(),
                opposite_vwap_cap: avg_rebound_dec("0.480"),
                qty: avg_rebound_dec("4"),
            }],
            giveback_guard: AvgReboundGivebackGuardConfig {
                trigger: avg_rebound_dec("0.480"),
                max_execution_vwap: avg_rebound_dec("0.480"),
            },
        }];
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("4");
        state.primary_total_cost = avg_rebound_dec("2.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.50"));
        state.open_primary_qty = avg_rebound_dec("4");
        state.primary_tier_ids = vec!["p50".to_string()];

        let decision =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.475, 4.0)]))
                .expect("profit is considered")
                .expect("configured cap remains executable");

        assert_eq!(decision.limit_price, avg_rebound_dec("0.475"));
        assert_eq!(decision.vwap, avg_rebound_dec("0.475"));
        assert_eq!(
            diagnostic_decimal(&decision.diagnostics["extra"]["dynamic_profit_effective_cap"]),
            avg_rebound_dec("0.475")
        );
    }

    #[test]
    fn target_profit_expands_stage_50_cap_when_primary_fills_at_40c() {
        let mut config = avg_rebound_default_config();
        config.target_profit_usdc = Some(avg_rebound_dec("0.10"));
        config.primary_ladder = vec![AvgReboundPrimaryTierConfig {
            id: "p50".to_string(),
            price_cap: avg_rebound_dec("0.50"),
            qty: avg_rebound_dec("4"),
        }];
        config.stages = vec![AvgReboundStageConfig {
            id: "stage_50".to_string(),
            required_primary_tier_ids: vec!["p50".to_string()],
            profit_legs: vec![AvgReboundProfitLegConfig {
                id: "s50_profit_10c".to_string(),
                opposite_vwap_cap: avg_rebound_dec("0.480"),
                qty: avg_rebound_dec("4"),
            }],
            giveback_guard: AvgReboundGivebackGuardConfig {
                trigger: avg_rebound_dec("0.480"),
                max_execution_vwap: avg_rebound_dec("0.480"),
            },
        }];
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("4");
        state.primary_total_cost = avg_rebound_dec("1.60");
        state.avg_primary_cost = Some(avg_rebound_dec("0.40"));
        state.open_primary_qty = avg_rebound_dec("4");
        state.primary_tier_ids = vec!["p50".to_string()];

        let decision =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.575, 4.0)]))
                .expect("profit is considered")
                .expect("dynamic cap is executable");

        assert_eq!(decision.limit_price, avg_rebound_dec("0.575"));
        assert_eq!(decision.vwap, avg_rebound_dec("0.575"));
        assert_eq!(
            diagnostic_decimal(&decision.diagnostics["extra"]["dynamic_profit_raw_cap"]),
            avg_rebound_dec("0.580")
        );
        assert_eq!(
            diagnostic_decimal(&decision.diagnostics["extra"]["dynamic_profit_effective_cap"]),
            avg_rebound_dec("0.575")
        );
        assert_eq!(
            diagnostic_decimal(&decision.diagnostics["projected_locked_pnl"]),
            avg_rebound_dec("0.100")
        );
    }

    #[test]
    fn target_profit_tightens_profit_cap_when_configured_cap_would_lose() {
        let mut config = avg_rebound_default_config();
        config.target_profit_usdc = Some(avg_rebound_dec("0.10"));
        config.primary_ladder = vec![AvgReboundPrimaryTierConfig {
            id: "p50".to_string(),
            price_cap: avg_rebound_dec("0.50"),
            qty: avg_rebound_dec("4"),
        }];
        config.stages = vec![AvgReboundStageConfig {
            id: "stage_50".to_string(),
            required_primary_tier_ids: vec!["p50".to_string()],
            profit_legs: vec![AvgReboundProfitLegConfig {
                id: "s50_profit_10c".to_string(),
                opposite_vwap_cap: avg_rebound_dec("0.480"),
                qty: avg_rebound_dec("4"),
            }],
            giveback_guard: AvgReboundGivebackGuardConfig {
                trigger: avg_rebound_dec("0.480"),
                max_execution_vwap: avg_rebound_dec("0.480"),
            },
        }];
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("100");
        state.primary_total_cost = avg_rebound_dec("3.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.03"));
        state.open_primary_qty = avg_rebound_dec("100");
        state.primary_tier_ids = vec!["p50".to_string()];

        let rejection =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.475, 4.0)]))
                .expect("profit leg is considered")
                .expect_err("dynamic cap rejects losing configured cap");

        assert_eq!(rejection["reason"], json!("vwap_rejected"));
        assert_eq!(
            diagnostic_decimal(&rejection["extra"]["dynamic_profit_effective_cap"]),
            avg_rebound_dec("0.225")
        );
    }

    #[test]
    fn profit_started_allows_primary_continuation_when_open_primary_remains() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("8");
        state.primary_total_cost = avg_rebound_dec("4.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.50"));
        state.opposite_filled_qty = avg_rebound_dec("4");
        state.opposite_total_cost = avg_rebound_dec("1.78");
        state.open_primary_qty = avg_rebound_dec("4");
        state.primary_tier_ids = vec!["p50".to_string()];
        state.opposite_leg_ids = vec!["s50_profit_45".to_string()];
        let decision =
            avg_rebound_primary_decision(&config, &state, &tokens(), &book(&[(0.30, 15.0)]))
                .expect("primary continuation")
                .expect("p30 is executable");
        assert_eq!(decision.intent, AVG_REBOUND_INTENT_PRIMARY_LADDER);
        assert_eq!(decision.tier_or_leg_id, "p30");
        assert_eq!(decision.limit_price, avg_rebound_dec("0.30"));
    }

    #[test]
    fn profit_started_blocks_primary_continuation_without_open_primary() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("8");
        state.primary_total_cost = avg_rebound_dec("4.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.50"));
        state.opposite_filled_qty = avg_rebound_dec("8");
        state.opposite_total_cost = avg_rebound_dec("3.60");
        state.open_primary_qty = rust_decimal::Decimal::ZERO;
        state.primary_tier_ids = vec!["p50".to_string()];
        state.opposite_leg_ids = vec!["s50_profit_45".to_string(), "s50_profit_40".to_string()];
        assert!(
            avg_rebound_primary_decision(&config, &state, &tokens(), &book(&[(0.30, 15.0)]))
                .is_none()
        );
    }

    #[test]
    fn guard_trigger_and_execution_cap_are_separate() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("47");
        state.primary_total_cost = avg_rebound_dec("10.90");
        state.avg_primary_cost = Some(avg_rebound_dec("0.231914893617"));
        state.opposite_filled_qty = avg_rebound_dec("15");
        state.opposite_total_cost = avg_rebound_dec("10.80");
        state.open_primary_qty = avg_rebound_dec("32");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string(), "p10".to_string()];
        state.opposite_leg_ids = vec!["full_profit_72".to_string()];
        let rejected = avg_rebound_guard_decision(
            &config,
            &state,
            &tokens(),
            &quote(0.76),
            &book(&[(0.79, 32.0)]),
        )
        .expect("guard wakes")
        .expect_err("guard execution cap blocks expensive book");
        assert_eq!(rejected["reason"], json!("vwap_rejected"));
    }

    #[test]
    fn rescue_is_inactive_before_full_ladder() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("23");
        state.primary_total_cost = avg_rebound_dec("8.50");
        state.open_primary_qty = avg_rebound_dec("23");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string()];
        assert!(
            avg_rebound_rescue_decision(&config, &state, &tokens(), &book(&[(0.78, 23.0)]))
                .is_none()
        );
    }

    #[test]
    fn last_chance_rescue_runs_after_hard_cap_when_full_ladder_filled() {
        let mut config = avg_rebound_default_config();
        config.session_budget_usdc = avg_rebound_dec("23");
        config.reserved_budget_buffer_usdc = avg_rebound_dec("0.25");
        config.rescue.normal_vwap_cap = avg_rebound_dec("0.770");
        config.rescue.emergency_vwap_cap = avg_rebound_dec("0.800");
        config.rescue.hard_max_vwap_cap = avg_rebound_dec("0.800");
        config.rescue.last_chance_vwap_cap = Some(avg_rebound_dec("0.850"));
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("19");
        state.primary_total_cost = avg_rebound_dec("4.50");
        state.avg_primary_cost = Some(avg_rebound_dec("0.236842105263"));
        state.open_primary_qty = avg_rebound_dec("19");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string(), "p10".to_string()];

        let decision =
            avg_rebound_rescue_decision(&config, &state, &tokens(), &book(&[(0.845, 19.0)]))
                .expect("last chance is considered")
                .expect("last chance is executable");

        assert_eq!(decision.intent, AVG_REBOUND_INTENT_LAST_CHANCE_RESCUE);
        assert_eq!(decision.tier_or_leg_id, "last_chance_rescue");
        assert_eq!(decision.limit_price, avg_rebound_dec("0.845"));
        assert_eq!(decision.vwap, avg_rebound_dec("0.845"));
        assert_eq!(decision.notional, avg_rebound_dec("16.055"));
    }

    #[test]
    fn last_chance_rescue_rejects_above_effective_cap() {
        let mut config = avg_rebound_default_config();
        config.session_budget_usdc = avg_rebound_dec("23");
        config.reserved_budget_buffer_usdc = avg_rebound_dec("0.25");
        config.rescue.normal_vwap_cap = avg_rebound_dec("0.770");
        config.rescue.emergency_vwap_cap = avg_rebound_dec("0.800");
        config.rescue.hard_max_vwap_cap = avg_rebound_dec("0.800");
        config.rescue.last_chance_vwap_cap = Some(avg_rebound_dec("0.850"));
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("19");
        state.primary_total_cost = avg_rebound_dec("4.50");
        state.open_primary_qty = avg_rebound_dec("19");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string(), "p10".to_string()];

        assert!(
            avg_rebound_rescue_decision(&config, &state, &tokens(), &book(&[(0.846, 19.0)]))
                .is_none()
        );
    }

    #[test]
    fn last_chance_rescue_is_inactive_before_full_ladder() {
        let mut config = avg_rebound_default_config();
        config.session_budget_usdc = avg_rebound_dec("23");
        config.reserved_budget_buffer_usdc = avg_rebound_dec("0.25");
        config.rescue.last_chance_vwap_cap = Some(avg_rebound_dec("0.850"));
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("9");
        state.primary_total_cost = avg_rebound_dec("3.50");
        state.open_primary_qty = avg_rebound_dec("9");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string()];

        assert!(
            avg_rebound_rescue_decision(&config, &state, &tokens(), &book(&[(0.845, 9.0)]))
                .is_none()
        );
    }

    #[test]
    fn pre_full_guard_does_not_block_primary_continuation() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("8");
        state.primary_total_cost = avg_rebound_dec("4.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.50"));
        state.opposite_filled_qty = avg_rebound_dec("4");
        state.opposite_total_cost = avg_rebound_dec("1.78");
        state.open_primary_qty = avg_rebound_dec("4");
        state.primary_tier_ids = vec!["p50".to_string()];
        state.opposite_leg_ids = vec!["s50_profit_45".to_string()];

        let (decision, rejections) = avg_rebound_select_decision(
            &config,
            &state,
            &tokens(),
            &book(&[(0.30, 15.0)]),
            &quote(0.47),
            &book(&[(0.465, 4.0)]),
        );
        let decision = decision.expect("p30 continuation should be selected");
        assert_eq!(decision.intent, AVG_REBOUND_INTENT_PRIMARY_LADDER);
        assert_eq!(decision.tier_or_leg_id, "p30");
        assert!(rejections
            .iter()
            .any(|entry| entry["reason"] == json!("vwap_rejected")));
    }

    #[test]
    fn full_guard_requires_profit_started() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("47");
        state.primary_total_cost = avg_rebound_dec("10.90");
        state.avg_primary_cost = Some(avg_rebound_dec("0.231914893617"));
        state.open_primary_qty = avg_rebound_dec("47");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string(), "p10".to_string()];

        assert!(avg_rebound_guard_decision(
            &config,
            &state,
            &tokens(),
            &quote(0.76),
            &book(&[(0.775, 47.0)])
        )
        .is_none());
    }

    #[test]
    fn stage_advance_uses_current_stage_and_ignores_old_unfilled_legs() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("23");
        state.primary_total_cost = avg_rebound_dec("8.50");
        state.avg_primary_cost = Some(avg_rebound_dec("0.369565217391"));
        state.opposite_filled_qty = avg_rebound_dec("4");
        state.opposite_total_cost = avg_rebound_dec("1.78");
        state.open_primary_qty = avg_rebound_dec("19");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string()];
        state.opposite_leg_ids = vec!["s50_profit_45".to_string()];

        let decision =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.585, 8.0)]))
                .expect("stage_30 profit leg")
                .expect("s30 first leg is executable");
        assert_eq!(decision.stage_id.as_deref(), Some("stage_30"));
        assert_eq!(decision.tier_or_leg_id, "s30_profit_59");
        assert_eq!(decision.qty, avg_rebound_dec("8"));
    }

    #[test]
    fn profit_leg_qty_is_clamped_to_open_primary_qty() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.profit_started = true;
        state.primary_total_qty = avg_rebound_dec("47");
        state.primary_total_cost = avg_rebound_dec("10.90");
        state.avg_primary_cost = Some(avg_rebound_dec("0.231914893617"));
        state.opposite_filled_qty = avg_rebound_dec("44");
        state.opposite_total_cost = avg_rebound_dec("28.00");
        state.open_primary_qty = avg_rebound_dec("3");
        state.primary_tier_ids = vec!["p50".to_string(), "p30".to_string(), "p10".to_string()];
        state.opposite_leg_ids = vec!["full_profit_72".to_string(), "full_profit_64".to_string()];

        let decision =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.535, 3.0)]))
                .expect("final full profit leg")
                .expect("clamped leg is executable");
        assert_eq!(decision.tier_or_leg_id, "full_profit_54");
        assert_eq!(decision.qty, avg_rebound_dec("3"));
    }

    #[test]
    fn primary_decision_does_not_apply_extra_vwap_safety_buffer() {
        let config = avg_rebound_default_config();
        let state = state_with_session();
        let decision =
            avg_rebound_primary_decision(&config, &state, &tokens(), &book(&[(0.50, 8.0)]))
                .expect("p50 decision")
                .expect("p50 exact cap is executable");
        assert_eq!(decision.limit_price, avg_rebound_dec("0.50"));
    }

    #[test]
    fn primary_decision_blocks_hidden_market_min_notional_top_up() {
        let mut config = avg_rebound_default_config();
        config.primary_ladder = vec![AvgReboundPrimaryTierConfig {
            id: "p50".to_string(),
            price_cap: avg_rebound_dec("0.50"),
            qty: avg_rebound_dec("4"),
        }];
        let state = state_with_session();
        let rejection =
            avg_rebound_primary_decision(&config, &state, &tokens(), &book(&[(0.04, 4.0)]))
                .expect("p50 decision considered")
                .expect_err("below-min-notional decision is rejected");
        assert_eq!(
            rejection["reason"],
            json!("marketable_buy_min_notional_below_strategy_qty")
        );
        assert_eq!(
            diagnostic_decimal(&rejection["notional"]),
            avg_rebound_dec("0.16")
        );
    }

    #[test]
    fn opposite_decision_applies_extra_vwap_safety_buffer() {
        let config = avg_rebound_default_config();
        let mut state = state_with_session();
        state.primary_total_qty = avg_rebound_dec("8");
        state.primary_total_cost = avg_rebound_dec("4.00");
        state.avg_primary_cost = Some(avg_rebound_dec("0.50"));
        state.open_primary_qty = avg_rebound_dec("8");
        state.primary_tier_ids = vec!["p50".to_string()];
        let decision =
            avg_rebound_profit_decision(&config, &state, &tokens(), &book(&[(0.445, 4.0)]))
                .expect("s50 profit leg")
                .expect("buffered cap is executable");
        assert_eq!(decision.limit_price, avg_rebound_dec("0.445"));
    }
}
