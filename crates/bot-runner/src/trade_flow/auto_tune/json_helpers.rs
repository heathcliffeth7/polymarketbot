fn auto_tune_json_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn auto_tune_json_f64_path(value: &Value, paths: &[&[&str]]) -> Option<f64> {
    paths
        .iter()
        .find_map(|path| auto_tune_json_path(value, path).and_then(value_as_f64))
        .filter(|value| value.is_finite())
}

fn auto_tune_json_i64_path(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    paths.iter().find_map(|path| {
        auto_tune_json_path(value, path)
            .and_then(Value::as_i64)
            .or_else(|| {
                auto_tune_json_path(value, path)
                    .and_then(Value::as_f64)
                    .map(|value| value.round() as i64)
            })
    })
}

fn auto_tune_json_bool_path(value: &Value, paths: &[&[&str]]) -> Option<bool> {
    paths
        .iter()
        .find_map(|path| auto_tune_json_path(value, path).and_then(Value::as_bool))
}

fn auto_tune_config_f64(node: &TradeFlowNode, key: &str) -> Option<f64> {
    node.config.get(key).and_then(value_as_f64)
}

fn auto_tune_config_bool(node: &TradeFlowNode, key: &str) -> Option<bool> {
    node.config.get(key).and_then(Value::as_bool)
}

fn auto_tune_price_value(value: Option<f64>) -> Option<f64> {
    value.map(|value| {
        if value.is_finite() && value > 1.0 {
            value / 100.0
        } else {
            value
        }
    })
}

fn auto_tune_scope_for_market(market_slug: &str) -> String {
    find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.scope.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn auto_tune_market_window_start(market_slug: &str) -> Option<DateTime<Utc>> {
    MarketCycleId(market_slug.to_string()).start_time()
}

fn auto_tune_remaining_sec(window_end_at: DateTime<Utc>, at: Option<DateTime<Utc>>) -> Option<i64> {
    at.map(|at| window_end_at.signed_duration_since(at).num_seconds().max(0))
}
