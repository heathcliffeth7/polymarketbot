fn avg_rebound_binding_trigger_key(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
) -> Result<String> {
    let trigger_key = find_upstream_market_price_trigger_key(&node.key, graph).ok_or_else(|| {
        anyhow::anyhow!(
            "avg_rebound_pairlock_rescue_v1 requires upstream trigger.market_price bindingMode=avg_rebound_pairlock_rescue_only"
        )
    })?;
    let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
        anyhow::anyhow!("avg_rebound_pairlock_rescue_v1 upstream trigger node not found")
    })?;
    let binding_mode = node_config_string(trigger_node, "bindingMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        binding_mode == AVG_REBOUND_PAIRLOCK_RESCUE_BINDING_MODE,
        "avg_rebound_pairlock_rescue_v1 requires upstream trigger.market_price bindingMode=avg_rebound_pairlock_rescue_only"
    );
    Ok(trigger_key)
}

fn avg_rebound_market_slug(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<String> {
    step_input_string(step, &["marketSlug", "market_slug", "wsMarketSlug"])
        .or_else(|| flow_context_string(context, "marketSlug"))
        .or_else(|| node_config_string(node, "marketSlug"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("avg_rebound_pairlock_rescue_v1 requires marketSlug"))
}

fn avg_rebound_output_skipped(
    node: &TradeFlowNode,
    market_slug: &str,
    reason: &str,
    details: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
            "market_slug": market_slug,
            "skipped": true,
            "reason": reason,
            "details": details,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}

async fn avg_rebound_resolved_quote(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    step: &TradeFlowRunStep,
    token_id: &str,
    outcome_label: &str,
) -> PairLockResolvedQuote {
    resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        token_id,
        outcome_label,
        step_input_f64(step, &["currentPrice", "price", "wsPrice"]),
    )
    .await
}

async fn avg_rebound_order_book(
    client: &dyn OrderExecutor,
    token_id: &str,
) -> Result<Option<OrderBookSnapshot>> {
    client.order_book(token_id).await
}

fn avg_rebound_child_node(
    node: &TradeFlowNode,
    market_slug: &str,
    decision: &AvgReboundDecision,
) -> TradeFlowNode {
    let mut config = node.config.as_object().cloned().unwrap_or_default();
    config.insert("mode".to_string(), json!(ACTION_PLACE_ORDER_MODE_SINGLE));
    config.insert("kind".to_string(), json!("immediate"));
    config.insert("side".to_string(), json!("buy"));
    config.insert("executionMode".to_string(), json!("limit"));
    config.insert("orderType".to_string(), json!("FOK"));
    config.insert("postOnly".to_string(), json!(false));
    config.insert("sizeMode".to_string(), json!("shares"));
    config.insert(
        "targetQty".to_string(),
        json!(avg_rebound_decimal_to_f64(decision.qty)),
    );
    config.insert(
        "triggerPrice".to_string(),
        json!(avg_rebound_decimal_to_f64(decision.vwap)),
    );
    config.insert(
        "sizeUsdc".to_string(),
        json!(avg_rebound_decimal_to_f64(decision.notional)),
    );
    config.insert(
        "targetNotionalUsdc".to_string(),
        json!(avg_rebound_decimal_to_f64(decision.notional)),
    );
    config.insert("marketSlug".to_string(), json!(market_slug));
    config.insert("tokenId".to_string(), json!(&decision.token_id));
    config.insert("outcomeLabel".to_string(), json!(&decision.outcome_label));
    config.insert(
        "maxPriceCent".to_string(),
        json!(avg_rebound_decimal_to_f64(
            decision.limit_price * avg_rebound_dec("100")
        )),
    );
    config.insert("minPriceDistanceCent".to_string(), json!(0.1));
    config.insert("tpEnabled".to_string(), json!(false));
    config.insert("slEnabled".to_string(), json!(false));
    config.insert("ptbStopLossEnabled".to_string(), json!(false));
    config.insert("tpRules".to_string(), json!([]));
    config.insert("slRules".to_string(), json!([]));
    config.insert("ptbStopLossRules".to_string(), json!([]));
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_ORDER_MARKER_KEY.to_string(),
        json!(true),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_ROOT_NODE_KEY.to_string(),
        json!(&node.key),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_SESSION_ID_KEY.to_string(),
        json!(decision.session_id),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_ROLE_KEY.to_string(),
        json!(decision.leg_role),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_INTENT_KEY.to_string(),
        json!(decision.intent),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_STAGE_ID_KEY.to_string(),
        json!(decision.stage_id),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_TIER_OR_LEG_ID_KEY.to_string(),
        json!(&decision.tier_or_leg_id),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_DECISION_ID_KEY.to_string(),
        json!(&decision.decision_id),
    );
    config.insert(
        AVG_REBOUND_PAIRLOCK_RESCUE_REQUESTED_QTY_KEY.to_string(),
        json!(decision.qty.to_string()),
    );
    config.insert(
        "avgReboundPairlockRescueDiagnostics".to_string(),
        json!({
            "decision": decision.diagnostics,
            "vwap": avg_rebound_vwap_quote_json(&decision.vwap_quote),
        }),
    );
    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_avg_rebound_pairlock_rescue(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let market_slug = avg_rebound_market_slug(node, step, context)?;
    let trigger_key = avg_rebound_binding_trigger_key(node, graph)?;
    let Some(client) = client else {
        return Ok(avg_rebound_output_skipped(
            node,
            &market_slug,
            "missing_order_executor",
            json!({}),
        ));
    };
    let config = resolve_avg_rebound_pairlock_rescue_config(node)?;
    let Some(lock) = repo
        .try_acquire_trade_builder_avg_rebound_pairlock_rescue_lock(
            run.user_id,
            run.definition_id,
            &node.key,
            &market_slug,
        )
        .await?
    else {
        return Ok(avg_rebound_output_skipped(
            node,
            &market_slug,
            "execution_lock_busy",
            json!({}),
        ));
    };

    let execution = async {
        if repo
            .has_active_trade_builder_avg_rebound_pairlock_rescue_order(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
            )
            .await?
        {
            return Ok(avg_rebound_output_skipped(
                node,
                &market_slug,
                "active_order_exists",
                json!({}),
            ));
        }

        let resolved_tokens =
            resolve_trade_builder_pair_lock_yes_no_tokens(cfg, &market_slug, &trigger_key, context)
                .await?;
        let active_session = repo
            .load_active_trade_builder_avg_rebound_pairlock_rescue_session(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
            )
            .await?;
        let mut selection_rejections = Vec::new();
        let (session, tokens, primary_book, opposite_book) = if let Some(session) = active_session {
            let tokens = avg_rebound_token_resolution_from_session(&session);
            let Some(primary_book) =
                avg_rebound_order_book(client, &tokens.primary_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "primary_order_book_unavailable",
                    json!({ "token_id": tokens.primary_token_id }),
                ));
            };
            let Some(opposite_book) =
                avg_rebound_order_book(client, &tokens.opposite_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "opposite_order_book_unavailable",
                    json!({ "token_id": tokens.opposite_token_id }),
                ));
            };
            (session, tokens, primary_book, opposite_book)
        } else if avg_rebound_primary_outcome_is_auto(&config) {
            let (yes_label, no_label) = pair_lock_monitor_outcome_labels(Some(&market_slug));
            let Some(yes_book) = avg_rebound_order_book(client, &resolved_tokens.yes_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "primary_order_book_unavailable",
                    json!({
                        "token_id": resolved_tokens.yes_token_id,
                        "outcome_label": yes_label,
                        "selection": &config.primary_side_selection,
                    }),
                ));
            };
            let Some(no_book) = avg_rebound_order_book(client, &resolved_tokens.no_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "primary_order_book_unavailable",
                    json!({
                        "token_id": resolved_tokens.no_token_id,
                        "outcome_label": no_label,
                        "selection": &config.primary_side_selection,
                    }),
                ));
            };
            let seed_state = AvgReboundRuntimeState::default();
            let selection = match avg_rebound_select_cheapest_primary(
                &config,
                &seed_state,
                &resolved_tokens,
                &market_slug,
                yes_book,
                no_book,
            ) {
                Ok(selection) => selection,
                Err(rejections) => {
                    return Ok(avg_rebound_output_skipped(
                        node,
                        &market_slug,
                        "no_eligible_avg_rebound_action",
                        json!({
                            "selection": &config.primary_side_selection,
                            "rejections": rejections,
                        }),
                    ));
                }
            };
            selection_rejections = selection.rejections;
            let session = repo
                .get_or_create_trade_builder_avg_rebound_pairlock_rescue_session(
                    &bot_infra::db::TradeBuilderAvgReboundPairlockRescueSessionInput {
                        user_id: run.user_id,
                        flow_definition_id: Some(run.definition_id),
                        flow_run_id: Some(run.id),
                        root_flow_node_key: node.key.clone(),
                        market_slug: market_slug.clone(),
                        primary_token_id: selection.tokens.primary_token_id.clone(),
                        primary_outcome_label: selection.tokens.primary_outcome_label.clone(),
                        opposite_token_id: selection.tokens.opposite_token_id.clone(),
                        opposite_outcome_label: selection.tokens.opposite_outcome_label.clone(),
                        payload_json: json!({
                            "config": node.config.get(AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG_KEY).cloned().unwrap_or(Value::Null),
                            "primary_side_selection": &config.primary_side_selection,
                            "selected_primary_outcome_label": selection.tokens.primary_outcome_label.clone(),
                            "resolved_yes_token_id": resolved_tokens.yes_token_id,
                            "resolved_no_token_id": resolved_tokens.no_token_id,
                            "token_resolution_source": resolved_tokens.token_resolution_source,
                            "trigger_node_market_slug": resolved_tokens.trigger_node_market_slug,
                        }),
                    },
                )
                .await?;
            (
                session,
                selection.tokens,
                selection.primary_book,
                selection.opposite_book,
            )
        } else {
            let tokens =
                avg_rebound_resolve_tokens(client, &resolved_tokens, &market_slug, &config).await?;
            let session = repo
                .get_or_create_trade_builder_avg_rebound_pairlock_rescue_session(
                    &bot_infra::db::TradeBuilderAvgReboundPairlockRescueSessionInput {
                        user_id: run.user_id,
                        flow_definition_id: Some(run.definition_id),
                        flow_run_id: Some(run.id),
                        root_flow_node_key: node.key.clone(),
                        market_slug: market_slug.clone(),
                        primary_token_id: tokens.primary_token_id.clone(),
                        primary_outcome_label: tokens.primary_outcome_label.clone(),
                        opposite_token_id: tokens.opposite_token_id.clone(),
                        opposite_outcome_label: tokens.opposite_outcome_label.clone(),
                        payload_json: json!({
                            "config": node.config.get(AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG_KEY).cloned().unwrap_or(Value::Null),
                            "primary_side_selection": "explicit",
                            "resolved_yes_token_id": resolved_tokens.yes_token_id,
                            "resolved_no_token_id": resolved_tokens.no_token_id,
                            "token_resolution_source": resolved_tokens.token_resolution_source,
                            "trigger_node_market_slug": resolved_tokens.trigger_node_market_slug,
                        }),
                    },
                )
                .await?;
            let Some(primary_book) =
                avg_rebound_order_book(client, &tokens.primary_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "primary_order_book_unavailable",
                    json!({ "token_id": tokens.primary_token_id }),
                ));
            };
            let Some(opposite_book) =
                avg_rebound_order_book(client, &tokens.opposite_token_id).await?
            else {
                return Ok(avg_rebound_output_skipped(
                    node,
                    &market_slug,
                    "opposite_order_book_unavailable",
                    json!({ "token_id": tokens.opposite_token_id }),
                ));
            };
            (session, tokens, primary_book, opposite_book)
        };
        let state = avg_rebound_state_from_db(
            repo.load_trade_builder_avg_rebound_pairlock_rescue_state(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
            )
            .await?,
        );
        if matches!(
            state.session_status.as_deref(),
            Some(AVG_REBOUND_STATUS_LOCKED | "CLOSED")
        ) {
            return Ok(avg_rebound_output_skipped(
                node,
                &market_slug,
                "session_terminal",
                json!({ "session_id": state.session_id }),
            ));
        }
        if state.primary_total_qty > rust_decimal::Decimal::ZERO
            && state.open_primary_qty <= avg_rebound_dec("0.0001")
        {
            repo.mark_trade_builder_avg_rebound_pairlock_rescue_session_status(
                session.id,
                AVG_REBOUND_STATUS_LOCKED,
            )
            .await?;
            return Ok(avg_rebound_output_skipped(
                node,
                &market_slug,
                "session_locked",
                json!({ "session_id": session.id }),
            ));
        }
        let opposite_quote = avg_rebound_resolved_quote(
            ws,
            client,
            step,
            &tokens.opposite_token_id,
            &tokens.opposite_outcome_label,
        )
        .await;
        let (decision, mut rejections) = avg_rebound_select_decision(
            &config,
            &state,
            &tokens,
            &primary_book,
            &opposite_quote,
            &opposite_book,
        );
        rejections.splice(0..0, selection_rejections);
        let Some(decision) = decision else {
            return Ok(avg_rebound_output_skipped(
                node,
                &market_slug,
                "no_eligible_avg_rebound_action",
                json!({
                    "session_id": session.id,
                    "state": {
                        "primary_total_qty": state.primary_total_qty.to_string(),
                        "primary_total_cost": state.primary_total_cost.to_string(),
                        "avg_primary_cost": state.avg_primary_cost.map(|value| value.to_string()),
                        "opposite_filled_qty": state.opposite_filled_qty.to_string(),
                        "open_primary_qty": state.open_primary_qty.to_string(),
                        "locked_pnl": state.locked_pnl.to_string(),
                        "profit_started": state.profit_started,
                        "primary_tier_ids": state.primary_tier_ids,
                        "opposite_leg_ids": state.opposite_leg_ids,
                    },
                    "rejections": rejections,
                }),
            ));
        };
        if repo
            .has_trade_builder_avg_rebound_pairlock_rescue_decision_order(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
                &decision.decision_id,
            )
            .await?
        {
            return Ok(avg_rebound_output_skipped(
                node,
                &market_slug,
                "duplicate_decision_order",
                json!({
                    "session_id": session.id,
                    "decision_id": decision.decision_id,
                }),
            ));
        }

        repo.mark_trade_builder_avg_rebound_pairlock_rescue_session_status(
            session.id,
            decision.session_status,
        )
        .await?;
        let child_node = avg_rebound_child_node(node, &market_slug, &decision);
        let mut child_execution = execute_action_place_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            Some(client),
            run,
            step,
            &child_node,
            graph,
            context,
        )
        .await?;
        if let Some(output) = child_execution.output.as_object_mut() {
            output.insert(
                "avg_rebound_pairlock_rescue".to_string(),
                json!({
                    "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
                    "session_id": decision.session_id,
                    "market_slug": market_slug,
                    "role": decision.leg_role,
                    "intent": decision.intent,
                    "stage_id": decision.stage_id,
                    "tier_or_leg_id": decision.tier_or_leg_id,
                    "decision_id": decision.decision_id,
                    "qty": decision.qty.to_string(),
                    "limit_price": decision.limit_price.to_string(),
                    "vwap": decision.vwap.to_string(),
                    "notional": decision.notional.to_string(),
                    "diagnostics": decision.diagnostics,
                }),
            );
        }
        if let Some(builder_order_id) = child_execution
            .output
            .get("builder_order_id")
            .and_then(Value::as_i64)
        {
            repo.append_trade_builder_order_event(
                builder_order_id,
                "avg_rebound_pairlock_rescue_decision",
                &json!({
                    "mode": ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1,
                    "session_id": session.id,
                    "market_slug": market_slug,
                    "decision": child_execution.output.get("avg_rebound_pairlock_rescue"),
                }),
            )
            .await?;
        }
        Ok(child_execution)
    }
    .await;
    lock.release().await;
    execution
}

