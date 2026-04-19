use serde::Deserialize;

const TRADE_BUILDER_EXIT_LADDER_KIND_TP: &str = "tp";
const TRADE_BUILDER_EXIT_LADDER_KIND_SL: &str = "sl";
const TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL: &str = "ptb_sl";
const TRADE_BUILDER_EXIT_LADDER_MAX_RULES: usize = 5;
const TRADE_BUILDER_EXIT_MODE_HARD: &str = "hard";
const TRADE_BUILDER_EXIT_MODE_STAGED: &str = "staged";
const TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL: &str = "cancel_all";
const TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING: &str = "resize_remaining";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActionPlaceOrderPriceExitRuleConfig {
    price_cent: f64,
    size_pct: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActionPlaceOrderTimeExitRuleConfig {
    elapsed_minutes: i64,
    remaining_pct: f64,
}

fn trade_builder_exit_ladder_kind(order: &TradeBuilderOrder) -> Option<&str> {
    order
        .exit_ladder_kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn trade_builder_is_price_exit_ladder_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_exit_ladder_kind(order).is_some_and(|value| {
        matches!(
            value,
            TRADE_BUILDER_EXIT_LADDER_KIND_TP
                | TRADE_BUILDER_EXIT_LADDER_KIND_SL
                | TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL
        )
    })
}

fn trade_builder_price_exit_rule_from_legacy(
    enabled: bool,
    raw_price: Option<f64>,
) -> Vec<TradeBuilderPriceExitRule> {
    if !enabled {
        return Vec::new();
    }
    raw_price
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(|price| {
            vec![TradeBuilderPriceExitRule {
                price,
                size_pct: 100.0,
            }]
        })
        .unwrap_or_default()
}

fn trade_builder_hard_price_exit_rule(
    enabled: bool,
    raw_price: Option<f64>,
) -> Option<TradeBuilderPriceExitRule> {
    trade_builder_price_exit_rule_from_legacy(enabled, raw_price)
        .into_iter()
        .next()
}

fn parse_trade_builder_price_exit_rules(
    raw_value: Option<&Value>,
    family: &str,
) -> Result<Vec<TradeBuilderPriceExitRule>> {
    let Some(raw_value) = raw_value else {
        return Ok(Vec::new());
    };
    let raw_rules: Vec<ActionPlaceOrderPriceExitRuleConfig> =
        serde_json::from_value(raw_value.clone())
        .with_context(|| format!("action.place_order {family}Rules must be an array"))?;
    let rules = raw_rules
        .into_iter()
        .map(|rule| TradeBuilderPriceExitRule {
            price: (rule.price_cent / 100.0).clamp(0.0, 1.0),
            size_pct: rule.size_pct,
        })
        .collect::<Vec<_>>();
    validate_trade_builder_price_exit_rules(&rules, family)?;
    Ok(rules)
}

fn validate_trade_builder_price_exit_rules(
    rules: &[TradeBuilderPriceExitRule],
    family: &str,
) -> Result<()> {
    anyhow::ensure!(
        rules.len() <= TRADE_BUILDER_EXIT_LADDER_MAX_RULES,
        "action.place_order {family}Rules supports at most {} rules",
        TRADE_BUILDER_EXIT_LADDER_MAX_RULES
    );
    let mut previous_price = None;
    let mut total_size_pct = 0.0_f64;
    for (index, rule) in rules.iter().enumerate() {
        anyhow::ensure!(
            rule.price.is_finite() && rule.price > 0.0 && rule.price <= 1.0,
            "action.place_order {family}Rules[{index}].priceCent must be in (0, 100]"
        );
        anyhow::ensure!(
            rule.size_pct.is_finite() && rule.size_pct > 0.0 && rule.size_pct <= 100.0,
            "action.place_order {family}Rules[{index}].sizePct must be in (0, 100]"
        );
        if let Some(previous_price) = previous_price {
            if family == TRADE_BUILDER_EXIT_LADDER_KIND_TP {
                anyhow::ensure!(
                    rule.price > previous_price,
                    "action.place_order tpRules prices must be strictly increasing"
                );
            } else {
                anyhow::ensure!(
                    rule.price < previous_price,
                    "action.place_order slRules prices must be strictly decreasing"
                );
            }
        }
        previous_price = Some(rule.price);
        total_size_pct += rule.size_pct;
    }
    anyhow::ensure!(
        (total_size_pct - 100.0).abs() <= 0.000001 || rules.is_empty(),
        "action.place_order {family}Rules sizePct total must equal 100"
    );
    Ok(())
}

fn parse_trade_builder_time_exit_rules(
    raw_value: Option<&Value>,
) -> Result<Vec<TradeBuilderTimeExitRule>> {
    let Some(raw_value) = raw_value else {
        return Ok(Vec::new());
    };
    let raw_rules: Vec<ActionPlaceOrderTimeExitRuleConfig> =
        serde_json::from_value(raw_value.clone())
        .context("action.place_order timeExitRules must be an array")?;
    let rules = raw_rules
        .into_iter()
        .map(|rule| TradeBuilderTimeExitRule {
            elapsed_minutes: rule.elapsed_minutes as i32,
            remaining_pct: rule.remaining_pct,
        })
        .collect::<Vec<_>>();
    validate_trade_builder_time_exit_rules(&rules)?;
    Ok(rules)
}

fn validate_trade_builder_time_exit_rules(rules: &[TradeBuilderTimeExitRule]) -> Result<()> {
    anyhow::ensure!(
        rules.len() <= TRADE_BUILDER_EXIT_LADDER_MAX_RULES,
        "action.place_order timeExitRules supports at most {} rules",
        TRADE_BUILDER_EXIT_LADDER_MAX_RULES
    );
    let mut previous_elapsed_minutes = None;
    for (index, rule) in rules.iter().enumerate() {
        anyhow::ensure!(
            rule.elapsed_minutes > 0,
            "action.place_order timeExitRules[{index}].elapsedMinutes must be > 0"
        );
        anyhow::ensure!(
            rule.remaining_pct.is_finite()
                && rule.remaining_pct > 0.0
                && rule.remaining_pct <= 100.0,
            "action.place_order timeExitRules[{index}].remainingPct must be in (0, 100]"
        );
        if let Some(previous_elapsed_minutes) = previous_elapsed_minutes {
            anyhow::ensure!(
                rule.elapsed_minutes > previous_elapsed_minutes,
                "action.place_order timeExitRules elapsedMinutes must be strictly increasing"
            );
        }
        previous_elapsed_minutes = Some(rule.elapsed_minutes);
    }
    Ok(())
}

fn trade_builder_normalized_tp_rules(order: &TradeBuilderOrder) -> Vec<TradeBuilderPriceExitRule> {
    order.tp_rules_json.clone()
}

fn trade_builder_normalized_sl_rules(order: &TradeBuilderOrder) -> Vec<TradeBuilderPriceExitRule> {
    order.sl_rules_json.clone()
}

fn trade_builder_hard_tp_rule(order: &TradeBuilderOrder) -> Option<TradeBuilderPriceExitRule> {
    trade_builder_hard_price_exit_rule(order.tp_enabled, order.tp_price)
}

fn trade_builder_hard_sl_rule(order: &TradeBuilderOrder) -> Option<TradeBuilderPriceExitRule> {
    trade_builder_hard_price_exit_rule(order.sl_enabled, order.sl_price)
}

fn trade_builder_is_hard_exit_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order) && trade_builder_exit_ladder_kind(order).is_none()
}

