const PAIR_LOCK_QUOTE_FRESHNESS_MAX_AGE_MS: i64 = 250;
const PAIR_LOCK_TRIGGER_CANDIDATE_QUOTES_KEY: &str = "pairLockCandidateQuotes";

#[derive(Debug, Clone, PartialEq)]
struct PairLockQuoteCandidate {
    token_id: String,
    outcome_label: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    event_ts: Option<i64>,
    snapshot_age_ms: Option<i64>,
    source_kind: &'static str,
    source_detail: String,
    ws_state: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct PairLockResolvedQuote {
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    current_price: f64,
    quote_source_kind: &'static str,
    quote_ws_state: &'static str,
    quote_event_ts: Option<i64>,
    quote_snapshot_age_ms: Option<i64>,
    quote_source_detail: String,
    quote_book_missing_fields: Vec<String>,
    quote_snapshot_used: Value,
}

fn pair_lock_quote_book_missing_fields(best_bid: Option<f64>, best_ask: Option<f64>) -> Vec<String> {
    let mut missing = Vec::new();
    if best_bid.is_none() {
        missing.push("best_bid".to_string());
    }
    if best_ask.is_none() {
        missing.push("best_ask".to_string());
    }
    missing
}

fn pair_lock_quote_ws_state_static(value: Option<&str>) -> &'static str {
    match value.unwrap_or("live_ws_not_subscribed") {
        "live_ws_seeded" => "live_ws_seeded",
        "live_ws_stale" => "live_ws_stale",
        "live_ws_subscribed_unseeded" => "live_ws_subscribed_unseeded",
        _ => "live_ws_not_subscribed",
    }
}

fn pair_lock_normalize_quote_price(value: Option<f64>) -> Option<f64> {
    normalize_trade_builder_reference_price(value).map(clamp_probability)
}

fn pair_lock_candidate_current_price(
    best_bid: Option<f64>,
    last_trade_price: Option<f64>,
    best_ask: Option<f64>,
    current_price_hint: Option<f64>,
) -> f64 {
    best_bid
        .or(last_trade_price)
        .or(best_ask)
        .or(current_price_hint.and_then(|value| pair_lock_normalize_quote_price(Some(value))))
        .unwrap_or(0.5)
}

fn pair_lock_quote_snapshot_used(candidate: &PairLockQuoteCandidate) -> Value {
    json!({
        "token_id": candidate.token_id,
        "outcome_label": candidate.outcome_label,
        "best_bid": candidate.best_bid,
        "best_ask": candidate.best_ask,
        "last_trade_price": candidate.last_trade_price,
        "quote_source_kind": candidate.source_kind,
        "quote_ws_state": candidate.ws_state,
        "quote_event_ts": candidate.event_ts,
        "quote_snapshot_age_ms": candidate.snapshot_age_ms,
        "quote_source_detail": candidate.source_detail,
        "quote_book_missing_fields": pair_lock_quote_book_missing_fields(candidate.best_bid, candidate.best_ask),
    })
}

fn pair_lock_finalize_resolved_quote(
    candidate: PairLockQuoteCandidate,
    current_price_hint: Option<f64>,
) -> PairLockResolvedQuote {
    PairLockResolvedQuote {
        best_bid: candidate.best_bid,
        best_ask: candidate.best_ask,
        last_trade_price: candidate.last_trade_price,
        current_price: pair_lock_candidate_current_price(
            candidate.best_bid,
            candidate.last_trade_price,
            candidate.best_ask,
            current_price_hint,
        ),
        quote_source_kind: candidate.source_kind,
        quote_ws_state: candidate.ws_state,
        quote_event_ts: candidate.event_ts,
        quote_snapshot_age_ms: candidate.snapshot_age_ms,
        quote_source_detail: candidate.source_detail.clone(),
        quote_book_missing_fields: pair_lock_quote_book_missing_fields(
            candidate.best_bid,
            candidate.best_ask,
        ),
        quote_snapshot_used: pair_lock_quote_snapshot_used(&candidate),
    }
}

