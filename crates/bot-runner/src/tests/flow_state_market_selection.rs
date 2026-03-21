use super::support::*;
use super::*;

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
fn market_once_idempotency_key_appends_reentry_generation_suffix() {
    let key = trade_flow_market_price_once_idempotency_key(
        77,
        "trigger_market",
        true,
        Some("btc-updown-5m-1772296200"),
        2,
    );
    assert_eq!(
        key,
        "flow-once-fired:77:trigger_market:btc-updown-5m-1772296200:gen2"
    );
}

#[test]
fn upstream_market_price_trigger_key_requires_exactly_one_trigger() {
    let unique_graph = TradeFlowGraphRuntime {
        context: json!({}),
        nodes: vec![
            TradeFlowNode {
                key: "trigger_market".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({ "repeatMode": "once" }),
            },
            TradeFlowNode {
                key: "logic_delay".to_string(),
                node_type: "logic.delay".to_string(),
                config: json!({}),
            },
            TradeFlowNode {
                key: "action_buy".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
        ],
        edges: vec![
            TradeFlowEdge {
                source: "trigger_market".to_string(),
                target: "logic_delay".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            },
            TradeFlowEdge {
                source: "logic_delay".to_string(),
                target: "action_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            },
        ],
    };
    assert_eq!(
        find_upstream_market_price_trigger_key("action_buy", &unique_graph),
        Some("trigger_market".to_string())
    );

    let multiple_graph = TradeFlowGraphRuntime {
        context: json!({}),
        nodes: vec![
            TradeFlowNode {
                key: "trigger_a".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({ "repeatMode": "once" }),
            },
            TradeFlowNode {
                key: "trigger_b".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({ "repeatMode": "once" }),
            },
            TradeFlowNode {
                key: "action_buy".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
        ],
        edges: vec![
            TradeFlowEdge {
                source: "trigger_a".to_string(),
                target: "action_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            },
            TradeFlowEdge {
                source: "trigger_b".to_string(),
                target: "action_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            },
        ],
    };
    assert_eq!(
        find_upstream_market_price_trigger_key("action_buy", &multiple_graph),
        None
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
fn publish_auto_scope_lock_blocks_same_market_for_run_scope_once() {
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_market": {
                "publish_auto_scope_locked_market_slug": "btc-updown-5m-1772296200"
            }
        }
    });

    assert!(trade_flow_market_price_once_fired_for_scope(
        &context,
        "trigger_market",
        false,
        Some("btc-updown-5m-1772296200")
    ));
    assert!(!trade_flow_market_price_once_fired_for_scope(
        &context,
        "trigger_market",
        false,
        Some("btc-updown-5m-1772296500")
    ));
}

#[test]
fn publish_auto_scope_lock_clears_when_market_changes() {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_market": {
                "publish_auto_scope_locked_market_slug": "btc-updown-5m-old"
            }
        }
    });

    sync_trade_flow_market_price_once_scope_state(
        &mut context,
        "trigger_market",
        false,
        Some("btc-updown-5m-new"),
    );

    assert_eq!(
        flow_node_state_string(
            &context,
            "trigger_market",
            FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
        ),
        None
    );
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
fn publish_marker_change_resets_fixed_once_trigger_publish_state() {
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
                "once_fired_market_slug": "tur-gal-iba-2026-03-14",
                "once_blocked_logged": true,
                "auto_scope_market_slug": "btc-updown-5m-1772296200",
                "auto_scope_market_scope": "btc_5m_updown",
                "auto_scope_market_asset": "btc",
                "auto_scope_market_timeframe": "5m",
                "auto_scope_yes_token_id": "yes-token",
                "auto_scope_no_token_id": "no-token",
                "auto_scope_resolved_token_id": "yes-token",
                "auto_scope_resolved_outcome_label": "Yes",
                "auto_scope_selection_reason": "in_window",
                "last_price": 0.62,
                "last_ws_market_slug": "tur-gal-iba-2026-03-14",
                "previous_price": 0.62,
                "previous_price_123": 0.62,
                "price_samples_123": [{ "price": 0.62, "ts_ms": 1773509181571i64 }],
                "cross_pending_at_123": "2026-03-14T17:26:21Z",
                "cross_pending_price_123": 0.62,
                "cross_pending_prev_123": 0.61,
                "cycle_window_boundary_marker_123": "open",
                "cycle_window_last_eval_123": "2026-03-14T17:26:21Z"
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
    for cleared_key in [
        "auto_scope_market_slug",
        "auto_scope_market_scope",
        "auto_scope_market_asset",
        "auto_scope_market_timeframe",
        "auto_scope_yes_token_id",
        "auto_scope_no_token_id",
        "auto_scope_resolved_token_id",
        "auto_scope_resolved_outcome_label",
        "auto_scope_selection_reason",
    ] {
        assert!(flow_node_state(&context, "trigger_once", cleared_key).is_none());
    }
    assert!(flow_node_state(&context, "trigger_once", "last_price").is_none());
    assert!(flow_node_state(&context, "trigger_once", "last_ws_market_slug").is_none());
    assert!(flow_node_state(&context, "trigger_once", "previous_price").is_none());
    assert!(flow_node_state(&context, "trigger_once", "previous_price_123").is_none());
    assert!(flow_node_state(&context, "trigger_once", "price_samples_123").is_none());
    assert!(flow_node_state(&context, "trigger_once", "cross_pending_at_123").is_none());
    assert!(flow_node_state(&context, "trigger_once", "cross_pending_price_123").is_none());
    assert!(flow_node_state(&context, "trigger_once", "cross_pending_prev_123").is_none());
    assert!(
        flow_node_state(&context, "trigger_once", "cycle_window_boundary_marker_123").is_none()
    );
    assert!(flow_node_state(&context, "trigger_once", "cycle_window_last_eval_123").is_none());
    assert!(flow_node_state_truthy(
        &context,
        "trigger_loop",
        FLOW_NODE_STATE_ONCE_FIRED
    ));
}

