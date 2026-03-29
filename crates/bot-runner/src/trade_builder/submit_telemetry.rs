#[derive(Debug, Clone)]
struct TradeBuilderSubmitAttemptContext {
    submit_path: &'static str,
    runtime_price_fetch_ms: i64,
    snapshot_age_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct TradeBuilderSubmitTiming {
    submit_started_at: DateTime<Utc>,
    submit_finished_at: DateTime<Utc>,
    guard_eval_ms: i64,
}

fn append_trade_builder_submit_telemetry(
    payload: &mut serde_json::Map<String, Value>,
    context: &TradeBuilderSubmitAttemptContext,
    timing: &TradeBuilderSubmitTiming,
    ack: Option<&bot_infra::exchange::OrderAck>,
) {
    payload.insert(
        "submit_started_at".to_string(),
        json!(timing.submit_started_at.to_rfc3339()),
    );
    payload.insert(
        "submit_finished_at".to_string(),
        json!(timing.submit_finished_at.to_rfc3339()),
    );
    payload.insert(
        "runtime_price_fetch_ms".to_string(),
        json!(context.runtime_price_fetch_ms),
    );
    payload.insert("guard_eval_ms".to_string(), json!(timing.guard_eval_ms));
    payload.insert("submit_path".to_string(), json!(context.submit_path));
    payload.insert("snapshot_age_ms".to_string(), json!(context.snapshot_age_ms));
    payload.insert(
        "place_sign_ms".to_string(),
        json!(ack.and_then(|value| value.sign_ms)),
    );
    payload.insert(
        "place_http_ms".to_string(),
        json!(ack.and_then(|value| value.http_ms)),
    );
}
