use super::block_event_coalescing::coalesce_pre_order_price_to_beat_block_event;
use super::iv_depth_diagnostics::{
    annotate_price_to_beat_iv_book_not_requested_for_early_block,
    hydrate_action_place_order_iv_mismatch_book_quotes, price_to_beat_iv_early_block_can_skip_book,
    price_to_beat_iv_mismatch_needs_book_hydration,
};
use super::iv_mismatch_adaptive::{
    PriceToBeatIvAdaptiveVolumeInput, PriceToBeatIvVolumeBaselineMode,
};
use super::iv_mismatch_protection::PriceToBeatIvProtectionMode;
use super::notification::{
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_recovered_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
};
use super::notification_state::{
    clear_price_to_beat_guard_waiting_context, price_to_beat_guard_notification_phase,
    price_to_beat_guard_waiting_state, set_price_to_beat_guard_notification_phase,
    set_price_to_beat_guard_notification_seed, set_price_to_beat_guard_waiting_state_with_snapshot,
    PriceToBeatGuardNotificationPhase, PriceToBeatGuardWaitingSnapshot,
};
use super::retry_policy::{
    clear_early_stale_side_guard_retry_count, early_stale_side_guard_retry_limit_reached,
    early_stale_side_retry_limit_execution, price_to_beat_guard_retry_delay_ms_for_reason,
};
use super::wait_reprice_guard::{
    maybe_apply_wait_reprice_guard, wait_reprice_reason_disables_retry,
    wait_reprice_snapshot_from_evaluation,
};
use super::*;
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use serde_json::{json, Value};