fn pair_lock_candidate_from_market_snapshot(
    token_id: &str,
    outcome_label: &str,
    inspection: &MarketSnapshotIntrospection,
) -> Option<PairLockQuoteCandidate> {
    let snapshot = inspection.snapshot.as_ref()?;
    let snapshot_age_ms = Some(
        Utc::now()
            .timestamp_millis()
            .saturating_sub(snapshot.updated_at_ms),
    );
    Some(PairLockQuoteCandidate {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid: pair_lock_normalize_quote_price(snapshot.best_bid),
        best_ask: pair_lock_normalize_quote_price(snapshot.best_ask),
        last_trade_price: pair_lock_normalize_quote_price(snapshot.last_trade_price),
        event_ts: Some(snapshot.updated_at_ms),
        snapshot_age_ms,
        source_kind: "live_ws",
        source_detail: snapshot.last_source.clone(),
        ws_state: inspection.state.as_str(),
    })
}

fn pair_lock_candidate_from_resolved_price(
    token_id: &str,
    outcome_label: &str,
    resolved_price: &ResolvedTriggerPrice,
    default_event_ts: Option<i64>,
) -> PairLockQuoteCandidate {
    PairLockQuoteCandidate {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid: pair_lock_normalize_quote_price(resolved_price.detail.best_bid),
        best_ask: pair_lock_normalize_quote_price(resolved_price.detail.best_ask),
        last_trade_price: pair_lock_normalize_quote_price(resolved_price.detail.last_trade_price),
        event_ts: resolved_price.ts.or(default_event_ts),
        snapshot_age_ms: resolved_price.detail.snapshot_age_ms,
        source_kind: "trigger_snapshot",
        source_detail: resolved_price.detail.source_detail.clone(),
        ws_state: "live_ws_not_subscribed",
    }
}

fn pair_lock_candidate_to_value(candidate: &PairLockQuoteCandidate) -> Value {
    json!({
        "token_id": candidate.token_id,
        "outcome_label": candidate.outcome_label,
        "best_bid": candidate.best_bid,
        "best_ask": candidate.best_ask,
        "last_trade_price": candidate.last_trade_price,
        "quote_source_kind": candidate.source_kind,
        "quote_ws_state": candidate.ws_state,
        "quote_event_ts": candidate.event_ts,
        "quote_snapshot_age_ms": candidate.snapshot_age_ms,
        "quote_source_detail": candidate.source_detail,
        "quote_book_missing_fields": pair_lock_quote_book_missing_fields(candidate.best_bid, candidate.best_ask),
    })
}

fn pair_lock_trigger_snapshot_allowed(step: &TradeFlowRunStep) -> bool {
    step.parent_step_id.is_none()
}