fn trade_builder_exit_mode(order: &TradeBuilderOrder) -> &'static str {
    if trade_builder_is_price_exit_ladder_child(order) {
        TRADE_BUILDER_EXIT_MODE_STAGED
    } else {
        TRADE_BUILDER_EXIT_MODE_HARD
    }
}

fn trade_builder_exit_family(order: &TradeBuilderOrder) -> Option<&str> {
    if trade_builder_exit_ladder_kind(order) == Some(TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL) {
        return Some(TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL);
    }
    if trade_builder_is_take_profit_child(order) {
        return Some(TRADE_BUILDER_EXIT_LADDER_KIND_TP);
    }
    if trade_builder_is_stop_loss_child(order) {
        return Some(TRADE_BUILDER_EXIT_LADDER_KIND_SL);
    }
    None
}

fn trade_builder_exit_sibling_policy(order: &TradeBuilderOrder) -> &'static str {
    if trade_builder_is_hard_exit_child(order) {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL
    } else {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
    }
}

fn trade_builder_normalized_time_exit_rules(
    order: &TradeBuilderOrder,
) -> Vec<TradeBuilderTimeExitRule> {
    order.time_exit_rules_json.clone()
}

fn trade_builder_order_has_exit_ladders(order: &TradeBuilderOrder) -> bool {
    trade_builder_hard_tp_rule(order).is_some()
        || trade_builder_hard_sl_rule(order).is_some()
        || !trade_builder_normalized_tp_rules(order).is_empty()
        || !trade_builder_normalized_sl_rules(order).is_empty()
        || !order.ptb_stop_loss_rules_json.is_empty()
        || !trade_builder_normalized_time_exit_rules(order).is_empty()
}

