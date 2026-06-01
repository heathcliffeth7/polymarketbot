#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipSideQuote {
    market_slug: String,
    revenge_side: String,
    token_id: String,
    outcome_label: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    current_price: f64,
    snapshot: Value,
}

fn revenge_flip_output_skipped(
    node: &TradeFlowNode,
    market_slug: &str,
    reason: &str,
    details: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1,
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

fn revenge_flip_binding_trigger_key(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
) -> Result<String> {
    let trigger_key = find_upstream_market_price_trigger_key(&node.key, graph).ok_or_else(|| {
        anyhow::anyhow!(
            "action.place_order revenge_flip_v1 requires upstream trigger.market_price bindingMode=revenge_flip_only"
        )
    })?;
    let trigger_node = flow_node(graph, &trigger_key)
        .ok_or_else(|| anyhow::anyhow!("revenge_flip_v1 upstream trigger node not found"))?;
    let binding_mode = node_config_string(trigger_node, "bindingMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        binding_mode == REVENGE_FLIP_BINDING_MODE,
        "action.place_order revenge_flip_v1 requires upstream trigger.market_price bindingMode=revenge_flip_only"
    );
    Ok(trigger_key)
}

fn revenge_flip_market_slug(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<String> {
    step_input_string(step, &["marketSlug", "market_slug", "wsMarketSlug"])
        .or_else(|| flow_context_string(context, "marketSlug"))
        .or_else(|| node_config_string(node, "marketSlug"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("revenge_flip_v1 requires marketSlug"))
}

async fn revenge_flip_resolve_side_quote(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    step: &TradeFlowRunStep,
    market_slug: &str,
    revenge_side: &str,
    token_id: &str,
    outcome_label: &str,
) -> RevengeFlipSideQuote {
    let quote = resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        token_id,
        outcome_label,
        step_input_f64(step, &["currentPrice", "price", "wsPrice"]),
    )
    .await;
    RevengeFlipSideQuote {
        market_slug: market_slug.to_string(),
        revenge_side: revenge_side.to_string(),
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid: quote.best_bid,
        best_ask: quote.best_ask,
        current_price: quote.current_price,
        snapshot: quote.quote_snapshot_used,
    }
}

async fn revenge_flip_resolve_quotes(
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    step: &TradeFlowRunStep,
    context: &Value,
    market_slug: &str,
    trigger_key: &str,
) -> Result<Vec<RevengeFlipSideQuote>> {
    let resolved =
        resolve_pair_lock_trigger_scoped_token_pair(cfg, market_slug, trigger_key, context).await?;
    let (up_label, down_label) = pair_lock_monitor_outcome_labels(Some(market_slug));
    let up = revenge_flip_resolve_side_quote(
        ws,
        client,
        step,
        market_slug,
        "up",
        &resolved.yes_token_id,
        up_label,
    )
    .await;
    let down = revenge_flip_resolve_side_quote(
        ws,
        client,
        step,
        market_slug,
        "down",
        &resolved.no_token_id,
        down_label,
    )
    .await;
    Ok(vec![up, down])
}

fn revenge_flip_quote_payload(quotes: &[RevengeFlipSideQuote]) -> Value {
    Value::Array(
        quotes
            .iter()
            .map(|quote| {
                json!({
                    "side": quote.revenge_side,
                    "token_id": quote.token_id,
                    "outcome_label": quote.outcome_label,
                    "best_bid": quote.best_bid,
                    "best_ask": quote.best_ask,
                    "current_price": quote.current_price,
                    "snapshot": quote.snapshot,
                })
            })
            .collect(),
    )
}

fn revenge_flip_ptb_stop_loss_payload(evaluation: &TradeBuilderPtbStopLossEvaluation) -> Value {
    json!({
        "asset": evaluation.asset,
        "direction": evaluation.direction,
        "threshold_gap_usd": evaluation.threshold_gap_usd,
        "ptb_reference_price": evaluation.ptb_reference_price,
        "current_price": evaluation.current_price,
        "current_price_source": evaluation.current_price_source,
        "current_chainlink_price": evaluation.current_chainlink_price,
        "directional_gap": evaluation.directional_gap,
        "reason_code": evaluation.reason_code,
        "should_trigger": evaluation.should_trigger,
        "source_evaluations": evaluation
            .source_evaluations
            .iter()
            .map(TradeBuilderPtbStopLossSourceEvaluation::to_value)
            .collect::<Vec<_>>(),
    })
}

fn revenge_flip_evaluate_ptb_stop_loss(
    config: &RevengeFlipConfig,
    quote: &RevengeFlipSideQuote,
) -> Option<TradeBuilderPtbStopLossEvaluation> {
    if !config.ptb_stop_loss.enabled {
        return None;
    }
    let gap_usd = config.ptb_stop_loss.gap_usd?;
    let current_price_source = PriceToBeatCurrentPriceSource::parse(Some(
        config.ptb_stop_loss.current_price_source.as_str(),
    ));
    Some(trade_builder_evaluate_ptb_stop_loss_inputs(
        &quote.market_slug,
        &quote.outcome_label,
        gap_usd,
        None,
        current_price_source,
        Some(config.ptb_stop_loss.time_decay_mode.as_str()),
    ))
}

fn revenge_flip_stop_loss_trigger_source(
    price_hit: bool,
    ptb_hit: bool,
    token_hit: bool,
) -> Option<&'static str> {
    if token_hit {
        return Some("token");
    }
    match (price_hit, ptb_hit) {
        (true, true) => Some("both"),
        (true, false) => Some("price"),
        (false, true) => Some("ptb"),
        (false, false) => None,
    }
}

fn revenge_flip_find_quote<'a>(
    quotes: &'a [RevengeFlipSideQuote],
    side: &str,
) -> Option<&'a RevengeFlipSideQuote> {
    quotes.iter().find(|quote| quote.revenge_side == side)
}