fn pair_lock_candidate_from_value(value: &Value) -> Option<PairLockQuoteCandidate> {
    let token_id = value
        .get("token_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let outcome_label = value
        .get("outcome_label")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(PairLockQuoteCandidate {
        token_id,
        outcome_label,
        best_bid: value.get("best_bid").and_then(value_as_f64).and_then(|value| pair_lock_normalize_quote_price(Some(value))),
        best_ask: value.get("best_ask").and_then(value_as_f64).and_then(|value| pair_lock_normalize_quote_price(Some(value))),
        last_trade_price: value
            .get("last_trade_price")
            .and_then(value_as_f64)
            .and_then(|value| pair_lock_normalize_quote_price(Some(value))),
        event_ts: value.get("quote_event_ts").and_then(Value::as_i64),
        snapshot_age_ms: value.get("quote_snapshot_age_ms").and_then(Value::as_i64),
        source_kind: "trigger_snapshot",
        source_detail: value
            .get("quote_source_detail")
            .and_then(Value::as_str)
            .unwrap_or("trigger_snapshot")
            .to_string(),
        ws_state: pair_lock_quote_ws_state_static(
            value.get("quote_ws_state").and_then(Value::as_str),
        ),
    })
}

fn pair_lock_step_trigger_candidate_quote(
    step: &TradeFlowRunStep,
    token_id: &str,
) -> Option<PairLockQuoteCandidate> {
    let quotes = step
        .input_json
        .as_ref()
        .and_then(|value| value.get(PAIR_LOCK_TRIGGER_CANDIDATE_QUOTES_KEY))
        .and_then(Value::as_object)?;
    let quote = quotes.get(token_id)?;
    let quote = pair_lock_candidate_from_value(quote)?;
    let age_ms = quote
        .event_ts
        .map(|event_ts| Utc::now().timestamp_millis().saturating_sub(event_ts))
        .or(quote.snapshot_age_ms)?;
    (age_ms <= PAIR_LOCK_QUOTE_FRESHNESS_MAX_AGE_MS).then_some(quote)
}

fn pair_lock_ws_event_name(payload: &Value) -> &str {
    payload
        .get("event_type")
        .or_else(|| payload.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn pair_lock_ws_event_token_matches(payload: &Value, token_id: &str) -> bool {
    payload
        .get("asset_id")
        .or_else(|| payload.get("assetId"))
        .or_else(|| payload.get("market"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.starts_with("0x"))
        == Some(token_id)
}

fn pair_lock_candidate_from_ws_payloads(
    token_id: &str,
    outcome_label: &str,
    ws_state: &'static str,
    payloads: &[Value],
) -> Option<PairLockQuoteCandidate> {
    let mut best_bid = None;
    let mut best_ask = None;
    let mut last_trade_price = None;
    let mut latest_ts = None;
    let mut sources: Vec<String> = Vec::new();

    for payload in payloads {
        if !pair_lock_ws_event_token_matches(payload, token_id) {
            continue;
        }
        let source = pair_lock_ws_event_name(payload).to_string();
        if !sources.iter().any(|existing| existing == &source) {
            sources.push(source);
        }
        if let Some(value) = payload
            .get("best_bid")
            .or_else(|| payload.get("bestBid"))
            .or_else(|| payload.get("bid"))
            .and_then(value_as_f64)
            .and_then(|value| pair_lock_normalize_quote_price(Some(value)))
        {
            best_bid = Some(value);
        }
        if let Some(value) = payload
            .get("best_ask")
            .or_else(|| payload.get("bestAsk"))
            .or_else(|| payload.get("ask"))
            .and_then(value_as_f64)
            .and_then(|value| pair_lock_normalize_quote_price(Some(value)))
        {
            best_ask = Some(value);
        }
        let last_trade_candidate = payload
            .get("last_trade_price")
            .or_else(|| payload.get("last"))
            .and_then(value_as_f64)
            .or_else(|| {
                matches!(
                    pair_lock_ws_event_name(payload),
                    "last_trade_price" | "trade" | "fill" | "price_change" | "price_changes"
                )
                .then(|| payload.get("price").and_then(value_as_f64))
                .flatten()
            })
            .and_then(|value| pair_lock_normalize_quote_price(Some(value)));
        if let Some(value) = last_trade_candidate {
            last_trade_price = Some(value);
        }
        if let Some(event_ts) = payload
            .get("timestamp")
            .or_else(|| payload.get("ts"))
            .and_then(Value::as_i64)
        {
            latest_ts = Some(latest_ts.map_or(event_ts, |current: i64| current.max(event_ts)));
        }
    }

    if best_bid.is_none() && best_ask.is_none() && last_trade_price.is_none() {
        return None;
    }

    let snapshot_age_ms = latest_ts.map(|event_ts| {
        Utc::now()
            .timestamp_millis()
            .saturating_sub(event_ts)
    });
    let source_detail = if sources.is_empty() {
        "ws_subscribe_once".to_string()
    } else {
        format!("ws_subscribe_once:{}", sources.join("+"))
    };

    Some(PairLockQuoteCandidate {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid,
        best_ask,
        last_trade_price,
        event_ts: latest_ts,
        snapshot_age_ms,
        source_kind: "ws_subscribe_once",
        source_detail,
        ws_state,
    })
}

fn pair_lock_candidate_from_ws_events(
    token_id: &str,
    outcome_label: &str,
    ws_state: &'static str,
    events: &[WsEvent],
) -> Option<PairLockQuoteCandidate> {
    let mut payloads = Vec::new();
    for event in events {
        if !matches!(event.channel, WsChannel::Market) {
            continue;
        }
        payloads.push(event.payload.clone());
        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(Value::as_array)
        {
            payloads.extend(changes.iter().cloned());
        }
    }
    pair_lock_candidate_from_ws_payloads(token_id, outcome_label, ws_state, &payloads)
}

async fn fetch_pair_lock_executor_quote(
    client: &dyn OrderExecutor,
    token_id: &str,
    outcome_label: &str,
    ws_state: &'static str,
) -> PairLockQuoteCandidate {
    let (best_bid_ask_result, last_trade_result) = tokio::join!(
        client.best_bid_ask(token_id),
        client.last_trade_price(token_id)
    );
    let (best_bid_raw, best_ask_raw) = best_bid_ask_result.unwrap_or((None, None));
    let last_trade_raw = last_trade_result.unwrap_or(None);
    PairLockQuoteCandidate {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid: pair_lock_normalize_quote_price(best_bid_raw),
        best_ask: pair_lock_normalize_quote_price(best_ask_raw),
        last_trade_price: pair_lock_normalize_quote_price(last_trade_raw),
        event_ts: None,
        snapshot_age_ms: None,
        source_kind: "executor_fallback",
        source_detail: "executor_best_bid_ask_last_trade".to_string(),
        ws_state,
    }
}

fn enrich_pair_lock_live_quote_with_executor(
    live_quote: &PairLockQuoteCandidate,
    executor_quote: &PairLockQuoteCandidate,
) -> PairLockQuoteCandidate {
    PairLockQuoteCandidate {
        token_id: live_quote.token_id.clone(),
        outcome_label: live_quote.outcome_label.clone(),
        best_bid: live_quote.best_bid.or(executor_quote.best_bid),
        best_ask: live_quote.best_ask.or(executor_quote.best_ask),
        last_trade_price: live_quote.last_trade_price.or(executor_quote.last_trade_price),
        event_ts: live_quote.event_ts,
        snapshot_age_ms: live_quote.snapshot_age_ms,
        source_kind: "live_ws_enriched_with_executor",
        source_detail: format!(
            "{}+{}",
            live_quote.source_detail, executor_quote.source_detail
        ),
        ws_state: live_quote.ws_state,
    }
}

async fn resolve_pair_lock_action_candidate_quote(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    step: &TradeFlowRunStep,
    token_id: &str,
    outcome_label: &str,
    current_price_hint: Option<f64>,
) -> PairLockResolvedQuote {
    let inspection = ws
        .inspect_market_snapshot(token_id, PAIR_LOCK_QUOTE_FRESHNESS_MAX_AGE_MS)
        .await;
    let ws_state = inspection.state.as_str();
    let live_quote = pair_lock_candidate_from_market_snapshot(token_id, outcome_label, &inspection);
    let mut executor_quote: Option<PairLockQuoteCandidate> = None;

    if matches!(
        inspection.state,
        MarketSnapshotWsState::NotSubscribed | MarketSnapshotWsState::SubscribedUnseeded
    ) {
        if let Ok(events) = ws.subscribe_once(WsChannel::Market, &[token_id.to_string()]).await {
            if let Some(direct_ws_quote) =
                pair_lock_candidate_from_ws_events(token_id, outcome_label, ws_state, &events)
            {
                return pair_lock_finalize_resolved_quote(direct_ws_quote, current_price_hint);
            }
        }
    }

    if inspection.state == MarketSnapshotWsState::Seeded {
        if let Some(live_quote) = live_quote.as_ref() {
            let missing_fields =
                pair_lock_quote_book_missing_fields(live_quote.best_bid, live_quote.best_ask);
            if missing_fields.is_empty() {
                return pair_lock_finalize_resolved_quote(live_quote.clone(), current_price_hint);
            }
            let fetched = fetch_pair_lock_executor_quote(client, token_id, outcome_label, ws_state).await;
            let enriched = enrich_pair_lock_live_quote_with_executor(live_quote, &fetched);
            executor_quote = Some(fetched.clone());
            if pair_lock_quote_book_missing_fields(enriched.best_bid, enriched.best_ask).len()
                < missing_fields.len()
            {
                return pair_lock_finalize_resolved_quote(enriched, current_price_hint);
            }
        }
    }

    if pair_lock_trigger_snapshot_allowed(step) {
        if let Some(mut trigger_quote) = pair_lock_step_trigger_candidate_quote(step, token_id) {
            trigger_quote.ws_state = ws_state;
            return pair_lock_finalize_resolved_quote(trigger_quote, current_price_hint);
        }
    }

    let executor_quote = match executor_quote {
        Some(quote) => quote,
        None => fetch_pair_lock_executor_quote(client, token_id, outcome_label, ws_state).await,
    };
    pair_lock_finalize_resolved_quote(executor_quote, current_price_hint)
}

async fn build_pair_lock_trigger_candidate_quotes(
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    market_snapshots: &HashMap<String, MarketDataSnapshot>,
    client: Option<&dyn OrderExecutor>,
    quote_observed_at: DateTime<Utc>,
) -> Value {
    if !node_spec.pair_lock_only_monitor {
        return Value::Null;
    }

    let mut quotes = serde_json::Map::new();
    for candidate_spec in run_spec.nodes.iter().filter(|candidate_spec| {
        candidate_spec.node_key == node_spec.node_key
            && candidate_spec.pair_lock_only_monitor
            && candidate_spec.market_slug == node_spec.market_slug
    }) {
        let resolved = market_snapshots
            .get(&candidate_spec.token_id)
            .and_then(|snapshot| {
                resolve_trigger_price_from_market_snapshot(
                    snapshot,
                    candidate_spec.price_mode,
                    Some(candidate_spec.trigger_condition.as_str()),
                )
            });
        let resolved = if let Some(resolved) = resolved {
            Some(resolved)
        } else if let Some(client) = client {
            resolve_trigger_price_from_rest(
                client,
                &candidate_spec.token_id,
                candidate_spec.price_mode,
                Some(candidate_spec.trigger_condition.as_str()),
            )
            .await
            .ok()
        } else {
            None
        };
        let Some(resolved) = resolved else {
            continue;
        };
        let quote = pair_lock_candidate_from_resolved_price(
            &candidate_spec.token_id,
            &candidate_spec.outcome_label,
            &resolved,
            Some(quote_observed_at.timestamp_millis()),
        );
        quotes.insert(candidate_spec.token_id.clone(), pair_lock_candidate_to_value(&quote));
    }

    Value::Object(quotes)
}

#[cfg(test)]
mod pair_lock_quote_tests {
    use super::*;

    #[test]
    fn pair_lock_step_trigger_candidate_quote_requires_first_attempt_and_fresh_age() {
        let fresh_step = TradeFlowRunStep {
            id: 1,
            run_id: 1,
            node_key: "action".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({
                "pairLockCandidateQuotes": {
                    "tok-yes": {
                        "token_id": "tok-yes",
                        "outcome_label": "Up",
                        "best_bid": 0.47,
                        "best_ask": 0.48,
                        "last_trade_price": 0.48,
                        "quote_event_ts": Utc::now().timestamp_millis(),
                        "quote_snapshot_age_ms": 0,
                        "quote_source_detail": "rest_composite_default_max_bid_last_trade"
                    }
                }
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
        assert!(pair_lock_step_trigger_candidate_quote(&fresh_step, "tok-yes").is_some());

        let repeat_step = TradeFlowRunStep {
            parent_step_id: Some(99),
            ..fresh_step.clone()
        };
        assert!(!pair_lock_trigger_snapshot_allowed(&repeat_step));

        let stale_step = TradeFlowRunStep {
            input_json: Some(json!({
                "pairLockCandidateQuotes": {
                    "tok-yes": {
                        "token_id": "tok-yes",
                        "outcome_label": "Up",
                        "best_bid": 0.47,
                        "best_ask": 0.48,
                        "last_trade_price": 0.48,
                        "quote_event_ts": Utc::now().timestamp_millis() - 5_000,
                        "quote_snapshot_age_ms": 5000,
                        "quote_source_detail": "rest_composite_default_max_bid_last_trade"
                    }
                }
            })),
            ..fresh_step
        };
        assert!(pair_lock_step_trigger_candidate_quote(&stale_step, "tok-yes").is_none());
    }

    #[test]
    fn enrich_pair_lock_live_quote_with_executor_fills_missing_best_ask() {
        let live_quote = PairLockQuoteCandidate {
            token_id: "tok-no".to_string(),
            outcome_label: "Down".to_string(),
            best_bid: Some(0.52),
            best_ask: None,
            last_trade_price: Some(0.53),
            event_ts: Some(123),
            snapshot_age_ms: Some(10),
            source_kind: "live_ws",
            source_detail: "book".to_string(),
            ws_state: "live_ws_seeded",
        };
        let executor_quote = PairLockQuoteCandidate {
            token_id: "tok-no".to_string(),
            outcome_label: "Down".to_string(),
            best_bid: Some(0.52),
            best_ask: Some(0.53),
            last_trade_price: Some(0.53),
            event_ts: None,
            snapshot_age_ms: None,
            source_kind: "executor_fallback",
            source_detail: "executor_best_bid_ask_last_trade".to_string(),
            ws_state: "live_ws_seeded",
        };

        let enriched = enrich_pair_lock_live_quote_with_executor(&live_quote, &executor_quote);
        assert_eq!(enriched.source_kind, "live_ws_enriched_with_executor");
        assert_eq!(enriched.best_ask, Some(0.53));
    }

    #[test]
    fn pair_lock_candidate_from_ws_events_extracts_best_ask_when_present() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(123),
            payload: json!({
                "event_type": "book",
                "asset_id": "tok-yes",
                "best_bid": "0.41",
                "best_ask": "0.43",
                "timestamp": 123
            }),
        }];

        let candidate = pair_lock_candidate_from_ws_events(
            "tok-yes",
            "Up",
            "live_ws_subscribed_unseeded",
            &events,
        )
        .expect("candidate");
        assert_eq!(candidate.best_bid, Some(0.41));
        assert_eq!(candidate.best_ask, Some(0.43));
        assert_eq!(candidate.last_trade_price, None);
        assert_eq!(candidate.source_kind, "ws_subscribe_once");
    }

    #[test]
    fn pair_lock_candidate_from_ws_events_keeps_missing_best_ask_when_absent() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            event_type: WsEventType::PriceChange,
            market: Some("tok-no".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: Some(0.52),
            size: None,
            ts: Some(456),
            payload: json!({
                "event_type": "last_trade_price",
                "asset_id": "tok-no",
                "best_bid": "0.51",
                "price": "0.52",
                "timestamp": 456
            }),
        }];

        let candidate = pair_lock_candidate_from_ws_events(
            "tok-no",
            "Down",
            "live_ws_subscribed_unseeded",
            &events,
        )
        .expect("candidate");
        assert_eq!(candidate.best_bid, Some(0.51));
        assert_eq!(candidate.best_ask, None);
        assert_eq!(candidate.last_trade_price, Some(0.52));
    }
}
