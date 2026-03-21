
        let specs = open_position_ws_price_node_specs(&node, &json!({}));
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].price_mode, WsPriceMode::Raw);
    }

    #[test]
    fn ws_market_slug_override_is_ignored_for_auto_scope_when_resolved_slug_exists() {
        let auto_scope_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope"
            }),
        };
        let fixed_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "fixed"
            }),
        };

        assert!(!should_accept_ws_market_slug_override(
            &auto_scope_node,
            "btc-updown-15m-1772300000"
        ));
        assert!(should_accept_ws_market_slug_override(&auto_scope_node, ""));
        assert!(should_accept_ws_market_slug_override(
            &fixed_node,
            "btc-updown-15m-1772300000"
        ));
    }

    #[test]
    fn auto_scope_market_cache_refresh_forces_current_window_reselection() {
        let node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "marketSelection": "latest_by_slug"
            }),
        };
        let now = DateTime::parse_from_rfc3339("2026-03-09T17:16:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let scope_def = find_updown_scope_by_scope("sol_5m_updown").unwrap();

        assert!(should_force_auto_scope_market_cache_refresh(
            &node,
            scope_def,
            Some("sol-updown-5m-1773076200"),
            now
        ));
        assert!(!should_force_auto_scope_market_cache_refresh(
            &node,
            scope_def,
            Some("sol-updown-5m-1773076500"),
            now
        ));
    }

    #[test]
    fn cross_below_requires_actual_crossing() {
        // None previous → false (ilk tick, sadece fiyat kaydedilir)
        assert!(!crossed_below_strict(None, 0.25, 0.30));
        assert!(!crossed_below_strict(None, 0.35, 0.30));
        // Gercek crossing: yukaridan asagiya
        assert!(crossed_below_strict(Some(0.31), 0.30, 0.30));
        assert!(crossed_below_strict(Some(0.35), 0.29, 0.30));
        // Zaten asagida, crossing yok
        assert!(!crossed_below_strict(Some(0.28), 0.27, 0.30));
    }

    #[test]
    fn cross_above_requires_actual_crossing() {
        assert!(!crossed_above_strict(None, 0.35, 0.30));
        assert!(!crossed_above_strict(None, 0.25, 0.30));
        assert!(crossed_above_strict(Some(0.29), 0.30, 0.30));
        assert!(crossed_above_strict(Some(0.25), 0.31, 0.30));
        assert!(!crossed_above_strict(Some(0.32), 0.33, 0.30));
    }

    #[test]
    fn trigger_market_price_allows_first_tick_threshold_hit() {
        let (pass_above, mode_above) =
            evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", true, None);
        assert!(pass_above);
        assert_eq!(mode_above, "first_tick_threshold");

        let (pass_below, mode_below) =
            evaluate_trigger_market_price_condition(None, 0.25, 0.30, "cross_below", true, None);
        assert!(pass_below);
        assert_eq!(mode_below, "first_tick_threshold");

        let (strict_pass, strict_mode) =
            evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", false, None);
        assert!(!strict_pass);
        assert_eq!(strict_mode, "no_previous");
    }

    #[test]
    fn extract_price_ignores_price_changes_without_asset_id() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [
                    { "price": "0.71", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        assert!(extract_price_from_market_events(&events, "tok-yes").is_none());

        let events_with_asset = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [
                    { "asset_id": "tok-yes", "price": "0.71", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let extracted = extract_price_from_market_events(&events_with_asset, "tok-yes");
        assert_eq!(extracted, Some((0.71, Some(12345))));
    }

    #[test]
    fn extract_price_midpoint_mode_prefers_best_bid_ask_over_price_changes() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
                "price_changes": [
                    { "asset_id": "tok-yes", "price": "0.14", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let raw = extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Raw);
        assert_eq!(
            raw,
            Some(ExtractedWsPrice {
                price: 0.14,
                ts: Some(12345),
                source: "price_changes",
            })
        );

        let midpoint =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Midpoint);
        assert_eq!(
            midpoint,
            Some(ExtractedWsPrice {
                price: 0.58,
                ts: Some(12345),
                source: "best_bid_ask",
            })
        );
    }

    #[test]
    fn ws_price_mode_parse_best_bid_ask_aliases() {
        assert_eq!(
            WsPriceMode::parse(Some("site_display"), WsPriceMode::Midpoint),
            WsPriceMode::SiteDisplay
        );
        assert_eq!(
            WsPriceMode::parse(Some("display"), WsPriceMode::Midpoint),
            WsPriceMode::SiteDisplay
        );
        assert_eq!(
            WsPriceMode::parse(Some("best_bid"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("bid"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("best_ask"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("ask"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("BEST_BID"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some(" Best_Ask "), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("BID"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("ASK"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
    }

    #[test]
    fn extract_price_best_bid_mode_returns_bid_only() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let best_bid =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
        assert_eq!(
            best_bid,
            Some(ExtractedWsPrice {
                price: 0.57,
                ts: Some(12345),
                source: "best_bid",
            })
        );
    }

    #[test]
    fn extract_price_best_ask_mode_returns_ask_only() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let best_ask =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
        assert_eq!(
            best_ask,
            Some(ExtractedWsPrice {
                price: 0.59,
                ts: Some(12345),
                source: "best_ask",
            })
        );
    }

    #[test]
    fn extract_price_site_display_uses_midpoint_for_tight_spread() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.77",
                "best_ask": "0.85",
                "price": "0.90",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let site_display = extract_price_from_market_events_with_mode(
            &events,
            "tok-yes",
            WsPriceMode::SiteDisplay,
        );
        assert_eq!(
            site_display,
            Some(ExtractedWsPrice {
                price: 0.81,
                ts: Some(12345),
                source: "site_display_midpoint",
            })
        );
    }

    #[test]
    fn extract_price_site_display_uses_last_trade_for_wide_spread() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.70",
                "best_ask": "0.90",
                "price": "0.77",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let site_display = extract_price_from_market_events_with_mode(
            &events,
            "tok-yes",
            WsPriceMode::SiteDisplay,
        );
        assert_eq!(
            site_display,
            Some(ExtractedWsPrice {
                price: 0.77,
                ts: Some(12345),
                source: "site_display_last_trade",
            })
        );
    }

    #[test]
    fn extract_price_best_bid_from_price_changes() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [{
                    "asset_id": "tok-yes",
                    "best_bid": "0.45",
                    "best_ask": "0.47",
                    "timestamp": 99999
                }]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(88888),
        }];

        let bid =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
        assert_eq!(
            bid,
            Some(ExtractedWsPrice {
                price: 0.45,
                ts: Some(99999),
                source: "best_bid",
            })
        );

        let ask =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
        assert_eq!(
            ask,
            Some(ExtractedWsPrice {
                price: 0.47,
                ts: Some(99999),
                source: "best_ask",
            })
        );
    }

    #[test]
    fn market_once_idempotency_key_contains_market_slug() {
        let key = trade_flow_market_price_once_idempotency_key(
            77,
            "trigger_market",
            true,
            Some("btc-updown-5m-1772296200"),
            0,
        );
        assert_eq!(
            key,
            "flow-once-fired:77:trigger_market:btc-updown-5m-1772296200"
        );
    }

    #[test]
    fn market_once_state_clears_when_market_changes() {
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {
                "trigger_market": {
                    "once_fired": true,
                    "once_fired_market_slug": "btc-updown-5m-old"
                }
            }
        });
        sync_trade_flow_market_price_once_scope_state(
            &mut context,
            "trigger_market",
            true,
            Some("btc-updown-5m-new"),
        );
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_market",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
    }

    #[test]
    fn parse_lock_pid_extracts_pid_value() {
        assert_eq!(parse_lock_pid("pid=12345\n"), Some(12345));
        assert_eq!(parse_lock_pid("foo=1\npid=77\n"), Some(77));
        assert_eq!(parse_lock_pid("pid=abc\n"), None);
        assert_eq!(parse_lock_pid(""), None);
    }

    #[test]
    fn market_price_once_detection_only_matches_once_mode() {
        let once_node = TradeFlowNode {
            key: "trigger_once".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({ "repeatMode": "once" }),
        };
        let loop_node = TradeFlowNode {
            key: "trigger_loop".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({ "repeatMode": "loop" }),
        };
        let open_positions_node = TradeFlowNode {
            key: "trigger_open".to_string(),
            node_type: "trigger.open_positions".to_string(),
            config: json!({ "repeatMode": "once" }),
        };

        assert!(is_trade_flow_market_price_once_node(&once_node));
        assert!(!is_trade_flow_market_price_once_node(&loop_node));
        assert!(!is_trade_flow_market_price_once_node(&open_positions_node));
    }

    #[test]
    fn publish_marker_change_resets_once_state_for_once_nodes() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![
                TradeFlowNode {
                    key: "trigger_once".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "repeatMode": "once" }),
                },
                TradeFlowNode {
                    key: "trigger_loop".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "repeatMode": "loop" }),
                },
            ],
            edges: vec![],
        };
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {
                "__publish_marker": "101:1000"
            },
            "refs": {},
            "nodeState": {
                "trigger_once": {
                    "once_fired": true,
                    "once_fired_at": "2026-02-28T00:00:00Z",
                    "once_blocked_logged": true
                },
                "trigger_loop": {
                    "once_fired": true
                }
            }
        });

        let (previous_marker, reset_nodes) =
            sync_trade_flow_once_state_for_publish(&graph, &mut context, "101:2000");
        assert_eq!(previous_marker.as_deref(), Some("101:1000"));
        assert_eq!(reset_nodes, vec!["trigger_once".to_string()]);
        assert_eq!(
            flow_state_string(&context, FLOW_STATE_PUBLISH_MARKER).as_deref(),
            Some("101:2000")
        );
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_once",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
        assert!(flow_node_state_truthy(
            &context,
            "trigger_loop",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
    }

    #[test]
    fn publish_marker_same_keeps_once_state_intact() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![TradeFlowNode {
                key: "trigger_once".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({ "repeatMode": "once" }),
            }],
            edges: vec![],
        };
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {
                "__publish_marker": "77:5000"
            },
            "refs": {},
            "nodeState": {
                "trigger_once": {
                    "once_fired": true
                }
            }
        });

        let (previous_marker, reset_nodes) =
            sync_trade_flow_once_state_for_publish(&graph, &mut context, "77:5000");
        assert_eq!(previous_marker.as_deref(), Some("77:5000"));
        assert!(reset_nodes.is_empty());
        assert!(flow_node_state_truthy(
            &context,
            "trigger_once",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
    }

    #[test]
    fn initial_seed_prefers_triggers_when_present() {
        let graph = runtime_graph(
            vec![
                ("trigger_tick", "trigger.market_price"),
                ("dual_root", "action.dual_dca"),
            ],
            vec![],
        );

        let (mode, start_nodes) =
            select_trade_flow_initial_seed_nodes(&graph).expect("selection should succeed");
        assert_eq!(mode, TradeFlowSeedMode::Trigger);
        assert_eq!(start_nodes.len(), 1);
        assert_eq!(start_nodes[0].key, "trigger_tick");
    }

    #[test]
    fn initial_seed_allows_dual_dca_roots_without_triggers() {
        let graph = runtime_graph(
            vec![
                ("dual_root", "action.dual_dca"),
                ("dual_child", "action.dual_dca"),
            ],
            vec![("dual_root", "dual_child")],
        );

        let (mode, start_nodes) =
            select_trade_flow_initial_seed_nodes(&graph).expect("selection should succeed");
        assert_eq!(mode, TradeFlowSeedMode::DualDcaRoot);
        assert_eq!(start_nodes.len(), 1);
        assert_eq!(start_nodes[0].key, "dual_root");
    }

    #[test]
    fn initial_seed_rejects_non_dual_roots_without_triggers() {
        let graph = runtime_graph(
            vec![
                ("dual_root", "action.dual_dca"),
                ("notify_root", "action.notify"),
            ],
            vec![],
        );

        let err = select_trade_flow_initial_seed_nodes(&graph).expect_err("should fail");
        assert_eq!(err, "flow_invalid_roots_without_trigger");
    }

    #[test]
    fn initial_seed_requires_trigger_when_dual_dca_absent() {
        let graph = runtime_graph(vec![("notify_root", "action.notify")], vec![]);

        let err = select_trade_flow_initial_seed_nodes(&graph).expect_err("should fail");
        assert_eq!(err, "flow_missing_trigger");
    }
    #[test]
    fn candidate_slugs_cover_prev_current_and_future_15m_windows() {
        let scope_def = find_updown_scope_by_scope("btc_15m_updown").expect("scope should exist");
        let now = DateTime::parse_from_rfc3339("2026-02-23T14:55:18Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let slugs = updown_scope_candidate_slugs(scope_def, now);
        assert_eq!(
            slugs,
            vec![
                "btc-updown-15m-1771857000".to_string(),
                "btc-updown-15m-1771857900".to_string(),
                "btc-updown-15m-1771858800".to_string(),
                "btc-updown-15m-1771859700".to_string(),
            ]
        );
    }

    #[test]
    fn candidate_slugs_cover_prev_current_and_future_5m_windows() {
        let scope_def = find_updown_scope_by_scope("btc_5m_updown").expect("scope should exist");
        let now = DateTime::parse_from_rfc3339("2026-02-23T14:55:18Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let slugs = updown_scope_candidate_slugs(scope_def, now);
        assert_eq!(
            slugs,
            vec![
                "btc-updown-5m-1771858200".to_string(),
                "btc-updown-5m-1771858500".to_string(),
                "btc-updown-5m-1771858800".to_string(),
                "btc-updown-5m-1771859100".to_string(),
            ]
        );
    }

    fn gamma_market_for_test(slug: &str) -> GammaMarket {
        GammaMarket {
            slug: slug.to_string(),
            end_date_iso: None,
            active: true,
            closed: false,
            yes_token_id: Some("yes-token".to_string()),
            no_token_id: Some("no-token".to_string()),
            maker_base_fee: 0,
            neg_risk: false,
            order_price_min_tick_size: None,
            order_min_size: None,
        }
    }

    #[test]
    fn select_preferred_live_market_prefers_current_window() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:33:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-1771885800"),
            gamma_market_for_test("btc-updown-15m-1771886700"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-1771885800");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::InWindow
        );
    }

    #[test]
    fn select_preferred_live_market_uses_nearest_future_when_no_current() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:29:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-1771885800"),
            gamma_market_for_test("btc-updown-15m-1771886700"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-1771885800");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::NearestFuture
        );
    }

    #[test]
    fn select_preferred_live_market_falls_back_to_latest_slug() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:33:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-alpha"),
            gamma_market_for_test("btc-updown-15m-beta"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-beta");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::LatestBySlugFallback
        );
    }

    fn make_leg(levels_filled: u32, last_fill_price: Option<f64>) -> DualLegRuntime {
        DualLegRuntime {
            side: LegSide::Yes,
            token_id: "tok".to_string(),
            qty: 10.0,
            avg_entry: 0.50,
            levels_filled,
            last_fill_price,
            last_dca_at: None,
        }
    }

    #[test]
    fn apply_fill_buy_increments_when_flag_true() {
        let mut leg = make_leg(0, None);
        apply_fill_to_leg(&mut leg, "buy", 0.50, 10.0, true);
        assert_eq!(leg.levels_filled, 1);
        assert_eq!(leg.last_fill_price, Some(0.50));
    }

    #[test]
    fn apply_fill_buy_no_increment_when_flag_false() {
        let mut leg = make_leg(2, Some(0.50));
        apply_fill_to_leg(&mut leg, "buy", 0.48, 5.0, false);
        assert_eq!(leg.levels_filled, 2);
        assert_eq!(leg.last_fill_price, Some(0.48));
    }

    #[test]
    fn apply_fill_sell_does_not_increment_levels() {
        let mut leg = make_leg(3, Some(0.60));
        apply_fill_to_leg(&mut leg, "sell", 0.65, 5.0, true);
        assert_eq!(leg.levels_filled, 3);
    }

    #[test]
    fn apply_fill_buy_avg_entry_correct() {
        let mut leg = make_leg(0, None);
        leg.qty = 10.0;
        leg.avg_entry = 0.50;
        apply_fill_to_leg(&mut leg, "buy", 0.40, 10.0, false);
        assert!((leg.avg_entry - 0.45).abs() < 1e-9);
        assert_eq!(leg.qty, 20.0);
    }

    #[test]
    fn dca_cap_simulation_max_3_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        let max = 3u32;
        let steps = [0.50f64, 0.48, 0.46];
        for price in steps {
            assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
            leg.levels_filled += 1;
            leg.last_fill_price = Some(price);
        }
        assert!(!strat.should_dca_leg(0.44, leg.last_fill_price, 0.02, leg.levels_filled, max));
        assert!(!strat.should_dca_leg(0.01, leg.last_fill_price, 0.02, leg.levels_filled, max));
    }

    #[test]
    fn dca_cap_simulation_max_1_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        assert!(strat.should_dca_leg(0.50, leg.last_fill_price, 0.02, leg.levels_filled, 1));
        leg.levels_filled += 1;
        leg.last_fill_price = Some(0.50);
        assert!(!strat.should_dca_leg(0.45, leg.last_fill_price, 0.02, leg.levels_filled, 1));
    }

    #[test]
    fn dca_cap_simulation_max_5_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        let max = 5u32;
        let prices = [0.50f64, 0.48, 0.46, 0.44, 0.42];
        for price in prices {
            assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
            leg.levels_filled += 1;
            leg.last_fill_price = Some(price);
        }
        assert!(!strat.should_dca_leg(0.40, leg.last_fill_price, 0.02, leg.levels_filled, max));
    }
}

#[cfg(test)]
mod place_order_binding_tests {
    use super::*;

    fn test_step(input_json: Value) -> TradeFlowRunStep {
        TradeFlowRunStep {
            id: 1,
            run_id: 42,
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(input_json),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        }
    }

    fn test_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn test_builder_order(side: &str, parent_order_id: Option<i64>) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "conditional".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: side.to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_above".to_string()),
            trigger_price: Some(0.8),
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            tp_enabled: false,
            tp_price: None,
            sl_enabled: false,
            sl_price: None,
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
        }
    }
