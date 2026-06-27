fn ensure_nested_object<'a>(
    context: &'a mut Value,
    key: &str,
) -> &'a mut serde_json::Map<String, Value> {
    let root = ensure_object_mut(context);
    if !root.get(key).map(Value::is_object).unwrap_or(false) {
        root.insert(key.to_string(), json!({}));
    }
    root.get_mut(key)
        .and_then(Value::as_object_mut)
        .expect("nested object should exist")
}

fn set_flow_state(context: &mut Value, key: &str, value: Value) {
    let state = ensure_nested_object(context, "state");
    if value.is_null() {
        state.remove(key);
    } else {
        state.insert(key.to_string(), value);
    }
}

fn flow_state_string(context: &Value, key: &str) -> Option<String> {
    context
        .get("state")
        .and_then(|state| state.get(key))
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn set_flow_var(context: &mut Value, key: &str, value: Value) {
    let vars = ensure_nested_object(context, "vars");
    vars.insert(key.to_string(), value);
}

fn set_flow_context(context: &mut Value, key: &str, value: Value) {
    let flow_context = ensure_nested_object(context, "flowContext");
    if value.is_null() {
        flow_context.remove(key);
    } else {
        flow_context.insert(key.to_string(), value);
    }
}

fn flow_context_value(context: &Value, key: &str) -> Option<Value> {
    context.get("flowContext").and_then(|v| v.get(key)).cloned()
}

fn set_flow_ref(context: &mut Value, key: &str, value: Value) {
    let refs = ensure_nested_object(context, "refs");
    if value.is_null() {
        refs.remove(key);
    } else {
        refs.insert(key.to_string(), value);
    }
}

fn set_flow_node_state(context: &mut Value, node_key: &str, state_key: &str, value: Value) {
    let node_state = ensure_nested_object(context, "nodeState");
    if !node_state
        .get(node_key)
        .map(Value::is_object)
        .unwrap_or(false)
    {
        node_state.insert(node_key.to_string(), json!({}));
    }
    if let Some(state_obj) = node_state.get_mut(node_key).and_then(Value::as_object_mut) {
        state_obj.insert(state_key.to_string(), value);
    }
}

fn remove_flow_node_state(context: &mut Value, node_key: &str, state_key: &str) {
    let Some(node_state) = context.get_mut("nodeState").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(node) = node_state.get_mut(node_key).and_then(Value::as_object_mut) else {
        return;
    };
    node.remove(state_key);
}

fn flow_node_state<'a>(context: &'a Value, node_key: &str, state_key: &str) -> Option<&'a Value> {
    context
        .get("nodeState")
        .and_then(|node_state| node_state.get(node_key))
        .and_then(|node| node.get(state_key))
}

fn flow_node_state_string(context: &Value, node_key: &str, state_key: &str) -> Option<String> {
    flow_node_state(context, node_key, state_key)
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn flow_node_state_truthy(context: &Value, node_key: &str, state_key: &str) -> bool {
    flow_node_state(context, node_key, state_key)
        .map(value_truthy)
        .unwrap_or(false)
}

fn flow_node_state_i64(context: &Value, node_key: &str, state_key: &str) -> Option<i64> {
    flow_node_state(context, node_key, state_key).and_then(Value::as_i64)
}

const FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG: &str = "auto_scope_market_slug";
const FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE: &str = "auto_scope_market_scope";
const FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET: &str = "auto_scope_market_asset";
const FLOW_NODE_STATE_AUTO_SCOPE_MARKET_TIMEFRAME: &str = "auto_scope_market_timeframe";
const FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID: &str = "auto_scope_yes_token_id";
const FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID: &str = "auto_scope_no_token_id";
const FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID: &str = "auto_scope_resolved_token_id";
const FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL: &str = "auto_scope_resolved_outcome_label";
const FLOW_NODE_STATE_AUTO_SCOPE_SELECTION_REASON: &str = "auto_scope_selection_reason";

fn set_flow_node_state_optional_string(
    context: &mut Value,
    node_key: &str,
    state_key: &str,
    value: Option<&str>,
) {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => set_flow_node_state(context, node_key, state_key, json!(value)),
        None => remove_flow_node_state(context, node_key, state_key),
    }
}

fn node_auto_scope_state_string(
    context: &Value,
    node_key: &str,
    state_key: &str,
) -> Option<String> {
    flow_node_state_string(context, node_key, state_key)
}

fn node_auto_scope_market_slug(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG)
        .or_else(|| flow_context_string(context, "marketSlug"))
}

fn node_auto_scope_market_scope(node: &TradeFlowNode, context: &Value) -> Option<String> {
    node_auto_scope_state_string(context, &node.key, FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE)
        .or_else(|| node_config_string(node, "marketScope"))
        .or_else(|| flow_context_string(context, "marketScope"))
}

fn node_auto_scope_market_asset(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET)
        .or_else(|| flow_context_string(context, "marketAsset"))
}

fn node_auto_scope_market_timeframe(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_TIMEFRAME,
    )
    .or_else(|| flow_context_string(context, "marketTimeframe"))
}

fn node_auto_scope_yes_token_id(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID)
        .or_else(|| flow_context_string(context, "yesTokenId"))
}

fn node_auto_scope_no_token_id(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID)
        .or_else(|| flow_context_string(context, "noTokenId"))
}

fn node_auto_scope_resolved_token_id(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
    )
    .or_else(|| flow_context_string(context, "tokenId"))
}