fn revenge_flip_select_entry_quote<'a>(
    state: &TradeBuilderRevengeFlipState,
    quotes: &'a [RevengeFlipSideQuote],
) -> Option<&'a RevengeFlipSideQuote> {
    if let Some(next_side) = state.next_entry_side.as_deref() {
        return revenge_flip_find_quote(quotes, next_side);
    }
    quotes
        .iter()
        .filter(|quote| {
            quote
                .best_ask
                .is_some_and(|ask| ask.is_finite() && ask > 0.0 && ask < 1.0)
        })
        .min_by(|left, right| {
            left.best_ask
                .unwrap_or(1.0)
                .partial_cmp(&right.best_ask.unwrap_or(1.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

struct RevengeFlipEntryCandidate<'a> {
    quote: &'a RevengeFlipSideQuote,
    best_ask: f64,
    effective_ptb: RevengeFlipEffectivePtb,
    effective_entry_price: RevengeFlipEffectiveEntryPrice,
    entry_ptb_guard: RevengeFlipEntryPtbGuard,
    selection_mode: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipEntrySizing {
    notional_usdc: f64,
    target_shares: f64,
    formula_target_shares: f64,
    min_reentry_shares: f64,
    min_reentry_shares_applied: bool,
    max_notional_usdc: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
enum RevengeFlipEntrySizingDecision {
    Ready(RevengeFlipEntrySizing),
    Unavailable,
    ReentryMinSharesExceedsLotLimit {
        formula_target_shares: f64,
        min_reentry_shares: f64,
        min_reentry_notional_usdc: f64,
        max_notional_usdc: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipEntryPtbGuard {
    enabled: bool,
    passed: bool,
    side: String,
    direction: Option<String>,
    threshold_usd: f64,
    ptb_reference_price: Option<f64>,
    current_price: Option<f64>,
    current_price_source: String,
    directional_gap: Option<f64>,
    abs_gap: Option<f64>,
    reason_code: String,
}

impl RevengeFlipEntryPtbGuard {
    fn to_value(&self) -> Value {
        json!({
            "enabled": self.enabled,
            "passed": self.passed,
            "side": self.side,
            "direction": self.direction,
            "threshold_usd": self.threshold_usd,
            "ptb_reference_price": self.ptb_reference_price,
            "current_price": self.current_price,
            "current_price_source": self.current_price_source,
            "directional_gap": self.directional_gap,
            "abs_gap": self.abs_gap,
            "reason_code": self.reason_code,
        })
    }
}

fn revenge_flip_entry_ptb_threshold_usd(effective_ptb: &RevengeFlipEffectivePtb) -> f64 {
    revenge_flip_value_to_usd(effective_ptb.max_diff, &effective_ptb.unit)
}

fn revenge_flip_entry_ptb_guard(
    quote: &RevengeFlipSideQuote,
    effective_ptb: &RevengeFlipEffectivePtb,
) -> RevengeFlipEntryPtbGuard {
    let threshold_usd = revenge_flip_entry_ptb_threshold_usd(effective_ptb);
    if !effective_ptb.enabled {
        return RevengeFlipEntryPtbGuard {
            enabled: false,
            passed: true,
            side: quote.revenge_side.clone(),
            direction: None,
            threshold_usd,
            ptb_reference_price: None,
            current_price: None,
            current_price_source: effective_ptb.current_price_source.clone(),
            directional_gap: None,
            abs_gap: None,
            reason_code: "ptb_disabled".to_string(),
        };
    }
    if !threshold_usd.is_finite() {
        return RevengeFlipEntryPtbGuard {
            enabled: true,
            passed: false,
            side: quote.revenge_side.clone(),
            direction: None,
            threshold_usd,
            ptb_reference_price: None,
            current_price: None,
            current_price_source: effective_ptb.current_price_source.clone(),
            directional_gap: None,
            abs_gap: None,
            reason_code: "invalid_threshold".to_string(),
        };
    }

    let current_price_source =
        PriceToBeatCurrentPriceSource::parse(Some(effective_ptb.current_price_source.as_str()));
    let evaluation = trade_builder_evaluate_ptb_stop_loss_inputs(
        &quote.market_slug,
        &quote.outcome_label,
        threshold_usd,
        None,
        current_price_source,
        Some("none"),
    );
    let passed = evaluation
        .directional_gap
        .is_some_and(|gap| gap >= threshold_usd);
    let abs_gap = evaluation
        .current_price
        .zip(evaluation.ptb_reference_price)
        .map(|(current, reference)| (current - reference).abs());
    let reason_code = if passed {
        "passed"
    } else if evaluation.directional_gap.is_some() {
        "price_to_beat_gap_below_threshold"
    } else {
        evaluation.reason_code
    };

    RevengeFlipEntryPtbGuard {
        enabled: true,
        passed,
        side: quote.revenge_side.clone(),
        direction: evaluation.direction,
        threshold_usd,
        ptb_reference_price: evaluation.ptb_reference_price,
        current_price: evaluation.current_price,
        current_price_source: evaluation.current_price_source.to_string(),
        directional_gap: evaluation.directional_gap,
        abs_gap,
        reason_code: reason_code.to_string(),
    }
}

fn revenge_flip_valid_best_ask(quote: &RevengeFlipSideQuote) -> Option<f64> {
    quote
        .best_ask
        .filter(|ask| ask.is_finite() && *ask > 0.0 && *ask < 1.0)
}

fn revenge_flip_last_stopped_side(state: &TradeBuilderRevengeFlipState) -> Option<&'static str> {
    if revenge_flip_position_blocks_reentry(state) {
        return None;
    }
    if let Some(next_entry_side) = state.next_entry_side.as_deref() {
        return revenge_flip_opposite_side(next_entry_side);
    }
    state.current_side.as_deref().and_then(|side| match side {
        "up" => Some("up"),
        "down" => Some("down"),
        _ => None,
    })
}

fn revenge_flip_rule_match_candidate<'a>(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    quote: &'a RevengeFlipSideQuote,
    remaining_sec: Option<i64>,
) -> Option<RevengeFlipEntryCandidate<'a>> {
    let best_ask = revenge_flip_valid_best_ask(quote)?;
    let entry_flip_index = revenge_flip_entry_flip_index(state);
    let last_stopped_side = revenge_flip_last_stopped_side(state);
    revenge_flip_matching_entry_ptb_rule(
        config,
        entry_flip_index,
        remaining_sec,
        Some(&quote.revenge_side),
        last_stopped_side,
    )?;
    let effective_entry_price = revenge_flip_effective_entry_price(
        config,
        entry_flip_index,
        remaining_sec,
        Some(&quote.revenge_side),
        last_stopped_side,
    );
    if !revenge_flip_entry_price_passes(config, &effective_entry_price, best_ask) {
        return None;
    }
    let effective_ptb = revenge_flip_effective_ptb(
        config,
        state,
        entry_flip_index,
        remaining_sec,
        Some(&quote.revenge_side),
        last_stopped_side,
    );
    let entry_ptb_guard = revenge_flip_entry_ptb_guard(quote, &effective_ptb);
    if !entry_ptb_guard.passed {
        return None;
    }
    Some(RevengeFlipEntryCandidate {
        quote,
        best_ask,
        effective_ptb,
        effective_entry_price,
        entry_ptb_guard,
        selection_mode: "rule_match_lowest_ask",
    })
}

fn revenge_flip_initial_rule_order_sides(side_mode: &str) -> &'static [&'static str] {
    match side_mode {
        "up" => &["up"],
        "down" => &["down"],
        "any" => &["up", "down"],
        _ => &[],
    }
}

fn revenge_flip_initial_rule_order_candidate<'a>(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    quotes: &'a [RevengeFlipSideQuote],
    remaining_sec: Option<i64>,
) -> Option<RevengeFlipEntryCandidate<'a>> {
    let entry_flip_index = revenge_flip_entry_flip_index(state);
    let last_stopped_side = revenge_flip_last_stopped_side(state);
    for rule in &config.entry_ptb_rules {
        for side in revenge_flip_initial_rule_order_sides(&rule.side_mode) {
            if !revenge_flip_entry_ptb_rule_matches(
                rule,
                entry_flip_index,
                remaining_sec,
                Some(side),
                last_stopped_side,
            ) {
                continue;
            }
            let Some(quote) = revenge_flip_find_quote(quotes, side) else {
                continue;
            };
            let Some(best_ask) = revenge_flip_valid_best_ask(quote) else {
                continue;
            };
            let effective_entry_price =
                revenge_flip_effective_entry_price_from_entry_rule(config, rule);
            if !revenge_flip_entry_price_passes(config, &effective_entry_price, best_ask) {
                continue;
            }
            let effective_ptb = revenge_flip_effective_ptb_from_entry_rule(config, state, rule);
            let entry_ptb_guard = revenge_flip_entry_ptb_guard(quote, &effective_ptb);
            if !entry_ptb_guard.passed {
                continue;
            }
            return Some(RevengeFlipEntryCandidate {
                quote,
                best_ask,
                effective_ptb,
                effective_entry_price,
                entry_ptb_guard,
                selection_mode: "initial_rule_order",
            });
        }
    }
    None
}

fn revenge_flip_select_entry_candidate<'a>(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    quotes: &'a [RevengeFlipSideQuote],
    remaining_sec: Option<i64>,
) -> Option<RevengeFlipEntryCandidate<'a>> {
    let entry_flip_index = revenge_flip_entry_flip_index(state);
    let last_stopped_side = revenge_flip_last_stopped_side(state);
    if config.reentry_side_mode != "rule_match" {
        let quote = revenge_flip_select_entry_quote(state, quotes)?;
        let best_ask = revenge_flip_valid_best_ask(quote)?;
        let effective_ptb = revenge_flip_effective_ptb(
            config,
            state,
            entry_flip_index,
            remaining_sec,
            Some(&quote.revenge_side),
            last_stopped_side,
        );
        let entry_ptb_guard = revenge_flip_entry_ptb_guard(quote, &effective_ptb);
        if !entry_ptb_guard.passed {
            return None;
        }
        return Some(RevengeFlipEntryCandidate {
            quote,
            best_ask,
            effective_ptb,
            effective_entry_price: revenge_flip_effective_entry_price(
                config,
                entry_flip_index,
                remaining_sec,
                Some(&quote.revenge_side),
                last_stopped_side,
            ),
            entry_ptb_guard,
            selection_mode: "fixed_or_lowest_ask",
        });
    }
    if entry_flip_index == 0 {
        return revenge_flip_initial_rule_order_candidate(config, state, quotes, remaining_sec);
    }
    quotes
        .iter()
        .filter_map(|quote| revenge_flip_rule_match_candidate(config, state, quote, remaining_sec))
        .min_by(|left, right| left.best_ask.total_cmp(&right.best_ask))
}

