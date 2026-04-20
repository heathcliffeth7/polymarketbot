use super::notification::{
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_recovered_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
};
use super::notification_state::{
    clear_price_to_beat_guard_waiting_context, price_to_beat_guard_notification_phase,
    price_to_beat_guard_waiting_state, set_price_to_beat_guard_notification_phase,
    set_price_to_beat_guard_notification_seed, set_price_to_beat_guard_waiting_state,
    PriceToBeatGuardNotificationPhase,
};
use super::*;
use chrono::Duration as ChronoDuration;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PriceToBeatGuardRuntimeOptions {
    pub(crate) send_guard_notifications: bool,
    pub(crate) send_relax_notifications: bool,
}

impl PriceToBeatGuardRuntimeOptions {
    pub(crate) const fn standard_action_place_order() -> Self {
        Self {
            send_guard_notifications: true,
            send_relax_notifications: true,
        }
    }

    pub(crate) const fn pair_lock_auto_primary() -> Self {
        Self {
            send_guard_notifications: false,
            send_relax_notifications: true,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PriceToBeatGuardRuntimeContext<'a> {
    pub(crate) repo: &'a crate::PostgresRepository,
    pub(crate) user_id: i64,
    pub(crate) cfg: &'a crate::AppConfig,
    pub(crate) client: Option<&'a dyn crate::OrderExecutor>,
    pub(crate) options: PriceToBeatGuardRuntimeOptions,
}

impl<'a> PriceToBeatGuardRuntimeContext<'a> {
    pub(crate) fn standard_action_place_order(
        repo: &'a crate::PostgresRepository,
        user_id: i64,
        cfg: &'a crate::AppConfig,
        client: Option<&'a dyn crate::OrderExecutor>,
    ) -> Self {
        Self {
            repo,
            user_id,
            cfg,
            client,
            options: PriceToBeatGuardRuntimeOptions::standard_action_place_order(),
        }
    }

    pub(crate) fn pair_lock_auto_primary(
        repo: &'a crate::PostgresRepository,
        user_id: i64,
        cfg: &'a crate::AppConfig,
        client: Option<&'a dyn crate::OrderExecutor>,
    ) -> Self {
        Self {
            repo,
            user_id,
            cfg,
            client,
            options: PriceToBeatGuardRuntimeOptions::pair_lock_auto_primary(),
        }
    }
}

pub(crate) async fn maybe_block_action_place_order_price_to_beat_guard(
    repo: &crate::PostgresRepository,
    cfg: &crate::AppConfig,
    client: Option<&dyn crate::OrderExecutor>,
    run: &crate::TradeFlowRun,
    node: &crate::TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
) -> Result<Option<crate::TradeFlowNodeExecution>> {
    crate::set_flow_context(context, "priceToBeatGuard", Value::Null);

    if side != "buy" {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }

    let guard_enabled = crate::node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false);
    if !guard_enabled {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }
    let runtime = PriceToBeatGuardRuntimeContext::standard_action_place_order(
        repo,
        run.user_id,
        cfg,
        client,
    );
    let retry_on_guard_block =
        crate::node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true);
    let evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        Some(runtime),
        context,
        node,
        run.id,
        market_slug,
        outcome_label,
    )
    .await?;
    let evaluation_output = evaluation.to_value();
    crate::set_flow_context(context, "priceToBeatGuard", evaluation_output.clone());
    let should_notify = runtime.options.send_guard_notifications
        && crate::node_config_bool(node, "notifyOnPriceToBeatGapBlocked").unwrap_or(true);
    let notification_phase =
        price_to_beat_guard_notification_phase(context, &node.key, market_slug, token_id);
    if evaluation.passed {
        let waiting_state = price_to_beat_guard_waiting_state(context);
        let recovered_from_reason_code = waiting_state
            .as_ref()
            .and_then(|prev| (prev.market_slug == market_slug).then(|| prev.reason_code.as_str()));
        if let Some(recovered_from_reason_code) = recovered_from_reason_code {
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "price_to_beat_guard_recovered",
                &json!({
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "market_slug": market_slug,
                    "token_id": token_id,
                    "outcome_label": outcome_label,
                    "side": side,
                    "execution_mode": execution_mode,
                    "recovered_from_reason_code": recovered_from_reason_code,
                    "price_to_beat_guard": evaluation_output.clone(),
                }),
            )
            .await?;

            if should_notify
                && notification_phase == Some(PriceToBeatGuardNotificationPhase::BlockedNotified)
            {
                let message = build_price_to_beat_guard_recovered_notification_message(
                    &evaluation,
                    recovered_from_reason_code,
                );
                if send_price_to_beat_guard_notification(repo, runtime.user_id, &message).await {
                    set_price_to_beat_guard_notification_phase(
                        context,
                        &node.key,
                        market_slug,
                        token_id,
                        PriceToBeatGuardNotificationPhase::PassedNotified,
                        recovered_from_reason_code,
                    );
                }
            }
        }
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pre_order_price_to_beat_blocked",
        &json!({
            "node_key": node.key,
            "node_type": node.node_type,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "price_to_beat_guard": evaluation_output.clone(),
        }),
    )
    .await?;

    let candidate_reason =
        crate::build_guard_notification_reason("price_to_beat", &evaluation.reason_code);
    if retry_on_guard_block {
        let entered_waiting = match price_to_beat_guard_waiting_state(context) {
            Some(prev) => {
                prev.market_slug != market_slug || prev.reason_code != evaluation.reason_code
            }
            None => true,
        };
        set_price_to_beat_guard_waiting_state(context, market_slug, &evaluation.reason_code);
        if entered_waiting && notification_phase.is_none() && should_notify {
            let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
            if send_price_to_beat_guard_notification(repo, runtime.user_id, &message).await {
                set_price_to_beat_guard_notification_seed(
                    context,
                    &node.key,
                    market_slug,
                    token_id,
                    &candidate_reason,
                );
                set_price_to_beat_guard_notification_phase(
                    context,
                    &node.key,
                    market_slug,
                    token_id,
                    PriceToBeatGuardNotificationPhase::BlockedNotified,
                    &evaluation.reason_code,
                );
            }
        } else if entered_waiting {
            set_price_to_beat_guard_notification_phase(
                context,
                &node.key,
                market_slug,
                token_id,
                PriceToBeatGuardNotificationPhase::BlockedNotified,
                &evaluation.reason_code,
            );
        }
        let repeat_at = crate::Utc::now()
            + ChronoDuration::milliseconds(crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS);
        return Ok(Some(crate::TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "blocked": true,
                "reason": "price_to_beat_guard_blocked",
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "side": side,
                "execution_mode": execution_mode,
                "retrying": true,
                "retry_delay_ms": crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS,
                "price_to_beat_guard": evaluation_output,
            }),
            routes: vec![],
            repeat_at: Some(repeat_at),
            repeat_idempotency_key: None,
        }));
    }
    if should_notify && notification_phase.is_none() {
        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
        if send_price_to_beat_guard_notification(repo, runtime.user_id, &message).await {
            set_price_to_beat_guard_notification_seed(
                context,
                &node.key,
                market_slug,
                token_id,
                &candidate_reason,
            );
            set_price_to_beat_guard_notification_phase(
                context,
                &node.key,
                market_slug,
                token_id,
                PriceToBeatGuardNotificationPhase::BlockedNotified,
                &evaluation.reason_code,
            );
        }
    }

    Ok(Some(crate::TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "reason": "price_to_beat_guard_blocked",
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "price_to_beat_guard": evaluation_output,
        }),
        routes: vec![crate::TradeFlowRouteDecision {
            edge_type: "on_error".to_string(),
            available_at: crate::Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

pub(crate) async fn evaluate_action_place_order_price_to_beat_guard_state(
    runtime: Option<PriceToBeatGuardRuntimeContext<'_>>,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    run_id: i64,
    market_slug: &str,
    outcome_label: &str,
) -> Result<PriceToBeatGuardEvaluation> {
    max_price_relax::ensure_max_price_relax_tracking_market(context, &node.key, market_slug);
    let resolution = resolve_action_place_order_price_to_beat_guard_resolution(
        node,
        context,
        market_slug,
        outcome_label,
    )?;
    if let Some(current_effective_ptb_usd) = resolution.current_effective_ptb_usd {
        let sync_resolution =
            crate::resolve_action_place_order_ptb_current_effective_threshold_resolution(
                &*context,
                node,
                &node.key,
                market_slug,
                outcome_label,
                Some(current_effective_ptb_usd),
                resolution.stop_loss_bump_usd,
            )
            .expect("current effective ptb resolution should exist when threshold exists");
        crate::sync_action_place_order_ptb_current_effective_threshold_state(
            context,
            node,
            &node.key,
            market_slug,
            outcome_label,
            &sync_resolution,
            crate::Utc::now(),
            "guard_eval",
        );
    }
    let mut evaluation = evaluate_price_to_beat_guard(
        market_slug,
        resolution.effective_mode,
        resolution.threshold_value,
        resolution.threshold_unit,
        outcome_label,
    )
    .await;
    resolution.apply_metadata(&mut evaluation);
    if resolution.effective_mode != PriceToBeatMode::Manual {
        apply_price_to_beat_risk_penalty(&mut evaluation, resolution.stop_loss_bump_usd);
    }
    if let Some(runtime) = runtime {
        if runtime.options.send_relax_notifications {
            max_price_relax::maybe_apply_action_place_order_max_price_relaxation(
                runtime.repo,
                runtime.user_id,
                context,
                node,
                run_id,
                market_slug,
                outcome_label,
                runtime.cfg,
                runtime.client,
                &mut evaluation,
            )
            .await?;
        } else {
            preview_action_place_order_max_price_relaxation(
                runtime.repo,
                context,
                node,
                run_id,
                market_slug,
                outcome_label,
                &mut evaluation,
            )
            .await?;
        }
    }
    Ok(evaluation)
}

pub(crate) async fn preview_action_place_order_max_price_relaxation(
    repo: &crate::PostgresRepository,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    run_id: i64,
    market_slug: &str,
    outcome_label: &str,
    evaluation: &mut PriceToBeatGuardEvaluation,
) -> Result<()> {
    if let Some(relaxation) = super::max_price_relax::preview_action_place_order_max_price_relaxation_state(
        repo,
        context,
        node,
        run_id,
        market_slug,
        outcome_label,
        evaluation,
    )
    .await?
    {
        evaluation.max_price_relax = Some(relaxation.to_value());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_runtime_keeps_guard_and_relax_notifications_enabled() {
        let options = PriceToBeatGuardRuntimeOptions::standard_action_place_order();

        assert!(options.send_guard_notifications);
        assert!(options.send_relax_notifications);
    }

    #[test]
    fn pair_lock_runtime_disables_guard_notifications_but_keeps_relax_notifications() {
        let options = PriceToBeatGuardRuntimeOptions::pair_lock_auto_primary();

        assert!(!options.send_guard_notifications);
        assert!(options.send_relax_notifications);
    }
}