#[test]
fn publish_marker_change_preserves_auto_scope_publish_lock_behavior() {
    let graph = TradeFlowGraphRuntime {
        context: json!({
            "marketSlug": "tur-gal-iba-2026-03-14"
        }),
        nodes: vec![TradeFlowNode {
            key: "trigger_auto".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "repeatMode": "once",
                "marketMode": "auto_scope",
                "onceScope": "run",
                "onceScopeVersion": 2
            }),
        }],
        edges: vec![],
    };
    let mut context = json!({
        "flowContext": {
            "marketSlug": "tur-gal-iba-2026-03-14"
        },
        "vars": {},
        "state": {
            "__publish_marker": "101:1000"
        },
        "refs": {},
        "nodeState": {
            "trigger_auto": {
                "once_fired": true,
                "once_fired_at": "2026-02-28T00:00:00Z",
                "once_fired_market_slug": "tur-gal-iba-2026-03-14",
                "once_blocked_logged": true,
                "previous_price_123": 0.62
            }
        }
    });

    let (previous_marker, reset_nodes) =
        sync_trade_flow_once_state_for_publish(&graph, &mut context, "101:2000");
    assert_eq!(previous_marker.as_deref(), Some("101:1000"));
    assert_eq!(reset_nodes, vec!["trigger_auto".to_string()]);
    assert_eq!(
        flow_state_string(&context, FLOW_STATE_PUBLISH_MARKER).as_deref(),
        Some("101:2000")
    );
    assert!(!flow_node_state_truthy(
        &context,
        "trigger_auto",
        FLOW_NODE_STATE_ONCE_FIRED
    ));
    assert_eq!(
        flow_node_state_string(
            &context,
            "trigger_auto",
            FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG
        )
        .as_deref(),
        Some("tur-gal-iba-2026-03-14")
    );
    assert_eq!(
        flow_node_state(&context, "trigger_auto", "previous_price_123").and_then(value_as_f64),
        Some(0.62)
    );
}