fn node_auto_scope_resolved_outcome_label(context: &Value, node_key: &str) -> Option<String> {
    node_auto_scope_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
    )
    .or_else(|| flow_context_string(context, "outcomeLabel"))
}

fn set_trigger_node_auto_scope_context(
    context: &mut Value,
    node_key: &str,
    market_scope: &str,
    market_asset: &str,
    market_timeframe: &str,
    selected: &SelectedLiveMarket,
    preferred_outcome: Option<&str>,
) {
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
        Some(selected.slug.as_str()),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE,
        Some(market_scope),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET,
        Some(market_asset),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_TIMEFRAME,
        Some(market_timeframe),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID,
        selected.yes_token_id.as_deref(),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID,
        selected.no_token_id.as_deref(),
    );
    set_flow_node_state_optional_string(
        context,
        node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_SELECTION_REASON,
        Some(selected.selection_reason.as_str()),
    );

    match preferred_outcome.and_then(normalized_binary_outcome_label) {
        Some("yes") => {
            set_flow_node_state_optional_string(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
                selected.yes_token_id.as_deref(),
            );
            set_flow_node_state_optional_string(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
                Some("Yes"),
            );
        }
        Some("no") => {
            set_flow_node_state_optional_string(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
                selected.no_token_id.as_deref(),
            );
            set_flow_node_state_optional_string(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
                Some("No"),
            );
        }
        _ => {
            remove_flow_node_state(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
            );
            remove_flow_node_state(
                context,
                node_key,
                FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
            );
        }
    }
}

fn clear_trigger_node_auto_scope_context(context: &mut Value, node_key: &str) {
    for state_key in [
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_TIMEFRAME,
        FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
        FLOW_NODE_STATE_AUTO_SCOPE_SELECTION_REASON,
    ] {
        remove_flow_node_state(context, node_key, state_key);
    }
}

fn promote_trigger_node_auto_scope_context_to_flow_context(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) {
    set_flow_context(context, "marketSlug", json!(market_slug));
    if let Some(market_scope) =
        node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE)
    {
        set_flow_context(context, "marketScope", json!(market_scope));
    }
    if let Some(market_asset) =
        node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET)
    {
        set_flow_context(context, "marketAsset", json!(market_asset));
    }
    if let Some(market_timeframe) = node_auto_scope_market_timeframe(context, node_key) {
        set_flow_context(context, "marketTimeframe", json!(market_timeframe));
    }
    if let Some(yes_token_id) =
        node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID)
    {
        set_flow_context(context, "yesTokenId", json!(yes_token_id));
    }
    if let Some(no_token_id) =
        node_auto_scope_state_string(context, node_key, FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID)
    {
        set_flow_context(context, "noTokenId", json!(no_token_id));
    }
}

fn flow_node_reentry_generation(context: &Value, node_key: &str) -> i64 {
    flow_node_state_i64(context, node_key, FLOW_NODE_STATE_REENTRY_GENERATION).unwrap_or(0)
}

fn flow_node_reentry_attempts_used(context: &Value, node_key: &str) -> i64 {
    flow_node_state_i64(context, node_key, FLOW_NODE_STATE_REENTRY_ATTEMPTS_USED).unwrap_or(0)
}

fn flow_node_reentry_market_slug(context: &Value, node_key: &str) -> Option<String> {
    flow_node_state_string(context, node_key, FLOW_NODE_STATE_REENTRY_MARKET_SLUG)
        .map(|slug| slug.trim().to_string())
        .filter(|slug| !slug.is_empty())
}

fn resolve_trade_flow_reentry_attempts_for_market(
    context: &Value,
    node_key: &str,
    market_slug: &str,
) -> (i64, Option<String>) {
    let stored_reentry_market_slug = flow_node_reentry_market_slug(context, node_key);
    let attempts_used = if stored_reentry_market_slug.as_deref() == Some(market_slug) {
        flow_node_reentry_attempts_used(context, node_key)
    } else {
        0
    };
    (attempts_used, stored_reentry_market_slug)
}

fn trade_flow_market_price_once_idempotency_key(
    run_id: i64,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
    generation: i64,
) -> String {
    let base = if once_scope_market {
        let market_scope = market_slug
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("unknown-market");
        format!("flow-once-fired:{run_id}:{node_key}:{market_scope}")
    } else {
        format!("flow-once-fired:{run_id}:{node_key}")
    };
    if generation > 0 {
        format!("{base}:gen{generation}")
    } else {
        base
    }
}

fn trade_flow_market_price_once_fired_for_scope(
    context: &Value,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
) -> bool {
    let current_market_slug = market_slug.map(str::trim).filter(|v| !v.is_empty());
    if let (Some(current_market_slug), Some(locked_market_slug)) = (
        current_market_slug,
        flow_node_state_string(
            context,
            node_key,
            FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
        ),
    ) {
        if locked_market_slug == current_market_slug {
            return true;
        }
    }
    if !flow_node_state_truthy(context, node_key, FLOW_NODE_STATE_ONCE_FIRED) {
        return false;
    }
    if !once_scope_market {
        return true;
    }
    let Some(current_market_slug) = current_market_slug else {
        return false;
    };
    flow_node_state_string(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG)
        .map(|fired_market_slug| fired_market_slug == current_market_slug)
        .unwrap_or(false)
}

