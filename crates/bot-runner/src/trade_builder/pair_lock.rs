const ACTION_PLACE_ORDER_MODE_SINGLE: &str = "single";
const ACTION_PLACE_ORDER_MODE_PAIR_LOCK: &str = "pair_lock";
const TRADE_BUILDER_PAIR_STATUS_WORKING: &str = "working";
const TRADE_BUILDER_PAIR_STATUS_LOCKED: &str = "locked";
const TRADE_BUILDER_PAIR_STATUS_UNWINDING: &str = "unwinding";
const TRADE_BUILDER_PAIR_STATUS_COMPLETED: &str = "completed";
const TRADE_BUILDER_PAIR_STATUS_EXPIRED: &str = "expired";
const TRADE_BUILDER_PAIR_STATUS_ERROR: &str = "error";
const TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE: &str = "lead_candidate";
const TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE: &str = "counter_candidate";
const TRADE_BUILDER_PAIR_ROLE_ORPHAN_UNWIND_SELL: &str = "orphan_unwind_sell";
const DEFAULT_PAIR_ORPHAN_GRACE_MS: i64 = 1_500;
const TRADE_BUILDER_PAIR_QTY_TOLERANCE: f64 = 0.0001;

#[derive(Debug, Clone, Copy)]
struct ActionPlaceOrderPairLockConfig {
    max_total_price: f64,
    orphan_grace_ms: i64,
    protective_unwind_enabled: bool,
    ignore_stop_loss_after_locked: bool,
    notify_on_pair_locked: bool,
    notify_on_pair_unwind: bool,
    sizing_mode: ActionPlaceOrderPairLockSizingMode,
    primary_leg_size_usdc: f64,
    total_budget_usdc: Option<f64>,
    counter_leg_size_usdc: Option<f64>,
}

#[derive(Debug, Clone)]
struct ActionPlaceOrderPairResolvedCounterLeg {
    token_id: String,
    outcome_label: String,
}

fn action_place_order_mode(node: &TradeFlowNode) -> &'static str {
    match node_config_string(node, "mode")
        .unwrap_or_else(|| ACTION_PLACE_ORDER_MODE_SINGLE.to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        ACTION_PLACE_ORDER_MODE_PAIR_LOCK => ACTION_PLACE_ORDER_MODE_PAIR_LOCK,
        _ => ACTION_PLACE_ORDER_MODE_SINGLE,
    }
}

fn action_place_order_uses_pair_lock(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_PAIR_LOCK
}

fn trade_builder_order_uses_pair_lock(order: &TradeBuilderOrder) -> bool {
    order.pair_session_id.is_some()
        && matches!(
            order.pair_leg_role.as_deref(),
            Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE)
                | Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE)
                | Some(TRADE_BUILDER_PAIR_ROLE_ORPHAN_UNWIND_SELL)
        )
}

fn trade_builder_pair_lock_is_candidate_order(order: &TradeBuilderOrder) -> bool {
    matches!(
        order.pair_leg_role.as_deref(),
        Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE)
            | Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE)
    )
}

fn trade_builder_pair_lock_is_unwind_order(order: &TradeBuilderOrder) -> bool {
    matches!(
        order.pair_leg_role.as_deref(),
        Some(TRADE_BUILDER_PAIR_ROLE_ORPHAN_UNWIND_SELL)
    )
}

fn resolve_action_place_order_pair_lock_config(
    node: &TradeFlowNode,
) -> Result<Option<ActionPlaceOrderPairLockConfig>> {
    if !action_place_order_uses_pair_lock(node) {
        return Ok(None);
    }

    let strategy = resolve_action_place_order_pair_lock_strategy(node)?;
    let uses_edge_strategy = strategy == PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1;
    let uses_biased_hedge_strategy = strategy == PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1;
    if strategy == PAIR_LOCK_STRATEGY_ADAPTIVE_MAX_PRICE_V1 {
        resolve_pair_lock_adaptive_max_price_config(node)?;
    }
    if strategy == PAIR_LOCK_STRATEGY_MANUAL_ADAPTIVE_RISK_V1 {
        resolve_pair_lock_manual_adaptive_risk_config(node)?;
    }
    let pair_max_total_cent = node_config_f64(node, "pairMaxTotalCent")
        .or_else(|| node_config_f64(node, "pairTargetTotalCent"))
        .unwrap_or(if uses_biased_hedge_strategy { 99.0 } else { f64::NAN });
    anyhow::ensure!(
        pair_max_total_cent.is_finite(),
        "action.place_order pair_lock requires pairMaxTotalCent"
    );
    anyhow::ensure!(
        pair_max_total_cent > 0.0 && pair_max_total_cent < 100.0,
        "action.place_order pairMaxTotalCent must be in (0, 100)"
    );

    let orphan_grace_ms = node_config_i64(node, "pairOrphanGraceMs")
        .unwrap_or(DEFAULT_PAIR_ORPHAN_GRACE_MS)
        .max(0);
    let primary_leg_size_usdc = resolve_action_place_order_pair_lock_primary_budget_usdc(node)
        .ok_or_else(|| anyhow::anyhow!("action.place_order pair_lock requires sizeUsdc > 0"))?;
    let sizing_mode = resolve_action_place_order_pair_lock_sizing_mode(node);
    let (total_budget_usdc, counter_leg_size_usdc) = match sizing_mode {
        ActionPlaceOrderPairLockSizingMode::Manual => {
            let counter_leg_size_usdc = node_config_f64(node, "counterLegSizeUsdc");
            if !uses_edge_strategy && !uses_biased_hedge_strategy {
                let counter_leg_size_usdc = counter_leg_size_usdc
                    .ok_or_else(|| anyhow::anyhow!("action.place_order pair_lock requires counterLegSizeUsdc > 0"))?;
                anyhow::ensure!(
                    counter_leg_size_usdc > 0.0,
                    "action.place_order counterLegSizeUsdc must be > 0"
                );
                (None, Some(counter_leg_size_usdc))
            } else if uses_biased_hedge_strategy {
                let hedge_budget = resolve_action_place_order_biased_hedge_config(node)?
                    .map(|config| config.hedge_budget_usdc);
                (None, hedge_budget)
            } else {
                (None, counter_leg_size_usdc.filter(|value| *value > 0.0))
            }
        }
        ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget => {
            let total_budget_usdc = node_config_f64(node, "pairTotalBudgetUsdc")
                .ok_or_else(|| anyhow::anyhow!("action.place_order pair_lock requires pairTotalBudgetUsdc > 0"))?;
            anyhow::ensure!(
                total_budget_usdc > primary_leg_size_usdc,
                "action.place_order pairTotalBudgetUsdc must be greater than sizeUsdc"
            );
            (
                Some(total_budget_usdc),
                trade_builder_pair_lock_initial_counter_budget(total_budget_usdc, primary_leg_size_usdc),
            )
        }
    };

    Ok(Some(ActionPlaceOrderPairLockConfig {
        max_total_price: clamp_probability(pair_max_total_cent / 100.0),
        orphan_grace_ms,
        protective_unwind_enabled: node_config_bool(node, "pairProtectiveUnwindEnabled").unwrap_or(true),
        ignore_stop_loss_after_locked: node_config_bool(node, "pairIgnoreStopLossAfterLocked").unwrap_or(false),
        notify_on_pair_locked: node_config_bool(node, "notifyOnPairLocked").unwrap_or(false),
        notify_on_pair_unwind: node_config_bool(node, "notifyOnPairUnwind").unwrap_or(false),
        sizing_mode,
        primary_leg_size_usdc,
        total_budget_usdc,
        counter_leg_size_usdc,
    }))
}

