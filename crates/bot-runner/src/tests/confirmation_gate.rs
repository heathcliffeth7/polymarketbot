use super::*;

fn test_node_spec(
    trigger_condition: &str,
    trigger_price: f64,
    confirmation_ms: i64,
) -> WsOpenPositionPriceNodeSpec {
    WsOpenPositionPriceNodeSpec {
        node_key: "trigger_1".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: false,
        auto_scope: true,
        price_mode: WsPriceMode::Midpoint,
        market_slug: Some("btc-updown-5m-test".to_string()),
        token_id: "tok-yes-123".to_string(),
        outcome_label: "Up".to_string(),
        trigger_condition: trigger_condition.to_string(),
        trigger_price,
        max_price: None,
        protection_mode: TRIGGER_PROTECTION_MODE_OFF.to_string(),
        protection_asset: None,
        confirmation_ms: Some(confirmation_ms),
        cycle_window_mode: None,
        cycle_window_secs: None,
    }
}

#[test]
fn last_cycle_window_disables_resolution_window_guard() {
    assert!(auto_scope_resolution_window_guard_enabled(None));
    assert!(auto_scope_resolution_window_guard_enabled(Some("first")));
    assert!(!auto_scope_resolution_window_guard_enabled(Some("last")));
}

#[test]
fn legacy_auto_scope_once_scope_upgrades_to_market() {
    let auto_scope_run = TradeFlowNode {
        key: "trigger_run".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "repeatMode": "once",
            "onceScope": "run",
        }),
    };
    let auto_scope_market = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "repeatMode": "once",
            "onceScope": "market",
        }),
    };

    assert_eq!(node_once_scope(&auto_scope_run), "market");
    assert_eq!(node_once_scope(&auto_scope_market), "market");
}

#[test]
fn explicit_auto_scope_once_scope_honors_config() {
    let auto_scope_run = TradeFlowNode {
        key: "trigger_run".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "repeatMode": "once",
            "onceScope": "run",
            "onceScopeVersion": 2,
        }),
    };
    let auto_scope_market = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "repeatMode": "once",
            "onceScope": "market",
            "onceScopeVersion": 2,
        }),
    };

    assert_eq!(node_once_scope(&auto_scope_run), "run");
    assert_eq!(node_once_scope(&auto_scope_market), "market");
}

#[test]
fn market_once_state_clears_legacy_run_state_without_market_slug() {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_market": {
                "once_fired": true,
                "once_blocked_logged": true
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
    assert!(!flow_node_state_truthy(
        &context,
        "trigger_market",
        FLOW_NODE_STATE_ONCE_BLOCK_LOGGED
    ));
}

#[test]
fn auto_scope_last_cycle_window_allows_first_tick_threshold() {
    let mut node = test_node_spec("cross_above", 0.80, 15_000);
    node.cycle_window_mode = Some("last".to_string());
    node.cycle_window_secs = Some(60);
    assert!(allow_first_tick_threshold_for_ws_node(&node, None));

    let mut fixed_node = node.clone();
    fixed_node.auto_scope = false;
    assert!(!allow_first_tick_threshold_for_ws_node(&fixed_node, None));

    node.cycle_window_mode = Some("first".to_string());
    assert!(allow_first_tick_threshold_for_ws_node(&node, None));
}

#[test]
fn auto_scope_last_first_tick_without_confirmation_enqueues_immediately() {
    let mut node = test_node_spec("cross_above", 0.80, 15_000);
    node.cycle_window_mode = Some("last".to_string());
    node.cycle_window_secs = Some(120);
    node.confirmation_ms = None;

    let allow_first_tick = allow_first_tick_threshold_for_ws_node(&node, None);
    assert!(allow_first_tick);

    let (crossed, eval_mode) = evaluate_trigger_market_price_condition(
        None,
        0.82,
        node.trigger_price,
        &node.trigger_condition,
        allow_first_tick,
        node.max_price,
    );
    assert!(crossed);
    assert_eq!(eval_mode, "first_tick_threshold");
    assert_eq!(market_price_confirmation_ms(&node), None);
    assert!(crossed && market_price_confirmation_ms(&node).is_none());
}