fn sync_trade_flow_market_price_once_scope_state(
    context: &mut Value,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
) {
    let current_market_slug = market_slug.map(str::trim).filter(|v| !v.is_empty());
    if let Some(locked_market_slug) = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
    ) {
        match current_market_slug {
            Some(current_market_slug) if current_market_slug != locked_market_slug => {
                remove_flow_node_state(
                    context,
                    node_key,
                    FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
                );
            }
            None => {}
            _ => {}
        }
    }
    if !once_scope_market {
        return;
    }
    let Some(current_market_slug) = current_market_slug else {
        return;
    };
    let Some(last_fired_market_slug) =
        flow_node_state_string(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG)
    else {
        if flow_node_state_truthy(context, node_key, FLOW_NODE_STATE_ONCE_FIRED) {
            clear_trade_flow_market_price_once_state(context, node_key);
        }
        return;
    };
    if last_fired_market_slug != current_market_slug {
        clear_trade_flow_market_price_once_state(context, node_key);
    }
}

fn mark_trade_flow_market_price_once_fired(
    context: &mut Value,
    node_key: &str,
    fired_at: DateTime<Utc>,
    market_slug: Option<&str>,
) {
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
    );
    set_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED, json!(true));
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_ONCE_FIRED_AT,
        json!(fired_at.to_rfc3339()),
    );
    if let Some(slug) = market_slug.map(str::trim).filter(|v| !v.is_empty()) {
        set_flow_node_state(
            context,
            node_key,
            FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
            json!(slug),
        );
    } else {
        remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG);
    }
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
        json!(false),
    );
}

fn clear_trade_flow_market_price_once_state(context: &mut Value, node_key: &str) {
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_AT);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED);
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
    );
}

fn clear_trade_flow_market_price_rotation_state(context: &mut Value, node_key: &str) {
    clear_trade_flow_market_price_once_state(context, node_key);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_REENTRY_GENERATION);
}

fn build_trade_flow_eval_data(context: &Value) -> Value {
    let mut root = serde_json::Map::new();
    for section in ["flowContext", "state", "vars"] {
        if let Some(obj) = context.get(section).and_then(Value::as_object) {
            for (key, value) in obj {
                root.insert(key.clone(), value.clone());
            }
        }
    }
    for section in ["flowContext", "state", "vars", "refs", "nodeState"] {
        if let Some(value) = context.get(section) {
            root.insert(section.to_string(), value.clone());
        }
    }
    Value::Object(root)
}

fn build_trade_flow_route_eval_data(context: &Value, output: &Value) -> Value {
    let mut eval_data = build_trade_flow_eval_data(context);
    let Some(root) = eval_data.as_object_mut() else {
        return eval_data;
    };
    root.insert("output".to_string(), output.clone());
    if let Some(output_obj) = output.as_object() {
        for (key, value) in output_obj {
            root.insert(key.clone(), value.clone());
        }
    }
    eval_data
}