fn normalize_pair_lock_binary_outcome(label: &str) -> Option<&'static str> {
    match label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "true" | "1" => Some("yes"),
        "no" | "down" | "false" | "0" => Some("no"),
        _ => None,
    }
}

fn resolve_pair_lock_opposite_outcome_label(label: &str) -> String {
    match normalize_pair_lock_binary_outcome(label) {
        Some("no") => {
            if label.trim().eq_ignore_ascii_case("down") {
                "Up".to_string()
            } else {
                "Yes".to_string()
            }
        }
        _ => {
            if label.trim().eq_ignore_ascii_case("up") {
                "Down".to_string()
            } else {
                "No".to_string()
            }
        }
    }
}

async fn resolve_trade_builder_pair_lock_yes_no_tokens(
    cfg: &AppConfig,
    market_slug: &str,
    trigger_node_key: &str,
    context: &Value,
) -> Result<PairLockResolvedTokenPair> {
    resolve_pair_lock_trigger_scoped_token_pair(cfg, market_slug, trigger_node_key, context).await
}

async fn resolve_action_place_order_pair_lock_counter_leg(
    node: &TradeFlowNode,
    resolved_tokens: &PairLockResolvedTokenPair,
    primary_token_id: &str,
    primary_outcome_label: &str,
) -> Result<ActionPlaceOrderPairResolvedCounterLeg> {
    let configured_outcome = node_config_string(node, "counterLegOutcomeLabel")
        .unwrap_or_else(|| "opposite".to_string());
    let outcome_label = if configured_outcome.trim().eq_ignore_ascii_case("opposite") {
        resolve_pair_lock_opposite_outcome_label(primary_outcome_label)
    } else {
        configured_outcome
    };
    let normalized_counter_outcome = normalize_pair_lock_binary_outcome(&outcome_label)
        .ok_or_else(|| anyhow::anyhow!("pair_lock counter outcome must resolve to yes/up or no/down"))?;
    let token_id = match normalized_counter_outcome {
        "yes" => Some(resolved_tokens.yes_token_id.clone()),
        "no" => Some(resolved_tokens.no_token_id.clone()),
        _ => None,
    }
    .ok_or_else(|| anyhow::anyhow!("pair_lock could not resolve counter leg token id"))?;
    anyhow::ensure!(
        !token_id.eq_ignore_ascii_case(primary_token_id),
        "pair_lock counter leg resolved to the same token as the primary leg"
    );

    Ok(ActionPlaceOrderPairResolvedCounterLeg {
        token_id,
        outcome_label,
    })
}

fn action_place_order_pair_lock_ref_key(node: &TradeFlowNode) -> String {
    node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone())
}

fn trigger_market_price_binding_mode(node: &TradeFlowNode) -> &'static str {
    match node_config_string(node, "bindingMode")
        .unwrap_or_else(|| "standard".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "pair_lock_only" => "pair_lock_only",
        _ => "standard",
    }
}

fn resolve_pair_lock_direct_trigger_node_key(
    node_key: &str,
    graph: &TradeFlowGraphRuntime,
) -> Result<String> {
    let incoming_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.target == node_key)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        incoming_edges.len() == 1,
        "action.place_order pair_lock requires exactly one direct upstream trigger.market_price"
    );
    let trigger_key = incoming_edges[0].source.clone();
    let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
        anyhow::anyhow!("action.place_order pair_lock upstream node missing: {trigger_key}")
    })?;
    anyhow::ensure!(
        trigger_node.node_type == "trigger.market_price",
        "action.place_order pair_lock only supports a direct upstream trigger.market_price"
    );
    anyhow::ensure!(
        trigger_market_price_binding_mode(trigger_node) == "pair_lock_only",
        "action.place_order pair_lock requires upstream trigger.market_price bindingMode=pair_lock_only"
    );
    Ok(trigger_key)
}

