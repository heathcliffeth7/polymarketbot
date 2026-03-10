use super::support::*;
use super::*;

#[test]
fn market_once_idempotency_key_contains_market_slug() {
    let key = trade_flow_market_price_once_idempotency_key(
        77,
        "trigger_market",
        true,
        Some("btc-updown-5m-1772296200"),
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