fn revenge_flip_entry_candidate_payload(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    quotes: &[RevengeFlipSideQuote],
    remaining_sec: Option<i64>,
) -> Value {
    let entry_flip_index = revenge_flip_entry_flip_index(state);
    let last_stopped_side = revenge_flip_last_stopped_side(state);
    Value::Array(
        quotes
            .iter()
            .map(|quote| {
                let effective_entry_price = revenge_flip_effective_entry_price(
                    config,
                    entry_flip_index,
                    remaining_sec,
                    Some(&quote.revenge_side),
                    last_stopped_side,
                );
                let matched_rule = revenge_flip_matching_entry_ptb_rule(
                    config,
                    entry_flip_index,
                    remaining_sec,
                    Some(&quote.revenge_side),
                    last_stopped_side,
                )
                .map(revenge_flip_entry_ptb_rule_json);
                let effective_ptb = revenge_flip_effective_ptb(
                    config,
                    state,
                    entry_flip_index,
                    remaining_sec,
                    Some(&quote.revenge_side),
                    last_stopped_side,
                );
                let entry_ptb_guard = revenge_flip_entry_ptb_guard(quote, &effective_ptb);
                json!({
                    "side": quote.revenge_side,
                    "best_ask": quote.best_ask,
                    "ask_valid": revenge_flip_valid_best_ask(quote).is_some(),
                    "matched_entry_rule": matched_rule,
                    "entry_ptb_guard": entry_ptb_guard.to_value(),
                    "ptb_passes": entry_ptb_guard.passed,
                    "effective_max_price_cent": effective_entry_price.max_cent,
                    "effective_max_price_source": effective_entry_price.max_source,
                    "price_passes": quote.best_ask.map_or(false, |ask| {
                        revenge_flip_entry_price_passes(config, &effective_entry_price, ask)
                    }),
                })
            })
            .collect(),
    )
}

