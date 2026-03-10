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
    aggressive_price_for_side, calc_level_size, clamp_probability, dual_dca_timeframe_duration,
    fetch_price_from_market_ws, find_updown_scope_by_asset_timeframe, list_markets_for_scope,
    load_user_app_config_cached, load_user_order_executor_cached, normalize_exchange_status,
    risk_gate_manual_order, select_preferred_live_market, sync_recent_trade_builder_fills,
    to_risk_limits,
};

const FLOW_DUAL_DCA_JOB_PROCESS_LIMIT: i64 = 100;
const FLOW_DUAL_DCA_RETRY_SECONDS: i64 = 30;
const FLOW_DUAL_DCA_ACTIVE_CHECK_SECONDS: i64 = 20;
const FLOW_DUAL_DCA_MAX_CONSECUTIVE_ERRORS: i32 = 20;
const FLOW_DUAL_DCA_MIN_ORDER_USDC: f64 = 1.0;

fn dual_dca_trigger_crossed_below_strict(current_price: f64, trigger_price: Option<f64>) -> bool {
    match trigger_price {
        None => true,
        Some(trigger) => current_price <= trigger,
    }
}

// ---------------------------------------------------------------------------
// Top-level job loop
// ---------------------------------------------------------------------------

pub async fn process_trade_flow_dual_dca_jobs(
    repo: &PostgresRepository,
    run_id: i64,
    _cfg: &AppConfig,
    _client: &dyn OrderExecutor,
    ws: &ClobWsClient,
) -> Result<()> {
    let jobs = repo
        .list_trade_flow_dual_dca_jobs_for_processing(FLOW_DUAL_DCA_JOB_PROCESS_LIMIT)
        .await?;
    if jobs.is_empty() {
        return Ok(());
    }

    let policy = DefaultRiskPolicy;
    let mut user_cfg_cache: HashMap<i64, AppConfig> = HashMap::new();
    let mut user_executor_cache = HashMap::new();

    for job in jobs {
        let user_id = if let Some(run) = repo.get_trade_flow_run(job.flow_run_id).await? {
            run.user_id
        } else if let Some(definition) = repo
            .get_trade_flow_definition(job.flow_definition_id)
            .await?
        {
            definition.user_id
        } else {
            warn!(
                run_id,
                dual_dca_job_id = job.id,
                "TRADE_FLOW_DUAL_DCA_JOB_USER_RESOLVE_FAILED"
            );
            continue;
        };
        let user_cfg = load_user_app_config_cached(repo, user_id, &mut user_cfg_cache).await?;
        let limits = to_risk_limits(&user_cfg);
        let client = load_user_order_executor_cached(
            repo,
            user_id,
            &mut user_cfg_cache,
            &mut user_executor_cache,
        )
        .await?;
        let gamma = GammaHttpClient::new(user_cfg.exchange.gamma_base_url.clone());

        if let Err(err) = process_trade_flow_dual_dca_job(
            repo,
            run_id,
            &user_cfg,
            &limits,
            &policy,
            client.as_ref(),
            ws,
            &gamma,
            &job,
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