fn trade_builder_child_rule_price(rule: &TradeBuilderPriceExitRule) -> f64 {
    rule.price.clamp(0.0, 1.0)
}

fn trade_builder_ladder_child_qty(
    canonical_entry_qty: f64,
    size_pct: f64,
) -> Option<TradeBuilderExitChildSizing> {
    let qty = round_trade_builder_share_qty(canonical_entry_qty * (size_pct / 100.0));
    (qty > 0.0).then_some(TradeBuilderExitChildSizing {
        size_usdc: 0.0,
        target_qty: qty,
        remaining_qty: qty,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderLadderTargetPlan<K: Copy + PartialEq> {
    requested_targets: Vec<(K, f64)>,
    targets: Vec<(K, f64)>,
    skipped_keys: Vec<K>,
    consolidation_target: Option<K>,
}

fn trade_builder_plan_weighted_ladder_targets<K: Copy + PartialEq>(
    weighted_keys: &[(K, f64)],
    current_parent_qty: f64,
    order_min_size: Option<f64>,
) -> TradeBuilderLadderTargetPlan<K> {
    let current_parent_qty = round_trade_builder_share_qty(current_parent_qty.max(0.0));
    if current_parent_qty <= TRADE_BUILDER_EXIT_QTY_TOLERANCE || weighted_keys.is_empty() {
        return TradeBuilderLadderTargetPlan {
            requested_targets: Vec::new(),
            targets: Vec::new(),
            skipped_keys: Vec::new(),
            consolidation_target: None,
        };
    }

    let family_weight_sum: f64 = weighted_keys.iter().map(|(_, weight_pct)| *weight_pct).sum();
    if family_weight_sum <= 0.0 {
        return TradeBuilderLadderTargetPlan {
            requested_targets: Vec::new(),
            targets: Vec::new(),
            skipped_keys: Vec::new(),
            consolidation_target: None,
        };
    }

    let mut requested_targets = Vec::new();
    let mut assigned_qty = 0.0;
    for (index, (key, weight_pct)) in weighted_keys.iter().enumerate() {
        let is_last = index + 1 == weighted_keys.len();
        let desired_qty = if is_last {
            round_trade_builder_share_qty((current_parent_qty - assigned_qty).max(0.0))
        } else {
            round_trade_builder_share_qty(current_parent_qty * (*weight_pct / family_weight_sum))
        };
        if desired_qty <= 0.0 {
            continue;
        }
        assigned_qty = round_trade_builder_share_qty((assigned_qty + desired_qty).max(0.0));
        requested_targets.push((*key, desired_qty));
    }

    let Some(order_min_size) = normalize_trade_builder_market_spec_number(order_min_size) else {
        return TradeBuilderLadderTargetPlan {
            requested_targets: requested_targets.clone(),
            targets: requested_targets,
            skipped_keys: Vec::new(),
            consolidation_target: None,
        };
    };

    let kept_targets = requested_targets
        .iter()
        .copied()
        .filter(|(_, qty)| *qty >= order_min_size)
        .collect::<Vec<_>>();
    if kept_targets.is_empty() {
        if current_parent_qty < order_min_size {
            return TradeBuilderLadderTargetPlan {
                skipped_keys: requested_targets.iter().map(|(key, _)| *key).collect(),
                requested_targets,
                targets: Vec::new(),
                consolidation_target: None,
            };
        }

        let Some((target_key, _)) = requested_targets.last().copied() else {
            return TradeBuilderLadderTargetPlan {
                requested_targets,
                targets: Vec::new(),
                skipped_keys: Vec::new(),
                consolidation_target: None,
            };
        };
        return TradeBuilderLadderTargetPlan {
            skipped_keys: requested_targets
                .iter()
                .map(|(key, _)| *key)
                .filter(|key| *key != target_key)
                .collect(),
            requested_targets,
            targets: vec![(target_key, current_parent_qty)],
            consolidation_target: Some(target_key),
        };
    }

    let deepest_kept_key = kept_targets.last().map(|(key, _)| *key);
    let mut targets = Vec::new();
    let mut assigned_kept_qty = 0.0;
    for (key, qty) in kept_targets {
        let desired_qty = if Some(key) == deepest_kept_key {
            round_trade_builder_share_qty((current_parent_qty - assigned_kept_qty).max(0.0))
        } else {
            qty
        };
        if desired_qty <= 0.0 {
            continue;
        }
        assigned_kept_qty = round_trade_builder_share_qty((assigned_kept_qty + desired_qty).max(0.0));
        targets.push((key, desired_qty));
    }

    let skipped_keys = requested_targets
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| !targets.iter().any(|(target_key, _)| target_key == key))
        .collect::<Vec<_>>();
    let consolidation_target = (!skipped_keys.is_empty())
        .then_some(deepest_kept_key)
        .flatten();

    TradeBuilderLadderTargetPlan {
        requested_targets,
        targets,
        skipped_keys,
        consolidation_target,
    }
}

fn trade_builder_ladder_rule_weight(order: &TradeBuilderOrder) -> Option<f64> {
    order
        .exit_ladder_size_pct
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn trade_builder_ladder_rule_target_plan(
    rules: &[TradeBuilderPriceExitRule],
    canonical_entry_qty: f64,
    order_min_size: Option<f64>,
) -> TradeBuilderLadderTargetPlan<usize> {
    let weighted_rules = rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (index, rule.size_pct))
        .collect::<Vec<_>>();
    trade_builder_plan_weighted_ladder_targets(
        &weighted_rules,
        canonical_entry_qty,
        order_min_size,
    )
}

fn trade_builder_ladder_family_target_plan(
    family_children: &[&TradeBuilderOrder],
    current_parent_qty: f64,
    order_min_size: Option<f64>,
) -> TradeBuilderLadderTargetPlan<i64> {
    let mut weighted_children = family_children
        .iter()
        .filter_map(|child| {
            trade_builder_ladder_rule_weight(child).map(|weight_pct| (child.id, weight_pct))
        })
        .collect::<Vec<_>>();
    weighted_children.sort_by_key(|(order_id, _)| {
        family_children
            .iter()
            .find(|child| child.id == *order_id)
            .map(|child| (child.exit_ladder_index.unwrap_or(i32::MAX), child.id))
            .unwrap_or((i32::MAX, *order_id))
    });

    trade_builder_plan_weighted_ladder_targets(
        &weighted_children,
        current_parent_qty,
        order_min_size,
    )
}

fn trade_builder_ladder_family_target_qtys(
    family_children: &[&TradeBuilderOrder],
    current_parent_qty: f64,
    order_min_size: Option<f64>,
) -> Vec<(i64, f64)> {
    trade_builder_ladder_family_target_plan(family_children, current_parent_qty, order_min_size).targets
}

fn trade_builder_live_ladder_children<'a>(
    children: &'a [TradeBuilderOrder],
    family: &str,
) -> Vec<&'a TradeBuilderOrder> {
    children
        .iter()
        .filter(|order| {
            trade_builder_exit_ladder_kind(order) == Some(family)
                && !trade_builder_is_terminal_status(&order.status)
        })
        .collect()
}

