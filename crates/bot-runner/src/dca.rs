use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

use bot_core::{DefaultRiskPolicy, RiskDecision, RiskLimits, RiskPolicy};
use bot_infra::config::AppConfig;
use bot_infra::contracts::OrderExecutor;
use bot_infra::db::{PostgresRepository, TradeFlowDualDcaJob};
use bot_infra::exchange::{GammaHttpClient, PlaceOrderRequest};
use bot_infra::ws::ClobWsClient;

use crate::{
    aggressive_price_for_side, calc_level_size, clamp_probability,
    dual_dca_timeframe_duration, fetch_price_from_market_ws,
    find_updown_scope_by_asset_timeframe, list_markets_for_scope,
    normalize_exchange_status, risk_gate_manual_order,
    select_preferred_live_market, sync_recent_trade_builder_fills,
    to_risk_limits,
};

const FLOW_DUAL_DCA_JOB_PROCESS_LIMIT: i64 = 100;
const FLOW_DUAL_DCA_RETRY_SECONDS: i64 = 30;
const FLOW_DUAL_DCA_ACTIVE_CHECK_SECONDS: i64 = 20;
const FLOW_DUAL_DCA_MAX_CONSECUTIVE_ERRORS: i32 = 20;
const FLOW_DUAL_DCA_MIN_ORDER_USDC: f64 = 1.0;

// ---------------------------------------------------------------------------
// Top-level job loop
// ---------------------------------------------------------------------------