const PRICE_TO_BEAT_IV_MISMATCH_EVENT_TYPE: &str = "price_to_beat_iv_mismatch_edge_decision";
const FLOW_NODE_STATE_IV_MISMATCH_DECISION_SIGNATURE: &str =
    "price_to_beat_iv_mismatch_edge_decision_signature";

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
    action_yes_token_id: Option<&str>,
    action_no_token_id: Option<&str>,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
    signal_market: Option<PriceToBeatSignalFormulaMarketInput>,
) -> Result<Option<crate::TradeFlowNodeExecution>> {
    crate::set_flow_context(context, "priceToBeatGuard", Value::Null);
    crate::set_flow_context(context, "cexDirectionGuard", Value::Null);
    crate::set_flow_context(context, "priceToBeatIvSelectedMaxPrice", Value::Null);

    if side != "buy" {
        clear_price_to_beat_guard_waiting_context(context);
        clear_early_stale_side_guard_retry_count(context, &node.key, market_slug);
        return Ok(None);
    }

    let guard_enabled = crate::node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false);
    let cex_direction_guard_enabled =
        crate::node_config_bool(node, "cexDirectionGuardEnabled").unwrap_or(false);
    if !guard_enabled && !cex_direction_guard_enabled {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }
    if !guard_enabled {
        return super::cex_direction_guard::maybe_block_action_place_order_cex_direction_guard_only(
            repo,
            run,
            node,
            context,
            market_slug,
            token_id,
            outcome_label,
            side,
            execution_mode,
        )
        .await;
    }
    let runtime =
        PriceToBeatGuardRuntimeContext::standard_action_place_order(repo, run.user_id, cfg, client);
    let retry_on_guard_block =
        crate::node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true);
    let mut evaluation = evaluate_action_place_order_price_to_beat_guard_state(
        Some(runtime),
        context,
        node,
        run.id,
        Some(run.definition_id),
        market_slug,
        Some(token_id),
        action_yes_token_id,
        action_no_token_id,
        outcome_label,
        signal_market,
    )
    .await?;
    let guard_evaluated_at_ms = Utc::now().timestamp_millis();
    maybe_apply_wait_reprice_guard(node, context, &mut evaluation, guard_evaluated_at_ms);
    let evaluation_output = evaluation.to_value();
    crate::set_flow_context(context, "priceToBeatGuard", evaluation_output.clone());
    maybe_emit_action_place_order_iv_mismatch_edge_decision_event(
        repo,
        run,
        context,
        node,
        market_slug,
        token_id,
        outcome_label,
        side,
        execution_mode,
        &evaluation,
        &evaluation_output,
    )
    .await?;
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
                && notification_phase.as_ref().map(|entry| entry.phase)
                    == Some(PriceToBeatGuardNotificationPhase::BlockedNotified)
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

    let candidate_reason =
        crate::build_guard_notification_reason("price_to_beat", &evaluation.reason_code);
    let should_retry_guard_block = retry_on_guard_block
        && !wait_reprice_reason_disables_retry(&evaluation.reason_code)
        && !price_to_beat_retry_window_elapsed(&evaluation);
    let early_stale_retry_limit_reached = if should_retry_guard_block {
        early_stale_side_guard_retry_limit_reached(context, node, market_slug)
    } else {
        false
    };
    let mut blocked_event_payload = json!({
        "node_key": node.key,
        "node_type": node.node_type,
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "side": side,
        "execution_mode": execution_mode,
        "price_to_beat_guard": evaluation_output.clone(),
    });
    let block_event_coalescing = if should_retry_guard_block && !early_stale_retry_limit_reached {
        coalesce_pre_order_price_to_beat_block_event(
            context,
            run.id,
            &node.key,
            market_slug,
            token_id,
            "price_to_beat",
            &evaluation.reason_code,
            &blocked_event_payload,
            guard_evaluated_at_ms,
        )
    } else {
        super::block_event_coalescing::BlockEventCoalescingOutcome {
            emit: true,
            summary: None,
        }
    };
    if let Some(summary) = block_event_coalescing.summary {
        if let Some(payload) = blocked_event_payload.as_object_mut() {
            payload.insert("coalesced_summary".to_string(), summary);
        }
    }
    if block_event_coalescing.emit {
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "pre_order_price_to_beat_blocked",
            &blocked_event_payload,
        )
        .await?;
    }
    if should_retry_guard_block {
        let retry_delay_ms =
            price_to_beat_guard_retry_delay_ms_for_reason(node, &evaluation.reason_code);
        if early_stale_retry_limit_reached {
            return Ok(Some(early_stale_side_retry_limit_execution(
                node,
                market_slug,
                token_id,
                outcome_label,
                side,
                execution_mode,
                &evaluation_output,
            )));
        }
        let entered_waiting = match price_to_beat_guard_waiting_state(context) {
            Some(prev) => {
                prev.market_slug != market_slug || prev.reason_code != evaluation.reason_code
            }
            None => true,
        };
        let wait_snapshot = wait_reprice_snapshot_from_evaluation(&evaluation);
        set_price_to_beat_guard_waiting_state_with_snapshot(
            context,
            market_slug,
            &evaluation.reason_code,
            PriceToBeatGuardWaitingSnapshot {
                now_ms: Some(guard_evaluated_at_ms),
                execution_ask_cent: wait_snapshot.execution_ask_cent,
                gap_strength: wait_snapshot.gap_strength,
                q_final_cent: wait_snapshot.q_final_cent,
            },
        );
        if entered_waiting
            && notification_phase
                .as_ref()
                .map(|entry| entry.reason_code != evaluation.reason_code)
                .unwrap_or(true)
            && should_notify
        {
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
        let repeat_at = crate::Utc::now() + ChronoDuration::milliseconds(retry_delay_ms);
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
                "retry_delay_ms": retry_delay_ms,
                "price_to_beat_guard": evaluation_output,
            }),
            routes: vec![],
            repeat_at: Some(repeat_at),
            repeat_idempotency_key: None,
        }));
    }
    // Dedup: aynı reason_code için tekrar bildirme. Terminal wait-reprice reason'ları
    // dahil tüm blocklar için tek bildirim yeterli; reason değiştiğinde tekrar bildir.
    let notification_reason_changed = notification_phase
        .as_ref()
        .map(|entry| entry.reason_code != evaluation.reason_code)
        .unwrap_or(true);
    if should_notify && notification_reason_changed {
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

fn price_to_beat_retry_window_elapsed(evaluation: &PriceToBeatGuardEvaluation) -> bool {
    evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(|value| value.get("seconds_left"))
        .and_then(Value::as_f64)
        .is_some_and(|seconds_left| seconds_left <= 0.0)
}

async fn maybe_emit_action_place_order_iv_mismatch_edge_decision_event(
    repo: &crate::PostgresRepository,
    run: &crate::TradeFlowRun,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
    evaluation: &PriceToBeatGuardEvaluation,
    evaluation_output: &Value,
) -> Result<()> {
    let Some(iv_mismatch_edge) = evaluation
        .iv_mismatch_edge
        .as_ref()
        .filter(|value| !value.is_null())
    else {
        return Ok(());
    };
    let signature = action_place_order_iv_mismatch_signature(
        market_slug,
        token_id,
        outcome_label,
        evaluation,
        iv_mismatch_edge,
    );
    if !remember_iv_mismatch_decision_signature(context, &node.key, &signature) {
        return Ok(());
    }
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        PRICE_TO_BEAT_IV_MISMATCH_EVENT_TYPE,
        &json!({
            "source": "action_place_order_guard",
            "run_id": run.id,
            "package_version": env!("CARGO_PKG_VERSION"),
            "git_sha": crate::bot_build_git_sha(),
            "build_time": crate::bot_build_time(),
            "config_hash": crate::bot_runtime_run_config_hash(),
            "strategy_config_hash": crate::bot_runtime_config_hash(&node.config),
            "node_config_hash": crate::bot_runtime_config_hash(&node.config),
            "feature_flags_hash": crate::bot_runtime_config_hash(&node.config),
            "node_key": node.key,
            "node_type": node.node_type,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "passed": evaluation.passed,
            "reason_code": evaluation.reason_code,
            "threshold_mode": evaluation.threshold_mode,
            "block_summary": super::iv_mismatch_decision_summary::build_iv_mismatch_block_summary(
                evaluation,
                iv_mismatch_edge,
            ),
            "iv_mismatch_edge": iv_mismatch_edge,
            "price_to_beat_guard": evaluation_output,
        }),
    )
    .await
}