fn trade_builder_live_hard_exit_children<'a>(children: &'a [TradeBuilderOrder]) -> Vec<&'a TradeBuilderOrder> {
    children
        .iter()
        .filter(|order| trade_builder_is_hard_exit_child(order) && !trade_builder_is_terminal_status(&order.status))
        .collect()
}

async fn trade_builder_sync_ladder_family_remaining_qty(
    repo: &PostgresRepository,
    family_children: &[TradeBuilderOrder],
    current_parent_qty: f64,
    order_min_size: Option<f64>,
    event_reason: &str,
) -> Result<Vec<i64>> {
    let family_child_refs = family_children.iter().collect::<Vec<_>>();
    let target_plan =
        trade_builder_ladder_family_target_plan(&family_child_refs, current_parent_qty, order_min_size);
    let target_qtys = target_plan.targets.clone();
    let family_weight_sum: f64 = family_children
        .iter()
        .filter_map(|order| trade_builder_ladder_rule_weight(order))
        .sum();
    if family_weight_sum <= 0.0 || (target_qtys.is_empty() && target_plan.skipped_keys.is_empty()) {
        return Ok(Vec::new());
    }
    let remainder_close_order_id = target_qtys.last().map(|(order_id, _)| *order_id);
    let consolidation_target = target_plan.consolidation_target;

    let mut updated_order_ids = Vec::new();
    for child in family_children {
        if target_plan.skipped_keys.contains(&child.id) {
            let status_after = if child.active_exchange_order_id.is_some() {
                "canceled_requested"
            } else {
                "canceled"
            };
            repo.set_trade_builder_order_status(
                child.id,
                status_after,
                Some("staged_child_skipped_due_to_min_size"),
            )
            .await?;
            let requested_target_qty = target_plan
                .requested_targets
                .iter()
                .find_map(|(order_id, qty)| (*order_id == child.id).then_some(*qty));
            repo.append_trade_builder_order_event(
                child.id,
                "staged_child_skipped_due_to_min_size",
                &json!({
                    "reason": event_reason,
                    "current_parent_qty": current_parent_qty,
                    "order_min_size": order_min_size,
                    "requested_target_qty": requested_target_qty,
                    "status_before": &child.status,
                    "status_after": status_after,
                    "family": trade_builder_exit_family(child),
                    "exit_mode": trade_builder_exit_mode(child),
                    "sibling_policy": trade_builder_exit_sibling_policy(child),
                    "active_exchange_order_id": child.active_exchange_order_id,
                }),
            )
            .await?;
            updated_order_ids.push(child.id);
            continue;
        }
        let Some(weight_pct) = trade_builder_ladder_rule_weight(child) else {
            continue;
        };
        let Some((_, desired_qty)) = target_qtys
            .iter()
            .find(|(order_id, _)| *order_id == child.id)
        else {
            continue;
        };
        let desired_qty = *desired_qty;
        let desired_size_usdc = trade_builder_scaled_size_usdc(child, desired_qty);
        if child.active_exchange_order_id.is_some() {
            repo.set_trade_builder_order_working_state(
                child.id,
                child.active_exchange_order_id.as_deref(),
                child.working_price,
                Some(desired_size_usdc),
                Some(desired_qty),
                &child.status,
            )
            .await?;
        } else {
            repo.update_trade_builder_order_sizing_and_state(
                child.id,
                &child.size_basis,
                desired_size_usdc,
                Some(desired_qty),
                Some(desired_size_usdc),
                Some(desired_qty),
                &child.status,
                child.last_error.as_deref(),
                child.eligible_after_at,
                child.eligible_before_at,
                None,
                None,
                None,
            )
            .await?;
        }
        repo.append_trade_builder_order_event(
            child.id,
            "ladder_sibling_resized",
            &json!({
                "reason": event_reason,
                "current_parent_qty": current_parent_qty,
                "weight_pct": weight_pct,
                "family_weight_sum": family_weight_sum,
                "next_target_qty": desired_qty,
                "remaining_qty_source": if Some(child.id) == remainder_close_order_id {
                    "remainder_close"
                } else {
                    "family_weight_sync"
                },
                "status_after": &child.status,
            }),
        )
        .await?;
        if Some(child.id) == consolidation_target {
            let skipped_order_ids = target_plan
                .skipped_keys
                .iter()
                .copied()
                .filter(|order_id| *order_id != child.id)
                .collect::<Vec<_>>();
            repo.append_trade_builder_order_event(
                child.id,
                "staged_family_consolidated_due_to_min_size",
                &json!({
                    "reason": event_reason,
                    "current_parent_qty": current_parent_qty,
                    "order_min_size": order_min_size,
                    "next_target_qty": desired_qty,
                    "skipped_order_ids": skipped_order_ids,
                    "family": trade_builder_exit_family(child),
                    "exit_mode": trade_builder_exit_mode(child),
                    "sibling_policy": trade_builder_exit_sibling_policy(child),
                }),
            )
            .await?;
        }
        updated_order_ids.push(child.id);
    }

    Ok(updated_order_ids)
}

