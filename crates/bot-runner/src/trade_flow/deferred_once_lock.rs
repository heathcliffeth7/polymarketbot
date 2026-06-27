const DEFERRED_ONCE_LOCK_FIELD: &str = "deferred_once_lock";
const DEFERRED_ONCE_DEFAULT_RETRY_DELAY_MS: i64 = 150;

#[derive(Debug, Clone)]
struct DeferredOnceLockMeta {
    lock_key: String,
    trigger_node_key: String,
    action_node_key: String,
    market_slug: String,
    token_id: String,
    outcome_label: String,
    once_scope_market: bool,
    expires_at: DateTime<Utc>,
}

fn trade_flow_defer_once_until_order_accepted(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node)
        && node_config_bool(node, "deferOnceUntilOrderAccepted").unwrap_or(false)
}

async fn reconcile_trade_flow_deferred_once_locks_for_run(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    context: &mut Value,
) -> Result<()> {
    let expired_count = repo
        .expire_trade_flow_deferred_once_locks_for_run(run.id)
        .await?;
    if expired_count > 0 {
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "deferred_once_lock",
            &json!({
                "pending_once_action": "expired",
                "pending_once_state": "expired",
                "expired_count": expired_count,
            }),
        )
        .await?;
    }

    for lock in repo
        .list_consumed_trade_flow_deferred_once_locks(run.id)
        .await?
    {
        if trade_flow_market_price_once_fired_for_scope(
            context,
            &lock.trigger_node_key,
            lock.once_scope_market,
            Some(lock.market_slug.as_str()),
        ) {
            continue;
        }
        mark_trade_flow_market_price_once_fired(
            context,
            &lock.trigger_node_key,
            Utc::now(),
            lock.once_scope_market.then_some(lock.market_slug.as_str()),
        );
    }
    Ok(())
}

async fn maybe_attach_deferred_once_lock_to_action_input(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    graph: &TradeFlowGraphRuntime,
    source_key: &str,
    target_node: &TradeFlowNode,
    input_json: &Value,
) -> Result<Option<Value>> {
    if target_node.node_type != "action.place_order" {
        return Ok(Some(input_json.clone()));
    }
    let Some(source_node) = flow_node(graph, source_key) else {
        return Ok(Some(input_json.clone()));
    };
    if !trade_flow_defer_once_until_order_accepted(source_node)
        || !input_json
            .get("pass")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Ok(Some(input_json.clone()));
    }

    let Some(meta) = build_deferred_once_lock_meta(run, source_node, target_node, input_json) else {
        return Ok(Some(input_json.clone()));
    };
    let input = bot_infra::db::TradeFlowDeferredOnceLockInput {
        run_id: run.id,
        definition_id: run.definition_id,
        version_id: run.version_id,
        trigger_node_key: meta.trigger_node_key.clone(),
        action_node_key: meta.action_node_key.clone(),
        market_slug: meta.market_slug.clone(),
        token_id: meta.token_id.clone(),
        outcome_label: meta.outcome_label.clone(),
        once_scope_market: meta.once_scope_market,
        lock_key: meta.lock_key.clone(),
        expires_at: meta.expires_at,
    };
    let acquired = repo.acquire_trade_flow_deferred_once_lock(&input).await?;
    if acquired.expired_count > 0 {
        append_deferred_once_event(
            repo,
            run,
            &meta,
            "expired",
            "expired",
            Some("ttl_expired"),
            None,
        )
        .await?;
    }
    if !acquired.created {
        append_deferred_once_event(repo, run, &meta, "held", "pending", None, None).await?;
        return Ok(None);
    }
    append_deferred_once_event(repo, run, &meta, "created", "pending", None, None).await?;
    Ok(Some(with_deferred_once_lock_fields(
        input_json,
        &meta,
        "created",
        "pending",
        None,
        None,
        None,
    )))
}