#[test]
fn publish_marker_change_keeps_auto_scope_market_once_state_intact() {
    let graph = TradeFlowGraphRuntime {
        context: json!({
            "marketSlug": "tur-gal-iba-2026-03-14"
        }),
        nodes: vec![TradeFlowNode {
            key: "trigger_auto_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "repeatMode": "once",
                "marketMode": "auto_scope",
                "onceScope": "market"
            }),
        }],
        edges: vec![],
    };
    let mut context = json!({
        "flowContext": {
            "marketSlug": "tur-gal-iba-2026-03-14"
        },
        "vars": {},
        "state": {
            "__publish_marker": "101:1000"
        },
        "refs": {},
        "nodeState": {
            "trigger_auto_market": {
                "once_fired": true,
                "once_fired_market_slug": "tur-gal-iba-2026-03-14",
                "previous_price_123": 0.62
            }
        }
    });

    let (previous_marker, reset_nodes) =
        sync_trade_flow_once_state_for_publish(&graph, &mut context, "101:2000");
    assert_eq!(previous_marker.as_deref(), Some("101:1000"));
    assert!(reset_nodes.is_empty());
    assert!(flow_node_state_truthy(
        &context,
        "trigger_auto_market",
        FLOW_NODE_STATE_ONCE_FIRED
    ));
    assert_eq!(
        flow_node_state_string(
            &context,
            "trigger_auto_market",
            FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG
        )
        .as_deref(),
        Some("tur-gal-iba-2026-03-14")
    );
    assert_eq!(
        flow_node_state(&context, "trigger_auto_market", "previous_price_123")
            .and_then(value_as_f64),
        Some(0.62)
    );
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

    let selected = select_preferred_live_market(markets, now).expect("market should be selected");
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

    let selected = select_preferred_live_market(markets, now).expect("market should be selected");
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

    let selected = select_preferred_live_market(markets, now).expect("market should be selected");
    assert_eq!(selected.slug, "btc-updown-15m-beta");
    assert_eq!(
        selected.selection_reason,
        LiveMarketSelectionReason::LatestBySlugFallback
    );
}

#[test]
fn scope_candidate_window_markets_prefers_current_window_after_probe_merge() {
    let scope_def = find_updown_scope_by_scope("btc_5m_updown").expect("scope should exist");
    let now = DateTime::<Utc>::from_timestamp(1_771_886_101, 0).expect("timestamp should exist");
    let markets = vec![
        gamma_market_for_test("btc-updown-5m-1771885800"),
        gamma_market_for_test("btc-updown-5m-1771886100"),
    ];

    let selected = scope_candidate_window_markets(scope_def, &markets, now);
    assert_eq!(
        selected
            .into_iter()
            .map(|market| market.slug)
            .collect::<Vec<_>>(),
        vec!["btc-updown-5m-1771886100".to_string()]
    );
}

#[test]
fn detect_trade_builder_stale_market_ignores_non_rolling_slugs() {
    let selected = SelectedLiveMarket {
        slug: "btc-updown-5m-1771885800".to_string(),
        yes_token_id: Some("yes-token".to_string()),
        no_token_id: Some("no-token".to_string()),
        maker_base_fee: 0,
        starts_at: None,
        ends_at: None,
        selection_reason: LiveMarketSelectionReason::InWindow,
    };

    assert_eq!(
        detect_trade_builder_stale_market("some-static-market", Some(&selected)),
        None
    );
}

#[test]
fn detect_trade_builder_stale_market_returns_none_for_current_cycle() {
    let selected = SelectedLiveMarket {
        slug: "btc-updown-5m-1771885800".to_string(),
        yes_token_id: Some("yes-token".to_string()),
        no_token_id: Some("no-token".to_string()),
        maker_base_fee: 0,
        starts_at: None,
        ends_at: None,
        selection_reason: LiveMarketSelectionReason::InWindow,
    };

    assert_eq!(
        detect_trade_builder_stale_market("btc-updown-5m-1771885800", Some(&selected)),
        None
    );
}

#[test]
fn detect_trade_builder_stale_market_flags_old_cycle() {
    let selected = SelectedLiveMarket {
        slug: "btc-updown-5m-1771886100".to_string(),
        yes_token_id: Some("yes-token".to_string()),
        no_token_id: Some("no-token".to_string()),
        maker_base_fee: 0,
        starts_at: None,
        ends_at: None,
        selection_reason: LiveMarketSelectionReason::NearestFuture,
    };

    assert_eq!(
        detect_trade_builder_stale_market("btc-updown-5m-1771885800", Some(&selected)),
        Some(TradeBuilderStaleRollingMarket {
            detected_scope: "btc_5m_updown",
            detected_asset: "btc",
            detected_timeframe: "5m",
            current_live_market_slug: "btc-updown-5m-1771886100".to_string(),
            current_live_selection_reason: LiveMarketSelectionReason::NearestFuture,
        })
    );
}