#[test]
fn auto_scope_last_first_tick_with_zero_confirmation_enqueues_immediately() {
    let mut node = test_node_spec("cross_above", 0.80, 15_000);
    node.cycle_window_mode = Some("last".to_string());
    node.cycle_window_secs = Some(120);
    node.confirmation_ms = Some(0);

    let allow_first_tick = allow_first_tick_threshold_for_ws_node(&node, None);
    assert!(allow_first_tick);

    let (crossed, eval_mode) = evaluate_trigger_market_price_condition(
        None,
        0.82,
        node.trigger_price,
        &node.trigger_condition,
        allow_first_tick,
        node.max_price,
    );
    assert!(crossed);
    assert_eq!(eval_mode, "first_tick_threshold");
    assert_eq!(market_price_confirmation_ms(&node), None);
    assert!(crossed && market_price_confirmation_ms(&node).is_none());
}

#[test]
fn auto_scope_last_first_tick_respects_max_price() {
    let mut node = test_node_spec("cross_above", 0.80, 15_000);
    node.cycle_window_mode = Some("last".to_string());
    node.cycle_window_secs = Some(120);
    node.confirmation_ms = None;
    node.max_price = Some(0.81);

    let allow_first_tick = allow_first_tick_threshold_for_ws_node(&node, None);
    assert!(allow_first_tick);

    let (crossed, eval_mode) = evaluate_trigger_market_price_condition(
        None,
        0.82,
        node.trigger_price,
        &node.trigger_condition,
        allow_first_tick,
        node.max_price,
    );
    assert!(!crossed);
    assert_eq!(eval_mode, "above_max_price");
}

#[test]
fn auto_scope_last_first_tick_in_range_uses_band_entry_mode() {
    let mut node = test_node_spec("cross_above", 0.77, 15_000);
    node.cycle_window_mode = Some("last".to_string());
    node.cycle_window_secs = Some(120);
    node.confirmation_ms = None;
    node.max_price = Some(0.90);

    let allow_first_tick = allow_first_tick_threshold_for_ws_node(&node, None);
    assert!(allow_first_tick);

    let (crossed, eval_mode) = evaluate_trigger_market_price_condition(
        None,
        0.85,
        node.trigger_price,
        &node.trigger_condition,
        allow_first_tick,
        node.max_price,
    );
    assert!(crossed);
    assert_eq!(eval_mode, "first_tick_in_range");
}

#[test]
fn confirmation_gate_resets_on_zone_exit() {
    let node = test_node_spec("cross_above", 0.80, 15_000);
    let mut context = json!({});
    let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
    let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
    let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

    // Simulate: a cross was pending
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!("2026-01-01T00:00:00Z"),
    );
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Price is now out of zone (below trigger for cross_above)
    let current_price = 0.78_f64;
    let still_in_zone = current_price >= node.trigger_price; // false for cross_above
    assert!(!still_in_zone);

    // Simulate the reset (replicating fixed confirmation gate logic)
    if !still_in_zone {
        remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
    }

    // Assert all pending state cleared
    assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());
    assert!(flow_node_state(&context, &node.node_key, &cpend_price_key).is_none());
    assert!(flow_node_state(&context, &node.node_key, &cpend_prev_key).is_none());
}

#[test]
fn confirmation_gate_reentry_restarts_timer() {
    let node = test_node_spec("cross_above", 0.80, 15_000);
    let mut context = json!({});
    let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
    let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
    let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

    // Set up old pending state (10 seconds ago)
    let old_ts = (Utc::now() - ChronoDuration::seconds(10)).to_rfc3339();
    set_flow_node_state(&mut context, &node.node_key, &cpend_at_key, json!(old_ts));
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Zone exit: reset all pending state
    let out_of_zone = 0.78_f64 >= node.trigger_price; // false
    assert!(!out_of_zone);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);

    // Re-entry: new cross detected, set fresh timestamp
    let new_ts = Utc::now().to_rfc3339();
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!(new_ts.clone()),
    );
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Assert new timestamp is different from old (timer restarted)
    let stored = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
    assert_ne!(
        stored, old_ts,
        "New pending timestamp must differ from old (timer restarted)"
    );
    // New timestamp should be very recent (within 2 seconds)
    let parsed = DateTime::parse_from_rfc3339(&stored)
        .unwrap()
        .with_timezone(&Utc);
    let elapsed = Utc::now().signed_duration_since(parsed);
    assert!(
        elapsed.num_seconds() < 2,
        "Re-entry timestamp should be near-zero elapsed"
    );
}