async fn apply_deferred_once_after_node_execution(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    execution: &mut TradeFlowNodeExecution,
    context: &mut Value,
) -> Result<()> {
    if node.node_type != "action.place_order" {
        return Ok(());
    }
    let Some(meta) = step
        .input_json
        .as_ref()
        .and_then(parse_deferred_once_lock_meta)
    else {
        return Ok(());
    };
    if execution.repeat_at.is_some()
        || execution
            .output
            .get("retrying")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        if deferred_once_lock_expired(&meta, Utc::now()) {
            let release_reason = "deferred_once_lock_ttl_expired";
            let released = repo
                .release_trade_flow_deferred_once_lock(&meta.lock_key, release_reason)
                .await?;
            if released.is_some() {
                execution.repeat_at = None;
                if let Some(obj) = execution.output.as_object_mut() {
                    obj.insert("retrying".to_string(), json!(false));
                    obj.insert("retry_expired".to_string(), json!(true));
                }
                annotate_deferred_once_output(
                    &mut execution.output,
                    &meta,
                    "released",
                    "released",
                    Some(release_reason),
                    None,
                    step.attempt,
                );
                append_deferred_once_event(
                    repo,
                    run,
                    &meta,
                    "released",
                    "released",
                    Some(release_reason),
                    None,
                )
                .await?;
            }
            return Ok(());
        }
        annotate_deferred_once_output(&mut execution.output, &meta, "held", "pending", None, None, step.attempt);
        append_deferred_once_event(repo, run, &meta, "held", "pending", None, None).await?;
        return Ok(());
    }

    if let Some(builder_order_id) = deferred_once_builder_order_id(&execution.output) {
        if let Some(consumed) = repo
            .consume_trade_flow_deferred_once_lock(&meta.lock_key, builder_order_id)
            .await?
        {
            let fired_at = Utc::now();
            mark_trade_flow_market_price_once_fired(
                context,
                &consumed.trigger_node_key,
                fired_at,
                consumed
                    .once_scope_market
                    .then_some(consumed.market_slug.as_str()),
            );
            let once_key = trade_flow_market_price_once_idempotency_key(
                run.id,
                &consumed.trigger_node_key,
                consumed.once_scope_market,
                Some(consumed.market_slug.as_str()),
                flow_node_reentry_generation(context, &consumed.trigger_node_key),
            );
            let _ = repo.try_record_idempotency_key(&once_key).await?;
            annotate_deferred_once_output(
                &mut execution.output,
                &meta,
                "consumed",
                "consumed",
                None,
                Some(builder_order_id),
                step.attempt,
            );
            append_deferred_once_event(
                repo,
                run,
                &meta,
                "consumed",
                "consumed",
                None,
                Some(builder_order_id),
            )
            .await?;
        }
        return Ok(());
    }

    let release_reason = deferred_once_release_reason(&execution.output);
    let released = repo
        .release_trade_flow_deferred_once_lock(&meta.lock_key, &release_reason)
        .await?;
    if released.is_some() {
        annotate_deferred_once_output(
            &mut execution.output,
            &meta,
            "released",
            "released",
            Some(&release_reason),
            None,
            step.attempt,
        );
        append_deferred_once_event(
            repo,
            run,
            &meta,
            "released",
            "released",
            Some(&release_reason),
            None,
        )
        .await?;
    }
    Ok(())
}

async fn release_deferred_once_after_step_error(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    step_input: Option<&Value>,
    output: &mut Value,
    error_text: &str,
) -> Result<()> {
    if node.node_type != "action.place_order" {
        return Ok(());
    }
    let Some(meta) = step_input.and_then(parse_deferred_once_lock_meta) else {
        return Ok(());
    };
    let reason = format!("step_failed:{error_text}");
    let released = repo
        .release_trade_flow_deferred_once_lock(&meta.lock_key, &reason)
        .await?;
    if released.is_some() {
        annotate_deferred_once_output(output, &meta, "released", "released", Some(&reason), None, 0);
        append_deferred_once_event(
            repo,
            run,
            &meta,
            "released",
            "released",
            Some(&reason),
            None,
        )
        .await?;
    }
    Ok(())
}

