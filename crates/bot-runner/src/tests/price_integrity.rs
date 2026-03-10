use super::*;

#[test]
fn prce01_no_legacy_fallback_for_previous_price() {
    // Build a context with only bare "previous_price" key, no per-token key.
    // The lookup must return None — not the legacy bare-key value.
    let node_key = "trigger_node_1";
    let token_id = "tok-yes";
    let prev_key = format!("previous_price_{}", token_id);
    let mut context = json!({
        "nodeStates": {
            node_key: {
                "previous_price": 0.55
            }
        }
    });
    // Per-token key absent — lookup must return None (PRCE-01 guarantee)
    let result = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
    assert!(
        result.is_none(),
        "Must not fall back to bare previous_price; got {:?}",
        result
    );

    // Now set per-token key — lookup must return its value
    set_flow_node_state(&mut context, node_key, &prev_key, json!(0.42));
    let result2 = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
    assert_eq!(
        result2,
        Some(0.42),
        "Per-token key must be returned when present"
    );
}

#[test]
fn prce01_per_token_key_works_independently() {
    // When flow node state has "previous_price_tok-yes" set to 0.42,
    // the per-token lookup returns Some(0.42).
    let node_key = "trigger_node_1";
    let token_id = "tok-yes";
    let prev_key = format!("previous_price_{}", token_id);
    let mut context = json!({
        "nodeStates": {
            node_key: {}
        }
    });
    set_flow_node_state(&mut context, node_key, &prev_key, json!(0.42));
    let result = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
    assert_eq!(
        result,
        Some(0.42),
        "Per-token key lookup must return stored value"
    );
}

#[test]
fn prce01_no_cross_token_contamination() {
    // Core PRCE-01 safety guarantee:
    // When flow node state has "previous_price_tok-A" = 0.60 but no "previous_price_tok-B",
    // looking up previous price for tok-B must return None (not 0.60).
    let node_key = "trigger_node_1";
    let token_a = "tok-A";
    let token_b = "tok-B";
    let key_a = format!("previous_price_{}", token_a);
    let key_b = format!("previous_price_{}", token_b);
    let mut context = json!({
        "nodeStates": {
            node_key: {}
        }
    });
    set_flow_node_state(&mut context, node_key, &key_a, json!(0.60));

    // tok-A lookup works
    let result_a = flow_node_state(&context, node_key, &key_a).and_then(value_as_f64);
    assert_eq!(result_a, Some(0.60), "tok-A lookup must return 0.60");

    // tok-B lookup must return None — no cross-token contamination
    let result_b = flow_node_state(&context, node_key, &key_b).and_then(value_as_f64);
    assert!(
        result_b.is_none(),
        "tok-B must not get tok-A's price; got {:?}",
        result_b
    );
}