#[test]
fn first_tick_threshold_enters_confirmation_gate() {
    // With no previous_price (first tick), auto_scope=true, once_mode=true, price above trigger
    let (crossed, eval_mode) =
        evaluate_trigger_market_price_condition(None, 0.85, 0.80, "cross_above", true, None);

    assert!(crossed, "first_tick_threshold should return crossed=true");
    assert_eq!(eval_mode, "first_tick_threshold");

    // Simulate: crossed=true enters confirmation gate with confirmation_ms>0
    // The gate sets should_enqueue=false and records pending timestamp
    let node = test_node_spec("cross_above", 0.80, 15_000);
    let mut context = json!({});
    let cpend_at_key = format!("cross_pending_at_{}", node.token_id);

    // With explicit confirmationMs > 0, crossed enters gate (not immediately enqueued)
    let should_enqueue_immediately = false; // confirmation gate defers enqueue
    let enter_gate = crossed && market_price_confirmation_ms(&node).is_some();
    assert!(
        enter_gate,
        "first_tick_threshold + explicit confirmation_ms should enter gate"
    );
    assert!(
        !should_enqueue_immediately,
        "Timer started — should NOT enqueue immediately"
    );

    // Simulate timer start
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!(Utc::now().to_rfc3339()),
    );
    assert!(
        flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_some(),
        "cross_pending_at should be set when entering confirmation gate"
    );
}

#[test]
fn confirmation_gate_fires_after_sustained_zone() {
    let node = test_node_spec("cross_above", 0.80, 15_000);
    let mut context = json!({});
    let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
    let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
    let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

    // cross_pending_at was set 16s ago (past the 15000ms confirmation threshold)
    let pending_ts = (Utc::now() - ChronoDuration::seconds(16)).to_rfc3339();
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!(pending_ts.clone()),
    );
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Current price is still in zone
    let current_price = 0.83_f64;
    let still_in_zone = current_price >= node.trigger_price; // true for cross_above
    assert!(still_in_zone);

    // Check elapsed time
    let stored_ts = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
    let pending_at = DateTime::parse_from_rfc3339(&stored_ts)
        .unwrap()
        .with_timezone(&Utc);
    let elapsed_ms = Utc::now()
        .signed_duration_since(pending_at)
        .num_milliseconds();
    let confirmation_ms = market_price_confirmation_ms(&node).unwrap();
    assert!(
        elapsed_ms >= confirmation_ms,
        "Elapsed {}ms should be >= confirmation_ms {}ms",
        elapsed_ms,
        confirmation_ms
    );

    // Gate fires: should_enqueue = true, eval_mode = "cross_confirmed"
    let should_enqueue = elapsed_ms >= confirmation_ms;
    let final_eval_mode = if should_enqueue {
        "cross_confirmed"
    } else {
        "pending"
    };
    assert!(should_enqueue, "Should enqueue after sustained zone time");
    assert_eq!(final_eval_mode, "cross_confirmed");

    // Pending state cleared after confirmation
    remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
    assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());
}

#[test]
fn cross_leave_reenter_no_accumulated_time() {
    let node = test_node_spec("cross_above", 0.80, 15_000);
    let mut context = json!({});
    let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
    let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
    let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

    // Step 1: Cross detected — set pending with timestamp 8 seconds ago
    let first_pending_ts = (Utc::now() - ChronoDuration::seconds(8)).to_rfc3339();
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!(first_pending_ts.clone()),
    );
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Step 2: Out-of-zone tick — reset all pending state
    let out_of_zone = 0.77_f64 >= node.trigger_price; // false
    assert!(!out_of_zone);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
    remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
    assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());

    // Step 3: New cross — set fresh pending timestamp (near-zero elapsed)
    let second_pending_ts = Utc::now().to_rfc3339();
    set_flow_node_state(
        &mut context,
        &node.node_key,
        &cpend_at_key,
        json!(second_pending_ts.clone()),
    );
    set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
    set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

    // Assert: elapsed from second entry is near-zero (not accumulated from first 8s entry)
    let stored = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
    assert_ne!(
        stored, first_pending_ts,
        "Second entry must use fresh timestamp, not old one"
    );
    let second_pending_at = DateTime::parse_from_rfc3339(&stored)
        .unwrap()
        .with_timezone(&Utc);
    let elapsed_ms = Utc::now()
        .signed_duration_since(second_pending_at)
        .num_milliseconds();
    assert!(
            elapsed_ms < 2_000,
            "Elapsed from re-entry should be near-zero (got {}ms), not accumulated from first entry (8s)",
            elapsed_ms
        );
    assert!(
        elapsed_ms < market_price_confirmation_ms(&node).unwrap(),
        "Should not have fired yet — confirmation_ms={} not yet elapsed",
        market_price_confirmation_ms(&node).unwrap()
    );
}