fn build_deferred_once_lock_meta(
    run: &TradeFlowRun,
    source_node: &TradeFlowNode,
    target_node: &TradeFlowNode,
    input_json: &Value,
) -> Option<DeferredOnceLockMeta> {
    let market_slug = deferred_once_string(input_json, &["market_slug", "marketSlug"])?;
    let token_id = deferred_once_string(
        input_json,
        &["triggered_token_id", "token_id", "tokenId", "triggeredTokenId"],
    )?;
    let outcome_label = deferred_once_string(
        input_json,
        &[
            "triggered_outcome_label",
            "outcome_label",
            "outcomeLabel",
            "triggeredOutcomeLabel",
        ],
    )?;
    let once_scope_market = input_json
        .get("once_scope")
        .and_then(Value::as_str)
        .map(|value| value == "market")
        .unwrap_or_else(|| is_trade_flow_market_price_once_scope_market(source_node));
    let lock_key = format!(
        "flow-deferred-once:{}:{}:{}:{}:{}:{}",
        run.id, source_node.key, target_node.key, market_slug, token_id, outcome_label
    );
    let expires_at = deferred_once_expires_at(target_node, input_json);
    Some(DeferredOnceLockMeta {
        lock_key,
        trigger_node_key: source_node.key.clone(),
        action_node_key: target_node.key.clone(),
        market_slug,
        token_id,
        outcome_label,
        once_scope_market,
        expires_at,
    })
}

fn parse_deferred_once_lock_meta(input_json: &Value) -> Option<DeferredOnceLockMeta> {
    let lock = input_json.get(DEFERRED_ONCE_LOCK_FIELD)?;
    Some(DeferredOnceLockMeta {
        lock_key: deferred_once_string(lock, &["lock_key"])?,
        trigger_node_key: deferred_once_string(lock, &["trigger_node_key"])?,
        action_node_key: deferred_once_string(lock, &["action_node_key"])?,
        market_slug: deferred_once_string(lock, &["market_slug"])?,
        token_id: deferred_once_string(lock, &["token_id"])?,
        outcome_label: deferred_once_string(lock, &["outcome_label"])?,
        once_scope_market: lock
            .get("once_scope_market")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        expires_at: lock
            .get("expires_at")
            .and_then(Value::as_str)
            .and_then(parse_deferred_once_rfc3339_utc)
            .unwrap_or_else(Utc::now),
    })
}

fn deferred_once_expires_at(action_node: &TradeFlowNode, input_json: &Value) -> DateTime<Utc> {
    let retry_delay_ms = if node_config_bool(action_node, "priceToBeatEarlyStaleSideEnabled")
        .unwrap_or(false)
    {
        node_config_i64(action_node, "priceToBeatEarlyStaleRetryCooldownMs")
            .unwrap_or(500)
            .clamp(100, 60_000)
    } else {
        DEFERRED_ONCE_DEFAULT_RETRY_DELAY_MS
    };
    let retry_count = node_config_i64(action_node, "priceToBeatEarlyStaleMaxGuardRetriesPerMarket")
        .unwrap_or(1)
        .max(1);
    let ttl_ms = ((retry_delay_ms * retry_count) + 15_000).max(30_000);
    let mut expires_at = Utc::now() + ChronoDuration::milliseconds(ttl_ms);
    if let Some(market_end) = input_json
        .get("cycleWindowEndAt")
        .and_then(Value::as_str)
        .and_then(parse_deferred_once_rfc3339_utc)
    {
        expires_at = expires_at.min(market_end);
    }
    expires_at
}