async fn trade_builder_sync_hard_exit_remaining_qty(
    repo: &PostgresRepository,
    hard_children: &[TradeBuilderOrder],
    current_parent_qty: f64,
    event_reason: &str,
) -> Result<Vec<i64>> {
    let mut updated_order_ids = Vec::new();
    for child in hard_children {
        if child.active_exchange_order_id.is_some() {
            continue;
        }
        let current_qty = trade_builder_share_remaining_qty(child).unwrap_or_default();
        let desired_qty = round_trade_builder_share_qty(current_parent_qty);
        if desired_qty <= 0.0
            || (current_qty - desired_qty).abs() < TRADE_BUILDER_EXIT_QTY_TOLERANCE
        {
            continue;
        }
        let desired_size_usdc = trade_builder_scaled_size_usdc(child, desired_qty);
        repo.update_trade_builder_order_sizing_and_state(
            child.id,
            &child.size_basis,
            desired_size_usdc,
            Some(desired_qty),
            Some(desired_size_usdc),
            Some(desired_qty),
            &child.status,
            child.last_error.as_deref(),
            child.eligible_after_at,
            child.eligible_before_at,
            None,
            None,
            None,
        )
        .await?;
        repo.append_trade_builder_order_event(
            child.id,
            "hard_exit_resized",
            &json!({
                "reason": event_reason,
                "family": trade_builder_exit_family(child),
                "exit_mode": trade_builder_exit_mode(child),
                "sibling_policy": trade_builder_exit_sibling_policy(child),
                "current_parent_qty": current_parent_qty,
                "next_target_qty": desired_qty,
                "status_after": &child.status,
            }),
        )
        .await?;
        updated_order_ids.push(child.id);
    }

    Ok(updated_order_ids)
}