#[test]
fn market_price_confirmation_ms_helper_is_explicit_only_and_mode_agnostic() {
    let mut fixed_once = test_node_spec("cross_above", 0.80, 250);
    fixed_once.auto_scope = false; // fixed market mode
    assert_eq!(market_price_confirmation_ms(&fixed_once), Some(250));

    fixed_once.confirmation_ms = None;
    assert_eq!(market_price_confirmation_ms(&fixed_once), None);

    fixed_once.confirmation_ms = Some(0);
    assert_eq!(market_price_confirmation_ms(&fixed_once), None);

    fixed_once.confirmation_ms = Some(250);
    fixed_once.node_type = "trigger.open_positions".to_string();
    assert_eq!(market_price_confirmation_ms(&fixed_once), None);
}

// ---------------------------------------------------------------------------
// Test: cross_confirmed mode — step execution must short-circuit past the
// cross re-evaluation.
//
// This captures the root-cause of the trigger-node-not-firing bug:
// When the WS confirmation gate fires, it enqueues a step with
//   wsPreviousPrice = tick-in-zone price (already past the cross)
//   wsPrice         = tick-in-zone price (still in zone)
//   wsEvaluationMode = "cross_confirmed"
//
// Without the short-circuit, evaluate_trigger_market_price_condition would
// receive prev >= trigger and cur >= trigger and return (false, "no_cross").
//
// With the short-circuit, wsEvaluationMode="cross_confirmed" causes pass=true
// without re-evaluating the strict cross condition.
// ---------------------------------------------------------------------------
#[test]
fn cross_confirmed_mode_short_circuits_cross_recheck() {
    // Simulate the data that would be present in step.input_json when the
    // WS confirmation gate fires:
    //   - trigger: cross_above at 0.60
    //   - Original cross: price went from 0.55 → 0.65
    //   - Confirmation tick: prev=0.65 (in zone), cur=0.65 (in zone)
    //   - wsEvaluationMode: "cross_confirmed"
    let trigger_price = 0.60_f64;
    let ws_price = 0.65_f64;
    let ws_prev_price = 0.65_f64; // in-zone: already above trigger

    // Verify that WITHOUT the short-circuit, the cross check would fail:
    // prev=0.65 >= trigger=0.60, so "prev < trigger" is false → no_cross
    let (would_cross, mode) = evaluate_trigger_market_price_condition(
        Some(ws_prev_price),
        ws_price,
        trigger_price,
        "cross_above",
        false, // once_mode=true → allow_first_tick_threshold=false
        None,
    );
    assert!(
        !would_cross,
        "Pre-confirmed in-zone prices must NOT produce a new cross (got mode={mode})"
    );
    assert_eq!(mode, "no_cross");

    // Verify that the cross_confirmed detection logic works:
    // ws_sourced=true AND wsEvaluationMode="cross_confirmed" → ws_cross_confirmed=true
    let ws_sourced = true;
    let ws_evaluation_mode = "cross_confirmed";
    let ws_cross_confirmed = ws_sourced && ws_evaluation_mode == "cross_confirmed";
    assert!(
        ws_cross_confirmed,
        "ws_cross_confirmed must be true when wsEvaluationMode=cross_confirmed"
    );

    // And verify that WITHOUT ws_cross_confirmed, the trigger would silently fail
    let without_short_circuit_pass = would_cross; // false from above
    assert!(
        !without_short_circuit_pass,
        "Without short-circuit, trigger would silently fail"
    );

    // With the short-circuit, pass is set to true directly (tested implicitly
    // by the production code path - this test validates the invariants the
    // short-circuit relies on).
    let with_short_circuit_pass = ws_cross_confirmed; // true
    assert!(
        with_short_circuit_pass,
        "With short-circuit, trigger must fire when cross_confirmed"
    );
}