fn avg_rebound_marker_config(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/node_snapshot/action_node/config")
        .filter(|config| {
            config
                .get(AVG_REBOUND_PAIRLOCK_RESCUE_ORDER_MARKER_KEY)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

fn avg_rebound_marker_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn avg_rebound_marker_i64(config: &Value, key: &str) -> Option<i64> {
    config.get(key).and_then(value_as_i64)
}

async fn maybe_record_avg_rebound_pairlock_rescue_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    flow_created_payload: Option<&Value>,
    fill_qty: f64,
    execution_price: f64,
) -> Result<()> {
    let Some(marker_config) = flow_created_payload.and_then(avg_rebound_marker_config) else {
        return Ok(());
    };
    let Some(session_id) =
        avg_rebound_marker_i64(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_SESSION_ID_KEY)
    else {
        return Ok(());
    };
    let root_node_key =
        avg_rebound_marker_string(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_ROOT_NODE_KEY)
            .unwrap_or_else(|| {
                order
                    .origin_flow_node_key
                    .clone()
                    .unwrap_or_else(|| order.market_slug.clone())
            });
    let leg_role = avg_rebound_marker_string(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_ROLE_KEY)
        .unwrap_or_else(|| "primary".to_string());
    let intent = avg_rebound_marker_string(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_INTENT_KEY)
        .unwrap_or_else(|| AVG_REBOUND_INTENT_PRIMARY_LADDER.to_string());
    let tier_or_leg_id = avg_rebound_marker_string(
        marker_config,
        AVG_REBOUND_PAIRLOCK_RESCUE_TIER_OR_LEG_ID_KEY,
    )
    .unwrap_or_else(|| intent.clone());
    let decision_id =
        avg_rebound_marker_string(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_DECISION_ID_KEY)
            .unwrap_or_else(|| format!("{}:{}:{}", session_id, intent, tier_or_leg_id));
    let stage_id =
        avg_rebound_marker_string(marker_config, AVG_REBOUND_PAIRLOCK_RESCUE_STAGE_ID_KEY);
    let inserted = repo
        .record_trade_builder_avg_rebound_pairlock_rescue_fill(
            &bot_infra::db::TradeBuilderAvgReboundPairlockRescueFillInput {
                session_id,
                user_id: order.user_id,
                flow_definition_id: order.origin_flow_definition_id,
                flow_run_id: order.origin_flow_run_id,
                root_flow_node_key: root_node_key.clone(),
                market_slug: order.market_slug.clone(),
                token_id: order.token_id.clone(),
                outcome_label: order.outcome_label.clone(),
                leg_role: leg_role.clone(),
                intent: intent.clone(),
                stage_id: stage_id.clone(),
                tier_or_leg_id: tier_or_leg_id.clone(),
                decision_id: decision_id.clone(),
                order_side: order.side.clone(),
                builder_order_id: order.id,
                quantity: fill_qty.max(0.0),
                execution_price: clamp_probability(execution_price.max(0.0)),
                notional_usdc: (fill_qty.max(0.0) * execution_price.max(0.0)).max(0.0),
                payload_json: json!({
                    "flow_created": flow_created_payload,
                    "session_id": session_id,
                    "role": leg_role,
                    "intent": intent,
                    "stage_id": stage_id,
                    "tier_or_leg_id": tier_or_leg_id,
                    "decision_id": decision_id,
                }),
            },
        )
        .await?;
    if !inserted {
        return Ok(());
    }
    let state = repo
        .load_trade_builder_avg_rebound_pairlock_rescue_state(
            order.user_id,
            order.origin_flow_definition_id,
            &root_node_key,
            &order.market_slug,
        )
        .await?;
    let next_status =
        if state.primary_total_qty > 0.0 && state.open_primary_qty <= AVG_REBOUND_QTY_EPSILON {
            AVG_REBOUND_STATUS_LOCKED
        } else {
            match intent.as_str() {
                AVG_REBOUND_INTENT_PROFIT_PAIRLOCK => AVG_REBOUND_STATUS_PROFIT_LOCKING,
                AVG_REBOUND_INTENT_GIVEBACK_GUARD => AVG_REBOUND_STATUS_GUARD_EXIT,
                AVG_REBOUND_INTENT_NORMAL_RESCUE
                | AVG_REBOUND_INTENT_EMERGENCY_RESCUE
                | AVG_REBOUND_INTENT_HARD_RESCUE
                | AVG_REBOUND_INTENT_LAST_CHANCE_RESCUE => AVG_REBOUND_STATUS_RESCUE_EXIT,
                _ => AVG_REBOUND_STATUS_BUILDING_PRIMARY,
            }
        };
    repo.mark_trade_builder_avg_rebound_pairlock_rescue_session_status(session_id, next_status)
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "avg_rebound_pairlock_rescue_fill_recorded",
        &json!({
            "session_id": session_id,
            "status": next_status,
            "primary_total_qty": state.primary_total_qty,
            "primary_total_cost": state.primary_total_cost,
            "avg_primary_cost": state.avg_primary_cost,
            "opposite_filled_qty": state.opposite_filled_qty,
            "open_primary_qty": state.open_primary_qty,
            "locked_pnl": state.locked_pnl,
        }),
    )
    .await?;
    Ok(())
}
