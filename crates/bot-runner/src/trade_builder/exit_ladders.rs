use serde::Deserialize;

const TRADE_BUILDER_EXIT_LADDER_KIND_TP: &str = "tp";
const TRADE_BUILDER_EXIT_LADDER_KIND_SL: &str = "sl";
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
    trade_builder_exit_ladder_kind(order)
        .is_some_and(|value| matches!(value, TRADE_BUILDER_EXIT_LADDER_KIND_TP | TRADE_BUILDER_EXIT_LADDER_KIND_SL))
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

fn trade_builder_ladder_rule_weight(order: &TradeBuilderOrder) -> Option<f64> {
    order
        .exit_ladder_size_pct
        .filter(|value| value.is_finite() && *value > 0.0)
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
    event_reason: &str,
) -> Result<Vec<i64>> {
    let family_weight_sum: f64 = family_children
        .iter()
        .filter_map(|order| trade_builder_ladder_rule_weight(order))
        .sum();
    if family_weight_sum <= 0.0 {
        return Ok(Vec::new());
    }

    let mut updated_order_ids = Vec::new();
    for child in family_children {
        if child.active_exchange_order_id.is_some() {
            continue;
        }
        let Some(weight_pct) = trade_builder_ladder_rule_weight(child) else {
            continue;
        };
        let desired_qty =
            round_trade_builder_share_qty(current_parent_qty * (weight_pct / family_weight_sum));
        if desired_qty <= 0.0 {
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
            "ladder_sibling_resized",
            &json!({
                "reason": event_reason,
                "current_parent_qty": current_parent_qty,
                "weight_pct": weight_pct,
                "family_weight_sum": family_weight_sum,
                "next_target_qty": desired_qty,
                "status_after": &child.status,
            }),
        )
        .await?;
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
            reason,
        )
        .await?,
    );
    updated_order_ids.extend(
        trade_builder_sync_ladder_family_remaining_qty(
            repo,
            &live_sl_children.into_iter().cloned().collect::<Vec<_>>(),
            current_parent_qty,
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
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
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

        let hard = test_builder_order("sell", Some(9));

        assert_eq!(
            trade_builder_exit_sibling_policy(&staged),
            TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
        );
        assert_eq!(
            trade_builder_exit_sibling_policy(&hard),
            TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL
        );
    }
}