fn revenge_flip_entry_notional(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    quote: &RevengeFlipSideQuote,
    available_collateral_usdc: Option<f64>,
) -> Result<RevengeFlipEntrySizingDecision> {
    let Some(best_ask) = quote
        .best_ask
        .filter(|ask| ask.is_finite() && *ask > 0.0 && *ask < 1.0)
    else {
        return Ok(RevengeFlipEntrySizingDecision::Unavailable);
    };
    let first_entry = state.flip_count == 0
        && state.total_loss_usdc <= 0.000001
        && state.next_entry_side.is_none();
    if first_entry {
        return Ok(RevengeFlipEntrySizingDecision::Ready(
            RevengeFlipEntrySizing {
                notional_usdc: positive_quantity_flip_grid_round_up_cent(config.initial_order_usdc),
                target_shares: 0.0,
                formula_target_shares: 0.0,
                min_reentry_shares: 0.0,
                min_reentry_shares_applied: false,
                max_notional_usdc: None,
            },
        ));
    }
    let denominator = 1.0 - best_ask;
    if denominator <= 0.0 || !denominator.is_finite() {
        return Ok(RevengeFlipEntrySizingDecision::Unavailable);
    }
    let formula_target_shares = positive_quantity_flip_grid_round_up_share_qty(
        (state.total_loss_usdc + config.profit_target_usdc) / denominator,
    );
    let min_reentry_shares =
        positive_quantity_flip_grid_round_up_share_qty(config.min_reentry_shares);
    let target_shares = formula_target_shares.max(min_reentry_shares);
    let min_reentry_shares_applied = min_reentry_shares > formula_target_shares;
    let mut notional = positive_quantity_flip_grid_round_up_cent(target_shares * best_ask);
    let Some(available_collateral_usdc) = available_collateral_usdc else {
        return Ok(RevengeFlipEntrySizingDecision::Unavailable);
    };
    let max_notional = positive_quantity_flip_grid_round_up_cent(
        (available_collateral_usdc * config.lot_limit_pct).max(0.0),
    );
    if max_notional <= 0.0 {
        return Ok(RevengeFlipEntrySizingDecision::Unavailable);
    }
    if min_reentry_shares > 0.0 {
        let min_reentry_notional =
            positive_quantity_flip_grid_round_up_cent(min_reentry_shares * best_ask);
        if min_reentry_notional > max_notional + 0.000001 {
            return Ok(
                RevengeFlipEntrySizingDecision::ReentryMinSharesExceedsLotLimit {
                    formula_target_shares,
                    min_reentry_shares,
                    min_reentry_notional_usdc: min_reentry_notional,
                    max_notional_usdc: max_notional,
                },
            );
        }
    }
    notional = notional.min(max_notional);
    if notional <= 0.0 {
        return Ok(RevengeFlipEntrySizingDecision::Unavailable);
    }
    Ok(RevengeFlipEntrySizingDecision::Ready(
        RevengeFlipEntrySizing {
            notional_usdc: notional,
            target_shares,
            formula_target_shares,
            min_reentry_shares,
            min_reentry_shares_applied,
            max_notional_usdc: Some(max_notional),
        },
    ))
}