pub(crate) async fn maybe_emit_pair_lock_primary_iv_mismatch_edge_decision_event(
    repo: &crate::PostgresRepository,
    run: &crate::TradeFlowRun,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
    selection_mode: &str,
    selected_primary_token_id: Option<&str>,
    selected_primary_outcome_label: Option<&str>,
    selected_primary_guard_reason: Option<&str>,
    failure_reason: Option<&str>,
    diagnostics: &Value,
) -> Result<()> {
    if !pair_lock_candidate_has_iv_mismatch(diagnostics, "yes_candidate_guard")
        && !pair_lock_candidate_has_iv_mismatch(diagnostics, "no_candidate_guard")
    {
        return Ok(());
    }
    let signature = pair_lock_primary_iv_mismatch_signature(
        market_slug,
        selection_mode,
        selected_primary_token_id
            .or(failure_reason)
            .unwrap_or("none"),
        diagnostics,
    );
    if !remember_iv_mismatch_decision_signature(context, &node.key, &signature) {
        return Ok(());
    }
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        PRICE_TO_BEAT_IV_MISMATCH_EVENT_TYPE,
        &json!({
            "source": "pair_lock_primary_selection",
            "run_id": run.id,
            "package_version": env!("CARGO_PKG_VERSION"),
            "git_sha": crate::bot_build_git_sha(),
            "build_time": crate::bot_build_time(),
            "config_hash": crate::bot_runtime_run_config_hash(),
            "strategy_config_hash": crate::bot_runtime_config_hash(&node.config),
            "node_config_hash": crate::bot_runtime_config_hash(&node.config),
            "feature_flags_hash": crate::bot_runtime_config_hash(&node.config),
            "node_key": node.key,
            "node_type": node.node_type,
            "market_slug": market_slug,
            "selection_mode": selection_mode,
            "selected_primary_token_id": selected_primary_token_id,
            "selected_primary_outcome_label": selected_primary_outcome_label,
            "selected_primary_guard_reason": selected_primary_guard_reason,
            "failure_reason": failure_reason,
            "candidates": {
                "yes": pair_lock_candidate_iv_mismatch_payload(diagnostics, "yes_candidate_guard"),
                "no": pair_lock_candidate_iv_mismatch_payload(diagnostics, "no_candidate_guard"),
            },
            "selection": diagnostics,
        }),
    )
    .await
}

fn remember_iv_mismatch_decision_signature(
    context: &mut Value,
    node_key: &str,
    signature: &str,
) -> bool {
    if crate::flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_IV_MISMATCH_DECISION_SIGNATURE,
    )
    .as_deref()
        == Some(signature)
    {
        return false;
    }
    crate::set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_IV_MISMATCH_DECISION_SIGNATURE,
        json!(signature),
    );
    true
}

fn action_place_order_iv_mismatch_signature(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    evaluation: &PriceToBeatGuardEvaluation,
    iv_mismatch_edge: &Value,
) -> String {
    format!(
        "action|{}|{}|{}|{}|{}|{}|{}",
        market_slug,
        token_id,
        outcome_label,
        evaluation.passed,
        evaluation.reason_code,
        json_field_str(iv_mismatch_edge, "candidate_side").unwrap_or("none"),
        json_field_str(iv_mismatch_edge, "selected_side").unwrap_or("none")
    )
}

fn pair_lock_primary_iv_mismatch_signature(
    market_slug: &str,
    selection_mode: &str,
    selected_or_failure: &str,
    diagnostics: &Value,
) -> String {
    format!(
        "pair_lock|{}|{}|{}|{}|{}|{}|{}",
        market_slug,
        selection_mode,
        selected_or_failure,
        pair_lock_candidate_field_str(diagnostics, "yes_candidate_guard", "decision")
            .unwrap_or("none"),
        pair_lock_candidate_field_str(diagnostics, "yes_candidate_guard", "reason_code")
            .unwrap_or("none"),
        pair_lock_candidate_field_str(diagnostics, "no_candidate_guard", "decision")
            .unwrap_or("none"),
        pair_lock_candidate_field_str(diagnostics, "no_candidate_guard", "reason_code")
            .unwrap_or("none")
    )
}

fn pair_lock_candidate_has_iv_mismatch(diagnostics: &Value, key: &str) -> bool {
    pair_lock_candidate_guard(diagnostics, key)
        .and_then(|guard| guard.get("iv_mismatch_edge"))
        .filter(|value| !value.is_null())
        .is_some()
}