async fn execute_action_place_order_pair_lock(
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
    let client = client.ok_or_else(|| anyhow::anyhow!("pair_lock requires an order executor"))?;
    let pair_lock = resolve_action_place_order_pair_lock_config(node)?
        .ok_or_else(|| anyhow::anyhow!("pair_lock config missing"))?;
    let trigger_node_key = resolve_pair_lock_direct_trigger_node_key(&node.key, graph)?;
    if action_place_order_uses_edge_pairlock_strategy(node) {
        return execute_action_place_order_pair_lock_edge_strategy(
            repo, run_id, cfg, limits, policy, client, ws, run, step, node, graph, context,
            &pair_lock, &trigger_node_key,
        )
        .await;
    }
    if action_place_order_uses_biased_hedge_strategy(node) {
        return execute_action_place_order_pair_lock_biased_hedge_strategy(
            repo, run_id, cfg, limits, policy, client, ws, run, step, node, graph, context,
            &pair_lock, &trigger_node_key,
        )
        .await;
    }

    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("pair_lock requires marketSlug"))?;
    let explicit_primary_token_id = resolve_action_place_order_string(
        node,
        context,
        step,
        "tokenId",
        "tokenId",
        &["triggered_token_id", "tokenId"],
    );
    let explicit_primary_outcome_label = resolve_action_place_order_string(
        node,
        context,
        step,
        "outcomeLabel",
        "outcomeLabel",
        &["triggered_outcome_label", "outcomeLabel"],
    );
    let resolved_tokens =
        resolve_trade_builder_pair_lock_yes_no_tokens(cfg, &market_slug, &trigger_node_key, context)
            .await?;
    let effective_market_slug = resolved_tokens
        .trigger_node_market_slug
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(market_slug.as_str());
    promote_trigger_node_auto_scope_context_to_flow_context(
        context,
        &trigger_node_key,
        effective_market_slug,
    );
    let token_resolution_payload = pair_lock_token_resolution_payload(&resolved_tokens);
    let yes_token_id = Some(resolved_tokens.yes_token_id.clone());
    let no_token_id = Some(resolved_tokens.no_token_id.clone());
    let (
        primary_token_id,
        primary_outcome_label,
        primary_selection_mode,
        selected_primary_guard_reason,
        primary_selection_diagnostics,
        selected_candidate_quotes,
        adaptive_max_price_override,
        manual_adaptive_risk_override,
    ) =
        if let Some(primary_token_id) = explicit_primary_token_id {
            let primary_outcome_label = explicit_primary_outcome_label
                .unwrap_or_else(|| primary_token_id.clone());
            maybe_send_pair_lock_primary_guard_recovered_notification(
                repo,
                run,
                &node.key,
                &market_slug,
                context,
            )
            .await;
            let mut diagnostics = json!({ "selection_mode": "explicit" });
            append_json_object_fields(&mut diagnostics, &token_resolution_payload);
            maybe_emit_pair_lock_primary_recovered_event(repo, run, &node.key, &market_slug, "explicit", &primary_token_id, &primary_outcome_label, "explicit", &diagnostics, &token_resolution_payload, context).await?;
            clear_pair_lock_primary_waiting_state(context, &node.key);
            clear_pair_lock_primary_notification_state(context, &node.key);
            (
                primary_token_id,
                primary_outcome_label,
                "explicit".to_string(),
                "explicit".to_string(),
                diagnostics,
                None,
                None,
                None,
            )
        } else {
            let selection_attempt = resolve_action_place_order_pair_lock_primary_selection(
                Some(
                    crate::trade_flow::guards::price_to_beat::PriceToBeatGuardRuntimeContext::pair_lock_auto_primary(
                        repo,
                        run.user_id,
                        cfg,
                        Some(client),
                    ),
                ),
                Some((repo, &pair_lock)),
                ws,
                client,
                run,
                step,
                node,
                context,
                &market_slug,
                yes_token_id.clone(),
                no_token_id.clone(),
            )
            .await?;
            let mut selection_diagnostics = selection_attempt.diagnostics.clone();
            append_json_object_fields(&mut selection_diagnostics, &token_resolution_payload);
            if selection_attempt.waiting {
                maybe_send_pair_lock_primary_guard_notification(
                    repo,
                    run,
                    node,
                    &market_slug,
                    &selection_diagnostics,
                    context,
                )
                .await?;
                maybe_emit_pair_lock_primary_waiting_event(repo, run, &node.key, &market_slug, &selection_diagnostics, &token_resolution_payload, context).await?;
                set_pair_lock_primary_waiting_state(context, &node.key, &market_slug, &selection_diagnostics);
                return Ok(build_pair_lock_primary_waiting_execution(
                    &node.key,
                    &market_slug,
                    &selection_diagnostics,
                ));
            }
            let Some(selection) = selection_attempt.selection else {
                let failure_reason = selection_attempt
                    .failure_reason
                    .unwrap_or("pair_lock_primary_leg_selection_failed");
                clear_pair_lock_primary_waiting_state(context, &node.key);
                maybe_send_pair_lock_primary_guard_notification(
                    repo,
                    run,
                    node,
                    &market_slug,
                    &selection_diagnostics,
                    context,
                )
                .await?;
                repo.append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "pair_lock_primary_leg_selection_failed",
                    &json!({
                        "node_key": node.key,
                        "market_slug": market_slug,
                        "reason": failure_reason,
                        "selection_mode": "auto_guarded",
                        "selection": selection_diagnostics,
                        "resolved_yes_token_id": resolved_tokens.yes_token_id,
                        "resolved_no_token_id": resolved_tokens.no_token_id,
                        "token_resolution_source": resolved_tokens.token_resolution_source,
                        "trigger_node_market_slug": resolved_tokens.trigger_node_market_slug,
                    }),
                )
                .await?;
                anyhow::bail!("{failure_reason}");
            };
            maybe_send_pair_lock_primary_guard_recovered_notification(
                repo,
                run,
                &node.key,
                &market_slug,
                context,
            )
            .await;
            maybe_emit_pair_lock_primary_recovered_event(repo, run, &node.key, &market_slug, selection.selection_mode, &selection.token_id, &selection.outcome_label, &selection.guard_reason, &selection_diagnostics, &token_resolution_payload, context).await?;
            clear_pair_lock_primary_waiting_state(context, &node.key);
            clear_pair_lock_primary_notification_state(context, &node.key);
            (
                selection.token_id,
                selection.outcome_label,
                selection.selection_mode.to_string(),
                selection.guard_reason,
                selection_diagnostics,
                Some((
                    selection_attempt.yes_candidate.quote.clone(),
                    selection_attempt.no_candidate.quote.clone(),
                )),
                selection.adaptive_max_price_override,
                selection.manual_adaptive_risk_override,
            )
        };
    let counter = resolve_action_place_order_pair_lock_counter_leg(
        node,
        &resolved_tokens,
        &primary_token_id,
        &primary_outcome_label,
    )
    .await?;

    if let Some(override_payload) = adaptive_max_price_override.as_ref() {
        mark_pair_lock_adaptive_max_price_relaxed(
            context,
            &node.key,
            &market_slug,
            &primary_outcome_label,
        );
        maybe_notify_pair_lock_adaptive_relax_allowed(
            repo,
            run,
            node,
            context,
            &market_slug,
            &primary_outcome_label,
            &override_payload.diagnostics,
        )
        .await?;
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "pair_lock_adaptive_max_price_relax_applied",
            &json!({
                "node_key": node.key,
                "market_slug": market_slug,
                "selected_primary_token_id": primary_token_id,
                "selected_primary_outcome_label": primary_outcome_label,
                "adaptive_max_price": override_payload.diagnostics,
            }),
        )
        .await?;
    }

    if let Some(override_payload) = manual_adaptive_risk_override.as_ref() {
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "pair_lock_manual_adaptive_risk_applied",
            &json!({
                "node_key": node.key,
                "market_slug": market_slug,
                "selected_primary_token_id": primary_token_id,
                "selected_primary_outcome_label": primary_outcome_label,
                "manual_adaptive_risk": override_payload.diagnostics,
            }),
        )
        .await?;
    }

    let primary_node = build_pair_lock_single_leg_node(
        node,
        &market_slug,
        &primary_token_id,
        &primary_outcome_label,
        &trigger_node_key,
        adaptive_max_price_override.as_ref(),
        manual_adaptive_risk_override.as_ref(),
    );
    let counter_node = build_pair_lock_counter_leg_node(
        node,
        &market_slug,
        &counter,
        &pair_lock,
        &trigger_node_key,
        manual_adaptive_risk_override.as_ref(),
    );
    let primary_step = pair_lock_candidate_quote_for_token(
        &primary_token_id,
        &resolved_tokens,
        &selected_candidate_quotes,
    )
    .map(|quote| clone_pair_lock_step_with_quote(step, quote))
    .unwrap_or_else(|| step.clone());
    let counter_step = pair_lock_candidate_quote_for_token(
        &counter.token_id,
        &resolved_tokens,
        &selected_candidate_quotes,
    )
    .map(|quote| clone_pair_lock_step_with_quote(step, quote))
    .unwrap_or_else(|| step.clone());

    let mut primary_context = context.clone();
    let primary_execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &primary_step,
        &primary_node,
        graph,
        &mut primary_context,
    )
    .await?;
    let primary_order_id = extract_builder_order_id(&primary_execution)?;
    let primary_source_trade_id = extract_source_trade_id(&primary_execution);

    let mut counter_context = context.clone();
    let counter_execution = match execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &counter_step,
        &counter_node,
        graph,
        &mut counter_context,
    )
    .await
    {
        Ok(execution) => execution,
        Err(err) => {
            cancel_pair_lock_order_if_created(repo, Some(primary_order_id), "pair_lock_counter_create_failed").await;
            return Err(err);
        }
    };
    let counter_order_id = extract_builder_order_id(&counter_execution)?;
    let counter_source_trade_id = extract_source_trade_id(&counter_execution);

    let pair_session_id = repo
        .create_trade_builder_pair_session(
            run.user_id,
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
            &market_slug,
            pair_lock.max_total_price * 100.0,
            0.0,
            0.0,
            pair_lock.orphan_grace_ms,
            pair_lock.ignore_stop_loss_after_locked,
            pair_lock.notify_on_pair_locked,
            pair_lock.notify_on_pair_unwind,
            false,
        )
        .await?;
    repo.attach_trade_builder_pair_session_orders(pair_session_id, primary_order_id, counter_order_id)
        .await?;
    repo.set_trade_builder_order_pair_session(
        primary_order_id,
        Some(pair_session_id),
        Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE),
    )
    .await?;
    repo.set_trade_builder_order_pair_session(
        counter_order_id,
        Some(pair_session_id),
        Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE),
    )
    .await?;

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pair_lock_session_created",
        &json!({
            "node_key": node.key,
            "pair_session_id": pair_session_id,
            "market_slug": market_slug,
            "primary_order_id": primary_order_id,
            "counter_order_id": counter_order_id,
            "primary_source_trade_id": primary_source_trade_id,
            "counter_source_trade_id": counter_source_trade_id,
            "trigger_node_key": trigger_node_key,
            "binding_mode": "pair_lock_only",
            "pair_sizing_mode": match pair_lock.sizing_mode {
                ActionPlaceOrderPairLockSizingMode::Manual => "manual",
                ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget => "auto_remaining_budget",
            },
            "pair_max_total_cent": pair_lock.max_total_price * 100.0,
            "pair_target_total_cent": pair_lock.max_total_price * 100.0,
            "pair_total_budget_usdc": pair_lock.total_budget_usdc,
            "primary_leg_size_usdc": pair_lock.primary_leg_size_usdc,
            "counter_initial_size_usdc": pair_lock.counter_leg_size_usdc,
            "pair_orphan_grace_ms": pair_lock.orphan_grace_ms,
            "ignore_stop_loss_after_locked": pair_lock.ignore_stop_loss_after_locked,
            "notify_on_pair_locked": pair_lock.notify_on_pair_locked,
            "notify_on_pair_unwind": pair_lock.notify_on_pair_unwind,
            "counter_outcome_label": counter.outcome_label,
            "counter_token_id": counter.token_id,
            "primary_selection_mode": &primary_selection_mode,
            "selected_primary_token_id": &primary_token_id,
            "selected_primary_outcome_label": &primary_outcome_label,
            "selected_primary_guard_reason": &selected_primary_guard_reason,
            "primary_selection": primary_selection_diagnostics.clone(),
            "adaptive_max_price": adaptive_max_price_override.as_ref().map(|value| value.diagnostics.clone()),
            "manual_adaptive_risk": manual_adaptive_risk_override.as_ref().map(|value| value.diagnostics.clone()),
        }),
    )
    .await?;

    let ref_key = action_place_order_pair_lock_ref_key(node);
    bind_action_place_order_ref_bindings(context, node, &ref_key, primary_order_id);
    if let Some(source_trade_id) = primary_source_trade_id {
        set_flow_context(context, "sourceTradeId", json!(source_trade_id));
    }
    set_flow_var(
        context,
        &format!("{ref_key}_pair_session_id"),
        json!(pair_session_id),
    );
    set_flow_var(
        context,
        &format!("{ref_key}_counter_builder_order_id"),
        json!(counter_order_id),
    );
    if let Some(source_trade_id) = counter_source_trade_id {
        set_flow_var(
            context,
            &format!("{ref_key}_counter_source_trade_id"),
            json!(source_trade_id),
        );
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": primary_order_id,
            "counter_builder_order_id": counter_order_id,
            "pair_session_id": pair_session_id,
            "ref_key": ref_key,
            "market_slug": market_slug,
            "token_id": primary_token_id,
            "counter_token_id": counter.token_id,
            "outcome_label": primary_outcome_label,
            "counter_outcome_label": counter.outcome_label,
            "source_trade_id": primary_source_trade_id,
            "counter_source_trade_id": counter_source_trade_id,
            "trigger_node_key": trigger_node_key,
            "binding_mode": "pair_lock_only",
            "pair_sizing_mode": match pair_lock.sizing_mode {
                ActionPlaceOrderPairLockSizingMode::Manual => "manual",
                ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget => "auto_remaining_budget",
            },
            "pair_max_total_cent": pair_lock.max_total_price * 100.0,
            "pair_target_total_cent": pair_lock.max_total_price * 100.0,
            "pair_total_budget_usdc": pair_lock.total_budget_usdc,
            "primary_leg_size_usdc": pair_lock.primary_leg_size_usdc,
            "counter_initial_size_usdc": pair_lock.counter_leg_size_usdc,
            "pair_orphan_grace_ms": pair_lock.orphan_grace_ms,
            "notify_on_pair_locked": pair_lock.notify_on_pair_locked,
            "notify_on_pair_unwind": pair_lock.notify_on_pair_unwind,
            "primary_selection_mode": primary_selection_mode,
            "selected_primary_token_id": primary_token_id,
            "selected_primary_outcome_label": primary_outcome_label,
            "selected_primary_guard_reason": selected_primary_guard_reason,
            "primary_selection": primary_selection_diagnostics,
            "adaptive_max_price": adaptive_max_price_override.map(|value| value.diagnostics),
            "manual_adaptive_risk": manual_adaptive_risk_override.map(|value| value.diagnostics),
            "resolved_yes_token_id": resolved_tokens.yes_token_id,
            "resolved_no_token_id": resolved_tokens.no_token_id,
            "token_resolution_source": resolved_tokens.token_resolution_source,
            "trigger_node_market_slug": resolved_tokens.trigger_node_market_slug,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_dispatch(
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
    if action_place_order_uses_pair_lock(node) {
        let (
            side,
            execution_mode,
            trigger_node_key,
            explicit_primary_token_id,
            explicit_primary_outcome_label,
        ) = pair_lock_pre_dispatch_resolution(node, context, step, graph)?;
        if let Some(stale_execution) = maybe_skip_stale_action_place_order_step(
            repo,
            run,
            node,
            graph,
            step,
            context,
            side.as_str(),
            execution_mode.as_str(),
            explicit_primary_token_id.as_deref(),
            explicit_primary_outcome_label.as_deref(),
            Some(trigger_node_key.as_str()),
        )
        .await?
        {
            return Ok(stale_execution);
        }
        return execute_action_place_order_pair_lock(
            repo, run_id, cfg, limits, policy, client, ws, run, step, node, graph, context,
        )
        .await;
    }

    execute_action_place_order(
        repo, run_id, cfg, limits, policy, client, run, step, node, graph, context,
    )
    .await
}

fn trade_builder_pair_lock_effective_counter_cap(
    session: &TradeBuilderPairSession,
) -> Option<f64> {
    let lead_price = if session.lead_order_id == session.primary_order_id {
        session.primary_avg_fill_price
    } else if session.lead_order_id == session.counter_order_id {
        session.counter_avg_fill_price
    } else {
        None
    }?;
    let cap = session.pair_target_total_cent / 100.0 - lead_price;
    (cap > 0.0).then_some(clamp_probability(cap))
}

async fn resolve_trade_builder_pair_lock_node(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
) -> Result<Option<TradeFlowNode>> {
    let Some(flow_run_id) = session.flow_run_id else {
        return Ok(None);
    };
    let Some(flow_node_key) = session.flow_node_key.as_deref() else {
        return Ok(None);
    };
    let Some(flow_run) = repo.get_trade_flow_run(flow_run_id).await? else {
        return Ok(None);
    };
    let Some(version) = repo.get_trade_flow_version(flow_run.version_id).await? else {
        return Ok(None);
    };
    let graph = parse_trade_flow_graph(&version)?;
    Ok(flow_node(&graph, flow_node_key).cloned())
}

async fn maybe_apply_trade_builder_pair_lock_runtime(
    repo: &PostgresRepository,
    order: &mut TradeBuilderOrder,
    now: DateTime<Utc>,
) -> Result<bool> {
    if !trade_builder_order_uses_pair_lock(order)
        || !trade_builder_pair_lock_is_candidate_order(order)
        || order.side != "buy"
    {
        return Ok(false);
    }
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(false);
    };
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(false);
    };
    let Some(pair_lock) = resolve_trade_builder_pair_lock_session_config(repo, &session).await?
    else {
        return Ok(false);
    };

    if matches!(session.status.as_str(), TRADE_BUILDER_PAIR_STATUS_COMPLETED | TRADE_BUILDER_PAIR_STATUS_EXPIRED | TRADE_BUILDER_PAIR_STATUS_ERROR) {
        if is_trade_builder_order_processable_status(&order.status) {
            repo.set_trade_builder_order_status(order.id, "canceled", Some("pair_session_closed"))
                .await?;
            return Ok(true);
        }
        return Ok(false);
    }
    if let Some(node) = resolve_trade_builder_pair_lock_node(repo, &session).await? {
        if action_place_order_uses_biased_hedge_strategy(&node) {
            if let Some(config) = resolve_action_place_order_biased_hedge_config(&node)? {
                if maybe_prepare_biased_hedge_counter_runtime(repo, order, &session, &config, now)
                    .await?
                {
                    return Ok(true);
                }
            }
        }
    }
    if maybe_prepare_trade_builder_pair_lock_auto_counter(repo, order, &session, &pair_lock)
        .await?
    {
        return Ok(true);
    }

    if session.status == TRADE_BUILDER_PAIR_STATUS_WORKING {
        if let (Some(lead_order_id), Some(lead_filled_at)) = (session.lead_order_id, session.lead_filled_at) {
            if lead_order_id != order.id {
                if let Some(dynamic_cap) = trade_builder_pair_lock_effective_counter_cap(&session) {
                    order.max_price = Some(
                        order
                            .max_price
                            .map(|configured| configured.min(dynamic_cap))
                            .unwrap_or(dynamic_cap),
                    );
                }
                if pair_lock.protective_unwind_enabled
                    && !pair_lock_counter_waits_until_market_end(&pair_lock, &order.market_slug)
                    && now >= lead_filled_at + ChronoDuration::milliseconds(session.orphan_grace_ms)
                {
                    let orders = repo
                        .list_trade_builder_orders_by_pair_session(pair_session_id)
                        .await?;
                    schedule_trade_builder_pair_session_unwind(
                        repo,
                        &session,
                        &orders,
                        TRADE_BUILDER_PAIR_STATUS_UNWINDING,
                        "pair_orphan_grace_elapsed",
                        None,
                    )
                    .await?;
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

fn trade_builder_pair_lock_estimated_fee_qty(
    fill_qty: f64,
    fill_price: f64,
    fee_rate_bps: i64,
) -> f64 {
    estimate_trade_builder_buy_fee_shares(fill_price, fill_qty, trade_builder_fee_rate_bps_or_default(fee_rate_bps))
}

fn trade_builder_pair_lock_fill_net_qty(fill_qty: f64, fee_qty: f64) -> f64 {
    round_trade_builder_share_qty((fill_qty - fee_qty).max(0.0))
}

fn trade_builder_pair_lock_common_qty(session: &TradeBuilderPairSession) -> Option<f64> {
    let primary_net_qty = session.primary_net_qty?;
    let counter_net_qty = session.counter_net_qty?;
    let common_qty = round_trade_builder_share_qty(primary_net_qty.min(counter_net_qty));
    (common_qty > TRADE_BUILDER_PAIR_QTY_TOLERANCE).then_some(common_qty)
}

fn trade_builder_pair_lock_common_cost(
    net_qty: f64,
    fill_qty: f64,
    fill_price: f64,
    common_qty: f64,
) -> Option<f64> {
    if net_qty <= 0.0 || fill_qty <= 0.0 || fill_price <= 0.0 || common_qty <= 0.0 {
        return None;
    }
    let net_ratio = (net_qty / fill_qty).clamp(0.0001, 1.0);
    let common_gross_qty = common_qty / net_ratio;
    Some(common_gross_qty * fill_price)
}

async fn append_trade_builder_pair_lock_event(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    event_type: &str,
    payload: Value,
) -> Result<()> {
    if let Some(order_id) = session.lead_order_id.or(session.primary_order_id).or(session.counter_order_id) {
        repo.append_trade_builder_order_event(order_id, event_type, &payload)
            .await?;
    }
    Ok(())
}

async fn maybe_send_trade_builder_pair_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    notification_type: &str,
    message: &str,
    enabled: bool,
) {
    if enabled {
        let _ = send_trade_builder_notification(repo, order, notification_type, message).await;
    }
}

fn build_trade_builder_pair_locked_message(
    session: &TradeBuilderPairSession,
    projected_net_profit_usdc: f64,
) -> String {
    format!(
        "Pair Lock Basarili\nMarket: {}\nLocked Qty: {:.2}\nProj. Net Kar: {:.4} USDC\nMax Total: {:.2}c",
        session.market_slug,
        session.locked_qty.unwrap_or_default(),
        projected_net_profit_usdc,
        session.pair_target_total_cent
    )
}

fn build_trade_builder_pair_unwind_message(
    session: &TradeBuilderPairSession,
    reason: &str,
) -> String {
    format!(
        "Pair Lock Unwind\nMarket: {}\nSebep: {}\nMax Total: {:.2}c",
        session.market_slug, reason, session.pair_target_total_cent
    )
}

async fn create_trade_builder_pair_unwind_order(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    pair_session_id: i64,
    unwind_qty: f64,
    reason: &str,
) -> Result<Option<i64>> {
    let unwind_qty = round_trade_builder_share_qty(unwind_qty);
    if unwind_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE {
        return Ok(None);
    }
    let reference_price = parent_order
        .last_seen_price
        .or_else(|| parent_order.target_qty.filter(|qty| *qty > 0.0).map(|qty| parent_order.size_usdc / qty))
        .unwrap_or(0.5);
    let unwind_order_id = repo
        .create_trade_builder_order_with_exit_ladders(
            parent_order.trade_id,
            "immediate",
            "triggered",
            &parent_order.market_slug,
            &parent_order.token_id,
            &parent_order.outcome_label,
            "sell",
            "market",
            None,
            None,
            None,
            None,
            None,
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            (unwind_qty * reference_price).max(0.0),
            Some(unwind_qty),
            Some(unwind_qty),
            parent_order.min_price_distance_cent,
            parent_order.expires_at,
            None,
            None,
            1,
            Some(parent_order.id),
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            parent_order.fee_rate_bps,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            None,
            false,
            0,
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            None,
            None,
            None,
        )
        .await?;
    repo.set_trade_builder_order_pair_session(
        unwind_order_id,
        Some(pair_session_id),
        Some(TRADE_BUILDER_PAIR_ROLE_ORPHAN_UNWIND_SELL),
    )
    .await?;
    repo.append_trade_builder_order_event(
        parent_order.id,
        "pair_lock_unwind_scheduled",
        &json!({
            "pair_session_id": pair_session_id,
            "reason": reason,
            "unwind_order_id": unwind_order_id,
            "unwind_qty": unwind_qty,
        }),
    )
    .await?;
    Ok(Some(unwind_order_id))
}

async fn schedule_trade_builder_pair_session_unwind(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    orders: &[TradeBuilderOrder],
    next_status: &str,
    reason: &str,
    keep_visible_qty: Option<f64>,
) -> Result<()> {
    let keep_visible_qty = keep_visible_qty.unwrap_or(0.0).max(0.0);
    for order in orders.iter().filter(|order| trade_builder_pair_lock_is_candidate_order(order)) {
        if !trade_builder_is_terminal_status(&order.status) && order.filled_qty <= 0.0 {
            let next_order_status = if order.active_exchange_order_id.is_some() {
                "canceled_requested"
            } else {
                "canceled"
            };
            repo.set_trade_builder_order_status(order.id, next_order_status, Some(reason))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "pair_lock_candidate_canceled",
                &json!({
                    "pair_session_id": session.id,
                    "reason": reason,
                    "status_after": next_order_status,
                }),
            )
            .await?;
        }
    }

    for order in orders.iter().filter(|order| trade_builder_pair_lock_is_candidate_order(order)) {
        let position = repo.get_trade_builder_parent_position(order.id).await?;
        let current_qty = position
            .as_ref()
            .map(|value| value.current_qty)
            .or_else(|| {
                if order.filled_qty > 0.0 {
                    Some(
                        trade_builder_pair_lock_fill_net_qty(
                            order.filled_qty,
                            trade_builder_pair_lock_estimated_fee_qty(
                                order.filled_qty,
                                if order.filled_qty > 0.0 {
                                    order.size_usdc / order.filled_qty
                                } else {
                                    0.5
                                },
                                order.fee_rate_bps,
                            ),
                        ),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let unwind_qty = (current_qty - keep_visible_qty).max(0.0);
        let _ = create_trade_builder_pair_unwind_order(repo, order, session.id, unwind_qty, reason)
            .await?;
    }

    repo.update_trade_builder_pair_session_state(session.id, next_status, None, None, Some(reason))
        .await?;
    append_trade_builder_pair_lock_event(
        repo,
        session,
        "pair_lock_session_state_changed",
        json!({
            "pair_session_id": session.id,
            "status_after": next_status,
            "reason": reason,
            "keep_visible_qty": keep_visible_qty,
        }),
    )
    .await?;
    if let Some(reference_order) = orders
        .iter()
        .find(|candidate| candidate.id == session.lead_order_id.unwrap_or_default())
        .or_else(|| orders.first())
    {
        maybe_send_trade_builder_pair_notification(
            repo,
            reference_order,
            "pair_unwind",
            &build_trade_builder_pair_unwind_message(session, reason),
            session.notify_on_pair_unwind,
        )
        .await;
    }
    Ok(())
}

async fn maybe_handle_trade_builder_pair_lock_buy_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    canonical_entry_qty: f64,
    execution_price: f64,
) -> Result<()> {
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(());
    };
    if !trade_builder_pair_lock_is_candidate_order(order) {
        return Ok(());
    }
    let fee_qty = trade_builder_pair_lock_estimated_fee_qty(
        canonical_entry_qty,
        execution_price,
        order.fee_rate_bps,
    );
    let net_qty = trade_builder_pair_lock_fill_net_qty(canonical_entry_qty, fee_qty);
    repo.record_trade_builder_pair_session_fill(
        pair_session_id,
        order
            .pair_leg_role
            .as_deref()
            .unwrap_or(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE),
        order.id,
        canonical_entry_qty,
        fee_qty,
        net_qty,
        execution_price,
        Utc::now(),
    )
    .await?;

    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(());
    };
    if maybe_handle_biased_hedge_pair_fill(repo, &session, order).await? {
        return Ok(());
    }
    if session.status != TRADE_BUILDER_PAIR_STATUS_WORKING {
        return Ok(());
    }

    let Some(common_qty) = trade_builder_pair_lock_common_qty(&session) else {
        return Ok(());
    };
    let primary_cost = trade_builder_pair_lock_common_cost(
        session.primary_net_qty.unwrap_or_default(),
        session.primary_fill_qty.unwrap_or_default(),
        session.primary_avg_fill_price.unwrap_or_default(),
        common_qty,
    )
    .unwrap_or_default();
    let counter_cost = trade_builder_pair_lock_common_cost(
        session.counter_net_qty.unwrap_or_default(),
        session.counter_fill_qty.unwrap_or_default(),
        session.counter_avg_fill_price.unwrap_or_default(),
        common_qty,
    )
    .unwrap_or_default();
    let projected_net_profit_usdc = common_qty - primary_cost - counter_cost;
    let orders = repo
        .list_trade_builder_orders_by_pair_session(pair_session_id)
        .await?;
    repo.update_trade_builder_pair_session_state(
        pair_session_id,
        TRADE_BUILDER_PAIR_STATUS_LOCKED,
        Some(common_qty),
        Some(projected_net_profit_usdc),
        None,
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_locked",
        json!({
            "pair_session_id": pair_session_id,
            "locked_qty": common_qty,
            "projected_net_profit_usdc": projected_net_profit_usdc,
        }),
    )
    .await?;
    schedule_trade_builder_pair_session_unwind(
        repo,
        &session,
        &orders,
        TRADE_BUILDER_PAIR_STATUS_LOCKED,
        "pair_locked_residue_unwind",
        Some(common_qty),
    )
    .await?;
    maybe_cancel_trade_builder_pair_lock_stop_loss_after_locked(repo, &session, &orders).await?;
    if let Some(reference_order) = orders
        .iter()
        .find(|candidate| candidate.id == session.lead_order_id.unwrap_or_default())
        .or_else(|| orders.first())
    {
        maybe_send_trade_builder_pair_notification(
            repo,
            reference_order,
            "pair_locked",
            &build_trade_builder_pair_locked_message(&session, projected_net_profit_usdc),
            session.notify_on_pair_locked,
        )
        .await;
    }
    Ok(())
}

async fn maybe_finalize_trade_builder_pair_lock_after_unwind_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<()> {
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(());
    };
    if !trade_builder_pair_lock_is_unwind_order(order) {
        return Ok(());
    }
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(());
    };
    if session.status != TRADE_BUILDER_PAIR_STATUS_UNWINDING {
        return Ok(());
    }

    let orders = repo
        .list_trade_builder_orders_by_pair_session(pair_session_id)
        .await?;
    let mut has_live_position = false;
    for candidate in orders.iter().filter(|candidate| trade_builder_pair_lock_is_candidate_order(candidate)) {
        if let Some(position) = repo.get_trade_builder_parent_position(candidate.id).await? {
            if position.current_qty > TRADE_BUILDER_PAIR_QTY_TOLERANCE {
                has_live_position = true;
                break;
            }
        }
    }
    if has_live_position {
        return Ok(());
    }

    repo.update_trade_builder_pair_session_state(
        pair_session_id,
        TRADE_BUILDER_PAIR_STATUS_COMPLETED,
        session.locked_qty,
        session.projected_net_profit_usdc,
        session.last_error.as_deref(),
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_session_completed",
        json!({
            "pair_session_id": pair_session_id,
            "status_after": TRADE_BUILDER_PAIR_STATUS_COMPLETED,
        }),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod pair_lock_binding_tests {
    use super::*;

    fn test_graph(trigger_binding_mode: &str, extra_incoming: bool) -> TradeFlowGraphRuntime {
        let mut edges = vec![TradeFlowEdge {
            source: "trigger_pair".to_string(),
            target: "pair_buy".to_string(),
            edge_type: "default".to_string(),
            condition: None,
        }];
        let mut nodes = vec![
            TradeFlowNode {
                key: "trigger_pair".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({
                    "bindingMode": trigger_binding_mode,
                }),
            },
            TradeFlowNode {
                key: "pair_buy".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({
                    "mode": "pair_lock",
                }),
            },
        ];
        if extra_incoming {
            nodes.push(TradeFlowNode {
                key: "trigger_extra".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({
                    "bindingMode": trigger_binding_mode,
                }),
            });
            edges.push(TradeFlowEdge {
                source: "trigger_extra".to_string(),
                target: "pair_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            });
        }
        TradeFlowGraphRuntime {
            context: json!({}),
            nodes,
            edges,
        }
    }

    #[test]
    fn resolve_pair_lock_direct_trigger_node_key_accepts_pair_lock_only_parent() {
        let graph = test_graph("pair_lock_only", false);
        let trigger_key =
            resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph).expect("pair lock trigger");
        assert_eq!(trigger_key, "trigger_pair");
    }

    #[test]
    fn resolve_pair_lock_direct_trigger_node_key_rejects_standard_parent() {
        let graph = test_graph("standard", false);
        let err = resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph)
            .expect_err("standard binding should fail");
        assert!(err
            .to_string()
            .contains("bindingMode=pair_lock_only"));
    }

    #[test]
    fn resolve_pair_lock_direct_trigger_node_key_rejects_multiple_direct_parents() {
        let graph = test_graph("pair_lock_only", true);
        let err = resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph)
            .expect_err("multiple direct parents should fail");
        assert!(err
            .to_string()
            .contains("exactly one direct upstream trigger.market_price"));
    }
}