fn revenge_flip_step_with_quote(
    step: &TradeFlowRunStep,
    quote: &RevengeFlipSideQuote,
    source_trade_id: Option<i64>,
) -> TradeFlowRunStep {
    let mut input = step.input_json.clone().unwrap_or_else(|| json!({}));
    if !input.is_object() {
        input = json!({});
    }
    if let Some(object) = input.as_object_mut() {
        object.insert("marketSlug".to_string(), json!(quote.market_slug));
        object.insert("tokenId".to_string(), json!(quote.token_id));
        object.insert("outcomeLabel".to_string(), json!(quote.outcome_label));
        object.insert("currentPrice".to_string(), json!(quote.current_price));
        object.insert("wsBestBid".to_string(), json!(quote.best_bid));
        object.insert("wsBestAsk".to_string(), json!(quote.best_ask));
        if let Some(source_trade_id) = source_trade_id {
            object.insert("sourceTradeId".to_string(), json!(source_trade_id));
        }
    }
    revenge_flip_child_step(step, input)
}

#[allow(clippy::too_many_arguments)]
async fn revenge_flip_submit_child_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    _step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    market_slug: &str,
    child_step: &TradeFlowRunStep,
    child_node: &TradeFlowNode,
) -> Result<TradeFlowNodeExecution> {
    let Some(lock) = repo
        .try_acquire_revenge_flip_execution_lock(
            run.user_id,
            run.definition_id,
            &node.key,
            market_slug,
        )
        .await?
    else {
        return Ok(revenge_flip_output_skipped(
            node,
            market_slug,
            "execution_lock_busy",
            json!({}),
        ));
    };
    let execution_result = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        child_step,
        child_node,
        graph,
        context,
    )
    .await;
    lock.release().await;
    execution_result
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_revenge_flip(
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
    let config = resolve_revenge_flip_config(node)?;
    let trigger_key = revenge_flip_binding_trigger_key(node, graph)?;
    let market_slug = revenge_flip_market_slug(node, step, context)?;
    let Some(client) = client else {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            "client_unavailable",
            json!({}),
        ));
    };
    if repo
        .has_active_trade_builder_revenge_flip_order(
            run.user_id,
            run.definition_id,
            &node.key,
            &market_slug,
        )
        .await?
    {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            "active_order_exists",
            json!({}),
        ));
    }

    let state = repo
        .load_trade_builder_revenge_flip_state(
            run.user_id,
            run.definition_id,
            &node.key,
            &market_slug,
        )
        .await?;
    let remaining_sec = positive_quantity_flip_grid_remaining_sec(&market_slug);
    let quotes =
        revenge_flip_resolve_quotes(cfg, client, ws, step, context, &market_slug, &trigger_key)
            .await?;

    if let Some(current_side) = state
        .current_side
        .as_deref()
        .filter(|_| revenge_flip_position_is_open(state.position_qty))
    {
        let Some(quote) = revenge_flip_find_quote(&quotes, current_side) else {
            return Ok(revenge_flip_output_skipped(
                node,
                &market_slug,
                "position_quote_unavailable",
                json!({ "quotes": revenge_flip_quote_payload(&quotes) }),
            ));
        };
        let best_bid = quote.best_bid.filter(|bid| bid.is_finite() && *bid > 0.0);
        let ptb_stop_loss_evaluation = revenge_flip_evaluate_ptb_stop_loss(&config, quote);
        if best_bid.is_none() && ptb_stop_loss_evaluation.is_none() {
            return Ok(revenge_flip_output_skipped(
                node,
                &market_slug,
                "stop_loss_best_bid_unavailable",
                json!({ "quotes": revenge_flip_quote_payload(&quotes) }),
            ));
        }
        let position_stop_loss_pct = revenge_flip_position_stop_loss_pct(&state);
        let position_stop_loss_enabled = state.position_stop_loss_enabled;
        let stop_loss_price = position_stop_loss_enabled
            .then_some(state.position_avg_cost * (1.0 - position_stop_loss_pct));
        let price_stop_loss_hit = if position_stop_loss_enabled {
            best_bid
                .map(|bid| revenge_flip_stop_loss_triggered(&state, bid))
                .unwrap_or(false)
        } else {
            false
        };
        let token_stop_loss_hit = config.token_stop_loss_enabled
            && best_bid.is_some_and(|bid| {
                bid.is_finite()
                    && bid > 0.0
                    && bid <= state.position_avg_cost * (1.0 - config.token_stop_loss_pct)
            });
        let ptb_stop_loss_hit = ptb_stop_loss_evaluation
            .as_ref()
            .map(|evaluation| evaluation.should_trigger)
            .unwrap_or(false);
        let trigger_source = revenge_flip_stop_loss_trigger_source(
            price_stop_loss_hit,
            ptb_stop_loss_hit,
            token_stop_loss_hit,
        );
        if trigger_source.is_none() {
            return Ok(TradeFlowNodeExecution {
                output: json!({
                    "node_key": node.key,
                    "mode": ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1,
                    "market_slug": market_slug,
                    "decision": "hold_position",
                    "position_side": current_side,
                    "position_qty": state.position_qty,
                    "position_avg_cost": state.position_avg_cost,
                    "position_stop_loss_enabled": position_stop_loss_enabled,
                    "position_stop_loss_pct": position_stop_loss_pct,
                    "best_bid": best_bid,
                    "stop_loss_price": stop_loss_price,
                    "ptb_stop_loss": ptb_stop_loss_evaluation
                        .as_ref()
                        .map(revenge_flip_ptb_stop_loss_payload),
                    "ptb_stop_loss_gap_unit": config.ptb_stop_loss.gap_unit.clone(),
                    "remaining_sec": remaining_sec,
                    "quotes": revenge_flip_quote_payload(&quotes),
                }),
                routes: Vec::new(),
                repeat_at: None,
                repeat_idempotency_key: None,
            });
        }
        let Some(sell_node) = revenge_flip_stop_loss_sell_node(
            node,
            &config,
            quote,
            &state,
            Utc::now().timestamp_millis(),
        ) else {
            return Ok(revenge_flip_output_skipped(
                node,
                &market_slug,
                "missing_position_source_trade_id",
                json!({ "state": { "position_builder_order_id": state.position_builder_order_id } }),
            ));
        };
        let sell_step = revenge_flip_step_with_quote(step, quote, state.position_source_trade_id);
        clear_action_place_order_ref_bindings(
            context,
            &sell_node,
            &action_place_order_ref_key(&sell_node),
        );
        let execution = revenge_flip_submit_child_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            node,
            graph,
            context,
            &market_slug,
            &sell_step,
            &sell_node,
        )
        .await?;
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "mode": ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1,
                "market_slug": market_slug,
                "decision": "stop_loss_sell",
                "position_side": current_side,
                "position_stop_loss_enabled": position_stop_loss_enabled,
                "position_stop_loss_pct": position_stop_loss_pct,
                "best_bid": best_bid,
                "stop_loss_price": stop_loss_price,
                "stop_loss_trigger_source": trigger_source,
                "ptb_stop_loss": ptb_stop_loss_evaluation
                    .as_ref()
                    .map(revenge_flip_ptb_stop_loss_payload),
                "ptb_stop_loss_gap_unit": config.ptb_stop_loss.gap_unit.clone(),
                "child": execution.output,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if remaining_sec.is_some_and(|remaining| remaining <= config.close_only_sec) {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            "close_only_window",
            json!({ "remaining_sec": remaining_sec, "close_only_sec": config.close_only_sec }),
        ));
    }
    if !revenge_flip_max_flip_allows(&config, &state) {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            "max_flip_reached",
            json!({ "flip_count": state.flip_count, "max_flip": config.max_flip }),
        ));
    }

    let Some(entry_candidate) =
        revenge_flip_select_entry_candidate(&config, &state, &quotes, remaining_sec)
    else {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            if config.reentry_side_mode == "rule_match" {
                "entry_rule_match_unavailable"
            } else {
                "entry_best_ask_unavailable"
            },
            json!({
                "reentry_side_mode": config.reentry_side_mode,
                "quotes": revenge_flip_quote_payload(&quotes),
                "candidate_evaluations": revenge_flip_entry_candidate_payload(
                    &config,
                    &state,
                    &quotes,
                    remaining_sec,
                ),
            }),
        ));
    };
    let entry_quote = entry_candidate.quote;
    let best_ask = entry_candidate.best_ask;
    let entry_flip_index = revenge_flip_entry_flip_index(&state);
    let effective_entry_price = entry_candidate.effective_entry_price;
    if !revenge_flip_entry_price_passes(&config, &effective_entry_price, best_ask) {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            "trigger_price_range_blocked",
            json!({
                "entry_side": entry_quote.revenge_side,
                "entry_flip_index": entry_flip_index,
                "best_ask": best_ask,
                "ask_cent": best_ask * 100.0,
                "min_cent": config.trigger_price.enabled.then_some(config.trigger_price.min_cent),
                "max_cent": effective_entry_price.max_cent,
                "max_price_source": effective_entry_price.max_source,
            }),
        ));
    }

    let entry_stop_loss_pct = revenge_flip_stop_loss_pct_for_entry(&config, entry_flip_index);
    let is_flip =
        state.flip_count > 0 || state.total_loss_usdc > 0.000001 || state.next_entry_side.is_some();
    let available_collateral = if is_flip {
        match client.available_collateral_usdc().await {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(
                    market_slug = %market_slug,
                    error = %err,
                    "revenge flip collateral lookup failed"
                );
                None
            }
        }
    } else {
        None
    };
    let sizing =
        match revenge_flip_entry_notional(&config, &state, entry_quote, available_collateral)? {
            RevengeFlipEntrySizingDecision::Ready(sizing) => sizing,
            RevengeFlipEntrySizingDecision::Unavailable => {
                return Ok(revenge_flip_output_skipped(
                    node,
                    &market_slug,
                    if is_flip {
                        "flip_sizing_unavailable"
                    } else {
                        "initial_sizing_unavailable"
                    },
                    json!({
                        "entry_side": entry_quote.revenge_side,
                        "best_ask": best_ask,
                        "available_collateral_usdc": available_collateral,
                        "total_loss_usdc": state.total_loss_usdc,
                        "profit_target_usdc": config.profit_target_usdc,
                        "min_reentry_shares": config.min_reentry_shares,
                    }),
                ));
            }
            RevengeFlipEntrySizingDecision::ReentryMinSharesExceedsLotLimit {
                formula_target_shares,
                min_reentry_shares,
                min_reentry_notional_usdc,
                max_notional_usdc,
            } => {
                return Ok(revenge_flip_output_skipped(
                    node,
                    &market_slug,
                    "reentry_min_shares_exceeds_lot_limit",
                    json!({
                        "entry_side": entry_quote.revenge_side,
                        "best_ask": best_ask,
                        "available_collateral_usdc": available_collateral,
                        "lot_limit_pct": config.lot_limit_pct,
                        "formula_target_shares": formula_target_shares,
                        "min_reentry_shares": min_reentry_shares,
                        "min_reentry_notional_usdc": min_reentry_notional_usdc,
                        "max_notional_usdc": max_notional_usdc,
                    }),
                ));
            }
        };
    let notional_usdc = sizing.notional_usdc;

    if notional_usdc <= 0.0 {
        return Ok(revenge_flip_output_skipped(
            node,
            &market_slug,
            if is_flip {
                "flip_sizing_unavailable"
            } else {
                "initial_sizing_unavailable"
            },
            json!({
                "entry_side": entry_quote.revenge_side,
                "best_ask": best_ask,
                "available_collateral_usdc": available_collateral,
                "total_loss_usdc": state.total_loss_usdc,
                "profit_target_usdc": config.profit_target_usdc,
                "min_reentry_shares": config.min_reentry_shares,
            }),
        ));
    }

    let effective_ptb = entry_candidate.effective_ptb;
    let intent = if is_flip { "flip_buy" } else { "initial_buy" };
    let buy_node = revenge_flip_buy_node(
        node,
        &config,
        &effective_ptb,
        effective_entry_price.max_cent,
        entry_quote,
        notional_usdc,
        entry_stop_loss_pct,
        intent,
        Utc::now().timestamp_millis(),
    );
    let buy_step = revenge_flip_step_with_quote(step, entry_quote, None);
    clear_action_place_order_ref_bindings(
        context,
        &buy_node,
        &action_place_order_ref_key(&buy_node),
    );
    let execution = revenge_flip_submit_child_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client,
        run,
        step,
        node,
        graph,
        context,
        &market_slug,
        &buy_step,
        &buy_node,
    )
    .await?;
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1,
            "market_slug": market_slug,
            "decision": intent,
            "reentry_side_mode": config.reentry_side_mode,
            "selection_mode": entry_candidate.selection_mode,
            "entry_side": entry_quote.revenge_side,
            "entry_flip_index": entry_flip_index,
            "entry_stop_loss_pct": entry_stop_loss_pct,
            "best_ask": best_ask,
            "notional_usdc": notional_usdc,
            "target_shares": sizing.target_shares,
            "formula_target_shares": sizing.formula_target_shares,
            "min_reentry_shares": sizing.min_reentry_shares,
            "min_reentry_shares_applied": sizing.min_reentry_shares_applied,
            "max_notional_usdc": sizing.max_notional_usdc,
            "remaining_sec": remaining_sec,
            "effective_max_price_cent": effective_entry_price.max_cent,
            "effective_max_price_source": effective_entry_price.max_source,
            "entry_ptb_guard": entry_candidate.entry_ptb_guard.to_value(),
            "effective_ptb": {
                "enabled": effective_ptb.enabled,
                "mode": effective_ptb.mode,
                "max_diff": effective_ptb.max_diff,
                "unit": effective_ptb.unit,
                "base_source": effective_ptb.base_source,
                "matched_entry_rule": effective_ptb.matched_entry_rule,
            },
            "child": execution.output,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