fn pair_lock_candidate_iv_mismatch_payload(diagnostics: &Value, key: &str) -> Value {
    let candidate = diagnostics.get(key).unwrap_or(&Value::Null);
    let guard = pair_lock_candidate_guard(diagnostics, key)
        .cloned()
        .unwrap_or(Value::Null);
    let iv_mismatch_edge = guard
        .get("iv_mismatch_edge")
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "token_id": candidate.get("token_id").cloned().unwrap_or(Value::Null),
        "outcome_label": candidate.get("outcome_label").cloned().unwrap_or(Value::Null),
        "decision": candidate.get("decision").cloned().unwrap_or(Value::Null),
        "reason_code": candidate.get("reason_code").cloned().unwrap_or(Value::Null),
        "iv_mismatch_edge": iv_mismatch_edge,
        "price_to_beat_guard": guard,
    })
}

fn pair_lock_candidate_guard<'a>(diagnostics: &'a Value, key: &str) -> Option<&'a Value> {
    diagnostics.get(key)?.get("price_to_beat_guard")
}

fn pair_lock_candidate_field_str<'a>(
    diagnostics: &'a Value,
    candidate_key: &str,
    field_key: &str,
) -> Option<&'a str> {
    diagnostics.get(candidate_key)?.get(field_key)?.as_str()
}

fn json_field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn action_place_order_iv_mismatch_edge_config(
    node: &crate::TradeFlowNode,
    signal_market: Option<PriceToBeatSignalFormulaMarketInput>,
) -> PriceToBeatIvMismatchEdgeConfig {
    let market = signal_market.unwrap_or(PriceToBeatSignalFormulaMarketInput {
        best_bid: None,
        best_ask: None,
    });
    let mut config = PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market);
    config.node_max_price = crate::node_config_f64(node, "maxPriceCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "maxPrice"))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    config.time_rules = node
        .config
        .get("priceToBeatIvTimeRules")
        .and_then(Value::as_array)
        .map(|rules| rules.iter().filter_map(parse_iv_time_rule).collect())
        .unwrap_or_default();
    if let Some(stale_ms) = crate::node_config_i64(node, "priceToBeatIvStalePenaltyMs")
        .or_else(|| crate::node_config_i64(node, "priceToBeatIvStaleGapStrengthPenaltyMs"))
    {
        config.stale_gap_strength_penalty_ms = stale_ms.max(0);
    }
    if let Some(penalty) = crate::node_config_f64(node, "priceToBeatIvStaleGapStrengthPenalty") {
        config.stale_gap_strength_penalty = penalty.max(0.0);
    }
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvNegativeVelocityGapStrengthPenalty")
    {
        config.negative_velocity_gap_strength_penalty = penalty.max(0.0);
    }
    if let Some(threshold) =
        crate::node_config_f64(node, "priceToBeatIvBinanceMissingAskThresholdCent")
            .map(|value| value / 100.0)
            .or_else(|| crate::node_config_f64(node, "priceToBeatIvBinanceMissingAskThreshold"))
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
    {
        config.binance_missing_ask_threshold = threshold;
    }
    if let Some(penalty) = crate::node_config_f64(node, "priceToBeatIvBinanceMissingPenalty") {
        config.binance_missing_penalty = penalty.max(0.0);
    }
    if let Some(margin) = crate::node_config_f64(node, "priceToBeatIvMinAdjustedMargin")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.min_adjusted_margin = margin;
    }
    config.min_final_q = crate::node_config_f64(node, "priceToBeatIvMinFinalQ")
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    config.binance_disagreement_threshold =
        crate::node_config_f64(node, "priceToBeatIvBinanceDisagreementThreshold")
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    if let Some(penalty) = crate::node_config_f64(node, "priceToBeatIvBinanceDisagreementPenalty")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.binance_disagreement_penalty = penalty;
    }
    config.large_binance_disagreement_threshold =
        crate::node_config_f64(node, "priceToBeatIvLargeBinanceDisagreementThreshold")
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvLargeBinanceDisagreementPenalty")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.large_binance_disagreement_penalty = penalty;
    }
    super::iv_mismatch_runtime_config::apply_action_place_order_iv_mismatch_risk_config(
        node,
        &mut config,
    );
    config.adaptive.volume_baseline_mode = PriceToBeatIvVolumeBaselineMode::parse(
        crate::node_config_string(node, "priceToBeatIvVolumeBaselineMode").as_deref(),
    )
    .unwrap_or(config.adaptive.volume_baseline_mode);
    if let Some(days) = crate::node_config_i64(node, "priceToBeatIvVolumeBaselineLookbackDays")
        .filter(|value| *value > 0)
    {
        config.adaptive.volume_baseline_lookback_days = days;
    }
    if let Some(seconds) =
        crate::node_config_i64(node, "priceToBeatIvVolumeWindowSec").filter(|value| *value > 0)
    {
        config.adaptive.volume_window_sec = seconds;
    }
    if let Some(samples) = crate::node_config_i64(node, "priceToBeatIvVolumeBaselineMinSamples")
        .filter(|value| *value >= 0)
    {
        config.adaptive.volume_baseline_min_samples = samples;
    }
    if let Some(ratio) = crate::node_config_f64(node, "priceToBeatIvLowHourlyVolumeRatio")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.adaptive.low_hourly_volume_ratio = ratio;
    }
    if let Some(ratio) = crate::node_config_f64(node, "priceToBeatIvHighHourlyVolumeRatio")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.adaptive.high_hourly_volume_ratio = ratio;
    }
    if let Some(ratio) = crate::node_config_f64(node, "priceToBeatIvExtremeHourlyVolumeRatio")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.adaptive.extreme_hourly_volume_ratio = ratio;
    }
    if let Some(threshold) = crate::node_config_f64(node, "priceToBeatIvBookReliabilityThreshold")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.adaptive.book_reliability_threshold = threshold;
    }
    if let Some(delta) = crate::node_config_f64(node, "priceToBeatIvAdaptiveGreenEdgeDelta")
        .filter(|value| value.is_finite())
    {
        config.adaptive.green_edge_delta = delta;
    }
    if let Some(delta) = crate::node_config_f64(node, "priceToBeatIvAdaptiveGreenGapStrengthDelta")
        .filter(|value| value.is_finite())
    {
        config.adaptive.green_gap_strength_delta = delta;
    }
    if let Some(delta) = crate::node_config_f64(node, "priceToBeatIvAdaptiveOrangeEdgeDelta")
        .filter(|value| value.is_finite())
    {
        config.adaptive.orange_edge_delta = delta;
    }
    if let Some(delta) = crate::node_config_f64(node, "priceToBeatIvAdaptiveOrangeGapStrengthDelta")
        .filter(|value| value.is_finite())
    {
        config.adaptive.orange_gap_strength_delta = delta;
    }
    if let Some(delta) =
        crate::node_config_f64(node, "priceToBeatIvAdaptiveOrangeGapUsdMarginDelta")
            .filter(|value| value.is_finite())
    {
        config.adaptive.orange_gap_usd_margin_delta = delta;
    }
    config.adaptive.red_block =
        crate::node_config_bool(node, "priceToBeatIvAdaptiveRedBlock").unwrap_or(true);
    config
}