pub async fn process_trade_flow_dual_dca_jobs(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
) -> Result<()> {
    let jobs = repo
        .list_trade_flow_dual_dca_jobs_for_processing(FLOW_DUAL_DCA_JOB_PROCESS_LIMIT)
        .await?;
    if jobs.is_empty() {
        return Ok(());
    }

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let limits = to_risk_limits(cfg);
    let policy = DefaultRiskPolicy;

    for job in jobs {
        if let Err(err) = process_trade_flow_dual_dca_job(
            repo, run_id, cfg, &limits, &policy, client, ws, &gamma, &job,
        )
        .await
        {
            let next_check = Utc::now() + ChronoDuration::seconds(FLOW_DUAL_DCA_RETRY_SECONDS);
            let _ = repo
                .schedule_trade_flow_dual_dca_job_check(job.id, next_check, Some(&err.to_string()))
                .await;
            let _ = repo
                .append_trade_flow_dual_dca_event(
                    job.id,
                    None,
                    "processing_error",
                    &json!({
                        "error": err.to_string(),
                        "next_check_at": next_check
                    }),
                )
                .await;
            warn!(
                run_id,
                dual_dca_job_id = job.id,
                error = %err,
                "TRADE_FLOW_DUAL_DCA_JOB_ERROR"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Per-job processing — DIRECT MARKET ORDER approach
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn process_trade_flow_dual_dca_job(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    gamma: &GammaHttpClient,
    job: &TradeFlowDualDcaJob,
) -> Result<()> {
    // --- Phase 0: Pre-checks ---
    let run = repo.get_trade_flow_run(job.flow_run_id).await?;
    let Some(run) = run else {
        cancel_dual_dca_active_orders(repo, client, job, None, "flow_run_missing").await?;
        repo.set_trade_flow_dual_dca_job_status(job.id, "canceled", Some("flow_run_missing"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "job_canceled",
            &json!({ "reason": "flow_run_missing" }),
        ).await?;
        return Ok(());
    };
    if run.status != "running" {
        cancel_dual_dca_active_orders(repo, client, job, None, "flow_run_not_running").await?;
        repo.set_trade_flow_dual_dca_job_status(job.id, "completed", Some("flow_run_not_running"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "job_completed",
            &json!({ "reason": "flow_run_not_running", "run_status": run.status }),
        ).await?;
        return Ok(());
    }
    if job.consecutive_errors >= FLOW_DUAL_DCA_MAX_CONSECUTIVE_ERRORS {
        cancel_dual_dca_active_orders(repo, client, job, None, "max_consecutive_errors").await?;
        repo.set_trade_flow_dual_dca_job_status(job.id, "paused", Some("max_consecutive_errors_reached"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "job_paused",
            &json!({
                "reason": "max_consecutive_errors_reached",
                "consecutive_errors": job.consecutive_errors,
                "last_error": job.last_error
            }),
        ).await?;
        warn!(
            run_id,
            dual_dca_job_id = job.id,
            consecutive_errors = job.consecutive_errors,
            "TRADE_FLOW_DUAL_DCA_JOB_PAUSED_MAX_ERRORS"
        );
        return Ok(());
    }

    // --- Phase 1: Market discovery ---
    let scope_def = find_updown_scope_by_asset_timeframe(&job.market_asset, &job.market_timeframe)
        .ok_or_else(|| anyhow::anyhow!("dual_dca job has unsupported market asset/timeframe"))?;
    let mut markets = match list_markets_for_scope(gamma, scope_def.scope).await {
        Ok(m) => m,
        Err(err) => {
            let next_check = Utc::now() + ChronoDuration::seconds(FLOW_DUAL_DCA_RETRY_SECONDS);
            repo.schedule_trade_flow_dual_dca_job_check(job.id, next_check, None).await?;
            repo.append_trade_flow_dual_dca_event(
                job.id, None, "not_buy_decision",
                &json!({
                    "reason_code": "market_discovery_fetch_failed",
                    "error": err.to_string(),
                    "next_check_at": next_check
                }),
            ).await?;
            return Ok(());
        }
    };
    markets.retain(|m| m.yes_token_id.is_some() && m.no_token_id.is_some());
    if markets.is_empty() {
        let next_check = Utc::now() + ChronoDuration::seconds(FLOW_DUAL_DCA_RETRY_SECONDS);
        repo.update_trade_flow_dual_dca_job_market_state(job.id, None, None, None, next_check, 0)
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "not_buy_decision",
            &json!({ "reason_code": "market_missing_token_ids", "next_check_at": next_check }),
        ).await?;
        return Ok(());
    }

    let now = Utc::now();
    let selected = select_preferred_live_market(markets, now)
        .ok_or_else(|| anyhow::anyhow!("no active market for dual_dca"))?;
    let market_slug = selected.slug.clone();
    let market_started_at = selected.starts_at;
    let market_ends_at = selected.ends_at;
    let market_selection_reason = selected.selection_reason.as_str();
    let maker_base_fee = selected.maker_base_fee;

    info!(
        run_id,
        dual_dca_job_id = job.id,
        market = %market_slug,
        selection_reason = market_selection_reason,
        "TRADE_FLOW_DUAL_DCA_MARKET_SELECTED"
    );

    let source_trade_id = job.source_trade_id.ok_or_else(|| {
        anyhow::anyhow!("dual_dca job missing source_trade_id")
    })?;
    let yes_token_id = selected.yes_token_id.clone()
        .ok_or_else(|| anyhow::anyhow!("selected market missing yes token id"))?;
    let no_token_id = selected.no_token_id.clone()
        .ok_or_else(|| anyhow::anyhow!("selected market missing no token id"))?;
    let (yes_price, no_price) =
        resolve_dual_dca_outcome_prices(ws, client, &market_slug, &yes_token_id, &no_token_id)
            .await?;
    let active_next_check = dual_dca_active_next_check(now, market_ends_at);
    let rollover_next_check =
        dual_dca_rollover_next_check(now, market_ends_at, &job.market_timeframe);

    // --- Phase 2: TP/SL check ---
    if let Some((unrealized_pnl_usdc, token_breakdown)) = evaluate_dual_dca_unrealized_pnl_usdc(
        repo, source_trade_id, &yes_token_id, &no_token_id, yes_price, no_price,
    ).await? {
        let tp_hit = job.tp_profit_pct > 0.0 && unrealized_pnl_usdc >= job.tp_profit_pct;
        let sl_hit = job.sl_loss_pct > 0.0 && unrealized_pnl_usdc <= -job.sl_loss_pct;
        if tp_hit || sl_hit {
            let reason = if tp_hit { "tp_profit_hit" } else { "sl_loss_hit" };
            cancel_dual_dca_active_orders(repo, client, job, Some(&market_slug), reason).await?;
            repo.update_trade_flow_dual_dca_job_market_state(
                job.id, Some(&market_slug), market_started_at, market_ends_at,
                rollover_next_check, 0,
            ).await?;
            repo.append_trade_flow_dual_dca_event(
                job.id, None, "risk_guard_triggered",
                &json!({
                    "market_slug": market_slug,
                    "reason": reason,
                    "unrealized_pnl_usdc": unrealized_pnl_usdc,
                    "token_breakdown": token_breakdown,
                    "next_check_at": rollover_next_check
                }),
            ).await?;
            return Ok(());
        }
    }

    // --- Phase 3: Cutoff check ---
    if let Some(ends_at) = market_ends_at {
        let cutoff_at = ends_at - ChronoDuration::minutes(job.cutoff_min.max(0) as i64);
        if now >= cutoff_at {
            cancel_dual_dca_active_orders(repo, client, job, Some(&market_slug), "cutoff_window").await?;
            repo.update_trade_flow_dual_dca_job_market_state(
                job.id, Some(&market_slug), market_started_at, Some(ends_at),
                rollover_next_check, 0,
            ).await?;
            repo.append_trade_flow_dual_dca_event(
                job.id, None, "market_cutoff_window",
                &json!({
                    "market_slug": market_slug,
                    "cutoff_min": job.cutoff_min,
                    "market_ends_at": ends_at,
                    "next_check_at": rollover_next_check
                }),
            ).await?;
            return Ok(());
        }
    }

    // --- Phase 4: Initialize legs if new market ---
    let is_new_market = job.last_market_slug
        .as_ref()
        .map(|prev| prev != &market_slug)
        .unwrap_or(true);

    let outcomes = resolve_dual_dca_outcomes(
        &job.side_mode, &yes_token_id, yes_price, &no_token_id, no_price,
    );

    if is_new_market {
        cancel_dual_dca_active_orders(repo, client, job, None, "market_change").await?;
        let mut created_count = 0i32;

        for (outcome_label, token_id, outcome_price) in &outcomes {
            let base_reference_price = if let Some(bp) = job.base_price_usdc {
                bp
            } else {
                *outcome_price
            };
            let level_reference_price = clamp_probability(base_reference_price);

            for level in 0..job.dca_levels {
                let level_f64 = level as f64;
                let step_distance = dual_dca_level_step_distance(
                    job.near_step, job.step_mult, level,
                );
                let trigger_price = if level == 0 && job.base_price_usdc.is_none() {
                    None
                } else {
                    Some(clamp_probability(level_reference_price - step_distance))
                };
                let trigger_condition: Option<&str> = trigger_price.map(|_| "cross_below");

                let size_multiplier = job.size_mult.powf(level_f64);
                let (size_usdc, _planned_shares) = if job.base_sizing == "usdc" {
                    let base_usdc = job.base_usdc.unwrap_or(0.0);
                    (base_usdc * size_multiplier, None::<f64>)
                } else {
                    let base_shares = job.base_shares.unwrap_or(0.0);
                    let shares = base_shares * size_multiplier;
                    let price_for_notional = trigger_price.unwrap_or(level_reference_price);
                    (shares * price_for_notional, Some(shares))
                };
                if !size_usdc.is_finite() || size_usdc < FLOW_DUAL_DCA_MIN_ORDER_USDC {
                    continue;
                }

                // Risk gate
                let risk = risk_gate_manual_order(
                    repo, run_id, cfg, source_trade_id, size_usdc, limits, policy,
                ).await?;
                if !matches!(risk, RiskDecision::Allow) {
                    repo.append_trade_flow_dual_dca_event(
                        job.id, None, "level_blocked_by_risk",
                        &json!({
                            "outcome_label": outcome_label,
                            "level_index": level,
                            "size_usdc": size_usdc,
                            "decision": format!("{risk:?}")
                        }),
                    ).await?;
                    continue;
                }

                // Create leg as pending (no builder order)
                let leg = repo.upsert_trade_flow_dual_dca_leg(
                    job.id, &market_slug, token_id, outcome_label, "buy", level,
                    trigger_condition, trigger_price, size_usdc,
                    Some(level_reference_price), None, "pending",
                ).await?;

                repo.append_trade_flow_dual_dca_event(
                    job.id, Some(leg.id), "leg_created",
                    &json!({
                        "market_slug": market_slug,
                        "outcome_label": outcome_label,
                        "level_index": level,
                        "trigger_price": trigger_price,
                        "size_usdc": size_usdc,
                        "reference_price": level_reference_price
                    }),
                ).await?;
                created_count += 1;
            }
        }

        repo.update_trade_flow_dual_dca_job_market_state(
            job.id, Some(&market_slug), market_started_at, market_ends_at,
            active_next_check, created_count,
        ).await?;
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "market_cycle_initialized",
            &json!({
                "market_slug": market_slug,
                "orders_created": created_count,
                "next_check_at": active_next_check
            }),
        ).await?;
    }

    // --- Phase 5: Check fills on active orders ---
    let active_legs = repo.list_dual_dca_legs_with_active_orders(job.id, &market_slug).await?;
    for leg in &active_legs {
        if let Some(exchange_oid) = leg.active_exchange_order_id.as_deref() {
            if let Ok(order_info) = client.status(exchange_oid).await {
                let status = normalize_exchange_status(&order_info.status);
                match status {
                    "filled" => {
                        let fill_price = order_info.price.unwrap_or(0.0);
                        let fill_size = order_info.filled_size.unwrap_or(order_info.size.unwrap_or(0.0));
                        repo.set_dual_dca_leg_filled(leg.id, fill_price, fill_size).await?;
                        repo.append_trade_flow_dual_dca_event(
                            job.id, Some(leg.id), "leg_filled",
                            &json!({
                                "market_slug": market_slug,
                                "outcome_label": leg.outcome_label,
                                "level_index": leg.level_index,
                                "exchange_order_id": exchange_oid,
                                "filled_price": fill_price,
                                "filled_size": fill_size
                            }),
                        ).await?;
                        info!(
                            run_id,
                            dual_dca_job_id = job.id,
                            leg_id = leg.id,
                            outcome = %leg.outcome_label,
                            level = leg.level_index,
                            filled_price = fill_price,
                            filled_size = fill_size,
                            "TRADE_FLOW_DUAL_DCA_LEG_FILLED"
                        );
                    }
                    "canceled" | "expired" | "rejected" => {
                        repo.reset_dual_dca_leg_to_pending(leg.id).await?;
                        repo.append_trade_flow_dual_dca_event(
                            job.id, Some(leg.id), "leg_order_canceled_retry",
                            &json!({
                                "exchange_order_id": exchange_oid,
                                "exchange_status": status,
                                "action": "reset_to_pending"
                            }),
                        ).await?;
                    }
                    _ => {} // still open, wait
                }
            }
        }
    }

    // Sync fills from exchange for PnL accuracy
    if let Err(err) = sync_recent_trade_builder_fills(repo, client).await {
        warn!(run_id, error = %err, "DUAL_DCA_FILL_SYNC_ERROR");
    }

    // --- Phase 6: Check triggers and place orders ---
    for (outcome_label, token_id, current_price) in &outcomes {
        let Some(leg) = repo
            .next_pending_dual_dca_leg(job.id, &market_slug, outcome_label)
            .await?
        else {
            continue; // all levels filled or no pending legs
        };

        // Level 0 (no trigger): fire immediately. Level 1+: check trigger.
        let should_fire = match leg.trigger_price {
            None => true,
            Some(trigger) => *current_price <= trigger,
        };
        if !should_fire {
            continue;
        }

        // Risk gate
        let risk = risk_gate_manual_order(
            repo, run_id, cfg, source_trade_id, leg.size_usdc, limits, policy,
        ).await?;
        if !matches!(risk, RiskDecision::Allow) {
            info!(
                run_id,
                dual_dca_job_id = job.id,
                outcome = %outcome_label,
                level = leg.level_index,
                "TRADE_FLOW_DUAL_DCA_LEVEL_RISK_BLOCKED"
            );
            continue;
        }

        // Compute order price and size
        let desired_price = aggressive_price_for_side(
            "buy", *current_price, job.min_price_distance_cent,
        );
        let size = calc_level_size(leg.size_usdc, desired_price);
        if size <= 0.0 {
            continue;
        }

        let client_order_id = format!("dca-{}", Uuid::new_v4());
        let fee_rate = if maker_base_fee > 0 { maker_base_fee } else { 1000 };
        let req = PlaceOrderRequest {
            market: market_slug.clone(),
            token_id: Some(token_id.clone()),
            side: "buy".to_string(),
            price: desired_price,
            size,
            intent: "dca_direct".to_string(),
            client_order_id: client_order_id.clone(),
            leg_side: None,
            fee_rate_bps: fee_rate,
        };

        let ack = client.place(&req).await?;
        let exchange_order_id = ack
            .exchange_order_id
            .clone()
            .unwrap_or_else(|| ack.client_order_id.clone());
        let normalized_status = normalize_exchange_status(&ack.status);

        // Record in orders table for PnL tracking
        let raw = json!({
            "dca_job_id": job.id,
            "leg_id": leg.id,
            "client_order_id": ack.client_order_id,
            "exchange_order_id": exchange_order_id,
            "status": ack.status,
            "normalized_status": normalized_status,
            "trigger_price": leg.trigger_price,
            "current_price": current_price,
            "execution_price": desired_price,
            "size": size,
            "reject_reason": ack.reject_reason,
            "exchange_ts": ack.exchange_ts
        });
        repo.upsert_order_by_exchange_id(
            source_trade_id,
            &exchange_order_id,
            Some(&client_order_id),
            "dca_direct",
            "buy",
            desired_price,
            size,
            normalized_status,
            ack.exchange_ts,
            ack.reject_reason.as_deref(),
            &raw,
        ).await?;

        // Update leg status
        if normalized_status == "filled" {
            repo.set_dual_dca_leg_submitted(leg.id, &exchange_order_id, &client_order_id).await?;
            repo.set_dual_dca_leg_filled(leg.id, desired_price, size).await?;
            repo.append_trade_flow_dual_dca_event(
                job.id, Some(leg.id), "leg_filled_immediately",
                &json!({
                    "market_slug": market_slug,
                    "outcome_label": outcome_label,
                    "level_index": leg.level_index,
                    "exchange_order_id": exchange_order_id,
                    "filled_price": desired_price,
                    "filled_size": size
                }),
            ).await?;
            info!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                price = desired_price,
                size,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_FILLED_IMMEDIATELY"
            );
        } else if normalized_status == "rejected" {
            repo.append_trade_flow_dual_dca_event(
                job.id, Some(leg.id), "leg_order_rejected",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "reject_reason": ack.reject_reason,
                    "status": ack.status
                }),
            ).await?;
            warn!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                reject_reason = ?ack.reject_reason,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_REJECTED"
            );
        } else {
            // Order is open on the book
            repo.set_dual_dca_leg_submitted(leg.id, &exchange_order_id, &client_order_id).await?;
            repo.append_trade_flow_dual_dca_event(
                job.id, Some(leg.id), "leg_order_placed",
                &json!({
                    "market_slug": market_slug,
                    "outcome_label": outcome_label,
                    "level_index": leg.level_index,
                    "exchange_order_id": exchange_order_id,
                    "desired_price": desired_price,
                    "size": size,
                    "current_price": current_price,
                    "trigger_price": leg.trigger_price
                }),
            ).await?;
            info!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                price = desired_price,
                size,
                exchange_order_id = %exchange_order_id,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_PLACED"
            );
        }
    }

    // --- Phase 7: Schedule next check ---
    repo.update_trade_flow_dual_dca_job_market_state(
        job.id, Some(&market_slug), market_started_at, market_ends_at,
        active_next_check, 0,
    ).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Cancel active CLOB orders for DCA legs
// ---------------------------------------------------------------------------

async fn cancel_dual_dca_active_orders(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    job: &TradeFlowDualDcaJob,
    market_slug: Option<&str>,
    reason: &str,
) -> Result<usize> {
    let legs = repo.cancel_dual_dca_active_legs(job.id, market_slug).await?;
    let mut canceled_count = 0usize;

    for (leg_id, exchange_oid) in &legs {
        if let Some(oid) = exchange_oid {
            if let Err(err) = client.cancel(oid).await {
                warn!(
                    dual_dca_job_id = job.id,
                    leg_id,
                    exchange_order_id = %oid,
                    error = %err,
                    "DUAL_DCA_CANCEL_CLOB_ORDER_FAILED"
                );
            }
        }
        canceled_count += 1;
    }

    if canceled_count > 0 {
        repo.append_trade_flow_dual_dca_event(
            job.id, None, "legs_canceled",
            &json!({
                "reason": reason,
                "market_slug": market_slug,
                "canceled_count": canceled_count
            }),
        ).await?;
    }

    Ok(canceled_count)
}

// ---------------------------------------------------------------------------
// Helper functions (moved from main.rs, unchanged)
// ---------------------------------------------------------------------------

fn resolve_dual_dca_outcomes<'a>(
    side_mode: &str,
    yes_token_id: &'a str,
    yes_price: f64,
    no_token_id: &'a str,
    no_price: f64,
) -> Vec<(&'static str, String, f64)> {
    let mut outcomes: Vec<(&'static str, String, f64)> = Vec::new();
    if matches!(side_mode, "up" | "all") {
        outcomes.push(("Yes", yes_token_id.to_string(), yes_price));
    }
    if matches!(side_mode, "down" | "all") {
        outcomes.push(("No", no_token_id.to_string(), no_price));
    }
    outcomes
}

async fn resolve_dual_dca_outcome_prices(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    market_slug: &str,
    yes_token_id: &str,
    no_token_id: &str,
) -> Result<(f64, f64)> {
    let midpoint_price = match client.midpoint(yes_token_id).await {
        Ok(snapshot) => clamp_probability(snapshot.price),
        Err(err) => {
            let fallback = 0.5;
            warn!(
                market = market_slug,
                error = %err,
                fallback_yes = fallback,
                "TRADE_FLOW_DUAL_DCA_MIDPOINT_FAILED_USING_FALLBACK"
            );
            fallback
        }
    };
    let fallback_yes = midpoint_price;
    let fallback_no = clamp_probability(1.0 - midpoint_price);

    let yes_ws_price = fetch_price_from_market_ws(ws, yes_token_id)
        .await
        .map(clamp_probability);
    let no_ws_price = fetch_price_from_market_ws(ws, no_token_id)
        .await
        .map(clamp_probability);

    let yes_price = yes_ws_price
        .or_else(|| no_ws_price.map(|v| clamp_probability(1.0 - v)))
        .unwrap_or(fallback_yes);
    let no_price = no_ws_price
        .or_else(|| yes_ws_price.map(|v| clamp_probability(1.0 - v)))
        .unwrap_or(fallback_no);

    Ok((clamp_probability(yes_price), clamp_probability(no_price)))
}

async fn evaluate_dual_dca_unrealized_pnl_usdc(
    repo: &PostgresRepository,
    source_trade_id: i64,
    yes_token_id: &str,
    no_token_id: &str,
    yes_price: f64,
    no_price: f64,
) -> Result<Option<(f64, Value)>> {
    let token_ids = vec![yes_token_id.to_string(), no_token_id.to_string()];
    let aggregates = repo
        .aggregate_trade_fill_by_token(source_trade_id, &token_ids)
        .await?;
    if aggregates.is_empty() {
        return Ok(None);
    }

    let mut by_token: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    for item in aggregates {
        by_token.insert(
            item.token_id,
            (item.buy_qty, item.buy_notional_usdc, item.sell_qty, item.sell_notional_usdc),
        );
    }

    let mut has_open_position = false;
    let mut total_unrealized_pnl_usdc = 0.0f64;
    let mut breakdown_rows = Vec::new();

    for (outcome_label, token_id, current_price) in [
        ("Yes", yes_token_id, yes_price),
        ("No", no_token_id, no_price),
    ] {
        let (buy_qty, buy_notional_usdc, sell_qty, sell_notional_usdc) =
            by_token.remove(token_id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let net_qty = (buy_qty - sell_qty).max(0.0);
        let net_cost_usdc = (buy_notional_usdc - sell_notional_usdc).max(0.0);
        let mark_value_usdc = net_qty * current_price;
        let unrealized_pnl_usdc = if net_qty > 0.0 {
            mark_value_usdc - net_cost_usdc
        } else {
            0.0
        };

        if net_qty > 0.0000001 {
            has_open_position = true;
            total_unrealized_pnl_usdc += unrealized_pnl_usdc;
        }

        breakdown_rows.push(json!({
            "outcome_label": outcome_label,
            "token_id": token_id,
            "buy_qty": buy_qty,
            "buy_notional_usdc": buy_notional_usdc,
            "sell_qty": sell_qty,
            "sell_notional_usdc": sell_notional_usdc,
            "net_qty": net_qty,
            "net_cost_usdc": net_cost_usdc,
            "current_price": current_price,
            "mark_value_usdc": mark_value_usdc,
            "unrealized_pnl_usdc": unrealized_pnl_usdc
        }));
    }

    if !has_open_position {
        return Ok(None);
    }

    Ok(Some((total_unrealized_pnl_usdc, Value::Array(breakdown_rows))))
}

fn dual_dca_level_step_distance(near_step: f64, step_mult: f64, level_index: i32) -> f64 {
    if level_index <= 0 {
        return 0.0;
    }
    let n = level_index as f64;
    if (step_mult - 1.0).abs() < 1e-9 {
        return near_step * n;
    }
    near_step * (step_mult.powf(n) - 1.0) / (step_mult - 1.0)
}

fn dual_dca_active_next_check(
    now: DateTime<Utc>,
    market_ends_at: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    let heartbeat = now + ChronoDuration::seconds(FLOW_DUAL_DCA_ACTIVE_CHECK_SECONDS);
    let candidate = market_ends_at
        .map(|ends_at| std::cmp::min(heartbeat, ends_at + ChronoDuration::seconds(3)))
        .unwrap_or(heartbeat);
    std::cmp::max(candidate, now + ChronoDuration::seconds(3))
}

fn dual_dca_rollover_next_check(
    now: DateTime<Utc>,
    market_ends_at: Option<DateTime<Utc>>,
    timeframe: &str,
) -> DateTime<Utc> {
    let base = market_ends_at
        .map(|ends_at| ends_at + ChronoDuration::seconds(3))
        .unwrap_or(now + dual_dca_timeframe_duration(timeframe));
    std::cmp::max(base, now + ChronoDuration::seconds(5))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_step_distance_grows_as_expected() {
        let near_step = 0.1;
        let step_mult = 1.1;
        let l1 = dual_dca_level_step_distance(near_step, step_mult, 1);
        let l2 = dual_dca_level_step_distance(near_step, step_mult, 2);
        assert!((l1 - 0.1).abs() < 1e-9);
        assert!((l2 - 0.21).abs() < 1e-9);
    }

    #[test]
    fn cumulative_step_distance_handles_unit_multiplier() {
        let near_step = 0.1;
        let step_mult = 1.0;
        let l2 = dual_dca_level_step_distance(near_step, step_mult, 2);
        let l3 = dual_dca_level_step_distance(near_step, step_mult, 3);
        assert!((l2 - 0.2).abs() < 1e-9);
        assert!((l3 - 0.3).abs() < 1e-9);
    }

    #[test]
    fn active_next_check_is_clamped_to_market_end_buffer() {
        let now = DateTime::parse_from_rfc3339("2026-02-22T12:00:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let market_ends_at = now + ChronoDuration::seconds(10);
        let next_check = dual_dca_active_next_check(now, Some(market_ends_at));
        assert_eq!(next_check, market_ends_at + ChronoDuration::seconds(3));
    }

    #[test]
    fn resolves_up_mode_to_yes_only() {
        let outcomes = resolve_dual_dca_outcomes("up", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].0, "Yes");
    }

    #[test]
    fn resolves_down_mode_to_no_only() {
        let outcomes = resolve_dual_dca_outcomes("down", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].0, "No");
    }

    #[test]
    fn resolves_all_mode_to_both_outcomes() {
        let outcomes = resolve_dual_dca_outcomes("all", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 2);
    }
}