fn deferred_once_lock_expired(meta: &DeferredOnceLockMeta, now: DateTime<Utc>) -> bool {
    meta.expires_at <= now
}

fn with_deferred_once_lock_fields(
    input_json: &Value,
    meta: &DeferredOnceLockMeta,
    action: &str,
    state: &str,
    release_reason: Option<&str>,
    consume_reason: Option<&str>,
    builder_order_id: Option<i64>,
) -> Value {
    let mut output = input_json.clone();
    annotate_deferred_once_output(
        &mut output,
        meta,
        action,
        state,
        release_reason,
        builder_order_id,
        0,
    );
    if let Some(obj) = output.as_object_mut() {
        obj.insert(
            DEFERRED_ONCE_LOCK_FIELD.to_string(),
            json!({
                "lock_key": meta.lock_key,
                "trigger_node_key": meta.trigger_node_key,
                "action_node_key": meta.action_node_key,
                "market_slug": meta.market_slug,
                "token_id": meta.token_id,
                "outcome_label": meta.outcome_label,
                "once_scope_market": meta.once_scope_market,
                "expires_at": meta.expires_at.to_rfc3339(),
            }),
        );
        if let Some(reason) = consume_reason {
            obj.insert("pending_once_consume_reason".to_string(), json!(reason));
        }
    }
    output
}

fn annotate_deferred_once_output(
    output: &mut Value,
    meta: &DeferredOnceLockMeta,
    action: &str,
    state: &str,
    release_reason: Option<&str>,
    builder_order_id: Option<i64>,
    action_attempt_count: i32,
) {
    if let Some(obj) = output.as_object_mut() {
        obj.insert("defer_once_until_order_accepted".to_string(), json!(true));
        obj.insert("pending_once_key".to_string(), json!(meta.lock_key));
        obj.insert("pending_once_state".to_string(), json!(state));
        obj.insert("pending_once_action".to_string(), json!(action));
        obj.insert(
            "pending_once_release_reason".to_string(),
            release_reason.map_or(Value::Null, |value| json!(value)),
        );
        obj.insert(
            "pending_once_consume_reason".to_string(),
            builder_order_id
                .map(|_| json!("order_accepted"))
                .unwrap_or(Value::Null),
        );
        obj.insert(
            "once_fired_set_at".to_string(),
            if action == "consumed" {
                json!(Utc::now().to_rfc3339())
            } else {
                Value::Null
            },
        );
        obj.insert("action_attempt_count".to_string(), json!(action_attempt_count));
        obj.insert(
            "action_accepted_order_count".to_string(),
            json!(i32::from(builder_order_id.is_some())),
        );
    }
}

async fn append_deferred_once_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    meta: &DeferredOnceLockMeta,
    action: &str,
    state: &str,
    release_reason: Option<&str>,
    builder_order_id: Option<i64>,
) -> Result<()> {
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "deferred_once_lock",
        &json!({
            "defer_once_until_order_accepted": true,
            "pending_once_key": meta.lock_key,
            "pending_once_state": state,
            "pending_once_action": action,
            "pending_once_release_reason": release_reason,
            "pending_once_consume_reason": builder_order_id.map(|_| "order_accepted"),
            "trigger_node_key": meta.trigger_node_key,
            "action_node_key": meta.action_node_key,
            "market_slug": meta.market_slug,
            "token_id": meta.token_id,
            "outcome_label": meta.outcome_label,
            "expires_at": meta.expires_at.to_rfc3339(),
            "builder_order_id": builder_order_id,
        }),
    )
    .await
}

fn deferred_once_release_reason(output: &Value) -> String {
    deferred_once_string(
        output,
        &["reason", "block_reason", "price_band_block_reason", "error"],
    )
    .unwrap_or_else(|| "terminal_no_order_block".to_string())
}

fn deferred_once_builder_order_id(output: &Value) -> Option<i64> {
    output
        .get("builder_order_id")
        .or_else(|| output.get("builderOrderId"))
        .and_then(value_as_i64)
        .filter(|value| *value > 0)
}