async fn hydrate_action_place_order_iv_mismatch_adaptive_volume(
    runtime: Option<&PriceToBeatGuardRuntimeContext<'_>>,
    market_slug: &str,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    if config.protection_mode != PriceToBeatIvProtectionMode::Adaptive
        || config.adaptive.volume_baseline_mode != PriceToBeatIvVolumeBaselineMode::Hourly
    {
        return;
    }
    let volume_window_sec = config.adaptive.volume_window_sec.max(1);
    let Some((window_start, window_end)) = crate::trade_builder_second_snapshot_window(market_slug)
    else {
        config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput::neutral(
            config.adaptive.volume_baseline_mode,
            volume_window_sec,
            None,
            "market_window_unavailable",
        ));
        return;
    };
    let Some(scope) = crate::find_updown_scope_by_slug(market_slug) else {
        config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput::neutral(
            config.adaptive.volume_baseline_mode,
            volume_window_sec,
            None,
            "asset_unavailable",
        ));
        return;
    };
    let now = crate::Utc::now();
    let seconds_left = window_end
        .signed_duration_since(now)
        .num_milliseconds()
        .max(0) as f64
        / 1_000.0;
    let Some((bucket_start, bucket_end, bucket_label)) =
        price_to_beat_iv_volume_seconds_bucket(seconds_left, window_start, window_end)
    else {
        config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput::neutral(
            config.adaptive.volume_baseline_mode,
            volume_window_sec,
            None,
            "seconds_bucket_unavailable",
        ));
        return;
    };
    let hour = now.hour() as i32;
    let baseline_key = Some(format!(
        "{}:{hour:02}UTC:{bucket_label}:{volume_window_sec}s",
        scope.asset.to_ascii_uppercase()
    ));
    let Some(runtime) = runtime else {
        config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput::neutral(
            config.adaptive.volume_baseline_mode,
            volume_window_sec,
            baseline_key,
            "runtime_unavailable_neutral",
        ));
        return;
    };
    let current_volume = runtime
        .repo
        .sum_market_trade_notional_usdc(market_slug, now, volume_window_sec)
        .await
        .ok();
    let hours = [hour, (hour + 23) % 24, (hour + 1) % 24];
    let baselines = runtime
        .repo
        .list_market_trade_hourly_volume_medians(
            scope.asset,
            &hours,
            bucket_start,
            bucket_end,
            config.adaptive.volume_baseline_lookback_days,
            volume_window_sec,
            market_slug,
            now,
        )
        .await
        .unwrap_or_default();
    let (baseline_median, sample_count, status) = smooth_hourly_volume_baseline(
        hour,
        &baselines,
        config.adaptive.volume_baseline_min_samples,
    );
    let (baseline_median, sample_count, status) = if baseline_median.is_some() {
        (baseline_median, sample_count, status)
    } else {
        match runtime
            .repo
            .market_trade_volume_bucket_median(
                scope.asset,
                bucket_start,
                bucket_end,
                config.adaptive.volume_baseline_lookback_days,
                volume_window_sec,
                market_slug,
                now,
            )
            .await
        {
            Ok(fallback)
                if fallback.sample_count >= config.adaptive.volume_baseline_min_samples
                    && fallback.median_volume_usdc.is_finite() =>
            {
                (
                    Some(fallback.median_volume_usdc),
                    fallback.sample_count,
                    "fallback_bucket_ready",
                )
            }
            _ => (None, sample_count, "cold_start_neutral"),
        }
    };
    config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput {
        baseline_mode: config.adaptive.volume_baseline_mode,
        volume_window_sec,
        current_volume_usdc: current_volume,
        baseline_median_usdc: baseline_median,
        baseline_sample_count: Some(sample_count),
        baseline_key,
        baseline_status: status,
    });
}