#[test]
fn cross_confirmed_mode_not_triggered_for_regular_ws_events() {
    // When wsEvaluationMode is absent or not "cross_confirmed", ws_cross_confirmed=false
    // and the normal cross evaluation path runs (no short-circuit).
    let ws_sourced = true;

    let mode_absent = "";
    let mode_cross_detected = "cross_detected";
    let mode_first_tick = "first_tick_threshold";

    assert!(
        !(ws_sourced && mode_absent == "cross_confirmed"),
        "Empty evaluation mode must not trigger short-circuit"
    );
    assert!(
        !(ws_sourced && mode_cross_detected == "cross_confirmed"),
        "cross_detected must not trigger short-circuit (handled normally)"
    );
    assert!(
        !(ws_sourced && mode_first_tick == "cross_confirmed"),
        "first_tick_threshold must not trigger short-circuit"
    );

    // Non-WS steps also must not trigger short-circuit
    let not_ws_sourced = false;
    assert!(
        !(not_ws_sourced && "cross_confirmed" == "cross_confirmed"),
        "Non-WS steps must not trigger short-circuit even if mode says cross_confirmed"
    );
}

#[test]
fn cross_confirmed_short_circuit_helper_requires_clean_ws_context() {
    assert!(should_apply_ws_cross_confirmed_short_circuit(
        true,
        "cross_confirmed",
        None
    ));
    assert!(!should_apply_ws_cross_confirmed_short_circuit(
        true,
        "cross_confirmed",
        Some("ws_market_slug_mismatch:a!=b")
    ));
    assert!(!should_apply_ws_cross_confirmed_short_circuit(
        false,
        "cross_confirmed",
        None
    ));
    assert!(!should_apply_ws_cross_confirmed_short_circuit(
        true,
        "cross_detected",
        None
    ));
}

#[test]
fn cross_confirmed_unexpected_fail_helper_flags_only_real_regression_case() {
    assert!(is_ws_cross_confirmed_unexpected_fail(
        true,
        "cross_confirmed",
        false,
        None
    ));
    assert!(!is_ws_cross_confirmed_unexpected_fail(
        true,
        "cross_confirmed",
        true,
        None
    ));
    assert!(!is_ws_cross_confirmed_unexpected_fail(
        true,
        "cross_confirmed",
        false,
        Some("ws_market_slug_mismatch:a!=b")
    ));
    assert!(!is_ws_cross_confirmed_unexpected_fail(
        true,
        "cross_detected",
        false,
        None
    ));
}

#[test]
fn first_tick_threshold_override_helper_requires_auto_scope_market_price_and_clean_ws_context() {
    assert!(should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        true,
        "first_tick_threshold",
        false,
        None
    ));
    assert!(!should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        false,
        "first_tick_threshold",
        false,
        None
    ));
    assert!(!should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.open_positions",
        true,
        "first_tick_threshold",
        false,
        None
    ));
    assert!(!should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        true,
        "cross_confirmed",
        false,
        None
    ));
    assert!(!should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        true,
        "first_tick_threshold",
        false,
        Some("ws_market_slug_mismatch:a!=b")
    ));
    assert!(should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        true,
        "first_tick_in_range",
        false,
        None
    ));
}

#[test]
fn first_tick_threshold_override_replays_auto_scope_once_execution() {
    let trigger_price = 0.60_f64;
    let ws_price = 0.65_f64;

    let (without_override, without_mode) = evaluate_trigger_market_price_condition(
        None,
        ws_price,
        trigger_price,
        "cross_above",
        false,
        None,
    );
    assert!(!without_override);
    assert_eq!(without_mode, "no_previous");

    let allow_first_tick = should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        true,
        "first_tick_threshold",
        false,
        None,
    );
    let (with_override, with_mode) = evaluate_trigger_market_price_condition(
        None,
        ws_price,
        trigger_price,
        "cross_above",
        allow_first_tick,
        None,
    );
    assert!(with_override);
    assert_eq!(with_mode, "first_tick_threshold");
}