fn evaluate_jsonlogic(expression: &Value, data: &Value) -> Value {
    if let Some(object) = expression.as_object() {
        if object.len() != 1 {
            return Value::Null;
        }
        let (operator, args) = object.iter().next().expect("single entry object");
        return match operator.as_str() {
            "var" => resolve_jsonlogic_var(args, data),
            "==" | "!=" => {
                let values = evaluate_jsonlogic_args(args, data);
                if values.len() < 2 {
                    return Value::Bool(false);
                }
                let eq = values_equal(&values[0], &values[1]);
                Value::Bool(if operator == "==" { eq } else { !eq })
            }
            ">" | ">=" | "<" | "<=" => {
                let values = evaluate_jsonlogic_args(args, data);
                if values.len() < 2 {
                    return Value::Bool(false);
                }
                let Some(left) = value_as_f64(&values[0]) else {
                    return Value::Bool(false);
                };
                let Some(right) = value_as_f64(&values[1]) else {
                    return Value::Bool(false);
                };
                let result = match operator.as_str() {
                    ">" => left > right,
                    ">=" => left >= right,
                    "<" => left < right,
                    "<=" => left <= right,
                    _ => false,
                };
                Value::Bool(result)
            }
            "and" => {
                let values = evaluate_jsonlogic_args(args, data);
                Value::Bool(values.iter().all(value_truthy))
            }
            "or" => {
                let values = evaluate_jsonlogic_args(args, data);
                Value::Bool(values.iter().any(value_truthy))
            }
            "!" => {
                let values = evaluate_jsonlogic_args(args, data);
                let value = values.first().cloned().unwrap_or(Value::Bool(false));
                Value::Bool(!value_truthy(&value))
            }
            "+" | "-" | "*" | "/" => {
                let values = evaluate_jsonlogic_args(args, data);
                let numeric_values = values.iter().filter_map(value_as_f64).collect::<Vec<_>>();
                if numeric_values.is_empty() {
                    return Value::Null;
                }
                let computed = match operator.as_str() {
                    "+" => numeric_values.iter().sum::<f64>(),
                    "-" => {
                        if numeric_values.len() == 1 {
                            -numeric_values[0]
                        } else {
                            numeric_values[0] - numeric_values[1..].iter().sum::<f64>()
                        }
                    }
                    "*" => numeric_values.iter().product::<f64>(),
                    "/" => {
                        if numeric_values.len() < 2 || numeric_values[1] == 0.0 {
                            return Value::Null;
                        }
                        numeric_values[0] / numeric_values[1]
                    }
                    _ => return Value::Null,
                };
                serde_json::Number::from_f64(computed)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            "if" => {
                let values = evaluate_jsonlogic_args(args, data);
                let mut idx = 0usize;
                while idx + 1 < values.len() {
                    if value_truthy(&values[idx]) {
                        return values[idx + 1].clone();
                    }
                    idx += 2;
                }
                if values.len() % 2 == 1 {
                    values.last().cloned().unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        };
    }

    if let Some(array) = expression.as_array() {
        return Value::Array(
            array
                .iter()
                .map(|item| evaluate_jsonlogic(item, data))
                .collect(),
        );
    }

    expression.clone()
}

fn evaluate_jsonlogic_args(args: &Value, data: &Value) -> Vec<Value> {
    if let Some(array) = args.as_array() {
        array
            .iter()
            .map(|value| evaluate_jsonlogic(value, data))
            .collect()
    } else {
        vec![evaluate_jsonlogic(args, data)]
    }
}

fn resolve_jsonlogic_var(args: &Value, data: &Value) -> Value {
    if let Some(path) = args.as_str() {
        return lookup_jsonlogic_path(data, path).unwrap_or(Value::Null);
    }
    if let Some(list) = args.as_array() {
        let path = list.first().and_then(Value::as_str).unwrap_or_default();
        let fallback = list.get(1).cloned().unwrap_or(Value::Null);
        return lookup_jsonlogic_path(data, path).unwrap_or(fallback);
    }
    Value::Null
}

fn lookup_jsonlogic_path(data: &Value, path: &str) -> Option<Value> {
    if path.is_empty() {
        return Some(data.clone());
    }
    if let Some(value) = lookup_json_path(data, path) {
        return Some(value.clone());
    }

    if !path.contains('.') {
        for section in ["vars", "state", "flowContext", "refs"] {
            if let Some(value) = data.get(section).and_then(|v| v.get(path)) {
                return Some(value.clone());
            }
        }
    }

    None
}

fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        let key = part.trim();
        if key.is_empty() {
            continue;
        }
        current = current.get(key)?;
    }
    Some(current)
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(v) => {
            let normalized = v.trim().to_lowercase();
            !normalized.is_empty() && normalized != "false" && normalized != "0"
        }
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(v) => v.as_f64(),
        Value::String(v) => v.parse::<f64>().ok(),
        Value::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn resolve_ws_previous_price(
    ws_sourced: bool,
    state_previous_price: Option<f64>,
    token_id: &str,
    ws_token_id_from_step: Option<&str>,
    ws_previous_price_from_step: Option<f64>,
    ws_previous_price_present: bool,
    ws_previous_prices_map: Option<&serde_json::Map<String, Value>>,
) -> Option<f64> {
    if !ws_sourced {
        return state_previous_price;
    }

    let token_id = token_id.trim();
    if !token_id.is_empty() {
        if let Some(map) = ws_previous_prices_map {
            if let Some(raw_value) = map.get(token_id) {
                // Explicit null means "no previous price"; do not fallback to context state.
                return value_as_f64(raw_value).map(clamp_probability);
            }
        }
    }

    let ws_token_matches = ws_token_id_from_step
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|step_token_id| token_id.is_empty() || step_token_id == token_id)
        .unwrap_or(token_id.is_empty());
    if ws_token_matches && ws_previous_price_present {
        // Explicit key presence (including null) should override state fallback.
        return ws_previous_price_from_step;
    }

    state_previous_price
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(v) => v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)),
        Value::String(v) => v.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_as_i64_strict(value: &Value) -> Option<i64> {
    match value {
        Value::Number(v) => v.as_i64(),
        Value::String(v) => v.parse::<i64>().ok(),
        _ => None,
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    if let (Some(left_num), Some(right_num)) = (value_as_f64(left), value_as_f64(right)) {
        return (left_num - right_num).abs() <= 0.0000001;
    }
    left == right
}

async fn process_trade_builder_orders(
    repo: &PostgresRepository,
    run_id: i64,
    _cfg: &AppConfig,
    _client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    builder_timing: &mut crate::trade_builder_order_housekeeping_timing::TradeBuilderOrderHousekeepingTimingStats,
) -> Result<()> {
    use crate::trade_builder_inventory_observation_due_gate::{
        filter_due_inventory_observations, record_inventory_observation_due_result,
        InventoryObservationDueResult,
    };
    use crate::trade_builder_inventory_observation_terminal::{
        stale_not_visible_terminal_reason, zero_fill_terminal_reason,
    };
    use crate::trade_builder_inventory_observation_timing::{
        InventoryObservationCacheKey, InventoryObservationPhase, InventoryObservationReadResult,
        InventoryObservationRecordTiming, InventoryObservationTimer,
        InventoryPositionsSnapshotCacheEntry,
    };
    use crate::trade_builder_order_housekeeping_timing::{
        measure_trade_builder_orders_phase, millis_u64, TradeBuilderOrdersPhase,
    };

    let orders_result = measure_trade_builder_orders_phase(
        builder_timing,
        TradeBuilderOrdersPhase::LoadOrders,
        repo.list_trade_builder_orders_for_processing(MANUAL_ORDER_PROCESS_LIMIT),
    )
    .await;
    let orders = orders_result?;
    builder_timing.loaded_count = orders.len() as u64;
    let orders_empty = orders.is_empty();
    let pending_inventory_observations_result = measure_trade_builder_orders_phase(
        builder_timing,
        TradeBuilderOrdersPhase::LoadPendingInventory,
        repo.list_pending_trade_builder_first_visible_inventory_observations(
            TRADE_BUILDER_INVENTORY_OBSERVATION_LIMIT,
        ),
    )
    .await;
    let pending_inventory_observations = pending_inventory_observations_result?;
    builder_timing.pending_inventory_count = pending_inventory_observations.len() as u64;
    let pending_inventory_observations_empty = pending_inventory_observations.is_empty();

    let policy = DefaultRiskPolicy;
    let mut user_cfg_cache: HashMap<i64, AppConfig> = HashMap::new();
    let mut user_executor_cache: HashMap<i64, SharedOrderExecutor> = HashMap::new();
    let mut user_gamma_cache: HashMap<i64, GammaHttpClient> = HashMap::new();
    let mut synced_user_ids: HashSet<i64> = HashSet::new();
    let mut final_fill_sync_required_user_ids: HashSet<i64> = HashSet::new();
    let mut initial_fill_sync_throttled_user_ids: HashSet<i64> = HashSet::new();

    let process_loop_started = Instant::now();
    for order in orders {
        let order_eval_started = Instant::now();
        let result: Result<()> = async {
            let user_cfg =
                load_user_app_config_cached(repo, order.user_id, &mut user_cfg_cache).await?;
            let gamma = user_gamma_cache
                .entry(order.user_id)
                .or_insert_with(|| GammaHttpClient::new(user_cfg.exchange.gamma_base_url.clone()))
                .clone();
            let client = load_user_order_executor_cached(
                repo,
                order.user_id,
                &mut user_cfg_cache,
                &mut user_executor_cache,
            )
            .await?;
            if synced_user_ids.insert(order.user_id) {
                builder_timing.fill_sync_user_count =
                    builder_timing.fill_sync_user_count.saturating_add(1);
                builder_timing.fill_sync_call_count =
                    builder_timing.fill_sync_call_count.saturating_add(1);
                let sync_result = sync_recent_trade_builder_fills(repo, client.as_ref()).await;
                if sync_result.is_err() {
                    builder_timing.fill_sync_error_count =
                        builder_timing.fill_sync_error_count.saturating_add(1);
                    final_fill_sync_required_user_ids.insert(order.user_id);
                }
                sync_result?;
            }
            final_fill_sync_required_user_ids.insert(order.user_id);
            let limits = to_risk_limits(&user_cfg);
            let processed = try_process_trade_builder_order(
                repo,
                run_id,
                &user_cfg,
                &limits,
                &policy,
                client.as_ref(),
                &gamma,
                ws,
                &order,
            )
            .await?;
            if processed {
                builder_timing.processed_count = builder_timing.processed_count.saturating_add(1);
            }
            Ok(())
        }
        .await;
        builder_timing.record_order_eval(
            millis_u64(order_eval_started.elapsed()),
            order.id,
            &order.market_slug,
            &order.status,
        );
        if let Err(err) = result {
            builder_timing.processing_error_count =
                builder_timing.processing_error_count.saturating_add(1);
            let err_text = format!("{err:#}");
            let latest_order = repo.get_trade_builder_order(order.id).await.ok().flatten();
            if latest_order
                .as_ref()
                .is_some_and(trade_builder_should_retry_after_processing_error)
            {
                let _ = repo
                    .set_trade_builder_order_status(order.id, "triggered", Some(&err_text))
                    .await;
                let _ = repo
                    .append_trade_builder_order_event(
                        order.id,
                        "processing_retry_scheduled",
                        &json!({ "error": err_text }),
                    )
                    .await;
            } else {
                let _ = repo
                    .set_trade_builder_order_status(order.id, "error", Some(&err_text))
                    .await;
                let _ = repo
                    .append_trade_builder_order_event(
                        order.id,
                        "processing_error",
                        &json!({ "error": err_text }),
                    )
                    .await;
                if let Some(ref latest) = latest_order {
                    let _ = maybe_send_order_not_filled_notification(
                        repo,
                        latest,
                        "processing_error",
                        &err_text,
                    )
                    .await;
                }
            }
            warn!(
                run_id,
                builder_order_id = order.id,
                error = %err_text,
                "TRADE_BUILDER_ORDER_ERROR"
            );
        }
        if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
            if let Err(e) =
                refresh_trade_flow_ws_fast_path_for_boundary(repo, run_id, ws, &mut user_cfg_cache)
                    .await
            {
                warn!(run_id, error = %e, "TRADE_FLOW_BOUNDARY_REFRESH_FAILED");
            }
        }
    }
    builder_timing.set_phase_ms(
        TradeBuilderOrdersPhase::ProcessLoop,
        millis_u64(process_loop_started.elapsed()),
    );

    let pending_inventory_observation_count = pending_inventory_observations.len();
    let (pending_inventory_observations, due_gate_snapshot) =
        filter_due_inventory_observations(pending_inventory_observations, Instant::now());
    builder_timing
        .inventory_observation
        .record_due_gate_snapshot(due_gate_snapshot);
    if pending_inventory_observation_count > 0 && pending_inventory_observations.is_empty() {
        builder_timing
            .inventory_observation
            .record_initial_fill_sync_skipped_no_due();
    }

    let inventory_loop_started = Instant::now();
    let mut inventory_observation_cache: HashMap<
        InventoryObservationCacheKey,
        InventoryObservationReadResult,
    > = HashMap::new();
    let mut inventory_positions_snapshot_cache: HashMap<i64, InventoryPositionsSnapshotCacheEntry> =
        HashMap::new();
    for observation in pending_inventory_observations {
        builder_timing
            .inventory_observation
            .record_attempt(observation.user_id, &observation.token_id);
        let observation_timer = InventoryObservationTimer::start();
        let mut record_timing = InventoryObservationRecordTiming::default();
        let result: Result<InventoryObservationDueResult> = async {
            if let Some(reason) = zero_fill_terminal_reason(&observation) {
                return record_trade_builder_terminal_not_visible_inventory_result(
                    repo,
                    &observation,
                    reason,
                    None,
                    &mut builder_timing.inventory_observation,
                    &mut record_timing,
                )
                .await;
            }

            let config_timer = InventoryObservationTimer::start();
            let _user_cfg =
                load_user_app_config_cached(repo, observation.user_id, &mut user_cfg_cache).await?;
            let config_lookup_ms = config_timer.elapsed_ms();
            builder_timing
                .inventory_observation
                .record_config_lookup(config_lookup_ms);
            record_timing.add_phase_ms(InventoryObservationPhase::ConfigLookup, config_lookup_ms);

            let executor_timer = InventoryObservationTimer::start();
            let client = load_user_order_executor_cached(
                repo,
                observation.user_id,
                &mut user_cfg_cache,
                &mut user_executor_cache,
            )
            .await?;
            let executor_lookup_ms = executor_timer.elapsed_ms();
            builder_timing
                .inventory_observation
                .record_executor_lookup(executor_lookup_ms);
            record_timing.add_phase_ms(
                InventoryObservationPhase::ExecutorLookup,
                executor_lookup_ms,
            );

            maybe_sync_trade_builder_inventory_initial_fills(
                repo,
                client.as_ref(),
                observation.user_id,
                builder_timing,
                &mut record_timing,
                &mut synced_user_ids,
                &mut final_fill_sync_required_user_ids,
                &mut initial_fill_sync_throttled_user_ids,
            )
            .await?;
            let read_result = if let Some(cache_key) =
                InventoryObservationCacheKey::new(observation.user_id, &observation.token_id)
            {
                let token_cache_timer = InventoryObservationTimer::start();
                let cached_token_result = inventory_observation_cache.get(&cache_key).copied();
                let token_cache_ms = token_cache_timer.elapsed_ms();
                builder_timing
                    .inventory_observation
                    .record_token_result_cache_ms(token_cache_ms);
                record_timing
                    .add_phase_ms(InventoryObservationPhase::TokenResultCache, token_cache_ms);

                if let Some(cached_result) = cached_token_result {
                    builder_timing
                        .inventory_observation
                        .record_cache_hit(&cache_key);
                    if matches!(cached_result, InventoryObservationReadResult::ReadError) {
                        builder_timing.inventory_observation.record_cached_error();
                    }
                    cached_result
                } else {
                    builder_timing
                        .inventory_observation
                        .record_cache_miss(&cache_key);
                    let snapshot_cache_timer = InventoryObservationTimer::start();
                    let cached_snapshot = inventory_positions_snapshot_cache
                        .get(&observation.user_id)
                        .cloned();
                    let snapshot_cache_ms = snapshot_cache_timer.elapsed_ms();
                    builder_timing
                        .inventory_observation
                        .record_snapshot_cache_ms(snapshot_cache_ms);
                    record_timing
                        .add_phase_ms(InventoryObservationPhase::SnapshotCache, snapshot_cache_ms);

                    let snapshot_entry = if let Some(cached_snapshot) = cached_snapshot {
                        builder_timing
                            .inventory_observation
                            .record_positions_snapshot_record_hit();
                        if matches!(
                            cached_snapshot,
                            InventoryPositionsSnapshotCacheEntry::ReadError
                        ) {
                            builder_timing
                                .inventory_observation
                                .record_positions_snapshot_cached_error();
                            builder_timing.inventory_observation.record_cached_error();
                        }
                        cached_snapshot
                    } else {
                        builder_timing
                            .inventory_observation
                            .record_positions_snapshot_record_miss();
                        let snapshot_timer = InventoryObservationTimer::start();
                        let snapshot_result = client.available_token_inventory_snapshot().await;
                        let snapshot_elapsed_ms = snapshot_timer.elapsed_ms();
                        record_timing.add_phase_ms(
                            InventoryObservationPhase::ExternalLookup,
                            snapshot_elapsed_ms,
                        );
                        let snapshot_entry = match snapshot_result {
                            Ok(Some(snapshot)) => {
                                builder_timing
                                    .inventory_observation
                                    .record_positions_snapshot_fetch(
                                        snapshot_elapsed_ms,
                                        snapshot.row_count(),
                                        snapshot.alias_count(),
                                    );
                                InventoryPositionsSnapshotCacheEntry::Snapshot(snapshot)
                            }
                            Ok(None) => {
                                builder_timing
                                    .inventory_observation
                                    .record_positions_snapshot_unsupported();
                                InventoryPositionsSnapshotCacheEntry::Unsupported
                            }
                            Err(err) => {
                                builder_timing
                                    .inventory_observation
                                    .record_positions_snapshot_fetch(snapshot_elapsed_ms, 0, 0);
                                builder_timing
                                    .inventory_observation
                                    .record_positions_snapshot_error();
                                builder_timing.inventory_observation.record_external_error();
                                warn!(
                                    run_id,
                                    builder_order_id = observation.parent_builder_order_id,
                                    token_id = %observation.token_id,
                                    error = %err,
                                    "TRADE_BUILDER_POSITIONS_SNAPSHOT_READ_FAILED"
                                );
                                InventoryPositionsSnapshotCacheEntry::ReadError
                            }
                        };
                        let snapshot_cache_insert_timer = InventoryObservationTimer::start();
                        inventory_positions_snapshot_cache
                            .insert(observation.user_id, snapshot_entry.clone());
                        let snapshot_cache_insert_ms = snapshot_cache_insert_timer.elapsed_ms();
                        builder_timing
                            .inventory_observation
                            .record_snapshot_cache_ms(snapshot_cache_insert_ms);
                        record_timing.add_phase_ms(
                            InventoryObservationPhase::SnapshotCache,
                            snapshot_cache_insert_ms,
                        );
                        snapshot_entry
                    };

                    let read_result = match snapshot_entry {
                        InventoryPositionsSnapshotCacheEntry::Snapshot(snapshot) => {
                            let token_lookup_timer = InventoryObservationTimer::start();
                            let read_result = trade_builder_first_visible_inventory_read_result(
                                snapshot.token_qty(&observation.token_id),
                            );
                            let token_lookup_ms = token_lookup_timer.elapsed_ms();
                            builder_timing
                                .inventory_observation
                                .record_token_lookup(token_lookup_ms, read_result);
                            record_timing.add_phase_ms(
                                InventoryObservationPhase::ExternalLookup,
                                token_lookup_ms,
                            );
                            read_result
                        }
                        InventoryPositionsSnapshotCacheEntry::Unsupported => {
                            let fallback_timer = InventoryObservationTimer::start();
                            let read_result = read_trade_builder_first_visible_inventory(
                                client.as_ref(),
                                &observation,
                            )
                            .await;
                            let fallback_ms = fallback_timer.elapsed_ms();
                            builder_timing
                                .inventory_observation
                                .record_fallback_available_token_qty_ms(fallback_ms);
                            record_timing.add_phase_ms(
                                InventoryObservationPhase::ExternalLookup,
                                fallback_ms,
                            );
                            match read_result {
                                Ok(InventoryObservationReadResult::ReadError) => {
                                    builder_timing.inventory_observation.record_external_error();
                                    InventoryObservationReadResult::ReadError
                                }
                                Ok(read_result) => read_result,
                                Err(err) => {
                                    builder_timing.inventory_observation.record_external_error();
                                    warn!(
                                        run_id,
                                        builder_order_id = observation.parent_builder_order_id,
                                        token_id = %observation.token_id,
                                        error = %err,
                                        "TRADE_BUILDER_FIRST_VISIBLE_INVENTORY_READ_FAILED"
                                    );
                                    InventoryObservationReadResult::ReadError
                                }
                            }
                        }
                        InventoryPositionsSnapshotCacheEntry::ReadError => {
                            InventoryObservationReadResult::ReadError
                        }
                    };

                    let token_cache_insert_timer = InventoryObservationTimer::start();
                    inventory_observation_cache.insert(cache_key, read_result);
                    let token_cache_insert_ms = token_cache_insert_timer.elapsed_ms();
                    builder_timing
                        .inventory_observation
                        .record_token_result_cache_ms(token_cache_insert_ms);
                    record_timing.add_phase_ms(
                        InventoryObservationPhase::TokenResultCache,
                        token_cache_insert_ms,
                    );
                    read_result
                }
            } else {
                builder_timing.inventory_observation.record_uncacheable();
                InventoryObservationReadResult::NotVisible
            };

            match read_result {
                InventoryObservationReadResult::Visible { qty } => {
                    let apply_timer = InventoryObservationTimer::start();
                    let apply_result = apply_trade_builder_first_visible_inventory(
                        repo,
                        &observation,
                        qty,
                        &mut builder_timing.inventory_observation,
                    )
                    .await;
                    let apply_total_ms = apply_timer.elapsed_ms();
                    builder_timing
                        .inventory_observation
                        .record_apply_total_ms(apply_total_ms);
                    record_timing
                        .add_phase_ms(InventoryObservationPhase::ApplyTotal, apply_total_ms);
                    match apply_result? {
                        TradeBuilderFirstVisibleInventoryApplyOutcome::Applied => {
                            builder_timing.inventory_observation.record_success();
                            Ok(InventoryObservationDueResult::Visible)
                        }
                        TradeBuilderFirstVisibleInventoryApplyOutcome::NotVisible => {
                            builder_timing.inventory_observation.record_not_visible();
                            Ok(InventoryObservationDueResult::NotVisible)
                        }
                    }
                }
                InventoryObservationReadResult::NotVisible => {
                    if let Some(reason) =
                        stale_not_visible_terminal_reason(&observation, Utc::now())
                    {
                        record_trade_builder_terminal_not_visible_inventory_result(
                            repo,
                            &observation,
                            reason,
                            Some(0.0),
                            &mut builder_timing.inventory_observation,
                            &mut record_timing,
                        )
                        .await?;
                    } else {
                        builder_timing.inventory_observation.record_not_visible();
                    }
                    Ok(InventoryObservationDueResult::NotVisible)
                }
                InventoryObservationReadResult::ReadError => {
                    Ok(InventoryObservationDueResult::ReadError)
                }
            }
        }
        .await;
        let record_finalize_timer = InventoryObservationTimer::start();
        let due_result = match result {
            Ok(due_result) => {
                builder_timing.inventory_observed_count =
                    builder_timing.inventory_observed_count.saturating_add(1);
                due_result
            }
            Err(err) => {
                builder_timing.inventory_error_count =
                    builder_timing.inventory_error_count.saturating_add(1);
                warn!(
                    run_id,
                    builder_order_id = observation.parent_builder_order_id,
                    error = %err,
                    "TRADE_BUILDER_FIRST_VISIBLE_INVENTORY_OBSERVATION_FAILED"
                );
                InventoryObservationDueResult::ReadError
            }
        };
        let due_gate_update =
            record_inventory_observation_due_result(&observation, due_result, Instant::now());
        builder_timing
            .inventory_observation
            .record_due_gate_update(due_gate_update);
        let record_finalize_ms = record_finalize_timer.elapsed_ms();
        builder_timing
            .inventory_observation
            .record_record_finalize_ms(record_finalize_ms);
        record_timing.add_phase_ms(
            InventoryObservationPhase::RecordFinalize,
            record_finalize_ms,
        );
        let record_elapsed_ms = observation_timer.elapsed_ms();
        let record_max_phase = record_timing.slowest_phase(record_elapsed_ms);
        builder_timing.inventory_observation.record_latency(
            record_elapsed_ms,
            observation.parent_builder_order_id,
            &observation.market_slug,
            &observation.token_id,
            observation.user_id,
            record_max_phase.phase,
        );
    }
    builder_timing.inventory_observation.total_ms = millis_u64(inventory_loop_started.elapsed());
    builder_timing.set_phase_ms(
        TradeBuilderOrdersPhase::InventoryObservationLoop,
        builder_timing.inventory_observation.total_ms,
    );

    let final_fill_sync_started = Instant::now();
    let mut final_fill_sync_stats =
        crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats::default();
    for user_id in synced_user_ids {
        let Some(client) = user_executor_cache.get(&user_id) else {
            continue;
        };
        if !final_fill_sync_required_user_ids.contains(&user_id) {
            final_fill_sync_stats.record_skipped_fresh(user_id);
            continue;
        }
        final_fill_sync_stats.record_required();
        builder_timing.fill_sync_call_count = builder_timing.fill_sync_call_count.saturating_add(1);
        final_fill_sync_stats.record_call(user_id);
        let user_sync_timer = crate::trade_builder_fill_sync_timing::FinalFillSyncTimer::start();
        let sync_result = sync_recent_trade_builder_fills_with_timing(
            repo,
            client.as_ref(),
            &mut final_fill_sync_stats,
        )
        .await;
        let user_sync_ms = user_sync_timer.elapsed_ms();
        final_fill_sync_stats.record_user_ms(user_id, user_sync_ms);
        match sync_result {
            Ok(_) => final_fill_sync_stats.record_success(),
            Err(err) => {
                final_fill_sync_stats.record_error();
                builder_timing.fill_sync_error_count =
                    builder_timing.fill_sync_error_count.saturating_add(1);
                let err_text = format!("{err:#}");
                warn!(
                    run_id,
                    user_id,
                    error = %err_text,
                    "TRADE_BUILDER_FILL_SYNC_ERROR"
                );
            }
        }
    }
    final_fill_sync_stats.total_ms = millis_u64(final_fill_sync_started.elapsed());
    builder_timing.final_fill_sync = final_fill_sync_stats;
    builder_timing.set_phase_ms(
        TradeBuilderOrdersPhase::FinalFillSync,
        millis_u64(final_fill_sync_started.elapsed()),
    );

    let armed_cache_started = Instant::now();
    let armed_orders_result = repo.list_armed_tp_sl_child_builder_orders().await;
    match armed_orders_result {
        Ok(armed_orders) => {
            builder_timing.armed_cache_count = armed_orders.len() as u64;
            refresh_armed_builder_order_cache(armed_orders).await;
            builder_timing.set_phase_ms(
                TradeBuilderOrdersPhase::RefreshArmedCache,
                millis_u64(armed_cache_started.elapsed()),
            );
        }
        Err(err) => {
            builder_timing.set_phase_ms(
                TradeBuilderOrdersPhase::RefreshArmedCache,
                millis_u64(armed_cache_started.elapsed()),
            );
            return Err(err);
        }
    }

    let guarded_cache_started = Instant::now();
    let guarded_buy_orders_result = repo.list_guard_blocked_immediate_buy_builder_orders().await;
    match guarded_buy_orders_result {
        Ok(guarded_buy_orders) => {
            builder_timing.guarded_buy_cache_count = guarded_buy_orders.len() as u64;
            refresh_guarded_buy_order_cache(guarded_buy_orders).await;
            builder_timing.set_phase_ms(
                TradeBuilderOrdersPhase::RefreshGuardedBuyCache,
                millis_u64(guarded_cache_started.elapsed()),
            );
        }
        Err(err) => {
            builder_timing.set_phase_ms(
                TradeBuilderOrdersPhase::RefreshGuardedBuyCache,
                millis_u64(guarded_cache_started.elapsed()),
            );
            return Err(err);
        }
    }

    let market_stream_union_started = Instant::now();
    let market_stream_union_result = ensure_fast_path_market_stream_union(ws).await;
    builder_timing.set_phase_ms(
        TradeBuilderOrdersPhase::MarketStreamUnion,
        millis_u64(market_stream_union_started.elapsed()),
    );
    if let Err(err) = market_stream_union_result {
        warn!(run_id, error = %err, "ARMED_ORDER_WS_STREAM_UNION_REFRESH_FAILED");
    }

    let auto_scope_backfill_started = Instant::now();
    let auto_scope_backfill_result =
        maybe_backfill_trade_builder_auto_scope_analysis_snapshots(repo).await;
    builder_timing.set_phase_ms(
        TradeBuilderOrdersPhase::AutoScopeBackfill,
        millis_u64(auto_scope_backfill_started.elapsed()),
    );
    if let Err(err) = auto_scope_backfill_result {
        builder_timing.auto_scope_backfill_error_count = builder_timing
            .auto_scope_backfill_error_count
            .saturating_add(1);
        warn!(run_id, error = %err, "AUTO_SCOPE_ANALYSIS_BACKFILL_CYCLE_FAILED");
    }

    if orders_empty && pending_inventory_observations_empty {
        return Ok(());
    }

    Ok(())
}

// DCA functions moved to dca.rs — direct market order approach.