fn price_to_beat_iv_volume_seconds_bucket(
    seconds_left: f64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Option<(f64, f64, &'static str)> {
    let window_secs = window_end
        .signed_duration_since(window_start)
        .num_seconds()
        .max(0) as f64;
    let buckets = [
        (180.0, 120.0, "180-120"),
        (120.0, 60.0, "120-60"),
        (60.0, 30.0, "60-30"),
        (30.0, 10.0, "30-10"),
    ];
    buckets
        .into_iter()
        .find(|(start, end, _)| seconds_left <= *start && seconds_left > *end)
        .or_else(|| (window_secs > 0.0).then_some((window_secs, 0.0, "full-window")))
}

fn smooth_hourly_volume_baseline(
    hour: i32,
    baselines: &[bot_infra::db::MarketTradeHourlyVolumeMedian],
    min_samples: i64,
) -> (Option<f64>, i64, &'static str) {
    let weights = [
        (hour, 0.60),
        ((hour + 23) % 24, 0.20),
        ((hour + 1) % 24, 0.20),
    ];
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    let mut sample_count = 0_i64;
    for (candidate_hour, weight) in weights {
        let Some(baseline) = baselines
            .iter()
            .find(|item| item.hour_utc == candidate_hour)
        else {
            continue;
        };
        if baseline.sample_count <= 0 || !baseline.median_volume_usdc.is_finite() {
            continue;
        }
        weighted_sum += baseline.median_volume_usdc * weight;
        weight_sum += weight;
        sample_count += baseline.sample_count;
    }
    if sample_count >= min_samples.max(0) && weight_sum > 0.0 {
        (
            Some(weighted_sum / weight_sum),
            sample_count,
            "hourly_ready",
        )
    } else {
        (None, sample_count, "hourly_insufficient_samples")
    }
}

fn parse_iv_time_rule(value: &Value) -> Option<PriceToBeatIvMismatchTimeRule> {
    let obj = value.as_object()?;
    let start_remaining_secs =
        iv_time_rule_number(obj, &["startRemainingSec", "start_remaining_secs"])?;
    let end_remaining_secs = iv_time_rule_number(obj, &["endRemainingSec", "end_remaining_secs"])?;
    if start_remaining_secs <= end_remaining_secs {
        return None;
    }
    let min_edge = iv_time_rule_number(obj, &["minEdge", "min_edge"])?;
    let min_gap_strength = iv_time_rule_number(obj, &["minGapStrength", "min_gap_strength"])?;
    let min_expected_move_usd =
        iv_time_rule_number(obj, &["minExpectedMoveUsd", "min_expected_move_usd"])
            .filter(|value| *value > 0.0);
    let min_gap_strength_margin =
        iv_time_rule_number(obj, &["minGapStrengthMargin", "min_gap_strength_margin"])
            .filter(|value| *value >= 0.0);
    let min_gap_usd_margin = iv_time_rule_number(obj, &["minGapUsdMargin", "min_gap_usd_margin"])
        .filter(|value| *value >= 0.0);
    if min_edge < 0.0 || min_gap_strength < 0.0 {
        return None;
    }
    Some(PriceToBeatIvMismatchTimeRule {
        start_remaining_secs,
        end_remaining_secs,
        max_price: iv_time_rule_max_price(obj),
        min_edge,
        min_gap_strength,
        min_expected_move_usd,
        min_gap_strength_margin,
        min_gap_usd_margin,
    })
}

fn iv_time_rule_number(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(crate::value_as_f64))
        .filter(|value| value.is_finite())
}

fn iv_time_rule_max_price(obj: &serde_json::Map<String, Value>) -> Option<f64> {
    iv_time_rule_number(obj, &["maxPriceCent", "max_price_cent"])
        .map(|value| value / 100.0)
        .or_else(|| iv_time_rule_number(obj, &["maxPrice", "max_price"]))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
}

fn sync_price_to_beat_iv_selected_max_price(
    context: &mut Value,
    evaluation: &PriceToBeatGuardEvaluation,
) {
    let max_price = evaluation
        .passed
        .then(|| {
            let iv_mismatch_edge = evaluation.iv_mismatch_edge.as_ref()?;
            iv_mismatch_edge
                .get("submit_limit_price_cent")
                .and_then(crate::value_as_f64)
                .map(|value| value / 100.0)
                .or_else(|| {
                    iv_mismatch_edge
                        .get("time_rule_max_price")
                        .and_then(crate::value_as_f64)
                })
        })
        .flatten()
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    crate::set_flow_context(
        context,
        "priceToBeatIvSelectedMaxPrice",
        max_price.map_or(Value::Null, |value| json!(value)),
    );
}

