async fn execute_biased_hedge_pair_exit(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    reason: &str,
    payload: Value,
) -> Result<TradeFlowNodeExecution> {
    if matches!(
        session.status.as_str(),
        TRADE_BUILDER_PAIR_STATUS_UNWINDING
            | TRADE_BUILDER_PAIR_STATUS_COMPLETED
            | TRADE_BUILDER_PAIR_STATUS_EXPIRED
            | TRADE_BUILDER_PAIR_STATUS_ERROR
    ) {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "skipped": true,
                "reason": "biased_hedge_exit_already_in_progress",
                "pair_session_id": session.id,
                "pair_lock_strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }
    let orders = repo.list_trade_builder_orders_by_pair_session(session.id).await?;
    schedule_trade_builder_pair_session_unwind(
        repo,
        session,
        &orders,
        TRADE_BUILDER_PAIR_STATUS_UNWINDING,
        reason,
        None,
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        session,
        "biased_hedge_exit_requested",
        json!({
            "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "phase": "EXITING",
            "decision": "exit",
            "exit_reason": reason,
            "exit_in_progress": true,
            "exit_attempt_count": 1,
            "last_exit_attempt_at": Utc::now().to_rfc3339(),
            "pair_session_id": session.id,
            "details": payload,
        }),
    )
    .await?;
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": session.flow_node_key,
            "pair_session_id": session.id,
            "pair_lock_strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "phase": "EXITING",
            "decision": "exit",
            "exit_reason": reason,
            "exit_in_progress": true,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_biased_hedge_time_exit(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let pair_session_id = step_input_i64(step, &["pairSessionId", "pair_session_id"])
        .ok_or_else(|| anyhow::anyhow!("biased hedge time exit requires pairSessionId"))?;
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        anyhow::bail!("biased hedge time exit pair session not found");
    };
    if session.status == TRADE_BUILDER_PAIR_STATUS_UNWINDING {
        return Ok(TradeFlowNodeExecution {
            output: json!({"skipped": true, "reason": "biased_hedge_exiting", "pair_session_id": pair_session_id}),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }
    let parent_builder_order_id = step_input_i64(step, &["parentBuilderOrderId", "parent_builder_order_id"])
        .ok_or_else(|| anyhow::anyhow!("biased hedge time exit requires parentBuilderOrderId"))?;
    let target_remaining_pct = step_input_f64(step, &["targetRemainingPct", "target_remaining_pct"])
        .ok_or_else(|| anyhow::anyhow!("biased hedge time exit requires targetRemainingPct"))?
        .clamp(0.0, 100.0);
    let Some(parent_order) = repo.get_trade_builder_order(parent_builder_order_id).await? else {
        anyhow::bail!("biased hedge time exit parent order not found");
    };
    let Some(position) = repo.get_trade_builder_parent_position(parent_builder_order_id).await? else {
        return Ok(TradeFlowNodeExecution {
            output: json!({"skipped": true, "reason": "biased_hedge_time_exit_no_position", "parent_builder_order_id": parent_builder_order_id}),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    };
    if position.current_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE || position.baseline_qty <= 0.0 {
        return Ok(TradeFlowNodeExecution {
            output: json!({"skipped": true, "reason": "biased_hedge_time_exit_closed", "parent_builder_order_id": parent_builder_order_id}),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }
    let target_remaining_qty = round_trade_builder_share_qty(position.baseline_qty * (target_remaining_pct / 100.0));
    let sell_qty = (position.current_qty - target_remaining_qty).max(0.0);
    if sell_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "skipped": true,
                "reason": "biased_hedge_time_exit_target_already_met",
                "parent_builder_order_id": parent_builder_order_id,
                "current_qty": position.current_qty,
                "target_remaining_qty": target_remaining_qty,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }
    let sell_pct = (sell_qty / position.current_qty * 100.0).clamp(0.0001, 100.0);
    let mut exit_step = step.clone();
    let mut input = exit_step
        .input_json
        .take()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    input.insert("internalMode".to_string(), json!("time_exit"));
    input.insert("parentBuilderOrderId".to_string(), json!(parent_builder_order_id));
    input.insert("sourceTradeId".to_string(), json!(parent_order.trade_id));
    input.insert("marketSlug".to_string(), json!(parent_order.market_slug));
    input.insert("tokenId".to_string(), json!(parent_order.token_id));
    input.insert("outcomeLabel".to_string(), json!(parent_order.outcome_label));
    input.insert("remainingPct".to_string(), json!(sell_pct));
    exit_step.input_json = Some(Value::Object(input));
    let mut exit_node = build_pair_lock_single_leg_node(
        node,
        &parent_order.market_slug,
        &parent_order.token_id,
        &parent_order.outcome_label,
        session.flow_node_key.as_deref().unwrap_or(&node.key),
    );
    if let Some(map) = exit_node.config.as_object_mut() {
        map.insert("mode".to_string(), json!(ACTION_PLACE_ORDER_MODE_SINGLE));
        map.insert("sizeMode".to_string(), json!("pct"));
        map.remove("sizeUsdc");
        map.remove("maxPriceCent");
        map.remove("priceToBeatGuardEnabled");
        map.remove("triggerPriceGuardEnabled");
        map.remove("executionFloorGuardEnabled");
    }
    let mut execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &exit_step,
        &exit_node,
        graph,
        context,
    )
    .await?;
    repo.append_trade_builder_order_event(
        parent_builder_order_id,
        "biased_hedge_time_exit_triggered",
        &json!({
            "pair_session_id": pair_session_id,
            "target_remaining_pct": target_remaining_pct,
            "target_remaining_qty": target_remaining_qty,
            "current_qty": position.current_qty,
            "sell_qty": sell_qty,
            "sell_pct_of_current": sell_pct,
            "exit_order_id": execution.output.get("builder_order_id").and_then(Value::as_i64),
        }),
    )
    .await?;
    if let Some(output) = execution.output.as_object_mut() {
        output.insert("pair_lock_strategy".to_string(), json!(PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1));
        output.insert("phase".to_string(), json!("EXITING"));
        output.insert("decision".to_string(), json!("time_exit"));
        output.insert("target_remaining_pct".to_string(), json!(target_remaining_pct));
    }
    Ok(execution)
}

#[allow(clippy::too_many_arguments)]
async fn execute_biased_hedge_monitor(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let pair_session_id = step_input_i64(step, &["pairSessionId", "pair_session_id"])
        .ok_or_else(|| anyhow::anyhow!("biased hedge monitor requires pairSessionId"))?;
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        anyhow::bail!("biased hedge monitor pair session not found");
    };
    let Some(config) = resolve_action_place_order_biased_hedge_config(node)? else {
        anyhow::bail!("biased hedge monitor requires biased hedge config");
    };
    if matches!(
        session.status.as_str(),
        TRADE_BUILDER_PAIR_STATUS_UNWINDING
            | TRADE_BUILDER_PAIR_STATUS_COMPLETED
            | TRADE_BUILDER_PAIR_STATUS_EXPIRED
            | TRADE_BUILDER_PAIR_STATUS_ERROR
    ) {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "skipped": true,
                "reason": "biased_hedge_monitor_session_inactive",
                "pair_session_id": pair_session_id,
                "status": session.status,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }
    let Some(primary_order_id) = session.primary_order_id else {
        anyhow::bail!("biased hedge monitor missing primary order");
    };
    let Some(primary_order) = repo.get_trade_builder_order(primary_order_id).await? else {
        anyhow::bail!("biased hedge monitor primary order not found");
    };
    let Some(position) = repo.get_trade_builder_parent_position(primary_order_id).await? else {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "skipped": true,
                "reason": "biased_hedge_monitor_no_position",
                "pair_session_id": pair_session_id,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    };
    if position.current_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "skipped": true,
                "reason": "biased_hedge_monitor_position_closed",
                "pair_session_id": pair_session_id,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    let mut monitor_node = node.clone();
    if let Some(map) = monitor_node.config.as_object_mut() {
        map.insert("maxPriceCent".to_string(), json!(99.0));
        map.insert("executionFloorGuardEnabled".to_string(), json!(false));
        map.insert("triggerPriceGuardEnabled".to_string(), json!(false));
        map.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(false));
    }
    let eval = evaluate_action_place_order_pair_lock_primary_candidate(
        Some(crate::trade_flow::guards::price_to_beat::PriceToBeatGuardRuntimeContext::pair_lock_auto_primary(
            repo,
            run.user_id,
            cfg,
            Some(client),
        )),
        ws,
        client,
        run,
        step,
        &monitor_node,
        context,
        &primary_order.market_slug,
        &primary_order.token_id,
        &primary_order.outcome_label,
    )
    .await?;
    let candidate = pair_lock_edge_candidate_from_eval(eval, 0.0);
    let iv_payload = biased_hedge_iv_payload(&candidate);
    let q_final = candidate.q;
    let edge = candidate.edge;
    let depth_ok = biased_hedge_depth_ok(&candidate);
    let binance_same_direction = biased_hedge_binance_same_direction(&candidate);
    let bias_invalidated =
        biased_hedge_bias_invalidated(q_final, edge, depth_ok, binance_same_direction, &config);
    let timing = biased_hedge_market_timing(&primary_order.market_slug, Utc::now());
    let payload = json!({
        "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
        "phase": if bias_invalidated { "EXITING" } else { "PRIMARY_FILLED" },
        "decision": if bias_invalidated { "exit" } else { "hold" },
        "exit_reason": if bias_invalidated { Some("bias_invalidated") } else { None },
        "pair_session_id": pair_session_id,
        "primary_trade_id": primary_order_id,
        "q_final": q_final,
        "edge_adjusted": edge,
        "cost": candidate.cost,
        "gap_strength": iv_payload.and_then(|payload| payload.get("gap_strength")).and_then(value_as_f64),
        "depth_guard_result": iv_payload.and_then(|payload| payload.get("depth_guard_result")).and_then(Value::as_str),
        "binance_same_direction": binance_same_direction,
        "bias_invalidated": bias_invalidated,
        "net_exposure_side": primary_order.outcome_label,
        "net_exposure_notional_usdc": position.current_qty * position.last_fill_price.unwrap_or(primary_order.last_seen_price.unwrap_or(0.5)),
        "redeem_expected": !bias_invalidated,
        "timing": biased_hedge_timing_payload(timing),
        "selected_iv_time_rule": biased_hedge_selected_time_rule(&candidate),
    });
    repo.append_trade_builder_order_event(primary_order_id, "biased_hedge_monitor_evaluated", &payload)
        .await?;
    if bias_invalidated {
        return execute_biased_hedge_pair_exit(
            repo,
            &session,
            "biased_hedge_bias_invalidated",
            payload,
        )
        .await;
    }
    if timing.remaining_sec.is_some_and(|remaining| remaining > 0) {
        enqueue_biased_hedge_monitor(
            repo,
            run.id,
            session.flow_node_key.as_deref().unwrap_or(&node.key),
            session.id,
            primary_order_id,
            Utc::now() + ChronoDuration::seconds(BIASED_HEDGE_MONITOR_INTERVAL_SEC),
        )
        .await?;
    }
    Ok(TradeFlowNodeExecution {
        output: payload,
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
