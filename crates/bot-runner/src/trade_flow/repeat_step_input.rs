fn clear_trade_flow_repeat_quote_keys(input_json: &mut serde_json::Map<String, Value>) {
    for key in [
        "price",
        "currentPrice",
        "wsBestBid",
        "wsBestAsk",
        "wsLastTradePrice",
        "wsSnapshotAgeMs",
        "wsPriceSource",
        "wsPriceSourceDetail",
        "ws_best_bid",
        "ws_best_ask",
        "ws_last_trade_price",
        "ws_snapshot_age_ms",
        "ws_price_source",
        "ws_price_source_detail",
        "ws_best_bid_from_step",
        "ws_best_ask_from_step",
        "ws_last_trade_price_from_step",
        "ws_snapshot_age_ms_from_step",
        "ws_price_source_from_step",
        "ws_price_source_detail_from_step",
    ] {
        input_json.remove(key);
    }
}

fn build_trade_flow_repeat_step_input(step: &TradeFlowRunStep, output: &Value) -> Option<Value> {
    let input_json = step.input_json.as_ref()?;
    if step.node_type != "action.place_order" {
        return Some(input_json.clone());
    }
    if output.get("reason").and_then(Value::as_str) != Some("pair_lock_primary_guard_waiting") {
        return Some(input_json.clone());
    }

    let mut normalized = input_json.as_object().cloned().unwrap_or_default();
    if let Some(market_slug) = output
        .get("market_slug")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        normalized.insert("market_slug".to_string(), json!(market_slug));
        normalized.insert("marketSlug".to_string(), json!(market_slug));
        normalized.insert("wsMarketSlug".to_string(), json!(market_slug));
    }
    if let Some(yes_token_id) = output
        .get("resolved_yes_token_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        normalized.insert("yesTokenId".to_string(), json!(yes_token_id));
    }
    if let Some(no_token_id) = output
        .get("resolved_no_token_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        normalized.insert("noTokenId".to_string(), json!(no_token_id));
    }
    clear_trade_flow_repeat_quote_keys(&mut normalized);
    Some(Value::Object(normalized))
}

#[cfg(test)]
mod repeat_step_input_tests {
    use super::*;

    #[test]
    fn pair_lock_waiting_repeat_input_uses_latest_market_and_clears_quote_fields() {
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 42,
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({
            "market_slug": "btc-updown-5m-old",
            "marketSlug": "btc-updown-5m-old",
            "wsMarketSlug": "btc-updown-5m-old",
            "yesTokenId": "yes-old",
            "noTokenId": "no-old",
            "price": 0.52,
            "wsBestBid": 0.51,
            "wsBestAsk": 0.52,
            "wsLastTradePrice": 0.52,
            "wsPriceSource": "live_ws",
            "wsPriceSourceDetail": "price_changes",
            "ws_best_bid_from_step": 0.51,
            "ws_best_ask_from_step": 0.52,
            "ws_price_source_from_step": "live_ws",
            "unchanged": true
            })),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };
        let output = json!({
            "reason": "pair_lock_primary_guard_waiting",
            "market_slug": "btc-updown-5m-new",
            "resolved_yes_token_id": "yes-new",
            "resolved_no_token_id": "no-new"
        });

        let normalized =
            build_trade_flow_repeat_step_input(&step, &output).expect("normalized input");
        assert_eq!(
            normalized.get("market_slug").and_then(Value::as_str),
            Some("btc-updown-5m-new")
        );
        assert_eq!(
            normalized.get("marketSlug").and_then(Value::as_str),
            Some("btc-updown-5m-new")
        );
        assert_eq!(
            normalized.get("wsMarketSlug").and_then(Value::as_str),
            Some("btc-updown-5m-new")
        );
        assert_eq!(
            normalized.get("yesTokenId").and_then(Value::as_str),
            Some("yes-new")
        );
        assert_eq!(
            normalized.get("noTokenId").and_then(Value::as_str),
            Some("no-new")
        );
        assert!(normalized.get("price").is_none());
        assert!(normalized.get("wsBestBid").is_none());
        assert!(normalized.get("wsBestAsk").is_none());
        assert!(normalized.get("wsLastTradePrice").is_none());
        assert!(normalized.get("wsPriceSource").is_none());
        assert!(normalized.get("wsPriceSourceDetail").is_none());
        assert!(normalized.get("ws_best_bid_from_step").is_none());
        assert!(normalized.get("ws_best_ask_from_step").is_none());
        assert!(normalized.get("ws_price_source_from_step").is_none());
        assert_eq!(normalized.get("unchanged").and_then(Value::as_bool), Some(true));
    }
}