async fn trade_builder_cancel_exit_children_without_inventory(
    repo: &PostgresRepository,
    parent_order_id: i64,
    children: &[TradeBuilderOrder],
    event_reason: &str,
) -> Result<Vec<i64>> {
    let mut canceled_order_ids = Vec::new();
    for child in children {
        if !trade_builder_is_child_exit_sell(child)
            || trade_builder_is_terminal_status(&child.status)
            || child.status == "canceled_requested"
        {
            continue;
        }
        let status_after = if child.active_exchange_order_id.is_some() {
            "canceled_requested"
        } else {
            "canceled"
        };
        repo.set_trade_builder_order_status(
            child.id,
            status_after,
            Some("parent_inventory_depleted"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            child.id,
            "exit_canceled_no_inventory",
            &json!({
                "reason": event_reason,
                "parent_order_id": parent_order_id,
                "status_before": &child.status,
                "status_after": status_after,
                "family": trade_builder_exit_family(child),
                "exit_mode": trade_builder_exit_mode(child),
                "sibling_policy": trade_builder_exit_sibling_policy(child),
                "active_exchange_order_id": child.active_exchange_order_id,
            }),
        )
        .await?;
        canceled_order_ids.push(child.id);
    }

    Ok(canceled_order_ids)
}

async fn trade_builder_sync_parent_exit_children(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    parent_order: &TradeBuilderOrder,
    reason: &str,
) -> Result<Vec<i64>> {
    let children = repo
        .list_trade_builder_child_orders_by_parent(parent_order.id, None)
        .await?;
    let current_parent_qty = resolve_trade_builder_parent_exit_inventory(repo, parent_order, reason)
        .await?
        .map(|(qty, _)| qty)
        .unwrap_or_default();
    let order_min_size = resolve_trade_builder_order_min_size(cfg, parent_order).await;
    if current_parent_qty <= TRADE_BUILDER_EXIT_QTY_TOLERANCE {
        return trade_builder_cancel_exit_children_without_inventory(
            repo,
            parent_order.id,
            &children,
            reason,
        )
        .await;
    }

    let mut updated_order_ids = Vec::new();
    let live_hard_children = trade_builder_live_hard_exit_children(&children);
    let live_tp_children = trade_builder_live_ladder_children(
        &children,
        TRADE_BUILDER_EXIT_LADDER_KIND_TP,
    );
    let live_sl_children = trade_builder_live_ladder_children(
        &children,
        TRADE_BUILDER_EXIT_LADDER_KIND_SL,
    );
    let live_ptb_sl_children = trade_builder_live_ladder_children(
        &children,
        TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL,
    );
    updated_order_ids.extend(
        trade_builder_sync_hard_exit_remaining_qty(
            repo,
            &live_hard_children.into_iter().cloned().collect::<Vec<_>>(),
            current_parent_qty,
            reason,
        )
        .await?,
    );
    updated_order_ids.extend(
        trade_builder_sync_ladder_family_remaining_qty(
            repo,
            &live_tp_children.into_iter().cloned().collect::<Vec<_>>(),
            current_parent_qty,
            order_min_size,
            reason,
        )
        .await?,
    );
    updated_order_ids.extend(
        trade_builder_sync_ladder_family_remaining_qty(
            repo,
            &live_sl_children.into_iter().cloned().collect::<Vec<_>>(),
            current_parent_qty,
            order_min_size,
            reason,
        )
        .await?,
    );
    updated_order_ids.extend(
        trade_builder_sync_ladder_family_remaining_qty(
            repo,
            &live_ptb_sl_children.into_iter().cloned().collect::<Vec<_>>(),
            current_parent_qty,
            order_min_size,
            reason,
        )
        .await?,
    );

    Ok(updated_order_ids)
}

#[cfg(test)]
mod exit_ladder_tests {
    use super::*;
    use chrono::Utc;

    fn test_builder_order(side: &str, parent_order_id: Option<i64>) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "conditional".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: side.to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_above".to_string()),
            trigger_price: Some(0.8),
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn price_exit_rules_require_exact_total() {
        let rules = vec![
            TradeBuilderPriceExitRule {
                price: 0.60,
                size_pct: 40.0,
            },
            TradeBuilderPriceExitRule {
                price: 0.70,
                size_pct: 50.0,
            },
        ];
        assert!(validate_trade_builder_price_exit_rules(
            &rules,
            TRADE_BUILDER_EXIT_LADDER_KIND_TP
        )
        .is_err());
    }

    #[test]
    fn time_exit_rules_require_strictly_increasing_elapsed_minutes() {
        let rules = vec![
            TradeBuilderTimeExitRule {
                elapsed_minutes: 12,
                remaining_pct: 20.0,
            },
            TradeBuilderTimeExitRule {
                elapsed_minutes: 12,
                remaining_pct: 100.0,
            },
        ];
        assert!(validate_trade_builder_time_exit_rules(&rules).is_err());
    }

    #[test]
    fn hard_exit_rule_can_coexist_with_staged_rules() {
        let order = TradeBuilderOrder {
            tp_enabled: true,
            tp_price: Some(0.82),
            tp_rules_json: vec![TradeBuilderPriceExitRule {
                price: 0.70,
                size_pct: 100.0,
            }],
            ..test_builder_order("buy", None)
        };

        assert_eq!(
            trade_builder_hard_tp_rule(&order),
            Some(TradeBuilderPriceExitRule {
                price: 0.82,
                size_pct: 100.0,
            })
        );
        assert_eq!(trade_builder_normalized_tp_rules(&order).len(), 1);
        assert!(trade_builder_order_has_exit_ladders(&order));
    }

    #[test]
    fn staged_children_use_resize_policy_and_hard_children_cancel_all() {
        let mut staged = test_builder_order("sell", Some(9));
        staged.exit_ladder_kind = Some("sl".to_string());

        let mut staged_ptb = test_builder_order("sell", Some(9));
        staged_ptb.exit_ladder_kind = Some("ptb_sl".to_string());

        let hard = test_builder_order("sell", Some(9));

        assert_eq!(
            trade_builder_exit_sibling_policy(&staged),
            TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
        );
        assert_eq!(
            trade_builder_exit_sibling_policy(&staged_ptb),
            TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
        );
        assert_eq!(
            trade_builder_exit_sibling_policy(&hard),
            TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL
        );
    }

    #[test]
    fn ptb_stop_loss_family_isolated_from_classic_sl_family() {
        let mut classic_sl = test_builder_order("sell", Some(9));
        classic_sl.id = 71;
        classic_sl.exit_ladder_kind = Some("sl".to_string());
        classic_sl.exit_ladder_index = Some(0);
        classic_sl.exit_ladder_size_pct = Some(50.0);

        let mut staged_ptb = test_builder_order("sell", Some(9));
        staged_ptb.id = 72;
        staged_ptb.exit_ladder_kind = Some("ptb_sl".to_string());
        staged_ptb.exit_ladder_index = Some(0);
        staged_ptb.exit_ladder_size_pct = Some(25.0);

        let classic_targets =
            trade_builder_ladder_family_target_qtys(&[&classic_sl], 3.2, None);
        let ptb_targets =
            trade_builder_ladder_family_target_qtys(&[&staged_ptb], 3.2, None);

        assert_eq!(classic_targets, vec![(71, 3.2)]);
        assert_eq!(ptb_targets, vec![(72, 3.2)]);
        assert_eq!(
            trade_builder_exit_family(&staged_ptb),
            Some(TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL)
        );
    }

    #[test]
    fn ladder_family_target_qtys_make_last_stage_absorb_rounding_remainder() {
        let mut first_tp = test_builder_order("sell", Some(9));
        first_tp.id = 11;
        first_tp.exit_ladder_kind = Some("tp".to_string());
        first_tp.exit_ladder_index = Some(0);
        first_tp.exit_ladder_size_pct = Some(33.33);

        let mut second_tp = first_tp.clone();
        second_tp.id = 12;
        second_tp.exit_ladder_index = Some(1);

        let mut third_tp = first_tp.clone();
        third_tp.id = 13;
        third_tp.exit_ladder_index = Some(2);
        third_tp.exit_ladder_size_pct = Some(33.34);

        let targets = trade_builder_ladder_family_target_qtys(
            &[&first_tp, &second_tp, &third_tp],
            1.0,
            None,
        );

        assert_eq!(targets, vec![(11, 0.33), (12, 0.33), (13, 0.34)]);
    }

    #[test]
    fn ladder_family_target_qtys_make_single_live_last_stage_full_close() {
        let mut last_tp = test_builder_order("sell", Some(9));
        last_tp.id = 22;
        last_tp.exit_ladder_kind = Some("tp".to_string());
        last_tp.exit_ladder_index = Some(1);
        last_tp.exit_ladder_size_pct = Some(50.0);

        let targets = trade_builder_ladder_family_target_qtys(&[&last_tp], 1.41, None);

        assert_eq!(targets, vec![(22, 1.41)]);
    }

    #[test]
    fn ladder_family_target_qtys_consolidate_all_sub_min_stages_into_last_stage() {
        let mut first_sl = test_builder_order("sell", Some(9));
        first_sl.id = 31;
        first_sl.exit_ladder_kind = Some("sl".to_string());
        first_sl.exit_ladder_index = Some(0);
        first_sl.exit_ladder_size_pct = Some(50.0);

        let mut second_sl = first_sl.clone();
        second_sl.id = 32;
        second_sl.exit_ladder_index = Some(1);

        let targets =
            trade_builder_ladder_family_target_qtys(&[&first_sl, &second_sl], 7.57, Some(5.0));

        assert_eq!(targets, vec![(32, 7.57)]);
    }

    #[test]
    fn ladder_family_target_qtys_pushes_sub_min_qty_into_deepest_surviving_stage() {
        let mut first_sl = test_builder_order("sell", Some(9));
        first_sl.id = 41;
        first_sl.exit_ladder_kind = Some("sl".to_string());
        first_sl.exit_ladder_index = Some(0);
        first_sl.exit_ladder_size_pct = Some(30.0);

        let mut second_sl = first_sl.clone();
        second_sl.id = 42;
        second_sl.exit_ladder_index = Some(1);
        second_sl.exit_ladder_size_pct = Some(70.0);

        let targets =
            trade_builder_ladder_family_target_qtys(&[&first_sl, &second_sl], 8.0, Some(5.0));

        assert_eq!(targets, vec![(42, 8.0)]);
    }

    #[test]
    fn ladder_family_target_qtys_return_empty_when_total_qty_is_below_minimum() {
        let mut first_sl = test_builder_order("sell", Some(9));
        first_sl.id = 51;
        first_sl.exit_ladder_kind = Some("sl".to_string());
        first_sl.exit_ladder_index = Some(0);
        first_sl.exit_ladder_size_pct = Some(50.0);

        let mut second_sl = first_sl.clone();
        second_sl.id = 52;
        second_sl.exit_ladder_index = Some(1);

        let targets =
            trade_builder_ladder_family_target_qtys(&[&first_sl, &second_sl], 4.2, Some(5.0));

        assert!(targets.is_empty());
    }
}