fn price_to_beat_guard_can_skip_book_recheck(evaluation: &PriceToBeatGuardEvaluation) -> bool {
    !evaluation.passed && price_to_beat_iv_early_block_can_skip_book(&evaluation.reason_code)
}

pub(crate) async fn evaluate_action_place_order_price_to_beat_guard_state(
    runtime: Option<PriceToBeatGuardRuntimeContext<'_>>,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    run_id: i64,
    flow_definition_id: Option<i64>,
    market_slug: &str,
    action_token_id: Option<&str>,
    action_yes_token_id: Option<&str>,
    action_no_token_id: Option<&str>,
    outcome_label: &str,
    signal_market: Option<PriceToBeatSignalFormulaMarketInput>,
) -> Result<PriceToBeatGuardEvaluation> {
    let node_max_price_relax_enabled =
        max_price_relax::action_place_order_max_price_relax_enabled(node);
    let strategy_max_price_relax_enabled = runtime
        .as_ref()
        .map(|runtime| runtime.cfg.strategy.max_price_relax_enabled)
        .unwrap_or(true);
    let max_price_relax_enabled = strategy_max_price_relax_enabled && node_max_price_relax_enabled;
    if max_price_relax_enabled {
        max_price_relax::ensure_max_price_relax_tracking_market(context, &node.key, market_slug);
    }
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
    let signal_config = signal_market.map(PriceToBeatSignalFormulaConfig::taker);
    let mut iv_mismatch_config = None;
    if resolution.effective_mode == PriceToBeatMode::IvMismatchEdge {
        let mut config = action_place_order_iv_mismatch_edge_config(node, signal_market);
        hydrate_action_place_order_iv_mismatch_adaptive_volume(
            runtime.as_ref(),
            market_slug,
            &mut config,
        )
        .await;
        super::iv_mismatch_participation::hydrate_price_to_beat_iv_participation(
            runtime.as_ref().map(|runtime| runtime.repo),
            flow_definition_id,
            &mut config,
        )
        .await;
        iv_mismatch_config = Some(config);
    }
    let iv_mismatch_needs_book_hydration = iv_mismatch_config
        .as_ref()
        .map(price_to_beat_iv_mismatch_needs_book_hydration)
        .unwrap_or(false);
    let current_price_source = PriceToBeatCurrentPriceSource::parse(
        crate::node_config_string(node, "priceToBeatCurrentPriceSource").as_deref(),
    );
    let cex_entry_consensus_config =
        super::entry_current_hybrid::CexEntryConsensusConfig::from_node(node);
    let mut evaluation = evaluate_price_to_beat_guard_with_current_source(
        market_slug,
        resolution.effective_mode,
        resolution.threshold_value,
        resolution.threshold_unit,
        outcome_label,
        signal_config.clone(),
        current_price_source,
        cex_entry_consensus_config.clone(),
        iv_mismatch_config,
    )
    .await;
    if resolution.effective_mode == PriceToBeatMode::IvMismatchEdge
        && iv_mismatch_needs_book_hydration
        && !price_to_beat_guard_can_skip_book_recheck(&evaluation)
    {
        let mut config = action_place_order_iv_mismatch_edge_config(node, signal_market);
        hydrate_action_place_order_iv_mismatch_book_quotes(
            runtime.as_ref(),
            context,
            node,
            market_slug,
            action_token_id,
            action_yes_token_id,
            action_no_token_id,
            outcome_label,
            signal_market,
            &mut config,
        )
        .await;
        hydrate_action_place_order_iv_mismatch_adaptive_volume(
            runtime.as_ref(),
            market_slug,
            &mut config,
        )
        .await;
        super::iv_mismatch_participation::hydrate_price_to_beat_iv_participation(
            runtime.as_ref().map(|runtime| runtime.repo),
            flow_definition_id,
            &mut config,
        )
        .await;
        evaluation = evaluate_price_to_beat_guard_with_current_source(
            market_slug,
            resolution.effective_mode,
            resolution.threshold_value,
            resolution.threshold_unit,
            outcome_label,
            signal_config,
            current_price_source,
            cex_entry_consensus_config,
            Some(config),
        )
        .await;
    } else if resolution.effective_mode == PriceToBeatMode::IvMismatchEdge
        && iv_mismatch_needs_book_hydration
        && price_to_beat_guard_can_skip_book_recheck(&evaluation)
    {
        annotate_price_to_beat_iv_book_not_requested_for_early_block(&mut evaluation);
    }
    resolution.apply_metadata(&mut evaluation);
    sync_price_to_beat_iv_selected_max_price(context, &evaluation);
    if matches!(
        resolution.effective_mode,
        PriceToBeatMode::AutoLast3AvgExcursion | PriceToBeatMode::AutoVolPct
    ) {
        apply_price_to_beat_risk_penalty(&mut evaluation, resolution.stop_loss_bump_usd);
    }
    super::apply_action_place_order_early_stale_side_guard(
        node,
        market_slug,
        outcome_label,
        &mut evaluation,
    );
    super::cex_direction_guard::apply_action_place_order_cex_direction_guard(
        node,
        market_slug,
        outcome_label,
        &mut evaluation,
    );
    super::entry_quality_policy::apply_action_place_order_entry_quality_policy(
        node,
        &mut evaluation,
    );
    sync_price_to_beat_iv_selected_max_price(context, &evaluation);
    if let Some(runtime) = runtime {
        if !runtime.cfg.strategy.max_price_relax_enabled || !node_max_price_relax_enabled {
            let disabled_reason = if !runtime.cfg.strategy.max_price_relax_enabled {
                "strategy_max_price_relax_disabled"
            } else {
                "node_max_price_relax_disabled"
            };
            evaluation.max_price_relax = Some(json!({
                "max_price_relax_enabled": false,
                "max_price_relax_applied": false,
                "max_price_relax_disabled_reason": disabled_reason,
            }));
        } else if runtime.options.send_relax_notifications {
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
    if let Some(relaxation) =
        super::max_price_relax::preview_action_place_order_max_price_relaxation_state(
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

    fn pair_lock_iv_diagnostics(edge: f64, yes_reason: &str) -> Value {
        json!({
            "yes_candidate_guard": {
                "token_id": "yes-token",
                "outcome_label": "Up",
                "decision": "passed",
                "reason_code": yes_reason,
                "price_to_beat_guard": {
                    "threshold_mode": "iv_mismatch_edge",
                    "iv_mismatch_edge": {
                        "decision_reason": "selected_edge_passed",
                        "candidate_side": "up",
                        "selected_side": "up",
                        "q": 0.72,
                        "edge": edge,
                        "sigma": 0.15
                    }
                }
            },
            "no_candidate_guard": {
                "token_id": "no-token",
                "outcome_label": "Down",
                "decision": "blocked",
                "reason_code": "blocked_edge_below_threshold",
                "price_to_beat_guard": {
                    "threshold_mode": "iv_mismatch_edge",
                    "iv_mismatch_edge": {
                        "decision_reason": "blocked_edge_below_threshold",
                        "candidate_side": "down",
                        "selected_side": null,
                        "q": 0.28,
                        "edge": -0.12,
                        "sigma": 0.15
                    }
                }
            }
        })
    }

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

    #[test]
    fn iv_mismatch_signature_state_logs_only_changed_decisions() {
        let mut context = json!({});

        assert!(remember_iv_mismatch_decision_signature(
            &mut context,
            "node_1",
            "signature_a"
        ));
        assert!(!remember_iv_mismatch_decision_signature(
            &mut context,
            "node_1",
            "signature_a"
        ));
        assert!(remember_iv_mismatch_decision_signature(
            &mut context,
            "node_1",
            "signature_b"
        ));
    }

    #[test]
    fn pair_lock_iv_mismatch_signature_ignores_numeric_formula_changes() {
        let left = pair_lock_iv_diagnostics(0.11, "selected_edge_passed");
        let right = pair_lock_iv_diagnostics(0.27, "selected_edge_passed");

        assert_eq!(
            pair_lock_primary_iv_mismatch_signature(
                "btc-updown-5m-1",
                "auto_guarded_iv_mismatch_edge",
                "yes-token",
                &left,
            ),
            pair_lock_primary_iv_mismatch_signature(
                "btc-updown-5m-1",
                "auto_guarded_iv_mismatch_edge",
                "yes-token",
                &right,
            )
        );
    }

    #[test]
    fn pair_lock_iv_mismatch_signature_changes_on_reason() {
        let left = pair_lock_iv_diagnostics(0.11, "selected_edge_passed");
        let right = pair_lock_iv_diagnostics(0.11, "blocked_chop");

        assert_ne!(
            pair_lock_primary_iv_mismatch_signature(
                "btc-updown-5m-1",
                "auto_guarded_iv_mismatch_edge",
                "yes-token",
                &left,
            ),
            pair_lock_primary_iv_mismatch_signature(
                "btc-updown-5m-1",
                "auto_guarded_iv_mismatch_edge",
                "yes-token",
                &right,
            )
        );
    }

    #[test]
    fn pair_lock_iv_mismatch_payload_carries_formula_output() {
        let diagnostics = pair_lock_iv_diagnostics(0.11, "selected_edge_passed");
        let payload = pair_lock_candidate_iv_mismatch_payload(&diagnostics, "yes_candidate_guard");

        assert_eq!(
            payload.get("token_id").and_then(Value::as_str),
            Some("yes-token")
        );
        assert_eq!(
            payload
                .get("iv_mismatch_edge")
                .and_then(|value| value.get("edge"))
                .and_then(Value::as_f64),
            Some(0.11)
        );
    }
}