fn deferred_once_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| match value {
            Value::String(raw) => Some(raw.trim().to_string()),
            Value::Number(raw) => Some(raw.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn parse_deferred_once_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

#[cfg(test)]
mod deferred_once_lock_tests {
    use super::*;

    fn test_run() -> TradeFlowRun {
        TradeFlowRun {
            id: 42,
            definition_id: 4364,
            version_id: 6127,
            user_id: 7,
            status: "running".to_string(),
            trigger_source: None,
            context_json: json!({}),
            started_at: None,
            ended_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_trigger() -> TradeFlowNode {
        TradeFlowNode {
            key: "trigger_btc_eq77_up".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "repeatMode": "once",
                "onceScope": "market",
                "deferOnceUntilOrderAccepted": true
            }),
        }
    }

    fn test_action() -> TradeFlowNode {
        TradeFlowNode {
            key: "action_btc_eq77_up".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({}),
        }
    }

    #[test]
    fn defer_flag_requires_once_market_price_trigger() {
        assert!(trade_flow_defer_once_until_order_accepted(&test_trigger()));
        let mut non_once = test_trigger();
        non_once.config["repeatMode"] = json!("loop");
        assert!(!trade_flow_defer_once_until_order_accepted(&non_once));
    }

    #[test]
    fn meta_key_includes_action_and_caps_ttl_at_market_end() {
        let market_end = (Utc::now() + ChronoDuration::seconds(5)).to_rfc3339();
        let input = json!({
            "pass": true,
            "market_slug": "btc-updown-5m-1",
            "triggered_token_id": "tok-up",
            "triggered_outcome_label": "Up",
            "once_scope": "market",
            "cycleWindowEndAt": market_end,
        });
        let meta = build_deferred_once_lock_meta(
            &test_run(),
            &test_trigger(),
            &test_action(),
            &input,
        )
        .expect("meta");
        assert!(meta.lock_key.contains(":trigger_btc_eq77_up:action_btc_eq77_up:"));
        assert_eq!(meta.market_slug, "btc-updown-5m-1");
        assert_eq!(meta.token_id, "tok-up");
        assert_eq!(meta.outcome_label, "Up");
        assert!(meta.once_scope_market);
        assert_eq!(meta.expires_at.to_rfc3339(), market_end);
    }

    #[test]
    fn output_annotation_sets_pending_fields() {
        let meta = DeferredOnceLockMeta {
            lock_key: "lock".to_string(),
            trigger_node_key: "trigger".to_string(),
            action_node_key: "action".to_string(),
            market_slug: "market".to_string(),
            token_id: "token".to_string(),
            outcome_label: "Up".to_string(),
            once_scope_market: true,
            expires_at: Utc::now(),
        };
        let output = with_deferred_once_lock_fields(
            &json!({"pass": true}),
            &meta,
            "created",
            "pending",
            None,
            None,
            None,
        );
        assert_eq!(output["pending_once_action"], "created");
        assert_eq!(output["pending_once_state"], "pending");
        assert_eq!(output[DEFERRED_ONCE_LOCK_FIELD]["action_node_key"], "action");
    }

    #[test]
    fn lock_expired_when_expires_at_reaches_now() {
        let now = Utc::now();
        let mut meta = DeferredOnceLockMeta {
            lock_key: "lock".to_string(),
            trigger_node_key: "trigger".to_string(),
            action_node_key: "action".to_string(),
            market_slug: "market".to_string(),
            token_id: "token".to_string(),
            outcome_label: "Up".to_string(),
            once_scope_market: true,
            expires_at: now,
        };

        assert!(deferred_once_lock_expired(&meta, now));
        meta.expires_at = now + ChronoDuration::milliseconds(1);
        assert!(!deferred_once_lock_expired(&meta, now));
    }
}
