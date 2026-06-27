use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    future::Future,
    hash::{Hash, Hasher},
};

const FLOW_NODE_STATE_PTB_BLOCK_EVENT_COALESCING: &str = "ptb_block_event_coalescing";
const BLOCK_EVENT_SUMMARY_INTERVAL_MS: i64 = 10_000;

tokio::task_local! {
    static PTB_BLOCK_EVENT_COALESCING_PASS_STATS: RefCell<PriceToBeatBlockEventCoalescingStats>;
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PriceToBeatBlockEventCoalescingStats {
    pub(crate) suppressed_count: u64,
}

pub(crate) async fn with_price_to_beat_block_event_coalescing_pass_stats<F>(future: F) -> F::Output
where
    F: Future,
{
    PTB_BLOCK_EVENT_COALESCING_PASS_STATS
        .scope(
            RefCell::new(PriceToBeatBlockEventCoalescingStats::default()),
            future,
        )
        .await
}

pub(crate) fn price_to_beat_block_event_coalescing_pass_stats(
) -> PriceToBeatBlockEventCoalescingStats {
    PTB_BLOCK_EVENT_COALESCING_PASS_STATS
        .try_with(|stats| stats.borrow().clone())
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockEventCoalescingOutcome {
    pub(crate) emit: bool,
    pub(crate) summary: Option<Value>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn coalesce_pre_order_price_to_beat_block_event(
    context: &mut Value,
    flow_run_id: i64,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    guard_name: &str,
    blocked_reason: &str,
    payload: &Value,
    now_ms: i64,
) -> BlockEventCoalescingOutcome {
    let key = coalescing_key(
        flow_run_id,
        node_key,
        market_slug,
        token_id,
        guard_name,
        blocked_reason,
    );
    let payload_hash = stable_value_hash(payload);
    let previous = crate::flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PTB_BLOCK_EVENT_COALESCING,
    )
    .cloned();
    let previous_summary = previous
        .as_ref()
        .filter(|previous| previous.get("key").and_then(Value::as_str) != Some(key.as_str()))
        .and_then(coalesced_summary_from_state);

    if previous
        .as_ref()
        .and_then(|previous| previous.get("key"))
        .and_then(Value::as_str)
        != Some(key.as_str())
    {
        store_coalescing_state(
            context,
            node_key,
            &key,
            now_ms,
            now_ms,
            now_ms,
            0,
            &payload_hash,
        );
        return BlockEventCoalescingOutcome {
            emit: true,
            summary: previous_summary,
        };
    }

    let first_seen_at = previous
        .as_ref()
        .and_then(|value| value.get("first_seen_at"))
        .and_then(Value::as_i64)
        .unwrap_or(now_ms);
    let last_emitted_at = previous
        .as_ref()
        .and_then(|value| value.get("last_emitted_at"))
        .and_then(Value::as_i64)
        .unwrap_or(first_seen_at);
    let suppressed_count = previous
        .as_ref()
        .and_then(|value| value.get("suppressed_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);

    if suppressed_count > 0
        && now_ms.saturating_sub(last_emitted_at) >= BLOCK_EVENT_SUMMARY_INTERVAL_MS
    {
        let summary = json!({
            "first_seen_at": first_seen_at,
            "last_seen_at": now_ms,
            "suppressed_count": suppressed_count,
            "last_payload_hash": payload_hash,
        });
        store_coalescing_state(
            context,
            node_key,
            &key,
            now_ms,
            now_ms,
            now_ms,
            0,
            &payload_hash,
        );
        return BlockEventCoalescingOutcome {
            emit: true,
            summary: Some(summary),
        };
    }

    store_coalescing_state(
        context,
        node_key,
        &key,
        first_seen_at,
        now_ms,
        last_emitted_at,
        suppressed_count + 1,
        &payload_hash,
    );
    record_suppressed_block_event();
    BlockEventCoalescingOutcome {
        emit: false,
        summary: None,
    }
}

fn record_suppressed_block_event() {
    let _ = PTB_BLOCK_EVENT_COALESCING_PASS_STATS.try_with(|stats| {
        stats.borrow_mut().suppressed_count += 1;
    });
}

fn store_coalescing_state(
    context: &mut Value,
    node_key: &str,
    key: &str,
    first_seen_at: i64,
    last_seen_at: i64,
    last_emitted_at: i64,
    suppressed_count: i64,
    payload_hash: &str,
) {
    crate::set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PTB_BLOCK_EVENT_COALESCING,
        json!({
            "key": key,
            "first_seen_at": first_seen_at,
            "last_seen_at": last_seen_at,
            "last_emitted_at": last_emitted_at,
            "suppressed_count": suppressed_count,
            "last_payload_hash": payload_hash,
        }),
    );
}

fn coalesced_summary_from_state(state: &Value) -> Option<Value> {
    let suppressed_count = state.get("suppressed_count").and_then(Value::as_i64)?;
    if suppressed_count <= 0 {
        return None;
    }
    Some(json!({
        "first_seen_at": state.get("first_seen_at").cloned().unwrap_or(Value::Null),
        "last_seen_at": state.get("last_seen_at").cloned().unwrap_or(Value::Null),
        "suppressed_count": suppressed_count,
        "last_payload_hash": state.get("last_payload_hash").cloned().unwrap_or(Value::Null),
    }))
}

fn coalescing_key(
    flow_run_id: i64,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    guard_name: &str,
    blocked_reason: &str,
) -> String {
    stable_hash(&format!(
        "{flow_run_id}|{node_key}|{market_slug}|{token_id}|{guard_name}|{blocked_reason}"
    ))
}

fn stable_value_hash(value: &Value) -> String {
    stable_hash(&serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()))
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalescing_suppresses_repeated_same_reason() {
        let mut context = json!({});
        let payload = json!({"reason_code": "order_book_unavailable"});
        let first = coalesce_pre_order_price_to_beat_block_event(
            &mut context,
            1,
            "action",
            "market",
            "token",
            "price_to_beat",
            "order_book_unavailable",
            &payload,
            1_000,
        );
        let second = coalesce_pre_order_price_to_beat_block_event(
            &mut context,
            1,
            "action",
            "market",
            "token",
            "price_to_beat",
            "order_book_unavailable",
            &payload,
            1_150,
        );

        assert!(first.emit);
        assert!(!second.emit);
    }

    #[test]
    fn coalescing_emits_when_reason_changes_with_previous_summary() {
        let mut context = json!({});
        let payload = json!({"reason_code": "a"});
        let _ = coalesce_pre_order_price_to_beat_block_event(
            &mut context,
            1,
            "action",
            "market",
            "token",
            "price_to_beat",
            "a",
            &payload,
            1_000,
        );
        let _ = coalesce_pre_order_price_to_beat_block_event(
            &mut context,
            1,
            "action",
            "market",
            "token",
            "price_to_beat",
            "a",
            &payload,
            1_150,
        );
        let changed = coalesce_pre_order_price_to_beat_block_event(
            &mut context,
            1,
            "action",
            "market",
            "token",
            "price_to_beat",
            "b",
            &json!({"reason_code": "b"}),
            1_300,
        );

        assert!(changed.emit);
        assert_eq!(
            changed
                .summary
                .as_ref()
                .and_then(|value| value.get("suppressed_count"))
                .and_then(Value::as_i64),
            Some(1)
        );
    }
}
