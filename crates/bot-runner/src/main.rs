mod dca;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use bot_core::{
    can_transition, DefaultRiskPolicy, DualSideStrategy, ExecutionMode, LegSide, MarketCycleId,
    PriceThresholdStrategy, RiskDecision, RiskInput, RiskLimits, RiskPolicy, Strategy,
    SymmetricDualDcaStrategy, TradeState,
};
use bot_infra::claim::AutoClaimService;
use bot_infra::config::AppConfig;
use bot_infra::contracts::{OrderExecutor, StateRepository};
use bot_infra::db::{
    PostgresRepository, TradeBuilderOrder, TradeBuilderWorkflow, TradeBuilderWorkflowLeg,
    TradeFlowDefinitionRuntime, TradeFlowRun, TradeFlowRunStep,
    TradeFlowVersionRuntime,
};
use bot_infra::exchange::{
    ClobHttpClient, ClobRestClient, FillInfo, GammaClient, GammaHttpClient, GammaMarket, OrderInfo,
    PlaceOrderRequest,
};
use bot_infra::market_data::{MarketDataProvider, MockMarketDataProvider};
use bot_infra::reconcile::reconcile_tick_and_snapshot;
use bot_infra::signer::ApiCredentials;
use ethers::{signers::{LocalWallet, Signer as _}, types::Address};
use bot_infra::ws::{ClobWsClient, WsChannel, WsEvent, WsEventType};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env,
    path::PathBuf,
    time::Instant,
};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

const CONFIG_ENC_PREFIX: &str = "enc:v1:";
const CONFIG_ENC_NONCE_LEN: usize = 12;
const CONFIG_ENC_TAG_LEN: usize = 16;
const DEFAULT_DROP_SELL_PCT: f64 = 15.0;
const MANUAL_ORDER_PROCESS_LIMIT: i64 = 250;
const WORKFLOW_PROCESS_LIMIT: i64 = 100;
const FLOW_DEFINITION_PROCESS_LIMIT: i64 = 100;
const FLOW_STEP_PROCESS_LIMIT: i64 = 250;
const PRESSURE_DROP_PCT_THRESHOLD: f64 = 1.5;
const WORKFLOW_MIN_BUY_INCREMENT_USDC: f64 = 1.0;
const SUPPORTED_UPDOWN_SCOPE_DEFS: [UpdownScopeDef; 8] = [
    UpdownScopeDef {
        scope: "btc_5m_updown",
        asset: "btc",
        timeframe: "5m",
        slug_prefix: "btc-updown-5m-",
    },
    UpdownScopeDef {
        scope: "btc_15m_updown",
        asset: "btc",
        timeframe: "15m",
        slug_prefix: "btc-updown-15m-",
    },
    UpdownScopeDef {
        scope: "eth_5m_updown",
        asset: "eth",
        timeframe: "5m",
        slug_prefix: "eth-updown-5m-",
    },
    UpdownScopeDef {
        scope: "eth_15m_updown",
        asset: "eth",
        timeframe: "15m",
        slug_prefix: "eth-updown-15m-",
    },
    UpdownScopeDef {
        scope: "sol_5m_updown",
        asset: "sol",
        timeframe: "5m",
        slug_prefix: "sol-updown-5m-",
    },
    UpdownScopeDef {
        scope: "sol_15m_updown",
        asset: "sol",
        timeframe: "15m",
        slug_prefix: "sol-updown-15m-",
    },
    UpdownScopeDef {
        scope: "xrp_5m_updown",
        asset: "xrp",
        timeframe: "5m",
        slug_prefix: "xrp-updown-5m-",
    },
    UpdownScopeDef {
        scope: "xrp_15m_updown",
        asset: "xrp",
        timeframe: "15m",
        slug_prefix: "xrp-updown-15m-",
    },
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdownScopeDef {
    scope: &'static str,
    asset: &'static str,
    timeframe: &'static str,
    slug_prefix: &'static str,
}

#[derive(Debug, Clone)]
struct TradeFlowNode {
    key: String,
    node_type: String,
    config: Value,
}

#[derive(Debug, Clone)]
struct TradeFlowEdge {
    source: String,
    target: String,
    edge_type: String,
}

#[derive(Debug, Clone)]
struct TradeFlowGraphRuntime {
    context: Value,
    nodes: Vec<TradeFlowNode>,
    edges: Vec<TradeFlowEdge>,
}

#[derive(Debug, Clone)]
struct TradeFlowRouteDecision {
    edge_type: String,
    available_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct TradeFlowNodeExecution {
    output: Value,
    routes: Vec<TradeFlowRouteDecision>,
    repeat_at: Option<DateTime<Utc>>,
    repeat_idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
struct TradeRuntime {
    trade_id: i64,
    market_slug: String,
    entry_price: f64,
    tp_price: f64,
    position_size: f64,
    state: TradeState,
}

#[derive(Debug, Clone)]
struct DualLegRuntime {
    side: LegSide,
    token_id: String,
    qty: f64,
    avg_entry: f64,
    levels_filled: u32,
    last_fill_price: Option<f64>,
    last_dca_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct DualBasketRuntime {
    trade_id: i64,
    market_slug: String,
    maker_base_fee: u64,
    state: TradeState,
    yes_leg: DualLegRuntime,
    no_leg: DualLegRuntime,
    cycle_ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct OrderMeta {
    leg_side: LegSide,
    side: String,
    intent: String,
}

#[derive(Debug, Clone)]
struct WsOpenPositionPriceNodeSpec {
    node_key: String,
    node_type: String,
    token_id: String,
    trigger_condition: String,
    trigger_price: f64,
}

#[derive(Debug, Clone)]
struct WsOpenPositionPriceRunSpec {
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    context: Value,
    nodes: Vec<WsOpenPositionPriceNodeSpec>,
    context_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileOutcome {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileErrorKind {
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialSource {
    Inline,
    Env,
}

impl CredentialSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Env => "env",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClobErrorClassification {
    reason_code: &'static str,
    reason_message: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedLiveMarket {
    slug: String,
    yes_token_id: Option<String>,
    no_token_id: Option<String>,
    maker_base_fee: u64,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    selection_reason: LiveMarketSelectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveMarketSelectionReason {
    InWindow,
    NearestFuture,
    LatestBySlugFallback,
    OverrideSlug,
}

impl LiveMarketSelectionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InWindow => "in_window",
            Self::NearestFuture => "nearest_future",
            Self::LatestBySlugFallback => "latest_by_slug_fallback",
            Self::OverrideSlug => "override_slug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketDiscoveryState {
    Ready,
    WaitingForMarket,
    Error,
}

impl MarketDiscoveryState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::WaitingForMarket => "waiting_for_market",
            Self::Error => "error",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config_dir = env::var("BOT_CONFIG_DIR").unwrap_or_else(|_| "./config".to_string());
    let database_url = env::var("DATABASE_URL").context(
        "DATABASE_URL is required, e.g. postgres://postgres:postgres@localhost:5432/dextrabot",
    )?;

    let cfg = AppConfig::load(&PathBuf::from(&config_dir))?;

    let repo = PostgresRepository::new(&database_url).await?;
    let mode = match cfg.bot.mode {
        ExecutionMode::Paper => "paper",
        ExecutionMode::Live => "live",
    };

    let run_id = repo
        .record_run_start(mode, env!("CARGO_PKG_VERSION"))
        .await?;
    repo.store_config_snapshot(
        run_id,
        "v5-live-trade-builder-and-pressure",
        &json!({
            "bot": {
                "mode": mode,
                "market_scope": cfg.bot.market_scope,
                "market_slug_override": cfg.bot.market_slug_override,
                "loop_interval_ms": cfg.bot.loop_interval_ms,
                "market_discovery_retry_interval_ms": cfg.bot.market_discovery_retry_interval_ms,
                "market_discovery_timeout_sec": cfg.bot.market_discovery_timeout_sec,
                "market_selection": cfg.bot.market_selection
            },
            "strategy": {
                "entry_price": cfg.strategy.entry_price,
                "tp_pct": cfg.strategy.tp_pct,
                "base_sl_pct": cfg.strategy.base_sl_pct,
                "aggressive_sl_pct": cfg.strategy.aggressive_sl_pct,
                "entry_window_sec": cfg.strategy.entry_window_sec,
                "max_hold_sec": cfg.strategy.max_hold_sec,
                "sl_renew_interval_ms": cfg.strategy.sl_renew_interval_ms,
                "flow_only": cfg.strategy.flow_only,
                "dual_side_enabled": cfg.strategy.dual_side_enabled,
                "total_notional_usdc": cfg.strategy.total_notional_usdc,
                "per_leg_initial_notional_usdc": cfg.strategy.per_leg_initial_notional_usdc,
                "dca_interval_sec": cfg.strategy.dca_interval_sec,
                "dca_step_pct": cfg.strategy.dca_step_pct,
                "max_dca_levels_per_leg": cfg.strategy.max_dca_levels_per_leg,
                "leg_tp_pct": cfg.strategy.leg_tp_pct,
                "basket_tp_usdc": cfg.strategy.basket_tp_usdc,
                "basket_sl_usdc": cfg.strategy.basket_sl_usdc,
                "force_flatten_sec_before_close": cfg.strategy.force_flatten_sec_before_close
            },
            "risk": {
                "max_daily_loss_usdc": cfg.risk.max_daily_loss_usdc,
                "max_consecutive_losses": cfg.risk.max_consecutive_losses,
                "max_notional_per_market_usdc": cfg.risk.max_notional_per_market_usdc,
                "max_open_orders": cfg.risk.max_open_orders,
                "max_stale_data_ms": cfg.risk.max_stale_data_ms,
                "kill_switch_mode": cfg.risk.kill_switch_mode,
                "manual_kill_switch_active": cfg.risk.manual_kill_switch_active
            },
            "execution": {
                "order_type": cfg.execution.order_type,
                "time_in_force": cfg.execution.time_in_force,
                "retry_count": cfg.execution.retry_count,
                "retry_backoff_ms": cfg.execution.retry_backoff_ms,
                "reconcile_interval_ms": cfg.execution.reconcile_interval_ms
            },
            "exchange": {
                "gamma_base_url": cfg.exchange.gamma_base_url,
                "clob_base_url": cfg.exchange.clob_base_url,
                "clob_ws_url": cfg.exchange.clob_ws_url,
                "chain_id": cfg.exchange.chain_id
            },
            "claim": {
                "enabled": cfg.claim.enabled,
                "rpc_url": cfg.claim.rpc_url,
                "rpc_url_env": cfg.claim.rpc_url_env,
                "data_api_base_url": cfg.claim.data_api_base_url,
                "user_address_inline_set": !cfg.claim.user_address.trim().is_empty(),
                "user_address_env": cfg.claim.user_address_env,
                "private_key_inline_set": !cfg.claim.private_key.trim().is_empty(),
                "private_key_env": cfg.claim.private_key_env,
                "chain_id": cfg.claim.chain_id,
                "ctf_contract_address": cfg.claim.ctf_contract_address,
                "collateral_token_address": cfg.claim.collateral_token_address,
                "discovery_interval_sec": cfg.claim.discovery_interval_sec,
                "positions_page_size": cfg.claim.positions_page_size,
                "positions_max_pages": cfg.claim.positions_max_pages,
                "process_batch_size": cfg.claim.process_batch_size,
                "max_attempts": cfg.claim.max_attempts,
                "retry_backoff_ms": cfg.claim.retry_backoff_ms
            }
        }),
    )
    .await?;

    info!(run_id, mode, "BOT_STARTED");

    let scopes = cfg.bot.resolve_scopes();

    if scopes.len() == 1 {
        loop {
            let result = match cfg.bot.mode {
                ExecutionMode::Paper => run_paper_loop(run_id, &repo, &cfg).await,
                ExecutionMode::Live => run_live_loop(run_id, &repo, &cfg).await,
            };
            if let Err(e) = result {
                repo.record_run_stop(run_id, "error").await.ok();
                info!(run_id, "BOT_STOPPED");
                return Err(e);
            }
            info!(run_id, "MARKET_CYCLE_COMPLETE_SEARCHING_NEXT");
            // no sleep: discovery loop starts immediately, auto_claim continues uninterrupted
        }
    } else {
        let mut handles = Vec::new();
        for scope in scopes {
            let mut scope_cfg = cfg.clone();
            scope_cfg.bot.market_scope = scope.clone();
            let scope_repo = repo.clone();
            let scope_run_id = scope_repo
                .record_run_start(mode, env!("CARGO_PKG_VERSION"))
                .await?;
            info!(run_id = scope_run_id, parent_run_id = run_id, scope = scope.as_str(), "SCOPE_STARTED");
            let handle = tokio::spawn(async move {
                loop {
                    let result = match scope_cfg.bot.mode {
                        ExecutionMode::Paper => {
                            run_paper_loop(scope_run_id, &scope_repo, &scope_cfg).await
                        }
                        ExecutionMode::Live => {
                            run_live_loop(scope_run_id, &scope_repo, &scope_cfg).await
                        }
                    };
                    if let Err(e) = result {
                        error!(run_id = scope_run_id, error = %e, "SCOPE_TASK_FAILED");
                        let _ = scope_repo.record_run_stop(scope_run_id, "error").await;
                        break;
                    }
                    info!(run_id = scope_run_id, "MARKET_CYCLE_COMPLETE_SEARCHING_NEXT");
                    // no sleep: discovery loop starts immediately
                }
            });
            handles.push(handle);
        }
        for handle in handles {
            let _ = handle.await;
        }
        repo.record_run_stop(run_id, "all scopes complete").await?;
        info!(run_id, "BOT_STOPPED");
    }
    Ok(())
}

async fn run_paper_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    if cfg.strategy.flow_only {
        return run_flow_only_loop(run_id, repo, cfg).await;
    }
    if cfg.strategy.dual_side_enabled {
        return run_paper_dual_loop(run_id, repo, cfg).await;
    }

    let cycle = MarketCycleId::from_now_rounded_5m(Utc::now());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    let market_slug = cycle.to_string();
    info!(run_id, market_id, market = %cycle, "MARKET_DISCOVERED");

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;

    let mut provider = MockMarketDataProvider::new();
    let strategy = PriceThresholdStrategy;
    let policy = DefaultRiskPolicy;
    let mut trade = create_runtime(repo, cfg, market_id, market_slug, &strategy).await?;
    transition(repo, &mut trade, TradeState::WaitingEntry, "cycle-start").await?;

    let limits = to_risk_limits(cfg);

    for iter in 0..12u32 {
        let tick = provider.next_tick(&trade.market_slug)?;
        let snapshot = provider.snapshot(&trade.market_slug)?;
        let now_ms = Utc::now().timestamp_millis();
        let merged = reconcile_tick_and_snapshot(tick.as_ref(), &snapshot, now_ms);

        if tick.is_none() {
            warn!(run_id, trade_id = trade.trade_id, iter, "WS_STALE");
        }

        match risk_gate(
            repo,
            run_id,
            cfg,
            &trade,
            &limits,
            merged.stale_data_ms,
            &policy,
        )
        .await?
        {
            RiskDecision::Halt => {
                halt_trade(repo, run_id, &mut trade, "risk-halt", None).await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        process_trade_step(
            repo,
            run_id,
            cfg,
            &mut trade,
            merged.chosen_price,
            true,
            &strategy,
        )
        .await?;

        if matches!(trade.state, TradeState::Settled | TradeState::Halted) {
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    if can_transition(trade.state, TradeState::Settled).is_ok() {
        transition(repo, &mut trade, TradeState::Settled, "paper-loop-end").await?;
    }

    Ok(())
}

fn resolve_api_credentials_with_source(
    cfg: &AppConfig,
) -> Result<(ApiCredentials, CredentialSource)> {
    let inline_address = cfg.exchange.api_address.trim();
    let inline_key = cfg.exchange.api_key.trim();
    let inline_secret = cfg.exchange.api_secret.trim();
    let inline_passphrase = cfg.exchange.api_passphrase.trim();
    let inline_any = !inline_address.is_empty()
        || !inline_key.is_empty()
        || !inline_secret.is_empty()
        || !inline_passphrase.is_empty();

    if inline_any {
        anyhow::ensure!(
            !inline_address.is_empty()
                && !inline_key.is_empty()
                && !inline_secret.is_empty()
                && !inline_passphrase.is_empty(),
            "api_address, api_key, api_secret, api_passphrase must all be set when using direct credentials"
        );

        let has_encrypted_inline = [inline_address, inline_key, inline_secret, inline_passphrase]
            .into_iter()
            .any(|value| value.starts_with(CONFIG_ENC_PREFIX));

        let key_material = if has_encrypted_inline {
            Some(load_config_encryption_key()?)
        } else {
            None
        };

        let address =
            decrypt_config_value_if_needed("api_address", inline_address, key_material.as_ref())?;
        let key = decrypt_config_value_if_needed("api_key", inline_key, key_material.as_ref())?;
        let secret =
            decrypt_config_value_if_needed("api_secret", inline_secret, key_material.as_ref())?;
        let passphrase = decrypt_config_value_if_needed(
            "api_passphrase",
            inline_passphrase,
            key_material.as_ref(),
        )?;

        return Ok((
            ApiCredentials {
                address,
                key,
                secret,
                passphrase,
            },
            CredentialSource::Inline,
        ));
    }

    Ok((
        ApiCredentials::from_env(
            &cfg.exchange.api_address_env,
            &cfg.exchange.api_key_env,
            &cfg.exchange.api_secret_env,
            &cfg.exchange.api_passphrase_env,
        )?,
        CredentialSource::Env,
    ))
}

fn masked_prefix(value: &str, take: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let prefix: String = trimmed.chars().take(take).collect();
    format!("{prefix}***")
}

fn extract_http_status_code(raw: &str) -> Option<i32> {
    if raw.contains("401 Unauthorized") {
        return Some(401);
    }
    if raw.contains("403 Forbidden") {
        return Some(403);
    }
    if raw.contains("400 Bad Request") {
        return Some(400);
    }
    if raw.contains("429 Too Many Requests") {
        return Some(429);
    }
    None
}

fn classify_clob_error(err: &anyhow::Error) -> ClobErrorClassification {
    let raw = err.to_string().to_ascii_lowercase();
    if raw.contains("trading restricted in your region") || raw.contains("geoblock") {
        return ClobErrorClassification {
            reason_code: "geoblock_restricted",
            reason_message: "Trading is restricted in this region.",
        };
    }
    if raw.contains("unauthorized/invalid api key") || raw.contains("invalid api key") {
        return ClobErrorClassification {
            reason_code: "invalid_api_key",
            reason_message: "CLOB API key credentials are invalid.",
        };
    }
    if raw.contains("401 unauthorized") {
        return ClobErrorClassification {
            reason_code: "unauthorized",
            reason_message: "Request is unauthorized.",
        };
    }
    if raw.contains("403 forbidden") {
        return ClobErrorClassification {
            reason_code: "forbidden",
            reason_message: "Request is forbidden.",
        };
    }
    if raw.contains("400 bad request") {
        return ClobErrorClassification {
            reason_code: "bad_request",
            reason_message: "Request payload or path is invalid.",
        };
    }
    if raw.contains("timed out") || raw.contains("timeout") {
        return ClobErrorClassification {
            reason_code: "request_timeout",
            reason_message: "Request timed out.",
        };
    }
    ClobErrorClassification {
        reason_code: "clob_request_failed",
        reason_message: "CLOB request failed.",
    }
}

fn log_resolved_api_credentials(run_id: i64, creds: &ApiCredentials, source: CredentialSource) {
    info!(
        run_id,
        credential_source = source.as_str(),
        api_address = %creds.address,
        api_key_prefix = %masked_prefix(&creds.key, 8),
        api_secret_len = creds.secret.trim().len(),
        api_passphrase_len = creds.passphrase.trim().len(),
        "CLOB_CREDENTIALS_RESOLVED"
    );
}

async fn record_clob_auth_preflight_event(
    repo: &PostgresRepository,
    run_id: i64,
    decision: &str,
    reason_code: &str,
    reason_message: &str,
    source: CredentialSource,
    creds: &ApiCredentials,
    status_code: Option<i32>,
    error: Option<&str>,
) {
    let details = json!({
        "run_id": run_id,
        "decision": decision,
        "reason_code": reason_code,
        "reason_message": reason_message,
        "status_code": status_code,
        "credential_source": source.as_str(),
        "api_address": creds.address,
        "api_key_prefix": masked_prefix(&creds.key, 8),
        "api_secret_len": creds.secret.trim().len(),
        "api_passphrase_len": creds.passphrase.trim().len(),
        "error": error
    })
    .to_string();

    if let Err(err) = repo
        .record_risk_event(None, "clob_auth_preflight", decision, &details)
        .await
    {
        warn!(
            run_id,
            error = %err,
            "CLOB_AUTH_PREFLIGHT_EVENT_WRITE_FAILED"
        );
    }
}

async fn run_clob_auth_preflight(
    run_id: i64,
    repo: &PostgresRepository,
    client: &ClobHttpClient,
    creds: &ApiCredentials,
    source: CredentialSource,
) {
    match ClobRestClient::list_fills(client, None).await {
        Ok(fills) => {
            info!(
                run_id,
                status_code = 200,
                reason_code = "allow",
                reason_message = "Auth preflight succeeded.",
                credential_source = source.as_str(),
                api_address = %creds.address,
                api_key_prefix = %masked_prefix(&creds.key, 8),
                fill_count = fills.len(),
                "CLOB_AUTH_PREFLIGHT_OK"
            );
            record_clob_auth_preflight_event(
                repo,
                run_id,
                "allow",
                "allow",
                "Auth preflight succeeded.",
                source,
                creds,
                Some(200),
                None,
            )
            .await;
        }
        Err(err) => {
            let classification = classify_clob_error(&err);
            let error_text = err.to_string();
            let status_code = extract_http_status_code(&error_text);
            warn!(
                run_id,
                status_code = ?status_code,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                credential_source = source.as_str(),
                api_address = %creds.address,
                api_key_prefix = %masked_prefix(&creds.key, 8),
                error = %err,
                "CLOB_AUTH_PREFLIGHT_FAILED"
            );
            record_clob_auth_preflight_event(
                repo,
                run_id,
                "block",
                classification.reason_code,
                classification.reason_message,
                source,
                creds,
                status_code,
                Some(error_text.as_str()),
            )
            .await;
        }
    }
}

async fn run_daily_pnl_startup_check(
    run_id: i64,
    repo: &PostgresRepository,
    max_daily_loss_usdc: f64,
) -> Result<()> {
    let daily_pnl = repo.daily_realized_pnl().await?;
    info!(
        run_id,
        daily_pnl_usdc = daily_pnl,
        max_daily_loss_usdc = max_daily_loss_usdc,
        "STARTUP_DAILY_PNL_CHECK"
    );
    if daily_pnl <= -max_daily_loss_usdc {
        anyhow::bail!(
            "Daily loss limit already breached at startup: pnl={:.2} limit={:.2}",
            daily_pnl,
            max_daily_loss_usdc
        );
    }
    Ok(())
}

async fn run_balance_preflight(
    run_id: i64,
    repo: &PostgresRepository,
    client: &ClobHttpClient,
    min_balance_usdc: f64,
) {
    match ClobRestClient::get_balance(client).await {
        Ok(balance) => {
            info!(
                run_id,
                balance_usdc = balance,
                min_balance_usdc = min_balance_usdc,
                "CLOB_BALANCE_PREFLIGHT_OK"
            );
            if balance < min_balance_usdc {
                let _ = repo
                    .record_risk_event(
                        None,
                        "balance_preflight",
                        "halt",
                        &format!("balance={:.2} < min={:.2}", balance, min_balance_usdc),
                    )
                    .await;
                panic!(
                    "Insufficient USDC balance at startup: {:.2} < {:.2}",
                    balance, min_balance_usdc
                );
            }
        }
        Err(err) => {
            warn!(run_id, error = %err, "CLOB_BALANCE_PREFLIGHT_FAILED_SKIP");
        }
    }
}

fn load_config_encryption_key() -> Result<[u8; 32]> {
    let encoded = env::var("CONFIG_ENCRYPTION_KEY")
        .context("CONFIG_ENCRYPTION_KEY is required to decrypt encrypted exchange credentials")?;
    let decoded = BASE64_STANDARD
        .decode(encoded.trim().as_bytes())
        .context("CONFIG_ENCRYPTION_KEY must be valid base64")?;

    anyhow::ensure!(
        decoded.len() == 32,
        "CONFIG_ENCRYPTION_KEY must decode to exactly 32 bytes"
    );

    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn decrypt_config_value_if_needed(
    field_name: &str,
    value: &str,
    key_material: Option<&[u8; 32]>,
) -> Result<String> {
    if !value.starts_with(CONFIG_ENC_PREFIX) {
        return Ok(value.to_string());
    }

    let key = key_material.context(
        "encrypted inline credentials found but CONFIG_ENCRYPTION_KEY was not available",
    )?;

    let payload = value.trim_start_matches(CONFIG_ENC_PREFIX);
    let decoded = BASE64_STANDARD
        .decode(payload.as_bytes())
        .with_context(|| format!("invalid encrypted payload for {field_name}"))?;

    anyhow::ensure!(
        decoded.len() > CONFIG_ENC_NONCE_LEN + CONFIG_ENC_TAG_LEN,
        "encrypted payload for {field_name} is too short"
    );

    let (nonce_bytes, encrypted_and_tag) = decoded.split_at(CONFIG_ENC_NONCE_LEN);
    let split_at = encrypted_and_tag.len() - CONFIG_ENC_TAG_LEN;
    let (ciphertext, auth_tag) = encrypted_and_tag.split_at(split_at);

    let mut combined = Vec::with_capacity(ciphertext.len() + auth_tag.len());
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(auth_tag);

    let cipher = Aes256Gcm::new_from_slice(key).context("invalid config encryption key")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), combined.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to decrypt encrypted value for {field_name}"))?;

    String::from_utf8(plaintext)
        .with_context(|| format!("decrypted value for {field_name} is not valid UTF-8"))
}

async fn record_market_discovery_event(
    repo: &PostgresRepository,
    run_id: i64,
    decision: &str,
    state: MarketDiscoveryState,
    scope: &str,
    selected_market_slug: Option<&str>,
    reason_code: &str,
    message: &str,
) {
    let details = json!({
        "run_id": run_id,
        "state": state.as_str(),
        "market_scope": scope,
        "selected_market_slug": selected_market_slug,
        "reason_code": reason_code,
        "message": message
    })
    .to_string();

    if let Err(err) = repo
        .record_risk_event(None, "market_discovery", decision, &details)
        .await
    {
        warn!(
            run_id,
            error = %err,
            "MARKET_DISCOVERY_EVENT_WRITE_FAILED"
        );
    }
}

fn supported_updown_scope_names_csv() -> String {
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .map(|def| def.scope)
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_updown_scope_by_scope(scope: &str) -> Option<UpdownScopeDef> {
    let normalized = scope.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| def.scope == normalized)
}

pub(crate) fn find_updown_scope_by_asset_timeframe(asset: &str, timeframe: &str) -> Option<UpdownScopeDef> {
    let normalized_asset = asset.trim().to_ascii_lowercase();
    let normalized_timeframe = timeframe.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| def.asset == normalized_asset && def.timeframe == normalized_timeframe)
}

fn find_updown_slug_prefix(raw: &str) -> Option<&'static str> {
    let normalized = raw.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .find(|def| normalized.starts_with(def.slug_prefix))
        .map(|def| def.slug_prefix)
}

fn find_updown_scope_by_slug(slug: &str) -> Option<UpdownScopeDef> {
    let normalized = slug.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| normalized.starts_with(def.slug_prefix))
}

fn updown_scope_window_seconds(scope_def: UpdownScopeDef) -> i64 {
    match scope_def.timeframe {
        "15m" => 900,
        _ => 300,
    }
}

fn updown_scope_candidate_slugs(scope_def: UpdownScopeDef, now: DateTime<Utc>) -> Vec<String> {
    let window = updown_scope_window_seconds(scope_def);
    let now_ts = now.timestamp();
    let base = now_ts - now_ts.rem_euclid(window);
    [base - window, base, base + window, base + (2 * window)]
        .into_iter()
        .filter(|ts| *ts > 0)
        .map(|ts| format!("{}{}", scope_def.slug_prefix, ts))
        .collect()
}

pub(crate) async fn list_markets_for_scope(gamma: &GammaHttpClient, scope: &str) -> Result<Vec<GammaMarket>> {
    let scope_def = find_updown_scope_by_scope(scope).ok_or_else(|| {
        anyhow::anyhow!(
            "unsupported market_scope: {scope} (supported: {})",
            supported_updown_scope_names_csv()
        )
    })?;
    let mut markets: Vec<GammaMarket> = gamma
        .list_active_updown_markets()
        .await?
        .into_iter()
        .filter(|market| market.slug.starts_with(scope_def.slug_prefix))
        .collect();

    if !markets.is_empty() {
        // Prefer markets around the current time window so DCA targets the
        // currently traded 5m/15m market instead of a far-future active slug.
        let candidate_slugs: HashSet<String> = updown_scope_candidate_slugs(scope_def, Utc::now())
            .into_iter()
            .collect();
        let now_for_filter = Utc::now();
        let in_window: Vec<GammaMarket> = markets
            .iter()
            .filter(|market| {
                if !candidate_slugs.contains(&market.slug) {
                    return false;
                }
                // Gamma API gecikmesi nedeniyle bitmiş market dönebilir — hariç tut
                let (_, ends_at) = infer_updown_market_window(market);
                ends_at.map(|e| e > now_for_filter).unwrap_or(true)
            })
            .cloned()
            .collect();
        if !in_window.is_empty() {
            return Ok(in_window);
        }
        return Ok(markets);
    }

    let mut seen_slugs: HashSet<String> = HashSet::new();
    for market in &markets {
        seen_slugs.insert(market.slug.clone());
    }

    for slug in updown_scope_candidate_slugs(scope_def, Utc::now()) {
        if !seen_slugs.insert(slug.clone()) {
            continue;
        }
        let fetched = match gamma.get_market_by_slug(&slug).await {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(market) = fetched else {
            continue;
        };
        if !market.slug.starts_with(scope_def.slug_prefix) {
            continue;
        }
        if !market.active || market.closed {
            continue;
        }
        markets.push(market);
    }

    Ok(markets)
}

fn infer_updown_market_window(
    market: &GammaMarket,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let starts_from_slug = MarketCycleId(market.slug.clone()).start_time();
    let ends_from_iso = market.end_date_iso.as_deref().and_then(parse_rfc3339_utc);
    let Some(scope_def) = find_updown_scope_by_slug(&market.slug) else {
        return (starts_from_slug, ends_from_iso);
    };
    let window = ChronoDuration::seconds(updown_scope_window_seconds(scope_def));
    match (starts_from_slug, ends_from_iso) {
        (Some(starts_at), Some(ends_at)) => (Some(starts_at), Some(ends_at)),
        (Some(starts_at), None) => (Some(starts_at), Some(starts_at + window)),
        (None, Some(ends_at)) => (Some(ends_at - window), Some(ends_at)),
        (None, None) => (None, None),
    }
}

fn select_live_market_with_reason(
    market: GammaMarket,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    reason: LiveMarketSelectionReason,
) -> SelectedLiveMarket {
    SelectedLiveMarket {
        slug: market.slug,
        yes_token_id: market.yes_token_id,
        no_token_id: market.no_token_id,
        maker_base_fee: market.maker_base_fee,
        starts_at,
        ends_at,
        selection_reason: reason,
    }
}

pub(crate) fn select_preferred_live_market(
    markets: Vec<GammaMarket>,
    now: DateTime<Utc>,
) -> Option<SelectedLiveMarket> {
    let timed: Vec<(GammaMarket, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> = markets
        .into_iter()
        .map(|market| {
            let (starts_at, ends_at) = infer_updown_market_window(&market);
            (market, starts_at, ends_at)
        })
        .collect();
    if timed.is_empty() {
        return None;
    }

    if let Some((market, starts_at, ends_at)) = timed
        .iter()
        .filter_map(|(market, starts_at, ends_at)| match (starts_at, ends_at) {
            (Some(start), Some(end)) if *start <= now && now < *end => {
                Some((market.clone(), Some(start.clone()), Some(end.clone())))
            }
            _ => None,
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.slug.cmp(&b.0.slug)))
    {
        return Some(select_live_market_with_reason(
            market,
            starts_at,
            ends_at,
            LiveMarketSelectionReason::InWindow,
        ));
    }

    if let Some((market, starts_at, ends_at)) = timed
        .iter()
        .filter_map(|(market, starts_at, ends_at)| match starts_at {
            Some(start) if *start >= now => Some((
                market.clone(),
                Some(start.clone()),
                ends_at.as_ref().cloned(),
            )),
            _ => None,
        })
        .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.slug.cmp(&b.0.slug)))
    {
        return Some(select_live_market_with_reason(
            market,
            starts_at,
            ends_at,
            LiveMarketSelectionReason::NearestFuture,
        ));
    }

    timed
        .into_iter()
        .max_by(|a, b| a.0.slug.cmp(&b.0.slug))
        .map(|(market, starts_at, ends_at)| {
            select_live_market_with_reason(
                market,
                starts_at,
                ends_at,
                LiveMarketSelectionReason::LatestBySlugFallback,
            )
        })
}

fn select_live_market(
    markets: Vec<GammaMarket>,
    selection: &str,
    require_yes_no_tokens: bool,
) -> Option<SelectedLiveMarket> {
    let candidates: Vec<GammaMarket> = markets
        .into_iter()
        .filter(|m| !require_yes_no_tokens || (m.yes_token_id.is_some() && m.no_token_id.is_some()))
        .collect();

    match selection {
        "latest_by_slug" => select_preferred_live_market(candidates, Utc::now()),
        _ => None,
    }
}

fn extract_slug_from_market_override(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let direct = trimmed
        .trim_end_matches('/')
        .split(['?', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    if find_updown_slug_prefix(direct).is_some() {
        return Some(direct.to_ascii_lowercase());
    }

    trimmed.split(['/', '?', '#', '&', '=']).find_map(|part| {
        let normalized = part.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }
        find_updown_slug_prefix(&normalized).map(|_| normalized)
    })
}

fn configured_market_override_slug(cfg: &AppConfig) -> Result<Option<String>> {
    if cfg.bot.market_slug_override.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(extract_slug_from_market_override(&cfg.bot.market_slug_override).ok_or_else(
        || {
            anyhow::anyhow!(
                "market_slug_override must include a supported updown slug (e.g. btc-updown-5m-..., eth-updown-15m-...) or a polymarket event URL"
            )
        },
    )?))
}

fn select_market_from_candidates(
    markets: Vec<GammaMarket>,
    override_slug: Option<&str>,
    selection: &str,
    require_yes_no_tokens: bool,
) -> Option<SelectedLiveMarket> {
    if let Some(forced_slug) = override_slug {
        return markets
            .into_iter()
            .find(|m| {
                m.slug == forced_slug
                    && (!require_yes_no_tokens
                        || (m.yes_token_id.is_some() && m.no_token_id.is_some()))
            })
            .map(|m| {
                let (starts_at, ends_at) = infer_updown_market_window(&m);
                select_live_market_with_reason(
                    m,
                    starts_at,
                    ends_at,
                    LiveMarketSelectionReason::OverrideSlug,
                )
            });
    }

    select_live_market(markets, selection, require_yes_no_tokens)
}

async fn discover_live_market_once(
    cfg: &AppConfig,
    gamma: &GammaHttpClient,
    require_yes_no_tokens: bool,
    override_slug: Option<&str>,
) -> Result<Option<SelectedLiveMarket>> {
    let markets = list_markets_for_scope(gamma, &cfg.bot.market_scope).await?;
    Ok(select_market_from_candidates(
        markets,
        override_slug,
        &cfg.bot.market_selection,
        require_yes_no_tokens,
    ))
}

async fn discover_live_market(
    run_id: i64,
    repo: &PostgresRepository,
    cfg: &AppConfig,
    gamma: &GammaHttpClient,
    require_yes_no_tokens: bool,
) -> Result<SelectedLiveMarket> {
    let override_slug = configured_market_override_slug(cfg)?;

    let retry_interval = Duration::from_millis(cfg.bot.market_discovery_retry_interval_ms);
    let timeout = if cfg.bot.market_discovery_timeout_sec == 0 {
        None
    } else {
        Some(Duration::from_secs(cfg.bot.market_discovery_timeout_sec))
    };
    let started_at = Instant::now();
    let mut waiting_event_emitted = false;

    loop {
        let markets = match list_markets_for_scope(gamma, &cfg.bot.market_scope).await {
            Ok(markets) => markets,
            Err(err) => {
                warn!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    error = %err,
                    "MARKET_DISCOVERY_FETCH_FAILED"
                );
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_discovery_fetch_failed",
                        "Failed to fetch market list. Retrying.",
                    )
                    .await;
                }

                if let Some(max_wait) = timeout {
                    if started_at.elapsed() >= max_wait {
                        let timeout_message = format!(
                            "Market discovery timed out after {}s while fetching market list.",
                            cfg.bot.market_discovery_timeout_sec
                        );
                        error!(
                            run_id,
                            scope = %cfg.bot.market_scope,
                            timeout_sec = cfg.bot.market_discovery_timeout_sec,
                            message = %timeout_message,
                            "MARKET_DISCOVERY_TIMEOUT"
                        );
                        record_market_discovery_event(
                            repo,
                            run_id,
                            "halt",
                            MarketDiscoveryState::Error,
                            &cfg.bot.market_scope,
                            None,
                            "market_discovery_timeout",
                            &timeout_message,
                        )
                        .await;
                        anyhow::bail!(timeout_message);
                    }
                }

                sleep(retry_interval).await;
                continue;
            }
        };

        let selected = select_market_from_candidates(
            markets,
            override_slug.as_deref(),
            &cfg.bot.market_selection,
            require_yes_no_tokens,
        );

        if let Some(selected) = selected {
            info!(
                run_id,
                scope = %cfg.bot.market_scope,
                selection = %cfg.bot.market_selection,
                override_slug = ?override_slug,
                market = %selected.slug,
                selection_reason = selected.selection_reason.as_str(),
                market_start_at = ?selected.starts_at,
                market_end_at = ?selected.ends_at,
                now_utc = %Utc::now(),
                "MARKET_DISCOVERY_FOUND"
            );
            record_market_discovery_event(
                repo,
                run_id,
                "allow",
                MarketDiscoveryState::Ready,
                &cfg.bot.market_scope,
                Some(&selected.slug),
                "market_discovery_ready",
                "Market selected successfully.",
            )
            .await;
            return Ok(selected);
        }

        if !waiting_event_emitted {
            waiting_event_emitted = true;
            let waiting_message = if let Some(forced_slug) = override_slug.as_ref() {
                if require_yes_no_tokens {
                    format!(
                        "Override market not active or missing YES/NO token IDs: {forced_slug}. Retrying."
                    )
                } else {
                    format!("Override market not active yet: {forced_slug}. Retrying.")
                }
            } else if require_yes_no_tokens {
                "No active market with YES/NO token IDs. Retrying.".to_string()
            } else {
                "No active market found. Retrying.".to_string()
            };
            info!(
                run_id,
                scope = %cfg.bot.market_scope,
                selection = %cfg.bot.market_selection,
                override_slug = ?override_slug,
                retry_interval_ms = cfg.bot.market_discovery_retry_interval_ms,
                timeout_sec = cfg.bot.market_discovery_timeout_sec,
                "MARKET_DISCOVERY_WAITING"
            );
            record_market_discovery_event(
                repo,
                run_id,
                "block",
                MarketDiscoveryState::WaitingForMarket,
                &cfg.bot.market_scope,
                None,
                if require_yes_no_tokens {
                    "market_missing_token_ids"
                } else {
                    "market_discovery_waiting"
                },
                &waiting_message,
            )
            .await;
        }

        if let Some(max_wait) = timeout {
            if started_at.elapsed() >= max_wait {
                let timeout_message = format!(
                    "Market discovery timed out after {}s.",
                    cfg.bot.market_discovery_timeout_sec
                );
                error!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    timeout_sec = cfg.bot.market_discovery_timeout_sec,
                    message = %timeout_message,
                    "MARKET_DISCOVERY_TIMEOUT"
                );
                record_market_discovery_event(
                    repo,
                    run_id,
                    "halt",
                    MarketDiscoveryState::Error,
                    &cfg.bot.market_scope,
                    None,
                    "market_discovery_timeout",
                    &timeout_message,
                )
                .await;
                anyhow::bail!(timeout_message);
            }
        }

        sleep(retry_interval).await;
    }
}

async fn run_live_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    if cfg.strategy.flow_only {
        return run_flow_only_loop(run_id, repo, cfg).await;
    }
    if cfg.strategy.dual_side_enabled {
        return run_live_dual_loop(run_id, repo, cfg).await;
    }

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let selected = discover_live_market(run_id, repo, cfg, &gamma, false).await?;
    let selected_reason = selected.selection_reason.as_str();
    let selected_start_at = selected.starts_at.as_ref().cloned();
    let selected_end_at = selected.ends_at.as_ref().cloned();
    let cycle_slug = selected.slug;
    let entry_token_id = selected
        .yes_token_id
        .clone()
        .or(selected.no_token_id.clone());

    let cycle = MarketCycleId(cycle_slug.clone());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    info!(
        run_id,
        market_id,
        market = %cycle_slug,
        selection_reason = selected_reason,
        market_start_at = ?selected_start_at,
        market_end_at = ?selected_end_at,
        "LIVE_MARKET_DISCOVERED"
    );

    let ws = ClobWsClient::new(cfg.exchange.clob_ws_url.clone());
    let mut user_ws_events: Vec<WsEvent> = Vec::new();
    let market_ws_ids = if let Some(token_id) = entry_token_id.as_ref() {
        vec![token_id.clone()]
    } else {
        vec![cycle_slug.clone()]
    };
    match ws.subscribe_once(WsChannel::Market, &market_ws_ids).await {
        Ok(messages) => {
            info!(run_id, market = %cycle_slug, ws_messages = messages.len(), "WS_CONNECT_OK")
        }
        Err(e) => {
            warn!(run_id, market = %cycle_slug, error = %e, "WS_CONNECT_FAILED_USING_REST_FALLBACK")
        }
    }

    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    let strategy = PriceThresholdStrategy;
    let policy = DefaultRiskPolicy;
    let mut clob_client = None;
    if live_enabled {
        let (creds, credential_source) = resolve_api_credentials_with_source(cfg)?;
        log_resolved_api_credentials(run_id, &creds, credential_source);
        let private_key = cfg
            .exchange
            .resolve_signer_private_key()
            .context("CLOB signer private key")?;
        let wallet = private_key
            .parse::<LocalWallet>()
            .context("parse signer private key")?
            .with_chain_id(cfg.exchange.chain_id);
        let exchange_address: Address = cfg
            .exchange
            .ctf_exchange_address
            .parse()
            .context("parse ctf_exchange_address")?;
        let gnosis_safe: Option<Address> = cfg
            .exchange
            .resolve_gnosis_safe_address()
            .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
            .transpose()?;
        let client = ClobHttpClient::from_credentials(
            cfg.exchange.clob_base_url.clone(),
            creds.clone(),
            wallet,
            exchange_address,
            cfg.exchange.chain_id,
            gnosis_safe,
        );
        run_clob_auth_preflight(run_id, repo, &client, &creds, credential_source).await;
        clob_client = Some(client);
        match ws
            .subscribe_once(WsChannel::User, &[cycle_slug.clone()])
            .await
        {
            Ok(messages) => {
                info!(run_id, market = %cycle_slug, ws_messages = messages.len(), "WS_USER_CONNECT_OK");
                user_ws_events = messages;
            }
            Err(e) => {
                warn!(run_id, market = %cycle_slug, error = %e, "WS_USER_CONNECT_FAILED_USING_REST_RECONCILE")
            }
        }
    }
    let mut auto_claim = match AutoClaimService::from_app_config(cfg) {
        Ok(service) => service,
        Err(err) => {
            warn!(run_id, error = %err, "AUTO_CLAIM_DISABLED_DUE_TO_CONFIG_ERROR");
            None
        }
    };

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;
    if let Some(client) = clob_client.as_ref() {
        run_balance_preflight(run_id, repo, client, cfg.risk.min_balance_usdc).await;
    }

    let mut trade = create_runtime(repo, cfg, market_id, cycle_slug.clone(), &strategy).await?;
    transition(
        repo,
        &mut trade,
        TradeState::WaitingEntry,
        "live-cycle-start",
    )
    .await?;
    let limits = to_risk_limits(cfg);

    let mut reconcile_errors = 0u32;
    for iter in 0..20u32 {
        let price = if let Some(client) = clob_client.as_ref() {
            let midpoint_key = entry_token_id
                .as_deref()
                .unwrap_or(trade.market_slug.as_str());
            match client.midpoint(midpoint_key).await {
                Ok(s) => s.price,
                Err(e) => {
                    warn!(run_id, trade_id = trade.trade_id, error = %e, "REST_SNAPSHOT_FAILED");
                    trade.entry_price
                }
            }
        } else {
            // Dry live mode: no API keys, evaluate state machine with deterministic price.
            if iter % 4 == 0 {
                trade.tp_price
            } else {
                trade.entry_price
            }
        };

        if live_enabled {
            if let Some(client) = clob_client.as_ref() {
                if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, client, &ws).await {
                    warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
                }
                if let Err(e) =
                    process_trade_builder_workflows(repo, run_id, cfg, client, &ws).await
                {
                    warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
                }
                if let Err(e) =
                    dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, client, &ws).await
                {
                    warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
                }
            }
        }
        if let Err(e) = process_trade_flows(
            repo,
            run_id,
            cfg,
            clob_client
                .as_ref()
                .map(|client| client as &dyn OrderExecutor),
            &ws,
        )
        .await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        if let Some(service) = auto_claim.as_mut() {
            if let Err(e) = service.maybe_tick(repo).await {
                warn!(run_id, error = %e, "AUTO_CLAIM_TICK_FAILED");
            }
        }

        match risk_gate(repo, run_id, cfg, &trade, &limits, 0, &policy).await? {
            RiskDecision::Halt => {
                halt_trade(
                    repo,
                    run_id,
                    &mut trade,
                    "risk-halt",
                    clob_client
                        .as_ref()
                        .map(|client| client as &dyn OrderExecutor),
                )
                .await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        if live_enabled
            && matches!(trade.state, TradeState::WaitingEntry)
            && strategy.entry_signal(price, trade.entry_price)
        {
            if let Some(client) = clob_client.as_ref() {
                transition(
                    repo,
                    &mut trade,
                    TradeState::EntryPlaced,
                    "entry-threshold-hit",
                )
                .await?;
                let client_order_id = Uuid::new_v4().to_string();
                let req = PlaceOrderRequest {
                    market: trade.market_slug.clone(),
                    token_id: entry_token_id.clone(),
                    side: "buy".to_string(),
                    price: trade.entry_price,
                    size: trade.position_size,
                    intent: "entry".to_string(),
                    client_order_id: client_order_id.clone(),
                    leg_side: None,
                    fee_rate_bps: 1000,
                };

                let ack = client.place(&req).await?;
                let exchange_order_id = ack
                    .exchange_order_id
                    .as_deref()
                    .unwrap_or(&ack.client_order_id);
                let raw = json!({
                    "client_order_id": ack.client_order_id,
                    "exchange_order_id": ack.exchange_order_id,
                    "status": ack.status,
                    "reject_reason": ack.reject_reason,
                    "raw_status": ack.raw_status,
                    "exchange_ts": ack.exchange_ts
                });
                repo.upsert_order_by_exchange_id(
                    trade.trade_id,
                    exchange_order_id,
                    Some(&ack.client_order_id),
                    "entry",
                    "buy",
                    trade.entry_price,
                    trade.position_size,
                    &ack.status,
                    ack.exchange_ts,
                    ack.reject_reason.as_deref(),
                    &raw,
                )
                .await?;
                info!(run_id, trade_id = trade.trade_id, client_order_id = %ack.client_order_id, "LIVE_ENTRY_ACK");
            }
        }

        if live_enabled {
            if let Some(client) = clob_client.as_ref() {
                match reconcile_live(repo, run_id, &mut trade, client, &user_ws_events).await? {
                    ReconcileOutcome::Ok => {
                        reconcile_errors = 0;
                    }
                    ReconcileOutcome::Warning => {
                        reconcile_errors = 0;
                    }
                    ReconcileOutcome::Error => {
                        reconcile_errors += 1;
                        if reconcile_errors >= 3 {
                            halt_trade(
                                repo,
                                run_id,
                                &mut trade,
                                "reconcile-error-threshold",
                                Some(client),
                            )
                            .await?;
                            warn!(
                                run_id,
                                trade_id = trade.trade_id,
                                "RECONCILE_ERROR_THRESHOLD_HALTED"
                            );
                            break;
                        }
                        sleep(Duration::from_millis(cfg.execution.retry_backoff_ms)).await;
                        continue;
                    }
                }
            }
        } else {
            process_trade_step(repo, run_id, cfg, &mut trade, price, true, &strategy).await?;
        }

        if matches!(trade.state, TradeState::Settled | TradeState::Halted) {
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    if can_transition(trade.state, TradeState::Settled).is_ok() {
        transition(repo, &mut trade, TradeState::Settled, "live-loop-end").await?;
    }

    Ok(())
}

async fn run_paper_dual_loop(
    run_id: i64,
    repo: &PostgresRepository,
    cfg: &AppConfig,
) -> Result<()> {
    let cycle = MarketCycleId::from_now_rounded_5m(Utc::now());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    let market_slug = cycle.to_string();
    let cycle_ends_at = cycle.start_time().unwrap_or_else(Utc::now) + ChronoDuration::seconds(300);
    info!(run_id, market_id, market = %market_slug, "PAPER_DUAL_MARKET_DISCOVERED");

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;

    let strategy = SymmetricDualDcaStrategy;
    let policy = DefaultRiskPolicy;
    let limits = to_risk_limits(cfg);

    let mut basket = create_dual_runtime(
        repo,
        cfg,
        market_id,
        market_slug.clone(),
        "paper-yes".to_string(),
        "paper-no".to_string(),
        0,
        cycle_ends_at,
    )
    .await?;

    transition_dual(
        repo,
        &mut basket,
        TradeState::WaitingEntry,
        "paper-dual-cycle-start",
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryPlaced,
        "paper-dual-initial-entry",
    )
    .await?;

    let mut provider = MockMarketDataProvider::new();
    let level_notional =
        cfg.strategy.per_leg_initial_notional_usdc / cfg.strategy.max_dca_levels_per_leg as f64;
    let start_yes = clamp_probability(cfg.strategy.entry_price);
    let start_no = clamp_probability(1.0 - start_yes);
    let start_yes_size = calc_level_size(level_notional, start_yes);
    let start_no_size = calc_level_size(level_notional, start_no);

    record_paper_leg_fill(
        repo,
        basket.trade_id,
        &mut basket.yes_leg,
        "entry",
        "buy",
        start_yes,
        start_yes_size,
    )
    .await?;
    record_paper_leg_fill(
        repo,
        basket.trade_id,
        &mut basket.no_leg,
        "entry",
        "buy",
        start_no,
        start_no_size,
    )
    .await?;

    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryFilled,
        "paper-dual-entry-filled",
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::TpPlaced,
        "paper-dual-tp-active",
    )
    .await?;

    for iter in 0..40u32 {
        let tick = provider.next_tick(&basket.market_slug)?;
        let snapshot = provider.snapshot(&basket.market_slug)?;
        let now_ms = Utc::now().timestamp_millis();
        let merged = reconcile_tick_and_snapshot(tick.as_ref(), &snapshot, now_ms);
        let yes_price = clamp_probability(merged.chosen_price);
        let no_price = clamp_probability(1.0 - yes_price);

        match risk_gate_dual(
            repo,
            run_id,
            cfg,
            basket.trade_id,
            &limits,
            merged.stale_data_ms,
            &policy,
        )
        .await?
        {
            RiskDecision::Halt => {
                transition_dual(repo, &mut basket, TradeState::Halted, "risk-halt").await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        let now = Utc::now();
        let force_flatten_at = basket.cycle_ends_at
            - ChronoDuration::seconds(cfg.strategy.force_flatten_sec_before_close as i64);
        let basket_pnl = basket_unrealized_pnl(&basket, yes_price, no_price);
        let force_flatten = now >= force_flatten_at;
        let basket_flatten = strategy.should_flatten_basket(
            basket_pnl,
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        );

        if force_flatten || basket_flatten {
            if force_flatten {
                info!(
                    run_id,
                    trade_id = basket.trade_id,
                    "PAPER_DUAL_FORCE_FLATTEN"
                );
            }

            if basket_pnl <= cfg.strategy.basket_sl_usdc && basket.state == TradeState::TpPlaced {
                transition_dual(repo, &mut basket, TradeState::SlArmed, "paper-basket-sl").await?;
            }

            if basket.yes_leg.qty > 0.0 {
                let qty = basket.yes_leg.qty;
                record_paper_leg_fill(
                    repo,
                    basket.trade_id,
                    &mut basket.yes_leg,
                    "basket_exit",
                    "sell",
                    yes_price,
                    qty,
                )
                .await?;
            }
            if basket.no_leg.qty > 0.0 {
                let qty = basket.no_leg.qty;
                record_paper_leg_fill(
                    repo,
                    basket.trade_id,
                    &mut basket.no_leg,
                    "basket_exit",
                    "sell",
                    no_price,
                    qty,
                )
                .await?;
            }
        } else {
            if should_leg_take_profit(
                &strategy,
                &basket.yes_leg,
                yes_price,
                cfg.strategy.leg_tp_pct,
            ) {
                let qty = basket.yes_leg.qty;
                if qty > 0.0 {
                    record_paper_leg_fill(
                        repo,
                        basket.trade_id,
                        &mut basket.yes_leg,
                        "leg_tp",
                        "sell",
                        yes_price,
                        qty,
                    )
                    .await?;
                }
            }
            if should_leg_take_profit(&strategy, &basket.no_leg, no_price, cfg.strategy.leg_tp_pct)
            {
                let qty = basket.no_leg.qty;
                if qty > 0.0 {
                    record_paper_leg_fill(
                        repo,
                        basket.trade_id,
                        &mut basket.no_leg,
                        "leg_tp",
                        "sell",
                        no_price,
                        qty,
                    )
                    .await?;
                }
            }

            maybe_paper_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &mut basket.yes_leg,
                yes_price,
                level_notional,
                now,
            )
            .await?;
            maybe_paper_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &mut basket.no_leg,
                no_price,
                level_notional,
                now,
            )
            .await?;
        }

        persist_leg_snapshots(repo, &basket).await?;

        if basket.yes_leg.qty <= 0.0 && basket.no_leg.qty <= 0.0 {
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "paper-dual-exit-filled",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(repo, &mut basket, TradeState::Settled, "paper-dual-settled")
                    .await?;
            }
            repo.close_trade(basket.trade_id, 0.5, basket_pnl).await?;
            info!(
                run_id,
                trade_id = basket.trade_id,
                pnl = basket_pnl,
                iter,
                "PAPER_DUAL_SETTLED"
            );
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    Ok(())
}

async fn run_flow_only_loop(
    run_id: i64,
    repo: &PostgresRepository,
    cfg: &AppConfig,
) -> Result<()> {
    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    anyhow::ensure!(
        live_enabled,
        "flow_only mode requires LIVE_TRADING_ENABLED=true"
    );

    // CLOB client init (flows need it for order placement)
    let (creds, credential_source) = resolve_api_credentials_with_source(cfg)?;
    log_resolved_api_credentials(run_id, &creds, credential_source);
    let private_key = cfg
        .exchange
        .resolve_signer_private_key()
        .context("CLOB signer private key")?;
    let wallet = private_key
        .parse::<LocalWallet>()
        .context("parse signer private key")?
        .with_chain_id(cfg.exchange.chain_id);
    let exchange_address: Address = cfg
        .exchange
        .ctf_exchange_address
        .parse()
        .context("parse ctf_exchange_address")?;
    let gnosis_safe: Option<Address> = cfg
        .exchange
        .resolve_gnosis_safe_address()
        .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
        .transpose()?;
    let client = ClobHttpClient::from_credentials(
        cfg.exchange.clob_base_url.clone(),
        creds.clone(),
        wallet,
        exchange_address,
        cfg.exchange.chain_id,
        gnosis_safe,
    );
    run_clob_auth_preflight(run_id, repo, &client, &creds, credential_source).await;
    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;
    run_balance_preflight(run_id, repo, &client, cfg.risk.min_balance_usdc).await;
    let ws = ClobWsClient::new(cfg.exchange.clob_ws_url.clone());
    let mut auto_claim = match AutoClaimService::from_app_config(cfg) {
        Ok(service) => service,
        Err(err) => {
            warn!(run_id, error = %err, "AUTO_CLAIM_DISABLED");
            None
        }
    };

    info!(run_id, "FLOW_ONLY_LOOP_STARTED");

    // Infinite loop — only processes canvas/flow systems, no automatic trades
    loop {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        if let Some(service) = auto_claim.as_mut() {
            if let Err(e) = service.maybe_tick(repo).await {
                warn!(run_id, error = %e, "AUTO_CLAIM_TICK_FAILED");
            }
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }
}

async fn run_live_dual_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    anyhow::ensure!(
        live_enabled,
        "dual_side strategy requires LIVE_TRADING_ENABLED=true"
    );

    let (creds, credential_source) = resolve_api_credentials_with_source(cfg)?;
    log_resolved_api_credentials(run_id, &creds, credential_source);
    let private_key = cfg
        .exchange
        .resolve_signer_private_key()
        .context("CLOB signer private key")?;
    let wallet = private_key
        .parse::<LocalWallet>()
        .context("parse signer private key")?
        .with_chain_id(cfg.exchange.chain_id);
    let exchange_address: Address = cfg
        .exchange
        .ctf_exchange_address
        .parse()
        .context("parse ctf_exchange_address")?;
    let gnosis_safe: Option<Address> = cfg
        .exchange
        .resolve_gnosis_safe_address()
        .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
        .transpose()?;
    let client = ClobHttpClient::from_credentials(
        cfg.exchange.clob_base_url.clone(),
        creds.clone(),
        wallet,
        exchange_address,
        cfg.exchange.chain_id,
        gnosis_safe,
    );
    run_clob_auth_preflight(run_id, repo, &client, &creds, credential_source).await;
    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;
    run_balance_preflight(run_id, repo, &client, cfg.risk.min_balance_usdc).await;
    let ws = ClobWsClient::new(cfg.exchange.clob_ws_url.clone());
    let mut auto_claim = match AutoClaimService::from_app_config(cfg) {
        Ok(service) => service,
        Err(err) => {
            warn!(run_id, error = %err, "AUTO_CLAIM_DISABLED_DUE_TO_CONFIG_ERROR");
            None
        }
    };
    let override_slug = configured_market_override_slug(cfg)?;

    let mut waiting_event_emitted = false;
    let selected = loop {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        if let Some(service) = auto_claim.as_mut() {
            if let Err(e) = service.maybe_tick(repo).await {
                warn!(run_id, error = %e, "AUTO_CLAIM_TICK_FAILED");
            }
        }

        match discover_live_market_once(cfg, &gamma, true, override_slug.as_deref()).await {
            Ok(Some(selected)) => {
                info!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    selection = %cfg.bot.market_selection,
                    override_slug = ?override_slug,
                    market = %selected.slug,
                    selection_reason = selected.selection_reason.as_str(),
                    market_start_at = ?selected.starts_at,
                    market_end_at = ?selected.ends_at,
                    now_utc = %Utc::now(),
                    "MARKET_DISCOVERY_FOUND_NON_BLOCKING"
                );
                record_market_discovery_event(
                    repo,
                    run_id,
                    "allow",
                    MarketDiscoveryState::Ready,
                    &cfg.bot.market_scope,
                    Some(&selected.slug),
                    "market_discovery_ready",
                    "Market selected successfully.",
                )
                .await;
                break selected;
            }
            Ok(None) => {
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    let waiting_message = if let Some(forced_slug) = override_slug.as_ref() {
                        format!(
                            "Override market not active or missing YES/NO token IDs: {forced_slug}. Retrying while trade-flow continues."
                        )
                    } else {
                        "No active market with YES/NO token IDs. Retrying while trade-flow continues."
                            .to_string()
                    };
                    info!(
                        run_id,
                        scope = %cfg.bot.market_scope,
                        selection = %cfg.bot.market_selection,
                        override_slug = ?override_slug,
                        "MARKET_DISCOVERY_WAITING_NON_BLOCKING"
                    );
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_missing_token_ids",
                        &waiting_message,
                    )
                    .await;
                }
            }
            Err(err) => {
                warn!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    error = %err,
                    "MARKET_DISCOVERY_FETCH_FAILED_NON_BLOCKING"
                );
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_discovery_fetch_failed",
                        "Failed to fetch market list. Retrying while trade-flow continues.",
                    )
                    .await;
                }
            }
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    };

    let selected_reason = selected.selection_reason.as_str();
    let selected_start_at = selected.starts_at.as_ref().cloned();
    let selected_end_at = selected.ends_at.as_ref().cloned();
    let market_slug = selected.slug.clone();
    let yes_token_id = selected
        .yes_token_id
        .clone()
        .context("missing YES token id for selected market")?;
    let no_token_id = selected
        .no_token_id
        .clone()
        .context("missing NO token id for selected market")?;

    let cycle = MarketCycleId(market_slug.clone());
    let cycle_ends_at = selected_end_at.as_ref().cloned().unwrap_or_else(|| {
        cycle.start_time().unwrap_or_else(Utc::now) + ChronoDuration::seconds(300)
    });
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    info!(
        run_id,
        market_id,
        market = %market_slug,
        selection_reason = selected_reason,
        market_start_at = ?selected_start_at,
        market_end_at = ?selected_end_at,
        yes_token_id,
        no_token_id,
        "LIVE_DUAL_MARKET_DISCOVERED"
    );

    let mut user_ws_events: Vec<WsEvent> = Vec::new();
    let market_ws_ids = vec![yes_token_id.clone(), no_token_id.clone()];
    match ws.subscribe_once(WsChannel::Market, &market_ws_ids).await {
        Ok(messages) => {
            info!(run_id, market = %market_slug, ws_messages = messages.len(), "WS_MARKET_CONNECT_OK")
        }
        Err(e) => warn!(run_id, market = %market_slug, error = %e, "WS_MARKET_CONNECT_FAILED"),
    }
    match ws
        .subscribe_once(WsChannel::User, &[market_slug.clone()])
        .await
    {
        Ok(messages) => {
            info!(run_id, market = %market_slug, ws_messages = messages.len(), "WS_USER_CONNECT_OK");
            user_ws_events = messages;
        }
        Err(e) => warn!(run_id, market = %market_slug, error = %e, "WS_USER_CONNECT_FAILED"),
    }

    let mut basket = create_dual_runtime(
        repo,
        cfg,
        market_id,
        market_slug.clone(),
        yes_token_id,
        no_token_id,
        selected.maker_base_fee,
        cycle_ends_at,
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::WaitingEntry,
        "live-dual-cycle-start",
    )
    .await?;

    let strategy = SymmetricDualDcaStrategy;
    let policy = DefaultRiskPolicy;
    let limits = to_risk_limits(cfg);
    let mut order_meta: HashMap<String, OrderMeta> = HashMap::new();
    let mut seen_fill_ids: HashSet<String> = HashSet::new();
    let mut seen_buy_fill_order_ids: HashSet<String> = HashSet::new();
    let level_notional =
        cfg.strategy.per_leg_initial_notional_usdc / cfg.strategy.max_dca_levels_per_leg as f64;

    let initial_yes = match client.midpoint(&basket.yes_leg.token_id).await {
        Ok(snapshot) => clamp_probability(snapshot.price),
        Err(err) => {
            let fallback = 0.5;
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                error = %err,
                fallback_yes = fallback,
                "LIVE_DUAL_MIDPOINT_FAILED_USING_FALLBACK"
            );
            fallback
        }
    };
    let initial_no = clamp_probability(1.0 - initial_yes);

    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryPlaced,
        "live-dual-initial-entry",
    )
    .await?;
    let yes_size = calc_level_size(level_notional, initial_yes);
    let no_size = calc_level_size(level_notional, initial_no);
    let mut entry_submit_failed = false;
    if let Err(err) = place_live_leg_order(
        repo,
        basket.trade_id,
        &basket.market_slug,
        &mut basket.yes_leg,
        "buy",
        "entry",
        initial_yes,
        yes_size,
        basket.maker_base_fee,
        &client,
        &mut order_meta,
    )
    .await
    {
        entry_submit_failed = true;
        let classification = classify_clob_error(&err);
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            leg = "yes",
            reason_code = classification.reason_code,
            reason_message = classification.reason_message,
            error = %err,
            "LIVE_DUAL_ENTRY_ORDER_FAILED"
        );
    }
    if let Err(err) = place_live_leg_order(
        repo,
        basket.trade_id,
        &basket.market_slug,
        &mut basket.no_leg,
        "buy",
        "entry",
        initial_no,
        no_size,
        basket.maker_base_fee,
        &client,
        &mut order_meta,
    )
    .await
    {
        entry_submit_failed = true;
        let classification = classify_clob_error(&err);
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            leg = "no",
            reason_code = classification.reason_code,
            reason_message = classification.reason_message,
            error = %err,
            "LIVE_DUAL_ENTRY_ORDER_FAILED"
        );
    }
    if entry_submit_failed {
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            "LIVE_DUAL_ENTRY_SUBMIT_FAILED_CONTINUING"
        );
        // If no orders were placed at all (both legs rejected), settle the empty trade
        // and wait until the market window closes before returning.
        // Without this, the scope task immediately starts a new run_live_dual_loop()
        // creating hundreds of empty trades per market window.
        if order_meta.is_empty() {
            persist_leg_snapshots(repo, &basket).await?;
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "entry-failed-no-fill",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::Settled,
                    "entry-failed-settle",
                )
                .await?;
            }
            repo.close_trade(basket.trade_id, 0.5, 0.0).await?;
            let remaining = basket.cycle_ends_at.signed_duration_since(Utc::now());
            if remaining > ChronoDuration::zero() {
                let wait_secs = remaining.num_seconds().min(300) as u64;
                info!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    wait_secs,
                    "LIVE_DUAL_ENTRY_FAILED_WAITING_WINDOW_END"
                );
                sleep(Duration::from_secs(wait_secs)).await;
            }
            return Ok(());
        }
    } else {
        transition_dual(
            repo,
            &mut basket,
            TradeState::EntryFilled,
            "live-dual-entry-submitted",
        )
        .await?;
        transition_dual(
            repo,
            &mut basket,
            TradeState::TpPlaced,
            "live-dual-tp-active",
        )
        .await?;
    }
    let mut last_yes_price: Option<f64> = None;

    for iter in 0..120u32 {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        if let Some(service) = auto_claim.as_mut() {
            if let Err(e) = service.maybe_tick(repo).await {
                warn!(run_id, error = %e, "AUTO_CLAIM_TICK_FAILED");
            }
        }

        let yes_price = match client.midpoint(&basket.yes_leg.token_id).await {
            Ok(snapshot) => clamp_probability(snapshot.price),
            Err(err) => {
                let fallback_yes = clamp_probability(last_yes_price.unwrap_or(0.5));
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    error = %err,
                    fallback_yes,
                    "LIVE_DUAL_MIDPOINT_FAILED_USING_PREVIOUS_PRICE"
                );
                fallback_yes
            }
        };
        let no_price = clamp_probability(1.0 - yes_price);

        if let Err(err) = process_live_dual_fills(
            repo,
            basket.trade_id,
            &client,
            &mut basket,
            &order_meta,
            &mut seen_fill_ids,
            &mut seen_buy_fill_order_ids,
        )
        .await
        {
            let classification = classify_clob_error(&err);
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                error = %err,
                "LIVE_DUAL_FILL_RECONCILE_FAILED_CONTINUING"
            );
        }

        let user_applied = apply_user_stream_events(repo, basket.trade_id, &user_ws_events).await?;
        if user_applied > 0 {
            info!(
                run_id,
                trade_id = basket.trade_id,
                user_applied,
                "LIVE_DUAL_USER_EVENTS_APPLIED"
            );
        }

        let risk = risk_gate_dual(repo, run_id, cfg, basket.trade_id, &limits, 0, &policy).await?;
        match risk {
            RiskDecision::Halt => {
                let mut trade = basket_to_trade_runtime(&basket);
                halt_trade(repo, run_id, &mut trade, "risk-halt", Some(&client)).await?;
                basket.state = trade.state;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        let now = Utc::now();
        let force_flatten_at = basket.cycle_ends_at
            - ChronoDuration::seconds(cfg.strategy.force_flatten_sec_before_close as i64);
        let basket_pnl = basket_unrealized_pnl(&basket, yes_price, no_price);
        let force_flatten = now >= force_flatten_at;
        let (pressure_score, bid_ask_imbalance, sell_ratio, pressure_triggered) =
            compute_pressure_score(last_yes_price, yes_price);
        let drop_sell_reason =
            detect_drop_sell_reason(repo, basket.trade_id, &basket, yes_price, no_price).await?;
        let pressure_reason = if pressure_triggered {
            Some("pressure_exit")
        } else {
            None
        };
        let custom_reason = drop_sell_reason.or(pressure_reason);

        repo.upsert_pressure_snapshot(
            basket.trade_id,
            pressure_score,
            Some(bid_ask_imbalance),
            Some(sell_ratio),
            Some(yes_price),
            Some(no_price),
            custom_reason,
            custom_reason.is_some(),
        )
        .await?;
        last_yes_price = Some(yes_price);

        let basket_flatten = strategy.should_flatten_basket(
            basket_pnl,
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        );

        if force_flatten || basket_flatten || custom_reason.is_some() {
            let reason = if let Some(reason) = custom_reason {
                reason
            } else if force_flatten {
                "force_flatten"
            } else if basket_pnl <= cfg.strategy.basket_sl_usdc {
                "basket_sl"
            } else {
                "basket_tp"
            };
            let flatten_key = format!("trade:{}:flatten:{reason}", basket.trade_id);
            if repo.try_record_idempotency_key(&flatten_key).await? {
                if reason == "basket_sl" && basket.state == TradeState::TpPlaced {
                    transition_dual(repo, &mut basket, TradeState::SlArmed, "basket-sl").await?;
                }
                if basket.yes_leg.qty > 0.0 {
                    let qty = basket.yes_leg.qty;
                    if let Err(err) = place_live_leg_order(
                        repo,
                        basket.trade_id,
                        &basket.market_slug,
                        &mut basket.yes_leg,
                        "sell",
                        "basket_exit",
                        yes_price,
                        qty,
                        basket.maker_base_fee,
                        &client,
                        &mut order_meta,
                    )
                    .await
                    {
                        let classification = classify_clob_error(&err);
                        warn!(
                            run_id,
                            trade_id = basket.trade_id,
                            market = %basket.market_slug,
                            leg = "yes",
                            reason_code = classification.reason_code,
                            reason_message = classification.reason_message,
                            error = %err,
                            "LIVE_DUAL_EXIT_ORDER_FAILED_CONTINUING"
                        );
                    }
                }
                if basket.no_leg.qty > 0.0 {
                    let qty = basket.no_leg.qty;
                    if let Err(err) = place_live_leg_order(
                        repo,
                        basket.trade_id,
                        &basket.market_slug,
                        &mut basket.no_leg,
                        "sell",
                        "basket_exit",
                        no_price,
                        qty,
                        basket.maker_base_fee,
                        &client,
                        &mut order_meta,
                    )
                    .await
                    {
                        let classification = classify_clob_error(&err);
                        warn!(
                            run_id,
                            trade_id = basket.trade_id,
                            market = %basket.market_slug,
                            leg = "no",
                            reason_code = classification.reason_code,
                            reason_message = classification.reason_message,
                            error = %err,
                            "LIVE_DUAL_EXIT_ORDER_FAILED_CONTINUING"
                        );
                    }
                }
            }
        } else {
            if let Err(err) = maybe_live_leg_tp(
                repo,
                cfg,
                &strategy,
                &mut basket,
                &mut order_meta,
                &client,
                yes_price,
                no_price,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_TP_CHECK_FAILED_CONTINUING"
                );
            }

            if let Err(err) = maybe_live_leg_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.yes_leg,
                yes_price,
                level_notional,
                basket.maker_base_fee,
                &client,
                &mut order_meta,
                now,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    leg = "yes",
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_DCA_CHECK_FAILED_CONTINUING"
                );
            }
            if let Err(err) = maybe_live_leg_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.no_leg,
                no_price,
                level_notional,
                basket.maker_base_fee,
                &client,
                &mut order_meta,
                now,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    leg = "no",
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_DCA_CHECK_FAILED_CONTINUING"
                );
            }
        }

        if let Err(err) = process_live_dual_fills(
            repo,
            basket.trade_id,
            &client,
            &mut basket,
            &order_meta,
            &mut seen_fill_ids,
            &mut seen_buy_fill_order_ids,
        )
        .await
        {
            let classification = classify_clob_error(&err);
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                error = %err,
                "LIVE_DUAL_FILL_RECONCILE_FINAL_FAILED_CONTINUING"
            );
        }
        persist_leg_snapshots(repo, &basket).await?;

        if basket.yes_leg.qty <= 0.0 && basket.no_leg.qty <= 0.0 {
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "live-dual-exit-filled",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(repo, &mut basket, TradeState::Settled, "live-dual-settled")
                    .await?;
            }
            let pnl = basket_unrealized_pnl(&basket, yes_price, no_price);
            repo.close_trade(basket.trade_id, 0.5, pnl).await?;
            info!(
                run_id,
                trade_id = basket.trade_id,
                pnl,
                iter,
                "LIVE_DUAL_SETTLED"
            );
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    Ok(())
}

async fn create_dual_runtime(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    market_id: i64,
    market_slug: String,
    yes_token_id: String,
    no_token_id: String,
    maker_base_fee: u64,
    cycle_ends_at: DateTime<Utc>,
) -> Result<DualBasketRuntime> {
    let trade_id = repo
        .create_trade_stub_dual(
            market_id,
            cfg.strategy.total_notional_usdc,
            "dual_side_dca",
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        )
        .await?;
    repo.ensure_position_exit_rule_defaults(trade_id, DEFAULT_DROP_SELL_PCT)
        .await?;

    Ok(DualBasketRuntime {
        trade_id,
        market_slug,
        maker_base_fee,
        state: TradeState::Idle,
        yes_leg: DualLegRuntime {
            side: LegSide::Yes,
            token_id: yes_token_id,
            qty: 0.0,
            avg_entry: 0.0,
            levels_filled: 0,
            last_fill_price: None,
            last_dca_at: None,
        },
        no_leg: DualLegRuntime {
            side: LegSide::No,
            token_id: no_token_id,
            qty: 0.0,
            avg_entry: 0.0,
            levels_filled: 0,
            last_fill_price: None,
            last_dca_at: None,
        },
        cycle_ends_at,
    })
}

async fn transition_dual(
    repo: &PostgresRepository,
    basket: &mut DualBasketRuntime,
    to: TradeState,
    reason: &str,
) -> Result<()> {
    let from = basket.state;
    StateRepository::transition_trade_state(repo, basket.trade_id, from, to, reason).await?;
    basket.state = to;
    Ok(())
}

fn basket_to_trade_runtime(basket: &DualBasketRuntime) -> TradeRuntime {
    TradeRuntime {
        trade_id: basket.trade_id,
        market_slug: basket.market_slug.clone(),
        entry_price: 0.5,
        tp_price: 0.5,
        position_size: basket.yes_leg.qty + basket.no_leg.qty,
        state: basket.state,
    }
}

async fn record_paper_leg_fill(
    repo: &PostgresRepository,
    trade_id: i64,
    leg: &mut DualLegRuntime,
    intent: &str,
    side: &str,
    price: f64,
    size: f64,
) -> Result<()> {
    if size <= 0.0 {
        return Ok(());
    }
    let client_order_id = Uuid::new_v4().to_string();
    let order_id = repo
        .append_order_event_with_meta(
            trade_id,
            intent,
            side,
            price,
            size,
            "filled",
            Some(client_order_id.as_str()),
            Some(leg_side_label(leg.side)),
            Some(&leg.token_id),
        )
        .await?;
    repo.append_fill_event(order_id, price, size, 0.0).await?;
    apply_fill_to_leg(leg, side, price, size, true);
    Ok(())
}

async fn place_live_leg_order(
    repo: &PostgresRepository,
    trade_id: i64,
    market_slug: &str,
    leg: &mut DualLegRuntime,
    side: &str,
    intent: &str,
    price: f64,
    size: f64,
    fee_rate_bps: u64,
    client: &dyn OrderExecutor,
    order_meta: &mut HashMap<String, OrderMeta>,
) -> Result<()> {
    if size <= 0.0 {
        return Ok(());
    }
    let client_order_id = Uuid::new_v4().to_string();
    let req = PlaceOrderRequest {
        market: market_slug.to_string(),
        token_id: Some(leg.token_id.clone()),
        side: side.to_string(),
        price,
        size,
        intent: intent.to_string(),
        client_order_id: client_order_id.clone(),
        leg_side: Some(leg_side_label(leg.side).to_string()),
        fee_rate_bps,
    };

    let ack = client.place(&req).await?;
    let exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let raw = json!({
        "client_order_id": ack.client_order_id,
        "exchange_order_id": ack.exchange_order_id,
        "status": ack.status,
        "reject_reason": ack.reject_reason,
        "raw_status": ack.raw_status,
        "exchange_ts": ack.exchange_ts,
        "intent": intent,
        "leg_side": leg_side_label(leg.side),
        "token_id": &leg.token_id
    });
    repo.upsert_order_by_exchange_id_with_meta(
        trade_id,
        &exchange_order_id,
        Some(&client_order_id),
        intent,
        side,
        price,
        size,
        &ack.status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
        Some(leg_side_label(leg.side)),
        Some(&leg.token_id),
    )
    .await?;
    order_meta.insert(
        exchange_order_id,
        OrderMeta {
            leg_side: leg.side,
            side: side.to_string(),
            intent: intent.to_string(),
        },
    );
    Ok(())
}

async fn process_live_dual_fills(
    repo: &PostgresRepository,
    _trade_id: i64,
    client: &dyn OrderExecutor,
    basket: &mut DualBasketRuntime,
    order_meta: &HashMap<String, OrderMeta>,
    seen_fill_ids: &mut HashSet<String>,
    seen_buy_fill_order_ids: &mut HashSet<String>,
) -> Result<()> {
    let fills = client.list_fills(None).await?;
    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            continue;
        }
        if !seen_fill_ids.insert(fill.fill_id.clone()) {
            continue;
        }
        let Some(meta) = order_meta.get(&fill.order_id) else {
            continue;
        };
        let Some(order_id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        else {
            continue;
        };
        let raw = json!({
            "fill_id": fill.fill_id,
            "order_id": fill.order_id,
            "price": fill.price,
            "size": fill.size,
            "fee": fill.fee,
            "timestamp": fill.ts,
            "leg_side": leg_side_label(meta.leg_side),
            "intent": meta.intent
        });
        repo.upsert_fill_by_exchange_fill_id(
            order_id,
            &fill.fill_id,
            fill.price,
            fill.size,
            fill.fee.unwrap_or_default(),
            fill.ts,
            &raw,
        )
        .await?;

        let leg = dual_leg_mut(basket, meta.leg_side);
        let _ = seen_buy_fill_order_ids.insert(fill.order_id.clone());
        apply_fill_to_leg(leg, &meta.side, fill.price, fill.size, false);
    }
    Ok(())
}

fn apply_fill_to_leg(
    leg: &mut DualLegRuntime,
    side: &str,
    price: f64,
    size: f64,
    increment_level: bool,
) {
    if side == "buy" {
        let prev_qty = leg.qty;
        let new_qty = prev_qty + size;
        if new_qty > 0.0 {
            leg.avg_entry = ((leg.avg_entry * prev_qty) + (price * size)) / new_qty;
        }
        leg.qty = new_qty;
        if increment_level {
            leg.levels_filled = leg.levels_filled.saturating_add(1);
        }
    } else {
        leg.qty = (leg.qty - size).max(0.0);
        if leg.qty <= 0.0 {
            leg.avg_entry = 0.0;
        }
    }
    leg.last_fill_price = Some(price);
}

async fn persist_leg_snapshots(
    repo: &PostgresRepository,
    basket: &DualBasketRuntime,
) -> Result<()> {
    repo.upsert_leg_position(
        basket.trade_id,
        basket.yes_leg.side,
        &basket.yes_leg.token_id,
        basket.yes_leg.qty,
        basket.yes_leg.avg_entry,
        basket.yes_leg.levels_filled as i32,
        basket.yes_leg.last_fill_price,
    )
    .await?;
    repo.upsert_leg_position(
        basket.trade_id,
        basket.no_leg.side,
        &basket.no_leg.token_id,
        basket.no_leg.qty,
        basket.no_leg.avg_entry,
        basket.no_leg.levels_filled as i32,
        basket.no_leg.last_fill_price,
    )
    .await?;
    Ok(())
}

fn basket_unrealized_pnl(basket: &DualBasketRuntime, yes_price: f64, no_price: f64) -> f64 {
    let yes = (yes_price - basket.yes_leg.avg_entry) * basket.yes_leg.qty;
    let no = (no_price - basket.no_leg.avg_entry) * basket.no_leg.qty;
    yes + no
}

fn dual_leg_mut(basket: &mut DualBasketRuntime, leg_side: LegSide) -> &mut DualLegRuntime {
    match leg_side {
        LegSide::Yes => &mut basket.yes_leg,
        LegSide::No => &mut basket.no_leg,
    }
}

fn should_leg_take_profit(
    strategy: &impl DualSideStrategy,
    leg: &DualLegRuntime,
    current_price: f64,
    leg_tp_pct: f64,
) -> bool {
    if leg.qty <= 0.0 || leg.avg_entry <= 0.0 {
        return false;
    }
    current_price >= strategy.leg_take_profit_price(leg.avg_entry, leg_tp_pct)
}

async fn maybe_paper_dca(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    trade_id: i64,
    leg: &mut DualLegRuntime,
    current_price: f64,
    level_notional: f64,
    now: DateTime<Utc>,
) -> Result<()> {
    if !can_dca_now(leg, cfg, now) {
        return Ok(());
    }
    if strategy.should_dca_leg(
        current_price,
        leg.last_fill_price,
        cfg.strategy.dca_step_pct,
        leg.levels_filled,
        cfg.strategy.max_dca_levels_per_leg,
    ) {
        let size = calc_level_size(level_notional, current_price);
        record_paper_leg_fill(repo, trade_id, leg, "dca", "buy", current_price, size).await?;
        leg.last_dca_at = Some(now);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn maybe_live_leg_dca(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    trade_id: i64,
    market_slug: &str,
    leg: &mut DualLegRuntime,
    current_price: f64,
    level_notional: f64,
    fee_rate_bps: u64,
    client: &dyn OrderExecutor,
    order_meta: &mut HashMap<String, OrderMeta>,
    now: DateTime<Utc>,
) -> Result<()> {
    if !can_dca_now(leg, cfg, now) {
        return Ok(());
    }
    if strategy.should_dca_leg(
        current_price,
        leg.last_fill_price,
        cfg.strategy.dca_step_pct,
        leg.levels_filled,
        cfg.strategy.max_dca_levels_per_leg,
    ) {
        let size = calc_level_size(level_notional, current_price);
        place_live_leg_order(
            repo,
            trade_id,
            market_slug,
            leg,
            "buy",
            "dca",
            current_price,
            size,
            fee_rate_bps,
            client,
            order_meta,
        )
        .await?;
        leg.last_dca_at = Some(now);
        leg.levels_filled = leg.levels_filled.saturating_add(1);
        leg.last_fill_price = Some(current_price);
    }
    Ok(())
}

async fn maybe_live_leg_tp(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    basket: &mut DualBasketRuntime,
    order_meta: &mut HashMap<String, OrderMeta>,
    client: &dyn OrderExecutor,
    yes_price: f64,
    no_price: f64,
) -> Result<()> {
    if should_leg_take_profit(
        strategy,
        &basket.yes_leg,
        yes_price,
        cfg.strategy.leg_tp_pct,
    ) {
        let key = format!("trade:{}:leg_tp:yes:{:.4}", basket.trade_id, yes_price);
        if repo.try_record_idempotency_key(&key).await? && basket.yes_leg.qty > 0.0 {
            let qty = basket.yes_leg.qty;
            place_live_leg_order(
                repo,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.yes_leg,
                "sell",
                "leg_tp",
                yes_price,
                qty,
                basket.maker_base_fee,
                client,
                order_meta,
            )
            .await?;
        }
    }
    if should_leg_take_profit(strategy, &basket.no_leg, no_price, cfg.strategy.leg_tp_pct) {
        let key = format!("trade:{}:leg_tp:no:{:.4}", basket.trade_id, no_price);
        if repo.try_record_idempotency_key(&key).await? && basket.no_leg.qty > 0.0 {
            let qty = basket.no_leg.qty;
            place_live_leg_order(
                repo,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.no_leg,
                "sell",
                "leg_tp",
                no_price,
                qty,
                basket.maker_base_fee,
                client,
                order_meta,
            )
            .await?;
        }
    }
    Ok(())
}

fn can_dca_now(leg: &DualLegRuntime, cfg: &AppConfig, now: DateTime<Utc>) -> bool {
    let Some(last_dca_at) = leg.last_dca_at else {
        return true;
    };
    now.signed_duration_since(last_dca_at).num_seconds() >= cfg.strategy.dca_interval_sec as i64
}

pub(crate) fn calc_level_size(level_notional: f64, price: f64) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    ((level_notional / price) * 100.0).round() / 100.0
}

pub(crate) fn clamp_probability(value: f64) -> f64 {
    value.clamp(0.01, 0.99)
}

fn leg_side_label(leg_side: LegSide) -> &'static str {
    match leg_side {
        LegSide::Yes => "yes",
        LegSide::No => "no",
    }
}

fn compute_pressure_score(
    previous_yes_price: Option<f64>,
    yes_price: f64,
) -> (f64, f64, f64, bool) {
    let Some(prev_yes) = previous_yes_price else {
        return (0.0, 0.0, 0.0, false);
    };

    if prev_yes <= 0.0 {
        return (0.0, 0.0, 0.0, false);
    }

    let drop_pct = ((prev_yes - yes_price) / prev_yes * 100.0).max(0.0);
    let sell_ratio = if yes_price < prev_yes { 1.0 } else { 0.0 };
    let score = drop_pct + (sell_ratio * 0.5);
    let triggered = drop_pct >= PRESSURE_DROP_PCT_THRESHOLD;
    (score, drop_pct, sell_ratio, triggered)
}

async fn detect_drop_sell_reason(
    repo: &PostgresRepository,
    trade_id: i64,
    basket: &DualBasketRuntime,
    yes_price: f64,
    no_price: f64,
) -> Result<Option<&'static str>> {
    let mut yes_drop_pct = DEFAULT_DROP_SELL_PCT;
    let mut no_drop_pct = DEFAULT_DROP_SELL_PCT;
    let mut yes_enabled = true;
    let mut no_enabled = true;

    for rule in repo.load_position_exit_rules(trade_id).await? {
        match rule.leg_side {
            LegSide::Yes => {
                yes_drop_pct = rule.drop_sell_pct;
                yes_enabled = rule.enabled;
            }
            LegSide::No => {
                no_drop_pct = rule.drop_sell_pct;
                no_enabled = rule.enabled;
            }
        }
    }

    if yes_enabled
        && basket.yes_leg.qty > 0.0
        && price_dropped_below_threshold(
            basket
                .yes_leg
                .last_fill_price
                .unwrap_or(basket.yes_leg.avg_entry),
            yes_price,
            yes_drop_pct,
        )
    {
        return Ok(Some("drop_sell_yes"));
    }

    if no_enabled
        && basket.no_leg.qty > 0.0
        && price_dropped_below_threshold(
            basket
                .no_leg
                .last_fill_price
                .unwrap_or(basket.no_leg.avg_entry),
            no_price,
            no_drop_pct,
        )
    {
        return Ok(Some("drop_sell_no"));
    }

    Ok(None)
}

fn price_dropped_below_threshold(
    reference_price: f64,
    current_price: f64,
    drop_sell_pct: f64,
) -> bool {
    if reference_price <= 0.0 || drop_sell_pct <= 0.0 {
        return false;
    }
    current_price <= reference_price * (1.0 - (drop_sell_pct / 100.0))
}

async fn process_trade_flows(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
) -> Result<()> {
    let definitions = repo
        .list_published_trade_flow_definitions(FLOW_DEFINITION_PROCESS_LIMIT)
        .await?;
    if definitions.is_empty() {
        return Ok(());
    }

    for definition in definitions {
        if let Err(err) = sync_trade_flow_definition_run(repo, run_id, &definition).await {
            warn!(
                run_id,
                definition_id = definition.id,
                error = %err,
                "TRADE_FLOW_RUN_SYNC_ERROR"
            );
        }
    }
    if let Err(err) = enqueue_trade_flow_ws_open_position_price_steps(repo, run_id, ws).await {
        warn!(run_id, error = %err, "TRADE_FLOW_WS_TRIGGER_ENQUEUE_FAILED");
    }

    let limits = to_risk_limits(cfg);
    let policy = DefaultRiskPolicy;
    let ready_steps = repo
        .list_ready_trade_flow_steps(FLOW_STEP_PROCESS_LIMIT)
        .await?;
    for step in ready_steps {
        if let Err(err) =
            process_trade_flow_step(repo, run_id, cfg, &limits, &policy, client, ws, &step).await
        {
            warn!(
                run_id,
                step_id = step.id,
                run_id = step.run_id,
                error = %err,
                "TRADE_FLOW_STEP_PROCESS_ERROR"
            );
            let error_json = json!({
                "error": err.to_string(),
                "node_key": step.node_key,
                "node_type": step.node_type
            });
            let _ = repo
                .mark_trade_flow_step_failed(step.id, Some(&error_json), &err.to_string())
                .await;
        }
    }

    Ok(())
}

fn open_position_ws_price_node_specs(
    node: &TradeFlowNode,
    context: &Value,
) -> Vec<WsOpenPositionPriceNodeSpec> {
    if node.node_type != "trigger.open_positions" && node.node_type != "trigger.market_price" {
        return Vec::new();
    }
    // Multi-outcome path
    if let Some(conditions) = node.config.get("outcomeConditions").and_then(|v| v.as_array()) {
        let mut specs = Vec::new();
        for cond in conditions {
            let token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let trigger_condition = cond
                .get("triggerCondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let trigger_price = cond
                .get("triggerPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
            if token_id.is_empty()
                || !matches!(trigger_condition.as_str(), "cross_above" | "cross_below")
            {
                continue;
            }
            let tp = match trigger_price {
                Some(v) if v > 0.0 && v <= 1.0 => v,
                _ => continue,
            };
            specs.push(WsOpenPositionPriceNodeSpec {
                node_key: node.key.clone(),
                node_type: node.node_type.clone(),
                token_id,
                trigger_condition,
                trigger_price: tp,
            });
        }
        return specs;
    }
    // Legacy single-token path
    let trigger_condition = match node_config_string(node, "triggerCondition") {
        Some(tc) => tc,
        None => return Vec::new(),
    };
    if !matches!(trigger_condition.as_str(), "cross_above" | "cross_below") {
        return Vec::new();
    }
    let trigger_price = match node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0))
    {
        Some(v) if v > 0.0 && v <= 1.0 => v,
        _ => return Vec::new(),
    };
    let token_id = match node_config_string(node, "tokenId")
        .or_else(|| flow_context_string(context, "tokenId"))
    {
        Some(id) if !id.is_empty() => id,
        _ => return Vec::new(),
    };
    vec![WsOpenPositionPriceNodeSpec {
        node_key: node.key.clone(),
        node_type: node.node_type.clone(),
        token_id,
        trigger_condition,
        trigger_price,
    }]
}

async fn enqueue_trade_flow_ws_open_position_price_steps(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
) -> Result<()> {
    let definitions = repo
        .list_published_trade_flow_definitions(FLOW_DEFINITION_PROCESS_LIMIT)
        .await?;
    if definitions.is_empty() {
        return Ok(());
    }

    let mut run_specs: Vec<WsOpenPositionPriceRunSpec> = Vec::new();
    let mut token_targets: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    for definition in definitions {
        let Some(run) = repo.get_active_trade_flow_run(definition.id).await? else {
            continue;
        };
        let Some(version) = repo.get_trade_flow_version(run.version_id).await? else {
            continue;
        };
        let graph = parse_trade_flow_graph(&version)?;
        let context = normalize_trade_flow_context(run.context_json.clone(), &graph.context);
        let mut nodes = Vec::new();
        for node in &graph.nodes {
            let specs = open_position_ws_price_node_specs(node, &context);
            nodes.extend(specs);
        }
        if nodes.is_empty() {
            continue;
        }

        let run_index = run_specs.len();
        for (node_index, node) in nodes.iter().enumerate() {
            token_targets
                .entry(node.token_id.clone())
                .or_default()
                .push((run_index, node_index));
        }

        run_specs.push(WsOpenPositionPriceRunSpec {
            run_id: run.id,
            definition_id: run.definition_id,
            version_id: run.version_id,
            context,
            nodes,
            context_dirty: false,
        });
    }

    if run_specs.is_empty() || token_targets.is_empty() {
        return Ok(());
    }

    for (token_id, targets) in token_targets {
        let events = match ws
            .subscribe_once(WsChannel::Market, &[token_id.clone()])
            .await
        {
            Ok(events) => events,
            Err(err) => {
                warn!(
                    run_id,
                    token_id = %token_id,
                    error = %err,
                    "TRADE_FLOW_WS_OPEN_POSITIONS_SUBSCRIBE_FAILED"
                );
                continue;
            }
        };

        let Some((price_raw, event_ts)) = extract_price_from_market_events(&events, &token_id)
        else {
            continue;
        };
        let current_price = clamp_probability(price_raw);

        for (run_index, node_index) in targets {
            let Some(run_spec) = run_specs.get_mut(run_index) else {
                continue;
            };
            let Some(node_spec) = run_spec.nodes.get(node_index).cloned() else {
                continue;
            };

            // Use per-token state key for multi-outcome nodes
            let prev_key = format!("previous_price_{}", node_spec.token_id);
            let previous_price =
                flow_node_state(&run_spec.context, &node_spec.node_key, &prev_key)
                    .and_then(value_as_f64)
                    .or_else(|| {
                        // Fallback to legacy key for backward compat
                        flow_node_state(&run_spec.context, &node_spec.node_key, "previous_price")
                            .and_then(value_as_f64)
                    });
            let crossed = match node_spec.trigger_condition.as_str() {
                "cross_above" => previous_price
                    .map(|prev| {
                        prev < node_spec.trigger_price && current_price >= node_spec.trigger_price
                    })
                    .unwrap_or(current_price >= node_spec.trigger_price),
                "cross_below" => previous_price
                    .map(|prev| {
                        prev > node_spec.trigger_price && current_price <= node_spec.trigger_price
                    })
                    .unwrap_or(current_price <= node_spec.trigger_price),
                _ => false,
            };

            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                "last_price",
                json!(current_price),
            );
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &prev_key,
                json!(current_price),
            );
            run_spec.context_dirty = true;

            if !crossed {
                continue;
            }

            let input_json = json!({
                "triggerSource": "ws_market_price",
                "tokenId": token_id,
                "wsPrice": current_price,
                "wsPrices": { token_id.clone(): current_price },
                "wsEventTs": event_ts
            });
            let dedupe_ts = event_ts.unwrap_or_else(|| Utc::now().timestamp_millis());
            let idempotency_key = format!(
                "ws-open-price:{}:{}:{}:{:.6}:{}",
                run_spec.run_id,
                node_spec.node_key,
                node_spec.trigger_condition,
                current_price,
                dedupe_ts
            );

            let enqueued = repo
                .enqueue_trade_flow_step(
                    run_spec.run_id,
                    &node_spec.node_key,
                    &node_spec.node_type,
                    1,
                    Some(&input_json),
                    Utc::now(),
                    None,
                    Some(&idempotency_key),
                )
                .await?;
            if enqueued.is_some() {
                repo.append_trade_flow_event(
                    Some(run_spec.run_id),
                    run_spec.definition_id,
                    Some(run_spec.version_id),
                    "trigger_ws_price_enqueued",
                    &json!({
                        "node_key": node_spec.node_key,
                        "token_id": token_id,
                        "price": current_price,
                        "event_ts": event_ts
                    }),
                )
                .await?;
            }
        }
    }

    for run_spec in run_specs {
        if run_spec.context_dirty {
            repo.update_trade_flow_run_context(run_spec.run_id, &run_spec.context)
                .await?;
        }
    }

    Ok(())
}

async fn sync_trade_flow_definition_run(
    repo: &PostgresRepository,
    run_id: i64,
    definition: &TradeFlowDefinitionRuntime,
) -> Result<()> {
    let Some(published_version_id) = definition.published_version_id else {
        return Ok(());
    };

    let version = repo
        .get_trade_flow_version(published_version_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("published trade flow version not found"))?;
    let graph = parse_trade_flow_graph(&version)?;

    let mut needs_new_run = false;
    if let Some(active_run) = repo.get_active_trade_flow_run(definition.id).await? {
        if active_run.version_id != version.id {
            repo.set_trade_flow_run_status(active_run.id, "canceled", Some("version_changed"))
                .await?;
            repo.append_trade_flow_event(
                Some(active_run.id),
                definition.id,
                Some(active_run.version_id),
                "run_canceled_version_changed",
                &json!({
                    "previous_version_id": active_run.version_id,
                    "next_version_id": version.id
                }),
            )
            .await?;
            needs_new_run = true;
        }
    } else {
        needs_new_run = true;
    }

    if !needs_new_run {
        return Ok(());
    }

    let context_json = build_initial_trade_flow_context(&graph.context);
    let run = repo
        .create_trade_flow_run(
            definition.id,
            version.id,
            Some("runner_auto_start"),
            &context_json,
        )
        .await?;
    repo.append_trade_flow_event(
        Some(run.id),
        definition.id,
        Some(version.id),
        "run_started",
        &json!({
            "run_id": run.id,
            "version_id": version.id,
            "definition_name": definition.name
        }),
    )
    .await?;
    seed_trade_flow_trigger_steps(repo, run_id, &run, &graph).await?;
    Ok(())
}

fn build_initial_trade_flow_context(graph_context: &Value) -> Value {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    let flow_context = if graph_context.is_object() {
        graph_context.clone()
    } else {
        json!({})
    };
    if let Some(map) = context.as_object_mut() {
        map.insert("flowContext".to_string(), flow_context);
    }
    context
}

fn normalize_trade_flow_context(context_json: Value, graph_context: &Value) -> Value {
    let mut normalized = if context_json.is_object() {
        context_json
    } else {
        json!({})
    };

    let graph_ctx = if graph_context.is_object() {
        graph_context.clone()
    } else {
        json!({})
    };

    {
        let root = ensure_object_mut(&mut normalized);
        if !root
            .get("flowContext")
            .map(Value::is_object)
            .unwrap_or(false)
        {
            root.insert("flowContext".to_string(), graph_ctx);
        }
        for key in ["vars", "state", "refs", "nodeState"] {
            if !root.get(key).map(Value::is_object).unwrap_or(false) {
                root.insert(key.to_string(), json!({}));
            }
        }
    }

    normalized
}

fn ensure_object_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value should be object after normalization")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeFlowSeedMode {
    Trigger,
    DualDcaRoot,
}

impl TradeFlowSeedMode {
    fn as_str(self) -> &'static str {
        match self {
            TradeFlowSeedMode::Trigger => "trigger",
            TradeFlowSeedMode::DualDcaRoot => "dual_dca_root",
        }
    }
}

fn collect_trade_flow_root_nodes<'a>(graph: &'a TradeFlowGraphRuntime) -> Vec<&'a TradeFlowNode> {
    let incoming_targets: HashSet<&str> = graph
        .edges
        .iter()
        .map(|edge| edge.target.as_str())
        .collect();

    graph
        .nodes
        .iter()
        .filter(|node| !incoming_targets.contains(node.key.as_str()))
        .collect()
}

fn select_trade_flow_initial_seed_nodes<'a>(
    graph: &'a TradeFlowGraphRuntime,
) -> std::result::Result<(TradeFlowSeedMode, Vec<&'a TradeFlowNode>), &'static str> {
    let trigger_nodes: Vec<&TradeFlowNode> = graph
        .nodes
        .iter()
        .filter(|node| node.node_type.starts_with("trigger."))
        .collect();
    if !trigger_nodes.is_empty() {
        return Ok((TradeFlowSeedMode::Trigger, trigger_nodes));
    }

    let has_dual_dca = graph
        .nodes
        .iter()
        .any(|node| node.node_type == "action.dual_dca");
    if !has_dual_dca {
        return Err("flow_missing_trigger");
    }

    let root_nodes = collect_trade_flow_root_nodes(graph);
    if !root_nodes.is_empty()
        && root_nodes
            .iter()
            .all(|node| node.node_type == "action.dual_dca")
    {
        return Ok((TradeFlowSeedMode::DualDcaRoot, root_nodes));
    }

    Err("flow_invalid_roots_without_trigger")
}

async fn seed_trade_flow_trigger_steps(
    repo: &PostgresRepository,
    run_id: i64,
    run: &TradeFlowRun,
    graph: &TradeFlowGraphRuntime,
) -> Result<()> {
    let now = Utc::now();
    let trigger_count = graph
        .nodes
        .iter()
        .filter(|node| node.node_type.starts_with("trigger."))
        .count();
    let (seed_mode, nodes_to_seed) = match select_trade_flow_initial_seed_nodes(graph) {
        Ok(selection) => selection,
        Err(reason) => {
            let has_dual_dca = graph
                .nodes
                .iter()
                .any(|node| node.node_type == "action.dual_dca");
            let root_nodes_payload: Vec<Value> = collect_trade_flow_root_nodes(graph)
                .iter()
                .map(|node| {
                    json!({
                        "key": node.key.as_str(),
                        "type": node.node_type.as_str()
                    })
                })
                .collect();
            repo.set_trade_flow_run_status(run.id, "failed", Some(reason))
                .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "run_failed",
                &json!({
                    "reason": reason,
                    "hasDualDca": has_dual_dca,
                    "rootNodes": root_nodes_payload
                }),
            )
            .await?;
            return Ok(());
        }
    };

    let start_count = nodes_to_seed.len();
    for node in nodes_to_seed {
        let idempotency_key = format!("seed:{}:{}", run.id, node.key);
        let _ = repo
            .enqueue_trade_flow_step(
                run.id,
                &node.key,
                &node.node_type,
                1,
                None,
                now,
                None,
                Some(&idempotency_key),
            )
            .await?;
    }

    info!(
        run_id,
        flow_run_id = run.id,
        trigger_count,
        start_mode = seed_mode.as_str(),
        start_count,
        "TRADE_FLOW_RUN_INITIAL_STEPS_SEEDED"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_trade_flow_step(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    step: &TradeFlowRunStep,
) -> Result<()> {
    repo.mark_trade_flow_step_running(step.id).await?;

    let run = match repo.get_trade_flow_run(step.run_id).await? {
        Some(run) => run,
        None => {
            repo.mark_trade_flow_step_skipped(step.id, None).await?;
            return Ok(());
        }
    };
    if run.status != "running" {
        repo.mark_trade_flow_step_skipped(step.id, None).await?;
        return Ok(());
    }

    let definition = repo
        .get_trade_flow_definition(run.definition_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("flow definition not found for run"))?;
    if definition.status != "published" {
        repo.set_trade_flow_run_status(run.id, "canceled", Some("definition_not_published"))
            .await?;
        repo.mark_trade_flow_step_skipped(step.id, None).await?;
        return Ok(());
    }

    let version = repo
        .get_trade_flow_version(run.version_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("flow version not found for run"))?;
    let graph = parse_trade_flow_graph(&version)?;
    let node = graph
        .nodes
        .iter()
        .find(|node| node.key == step.node_key)
        .ok_or_else(|| anyhow::anyhow!("flow node not found for step"))?;

    let mut context = normalize_trade_flow_context(run.context_json.clone(), &graph.context);
    let result = execute_trade_flow_node(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client,
        ws,
        &run,
        step,
        node,
        &graph,
        &mut context,
    )
    .await;

    match result {
        Ok(execution) => {
            repo.update_trade_flow_run_context(run.id, &context).await?;
            repo.mark_trade_flow_step_completed(step.id, Some(&execution.output))
                .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "step_completed",
                &json!({
                    "step_id": step.id,
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "routes": execution.routes.iter().map(|r| r.edge_type.clone()).collect::<Vec<_>>()
                }),
            )
            .await?;

            for route in &execution.routes {
                enqueue_trade_flow_edges(
                    repo,
                    &run,
                    &graph,
                    &node.key,
                    &route.edge_type,
                    route.available_at,
                    step.id,
                    &execution.output,
                )
                .await?;
            }

            if let Some(repeat_at) = execution.repeat_at {
                let _ = repo
                    .enqueue_trade_flow_step(
                        run.id,
                        &node.key,
                        &node.node_type,
                        step.attempt,
                        None,
                        repeat_at,
                        Some(step.id),
                        execution.repeat_idempotency_key.as_deref(),
                    )
                    .await?;
            }
        }
        Err(err) => {
            let output_json = json!({
                "error": err.to_string(),
                "node_key": node.key,
                "node_type": node.node_type
            });
            repo.mark_trade_flow_step_failed(step.id, Some(&output_json), &err.to_string())
                .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "step_failed",
                &json!({
                    "step_id": step.id,
                    "node_key": node.key,
                    "error": err.to_string()
                }),
            )
            .await?;
            enqueue_trade_flow_edges(
                repo,
                &run,
                &graph,
                &node.key,
                "on_error",
                Utc::now(),
                step.id,
                &output_json,
            )
            .await?;
        }
    }

    Ok(())
}

fn parse_trade_flow_graph(version: &TradeFlowVersionRuntime) -> Result<TradeFlowGraphRuntime> {
    let root = version
        .graph_json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("trade flow graph_json must be an object"))?;

    let context = root.get("context").cloned().unwrap_or_else(|| json!({}));

    let mut nodes = Vec::new();
    if let Some(raw_nodes) = root.get("nodes").and_then(Value::as_array) {
        for raw in raw_nodes {
            let Some(raw_obj) = raw.as_object() else {
                continue;
            };
            let key = raw_obj
                .get("key")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let node_type = raw_obj
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if key.is_empty() || node_type.is_empty() {
                continue;
            }
            nodes.push(TradeFlowNode {
                key: key.to_string(),
                node_type: node_type.to_string(),
                config: raw_obj.get("config").cloned().unwrap_or_else(|| json!({})),
            });
        }
    }

    let mut edges = Vec::new();
    if let Some(raw_edges) = root.get("edges").and_then(Value::as_array) {
        for raw in raw_edges {
            let Some(raw_obj) = raw.as_object() else {
                continue;
            };
            let source = raw_obj
                .get("source")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let target = raw_obj
                .get("target")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if source.is_empty() || target.is_empty() {
                continue;
            }
            let edge_type = raw_obj
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("default");
            edges.push(TradeFlowEdge {
                source: source.to_string(),
                target: target.to_string(),
                edge_type: edge_type.to_string(),
            });
        }
    }

    Ok(TradeFlowGraphRuntime {
        context,
        nodes,
        edges,
    })
}

fn flow_node<'a>(graph: &'a TradeFlowGraphRuntime, key: &str) -> Option<&'a TradeFlowNode> {
    graph.nodes.iter().find(|node| node.key == key)
}

fn resolve_trade_flow_targets(
    graph: &TradeFlowGraphRuntime,
    source_key: &str,
    edge_type: &str,
) -> Vec<String> {
    let mut targets = graph
        .edges
        .iter()
        .filter(|edge| edge.source == source_key && edge.edge_type == edge_type)
        .map(|edge| edge.target.clone())
        .collect::<Vec<_>>();

    if targets.is_empty() && edge_type != "default" {
        targets = graph
            .edges
            .iter()
            .filter(|edge| edge.source == source_key && edge.edge_type == "default")
            .map(|edge| edge.target.clone())
            .collect::<Vec<_>>();
    }

    targets
}

async fn enqueue_trade_flow_edges(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    graph: &TradeFlowGraphRuntime,
    source_key: &str,
    edge_type: &str,
    available_at: DateTime<Utc>,
    parent_step_id: i64,
    input_json: &Value,
) -> Result<()> {
    let targets = resolve_trade_flow_targets(graph, source_key, edge_type);
    for target_key in targets {
        let Some(target_node) = flow_node(graph, &target_key) else {
            continue;
        };

        let attempt = if target_key == source_key {
            input_json
                .get("next_attempt")
                .and_then(value_as_i64)
                .unwrap_or(1)
                .max(1) as i32
        } else {
            1
        };

        let _ = repo
            .enqueue_trade_flow_step(
                run.id,
                &target_node.key,
                &target_node.node_type,
                attempt,
                Some(input_json),
                available_at,
                Some(parent_step_id),
                None,
            )
            .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute_trade_flow_node(
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
    _graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    match node.node_type.as_str() {
        "trigger.market_price" => {
            execute_trigger_market_price(repo, client, ws, run, step, node, context).await
        }
        "trigger.sell_progress" => execute_trigger_sell_progress(repo, run, node, context).await,
        "trigger.open_positions" => {
            execute_trigger_open_positions(repo, client, ws, run, step, node, context).await
        }
        "trigger.time_window" => execute_trigger_time_window(node, context),
        "logic.if" => execute_logic_if(node, context),
        "logic.switch" => execute_logic_switch(node, context),
        "logic.delay" => execute_logic_delay(node),
        "logic.retry" => execute_logic_retry(node, step, context),
        "action.resolve_market" => execute_action_resolve_market(cfg, node, context).await,
        "action.dual_dca" => execute_action_dual_dca(repo, run, node, context).await,
        "action.place_order" => {
            execute_action_place_order(repo, run_id, cfg, limits, policy, run, node, context).await
        }
        "action.cancel_order" => execute_action_cancel_order(repo, node, context).await,
        "action.update_order" => execute_action_update_order(repo, node, context).await,
        "action.set_state" => execute_action_set_state(node, context),
        "action.notify" => execute_action_notify(repo, run, node, context).await,
        "action.telegram_notify" => execute_action_telegram_notify(repo, run, node, context).await,
        _ => Err(anyhow::anyhow!(
            "unsupported flow node type: {}",
            node.node_type
        )),
    }
}

async fn execute_trigger_market_price(
    _repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .ok_or_else(|| anyhow::anyhow!("trigger.market_price requires marketSlug"))?;
    let var_key = node_config_string(node, "varKey").unwrap_or_else(|| node.key.clone());
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .or_else(|| node_config_i64(node, "pollIntervalMs"))
        .unwrap_or(10000)
        .max(250) as i64;
    let repeat_mode = node.config
        .get("repeatMode")
        .and_then(Value::as_str)
        .unwrap_or("loop");

    // --- WS-sourced step detection ---
    let trigger_source = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(|v| v.as_str());
    let ws_sourced = trigger_source == Some("ws_market_price");
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(|v| v.as_object());
    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);

    // --- Multi-outcome conditions (outcomeConditions array) ---
    let outcome_conditions = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
        .cloned();

    let mut triggered_token_id = String::new();
    let mut triggered_outcome_label = String::new();
    let mut triggered_condition = String::new();
    let mut triggered_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut pass: bool;

    if let Some(ref conditions) = outcome_conditions {
        // Multi-outcome: OR logic
        pass = false;
        for cond in conditions {
            let cond_token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_outcome_label = cond
                .get("outcomeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_condition = cond
                .get("triggerCondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_price = cond
                .get("triggerPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
            if cond_token_id.is_empty() || cond_trigger_condition.is_empty() {
                continue;
            }
            let tp = match cond_trigger_price {
                Some(v) => v,
                None => continue,
            };
            let prev_state_key = format!("previous_price_{}", cond_token_id);
            let prev = flow_node_state(context, &node.key, &prev_state_key)
                .and_then(value_as_f64);
            let cur_result = if let Some(sp) = ws_prices_map
                .and_then(|m| m.get(&cond_token_id))
                .and_then(value_as_f64)
                .map(clamp_probability)
            {
                Ok(sp)
            } else {
                fetch_trade_flow_market_price(
                    ws,
                    client,
                    &market_slug,
                    Some(cond_token_id.as_str()),
                )
                .await
            };
            let cur = match cur_result {
                Ok(price) => price,
                Err(err) => {
                    let retry_ms = interval_ms.max(5000);
                    let repeat_at = Utc::now() + ChronoDuration::milliseconds(retry_ms);
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "run_id": run.id,
                            "node_key": node.key,
                            "market_slug": market_slug,
                            "error": err.to_string(),
                            "retry": true,
                            "retry_at": repeat_at.to_rfc3339()
                        }),
                        routes: Vec::new(),
                        repeat_at: Some(repeat_at),
                        repeat_idempotency_key: None,
                    });
                }
            };
            // Store per-token previous price
            set_flow_node_state(context, &node.key, &prev_state_key, json!(cur));
            let pass_this = if ws_sourced {
                // WS batch already confirmed the cross
                true
            } else {
                match cond_trigger_condition.as_str() {
                    "cross_above" => prev
                        .map(|pv| pv < tp && cur >= tp)
                        .unwrap_or(cur >= tp),
                    "cross_below" => prev
                        .map(|pv| pv > tp && cur <= tp)
                        .unwrap_or(cur <= tp),
                    _ => false,
                }
            };
            if pass_this && !pass {
                pass = true;
                triggered_token_id = cond_token_id;
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_price = Some(cur);
                current_price = Some(cur);
            }
        }
    } else {
        // Legacy single-token path
        let token_id = node_config_string(node, "tokenId")
            .or_else(|| flow_context_string(context, "tokenId"));
        let cur_result = if let Some(sp) = ws_price_from_step {
            Ok(sp)
        } else {
            fetch_trade_flow_market_price(ws, client, &market_slug, token_id.as_deref()).await
        };
        let cur = match cur_result {
            Ok(price) => price,
            Err(err) => {
                let retry_ms = interval_ms.max(5000);
                let repeat_at = Utc::now() + ChronoDuration::milliseconds(retry_ms);
                return Ok(TradeFlowNodeExecution {
                    output: json!({
                        "run_id": run.id,
                        "node_key": node.key,
                        "market_slug": market_slug,
                        "error": err.to_string(),
                        "retry": true,
                        "retry_at": repeat_at.to_rfc3339()
                    }),
                    routes: Vec::new(),
                    repeat_at: Some(repeat_at),
                    repeat_idempotency_key: None,
                });
            }
        };
        current_price = Some(cur);
        set_flow_var(context, &var_key, json!(cur));
        set_flow_node_state(context, &node.key, "last_price", json!(cur));

        let trigger_condition = node_config_string(node, "triggerCondition");
        let trigger_price = node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
        let previous_price =
            flow_node_state(context, &node.key, "previous_price").and_then(value_as_f64);
        pass = if ws_sourced {
            // WS batch already confirmed the cross
            true
        } else {
            match (trigger_condition.as_deref(), trigger_price) {
                (Some("cross_above"), Some(tp)) => previous_price
                    .map(|prev| prev < tp && cur >= tp)
                    .unwrap_or(cur >= tp),
                (Some("cross_below"), Some(tp)) => previous_price
                    .map(|prev| prev > tp && cur <= tp)
                    .unwrap_or(cur <= tp),
                _ => false,
            }
        };
        set_flow_node_state(context, &node.key, "previous_price", json!(cur));
        triggered_token_id = token_id.unwrap_or_default();
        triggered_condition = trigger_condition.unwrap_or_default();
        triggered_price = Some(cur);
    }

    // Write triggered outcome info to context
    if let Some(price) = current_price {
        set_flow_node_state(context, &node.key, "last_price", json!(price));
        set_flow_var(context, &format!("{var_key}_price"), json!(price));
    }
    if !triggered_token_id.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_token_id"),
            json!(triggered_token_id),
        );
    }
    if !triggered_outcome_label.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_outcome_label"),
            json!(triggered_outcome_label),
        );
    }
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(
            context,
            &format!("{var_key}_triggered_price"),
            json!(tp),
        );
    }

    let repeat_at = if ws_sourced {
        None
    } else if repeat_mode == "once" {
        None // once modu: sadece 1 kere çalış, tekrar etme
    } else {
        Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
    };
    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "market_slug": market_slug,
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "triggered_price": triggered_price,
        "price": current_price,
        "pass": pass,
        "var_key": var_key,
        "multi_outcome": outcome_conditions.is_some(),
        "ws_sourced": ws_sourced
    });
    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    })
}

async fn execute_trigger_sell_progress(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("trigger.sell_progress requires sourceTradeId"))?;
    let (filled_notional_usdc, _) = repo.aggregate_trade_fills(source_trade_id).await?;
    let target_notional_usdc = repo
        .trade_notional_usdc(source_trade_id)
        .await?
        .unwrap_or(0.0);

    let progress = if target_notional_usdc > 0.0 {
        ((filled_notional_usdc / target_notional_usdc) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let var_key =
        node_config_string(node, "varKey").unwrap_or_else(|| "sell_progress_pct".to_string());
    set_flow_var(context, &var_key, json!(progress));
    set_flow_node_state(context, &node.key, "last_progress_pct", json!(progress));

    let min_progress = node_config_f64(node, "minProgressPct");
    let pass = min_progress.map(|v| progress >= v).unwrap_or(true);
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(1500)
        .max(250) as i64;

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "filled_notional_usdc": filled_notional_usdc,
        "target_notional_usdc": target_notional_usdc,
        "progress_pct": progress,
        "pass": pass
    });
    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at: Some(Utc::now() + ChronoDuration::milliseconds(interval_ms)),
        repeat_idempotency_key: None,
    })
}

async fn execute_trigger_open_positions(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("trigger.open_positions requires sourceTradeId"))?;
    let positions = repo.load_leg_positions(source_trade_id).await?;

    let mut qty_yes = 0.0_f64;
    let mut qty_no = 0.0_f64;
    for leg in &positions {
        match leg.leg_side {
            LegSide::Yes => qty_yes += leg.qty,
            LegSide::No => qty_no += leg.qty,
        }
    }
    let qty_total = qty_yes + qty_no;

    let min_position_qty = node_config_f64(node, "minPositionQty")
        .unwrap_or(0.0)
        .max(0.0);
    let exists = qty_total > 0.0;
    let qty_pass = qty_total >= min_position_qty;
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(3000)
        .max(250) as i64;

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| node.key.clone());

    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();

    // --- Multi-outcome conditions (outcomeConditions array) ---
    let outcome_conditions = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
        .cloned();

    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    // Multi-outcome ws prices: input may carry wsPrices: { "tokenId": price, ... }
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(|v| v.as_object());

    // Shared mutable state for triggered outcome
    let mut triggered_token_id = String::new();
    let mut triggered_outcome_label = String::new();
    let mut triggered_condition = String::new();
    let mut triggered_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut price_pass = false;
    let mut websocket_price_mode = false;

    if let Some(ref conditions) = outcome_conditions {
        // Multi-outcome path: OR logic
        anyhow::ensure!(
            !market_slug.is_empty(),
            "trigger.open_positions with outcomeConditions requires marketSlug"
        );
        for cond in conditions {
            let cond_token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_outcome_label = cond
                .get("outcomeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_condition = cond
                .get("triggerCondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_price = cond
                .get("triggerPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
            if cond_token_id.is_empty() || cond_trigger_condition.is_empty() {
                continue;
            }
            let tp = match cond_trigger_price {
                Some(v) => v,
                None => continue,
            };
            let is_ws_mode = matches!(
                cond_trigger_condition.as_str(),
                "cross_above" | "cross_below"
            );
            if is_ws_mode {
                websocket_price_mode = true;
            }
            let prev_state_key = format!("previous_price_{}", cond_token_id);
            let prev = flow_node_state(context, &node.key, &prev_state_key)
                .and_then(value_as_f64);
            // Get current price for this token
            let step_ws_price = ws_prices_map
                .and_then(|m| m.get(&cond_token_id))
                .and_then(value_as_f64)
                .map(clamp_probability);
            let cur = if let Some(sp) = step_ws_price {
                Some(sp)
            } else if is_ws_mode {
                fetch_price_from_market_ws(ws, &cond_token_id)
                    .await
                    .map(clamp_probability)
            } else {
                Some(
                    fetch_trade_flow_market_price(
                        ws,
                        client,
                        &market_slug,
                        Some(cond_token_id.as_str()),
                    )
                    .await?,
                )
            };
            // Store previous_price per token
            if let Some(p) = cur {
                set_flow_node_state(context, &node.key, &prev_state_key, json!(p));
            }
            let pass_this = match cond_trigger_condition.as_str() {
                "cross_above" => cur
                    .map(|v| prev.map(|pv| pv < tp && v >= tp).unwrap_or(v >= tp))
                    .unwrap_or(false),
                "cross_below" => cur
                    .map(|v| prev.map(|pv| pv > tp && v <= tp).unwrap_or(v <= tp))
                    .unwrap_or(false),
                _ => false,
            };
            if pass_this && !price_pass {
                price_pass = true;
                triggered_token_id = cond_token_id.clone();
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_price = cur;
                current_price = cur;
            }
        }
    } else {
        // Legacy single-token path (backward compatibility)
        let token_id = node_config_string(node, "tokenId")
            .or_else(|| flow_context_string(context, "tokenId"))
            .unwrap_or_default();
        let outcome_label = node_config_string(node, "outcomeLabel")
            .or_else(|| flow_context_string(context, "outcomeLabel"))
            .unwrap_or_default();
        let trigger_condition = node_config_string(node, "triggerCondition");
        let trigger_price = node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
        websocket_price_mode = matches!(
            trigger_condition.as_deref(),
            Some("cross_above" | "cross_below")
        ) && trigger_price.is_some();
        if trigger_condition.is_some() {
            anyhow::ensure!(
                !market_slug.is_empty(),
                "trigger.open_positions with triggerCondition requires marketSlug"
            );
            anyhow::ensure!(
                !token_id.is_empty(),
                "trigger.open_positions with triggerCondition requires tokenId"
            );
        }
        let previous_price =
            flow_node_state(context, &node.key, "previous_price").and_then(value_as_f64);
        let token_id_ref = if token_id.is_empty() {
            None
        } else {
            Some(token_id.as_str())
        };
        let (cur, pass_single) = match (trigger_condition.as_deref(), trigger_price) {
            (Some("cross_above"), Some(tp)) => {
                let cur_val = if let Some(step_price) = ws_price_from_step {
                    Some(step_price)
                } else if websocket_price_mode {
                    if let Some(token) = token_id_ref {
                        fetch_price_from_market_ws(ws, token)
                            .await
                            .map(clamp_probability)
                    } else {
                        None
                    }
                } else {
                    Some(
                        fetch_trade_flow_market_price(ws, client, &market_slug, token_id_ref)
                            .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        previous_price
                            .map(|prev| prev < tp && value >= tp)
                            .unwrap_or(value >= tp)
                    })
                    .unwrap_or(false);
                (cur_val, p)
            }
            (Some("cross_below"), Some(tp)) => {
                let cur_val = if let Some(step_price) = ws_price_from_step {
                    Some(step_price)
                } else if websocket_price_mode {
                    if let Some(token) = token_id_ref {
                        fetch_price_from_market_ws(ws, token)
                            .await
                            .map(clamp_probability)
                    } else {
                        None
                    }
                } else {
                    Some(
                        fetch_trade_flow_market_price(ws, client, &market_slug, token_id_ref)
                            .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        previous_price
                            .map(|prev| prev > tp && value <= tp)
                            .unwrap_or(value <= tp)
                    })
                    .unwrap_or(false);
                (cur_val, p)
            }
            (Some(other), Some(_)) => {
                return Err(anyhow::anyhow!(
                    "trigger.open_positions triggerCondition must be cross_above/cross_below (got {other})"
                ));
            }
            (Some(_), None) => {
                return Err(anyhow::anyhow!(
                    "trigger.open_positions triggerCondition requires triggerPrice or triggerPriceCent"
                ));
            }
            _ => (None, true),
        };
        current_price = cur;
        price_pass = pass_single;
        triggered_token_id = token_id;
        triggered_outcome_label = outcome_label;
        triggered_condition = trigger_condition.unwrap_or_default();
        triggered_price = cur;
        if let Some(p) = cur {
            set_flow_node_state(context, &node.key, "previous_price", json!(p));
        }
    }

    let pass = exists && qty_pass && price_pass;
    if let Some(price) = current_price {
        set_flow_node_state(context, &node.key, "last_price", json!(price));
        set_flow_var(context, &format!("{var_prefix}_price"), json!(price));
    }

    set_flow_var(context, &format!("{var_prefix}_exists"), json!(exists));
    set_flow_var(
        context,
        &format!("{var_prefix}_qty_total"),
        json!(qty_total),
    );
    set_flow_var(context, &format!("{var_prefix}_qty_yes"), json!(qty_yes));
    set_flow_var(context, &format!("{var_prefix}_qty_no"), json!(qty_no));
    set_flow_var(
        context,
        &format!("{var_prefix}_trade_id"),
        json!(source_trade_id),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_market_slug"),
        json!(market_slug),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        json!(triggered_token_id),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        json!(triggered_outcome_label),
    );
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_prefix}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(
            context,
            &format!("{var_prefix}_triggered_price"),
            json!(tp),
        );
    }
    set_flow_node_state(
        context,
        &node.key,
        "last_position_qty_total",
        json!(qty_total),
    );

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "positions_count": positions.len(),
        "qty_yes": qty_yes,
        "qty_no": qty_no,
        "qty_total": qty_total,
        "min_position_qty": min_position_qty,
        "qty_pass": qty_pass,
        "triggered_condition": triggered_condition,
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_price": triggered_price,
        "websocket_price_mode": websocket_price_mode,
        "price": current_price,
        "price_pass": price_pass,
        "exists": exists,
        "pass": pass,
        "multi_outcome": outcome_conditions.is_some()
    });
    let repeat_at = if websocket_price_mode {
        None
    } else {
        Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
    };

    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    })
}

fn execute_trigger_time_window(
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let start_at = node_config_datetime(node, "startAt")?;
    let end_at = node_config_datetime(node, "endAt")?;
    let now = Utc::now();

    let in_window = match (start_at, end_at) {
        (Some(start), Some(end)) => now >= start && now <= end,
        (Some(start), None) => now >= start,
        (None, Some(end)) => now <= end,
        (None, None) => true,
    };
    let var_key =
        node_config_string(node, "varKey").unwrap_or_else(|| "time_window_open".to_string());
    set_flow_var(context, &var_key, json!(in_window));

    let routes = if in_window {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: now,
        }]
    } else {
        Vec::new()
    };
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(5000)
        .max(250) as i64;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "in_window": in_window,
            "start_at": start_at,
            "end_at": end_at
        }),
        routes,
        repeat_at: Some(now + ChronoDuration::milliseconds(interval_ms)),
        repeat_idempotency_key: None,
    })
}

fn execute_logic_if(node: &TradeFlowNode, context: &Value) -> Result<TradeFlowNodeExecution> {
    let expression = node
        .config
        .get("expression")
        .ok_or_else(|| anyhow::anyhow!("logic.if requires expression"))?;
    let eval_data = build_trade_flow_eval_data(context);
    let decision = evaluate_jsonlogic(expression, &eval_data);
    let branch = if value_truthy(&decision) {
        "on_true"
    } else {
        "on_false"
    };
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "result": decision,
            "branch": branch
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: branch.to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_switch(node: &TradeFlowNode, context: &Value) -> Result<TradeFlowNodeExecution> {
    let expression = node
        .config
        .get("expression")
        .ok_or_else(|| anyhow::anyhow!("logic.switch requires expression"))?;
    let eval_data = build_trade_flow_eval_data(context);
    let switch_value = evaluate_jsonlogic(expression, &eval_data);

    let mut edge_type = "default".to_string();
    if let Some(cases) = node.config.get("cases").and_then(Value::as_array) {
        for case_item in cases {
            let Some(case_obj) = case_item.as_object() else {
                continue;
            };
            let Some(label) = case_obj
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            else {
                continue;
            };
            let expected = case_obj.get("value").cloned().unwrap_or(Value::Null);
            if values_equal(&switch_value, &expected) {
                edge_type = format!("case:{label}");
                break;
            }
        }
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "switch_value": switch_value,
            "edge_type": edge_type
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type,
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_delay(node: &TradeFlowNode) -> Result<TradeFlowNodeExecution> {
    let delay_ms = node_config_i64(node, "delayMs")
        .or_else(|| node_config_i64(node, "ms"))
        .unwrap_or(1000)
        .max(0) as i64;
    let available_at = Utc::now() + ChronoDuration::milliseconds(delay_ms);
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "delay_ms": delay_ms
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at,
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_retry(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let max_attempts = node_config_i64(node, "maxAttempts").unwrap_or(3).max(1) as i32;
    let backoff_ms = node_config_i64(node, "backoffMs").unwrap_or(1000).max(0) as i64;
    let strategy = node_config_string(node, "strategy").unwrap_or_else(|| "fixed".to_string());

    let should_retry = if let Some(expression) = node.config.get("expression") {
        let eval_data = build_trade_flow_eval_data(context);
        value_truthy(&evaluate_jsonlogic(expression, &eval_data))
    } else {
        step.input_json
            .as_ref()
            .and_then(|input| input.get("error"))
            .is_some()
    };

    if should_retry && step.attempt < max_attempts {
        let multiplier = if strategy == "exponential" {
            2i64.pow((step.attempt.saturating_sub(1)) as u32)
        } else {
            1
        };
        let delay_ms = backoff_ms.saturating_mul(multiplier);
        let next_attempt = step.attempt + 1;
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "should_retry": true,
                "attempt": step.attempt,
                "next_attempt": next_attempt,
                "max_attempts": max_attempts,
                "delay_ms": delay_ms
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_retry".to_string(),
                available_at: Utc::now() + ChronoDuration::milliseconds(delay_ms),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if should_retry && step.attempt >= max_attempts {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "should_retry": true,
                "attempt": step.attempt,
                "max_attempts": max_attempts,
                "attempts_exhausted": true
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "should_retry": false,
            "attempt": step.attempt
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_resolve_market(
    cfg: &AppConfig,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let legacy_scope = node_config_string(node, "marketScope")
        .or_else(|| flow_context_string(context, "marketScope"))
        .unwrap_or_else(|| cfg.bot.market_scope.clone());
    let legacy_scope_def = find_updown_scope_by_scope(&legacy_scope);
    let asset = node_config_string(node, "asset")
        .or_else(|| legacy_scope_def.map(|def| def.asset.to_string()))
        .unwrap_or_else(|| "btc".to_string());
    let timeframe = node_config_string(node, "timeframe")
        .or_else(|| legacy_scope_def.map(|def| def.timeframe.to_string()))
        .unwrap_or_else(|| "5m".to_string());
    let scope_def = find_updown_scope_by_asset_timeframe(&asset, &timeframe).ok_or_else(|| {
        anyhow::anyhow!(
            "action.resolve_market unsupported asset/timeframe ({asset}/{timeframe}); supported assets: btc, eth, sol, xrp; timeframes: 5m, 15m"
        )
    })?;
    let market_scope = scope_def.scope.to_string();
    let slug_prefix =
        node_config_string(node, "slugPrefix").unwrap_or_else(|| scope_def.slug_prefix.to_string());
    let selection =
        node_config_string(node, "selection").unwrap_or_else(|| "latest_by_slug".to_string());
    let fail_on_missing_market = node_config_bool(node, "failOnMissingMarket").unwrap_or(true);
    let require_yes_no_tokens = node_config_bool(node, "requireYesNoTokens").unwrap_or(true);
    let require_token_id = node_config_bool(node, "requireTokenId").unwrap_or(true);
    let preferred_outcome = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_else(|| "yes".to_string());
    let normalized_outcome = match preferred_outcome.trim().to_ascii_lowercase().as_str() {
        "no" | "false" | "0" => "no",
        _ => "yes",
    };

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let mut markets = list_markets_for_scope(&gamma, &market_scope).await?;
    let candidate_count_before_prefix = markets.len();
    if !slug_prefix.is_empty() {
        markets.retain(|market| market.slug.starts_with(&slug_prefix));
    }
    let candidate_count = markets.len();

    let selected = select_live_market(markets, &selection, require_yes_no_tokens);
    let Some(selected) = selected else {
        let message = format!(
            "action.resolve_market could not find active market (scope={market_scope}, asset={}, timeframe={}, selection={selection}, slugPrefix={slug_prefix})",
            scope_def.asset,
            scope_def.timeframe
        );
        if fail_on_missing_market {
            anyhow::bail!(message);
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "found": false,
                "reason": "market_not_found",
                "market_scope": market_scope,
                "asset": scope_def.asset,
                "timeframe": scope_def.timeframe,
                "selection": selection,
                "slug_prefix": slug_prefix,
                "candidate_count_before_prefix": candidate_count_before_prefix,
                "candidate_count": candidate_count
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    };

    let resolved_token_id = if normalized_outcome == "no" {
        selected.no_token_id.clone()
    } else {
        selected.yes_token_id.clone()
    };
    if require_token_id && resolved_token_id.is_none() {
        let message = format!(
            "action.resolve_market missing tokenId for outcome={normalized_outcome} on market={}",
            selected.slug
        );
        if fail_on_missing_market {
            anyhow::bail!(message);
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "found": false,
                "reason": "token_not_found",
                "market_slug": selected.slug,
                "outcome_label": normalized_outcome,
                "yes_token_id": selected.yes_token_id,
                "no_token_id": selected.no_token_id
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    let selected_slug = selected.slug;
    let yes_token_id = selected.yes_token_id;
    let no_token_id = selected.no_token_id;
    let resolved_outcome_label = if normalized_outcome == "no" {
        "No".to_string()
    } else {
        "Yes".to_string()
    };
    set_flow_context(context, "marketSlug", json!(selected_slug));
    set_flow_context(context, "marketScope", json!(scope_def.scope));
    set_flow_context(context, "marketAsset", json!(scope_def.asset));
    set_flow_context(context, "marketTimeframe", json!(scope_def.timeframe));
    set_flow_context(context, "outcomeLabel", json!(resolved_outcome_label));
    set_flow_context(context, "yesTokenId", json!(yes_token_id));
    set_flow_context(context, "noTokenId", json!(no_token_id));
    set_flow_context(context, "tokenId", json!(resolved_token_id));

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "resolved_market".to_string());
    let resolved_market_slug = flow_context_value(context, "marketSlug").unwrap_or(Value::Null);
    let resolved_outcome = flow_context_value(context, "outcomeLabel").unwrap_or(Value::Null);
    let resolved_token = flow_context_value(context, "tokenId").unwrap_or(Value::Null);
    let resolved_yes_token = flow_context_value(context, "yesTokenId").unwrap_or(Value::Null);
    let resolved_no_token = flow_context_value(context, "noTokenId").unwrap_or(Value::Null);
    let resolved_scope = flow_context_value(context, "marketScope").unwrap_or(Value::Null);
    let resolved_asset = flow_context_value(context, "marketAsset").unwrap_or(Value::Null);
    let resolved_timeframe = flow_context_value(context, "marketTimeframe").unwrap_or(Value::Null);
    set_flow_var(
        context,
        &format!("{var_prefix}_slug"),
        resolved_market_slug.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        resolved_outcome.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        resolved_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_yes_token_id"),
        resolved_yes_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_no_token_id"),
        resolved_no_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_scope"),
        resolved_scope.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_asset"),
        resolved_asset.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_timeframe"),
        resolved_timeframe.clone(),
    );

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "found": true,
            "market_scope": market_scope,
            "asset": scope_def.asset,
            "timeframe": scope_def.timeframe,
            "selection": selection,
            "slug_prefix": slug_prefix,
            "market_slug": flow_context_string(context, "marketSlug"),
            "token_id": flow_context_string(context, "tokenId"),
            "outcome_label": flow_context_string(context, "outcomeLabel"),
            "yes_token_id": flow_context_string(context, "yesTokenId"),
            "no_token_id": flow_context_string(context, "noTokenId"),
            "candidate_count_before_prefix": candidate_count_before_prefix,
            "candidate_count": candidate_count
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_dual_dca(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sourceTradeId"))?;

    let asset = node_config_string(node, "asset")
        .or_else(|| node_config_string(node, "coin"))
        .or_else(|| flow_context_string(context, "marketAsset"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires asset (btc/eth/sol/xrp)"))?
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        matches!(asset.as_str(), "btc" | "eth" | "sol" | "xrp"),
        "action.dual_dca asset must be one of: btc, eth, sol, xrp"
    );

    let timeframe_raw = node_config_string(node, "timeframe")
        .or_else(|| node_config_string(node, "marketPeriod"))
        .or_else(|| flow_context_string(context, "marketTimeframe"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires timeframe (5m/15m)"))?
        .trim()
        .to_ascii_lowercase();
    let timeframe = match timeframe_raw.as_str() {
        "5" | "5m" | "5min" | "5 min" => "5m",
        "15" | "15m" | "15min" | "15 min" => "15m",
        _ => {
            anyhow::bail!("action.dual_dca timeframe must be 5m or 15m");
        }
    };

    let side_mode_raw = node_config_string(node, "sideMode")
        .or_else(|| node_config_string(node, "side"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sideMode (up/down/all)"))?
        .trim()
        .to_ascii_lowercase();
    let side_mode = match side_mode_raw.as_str() {
        "up" => "up",
        "down" => "down",
        "all" => "all",
        _ => {
            anyhow::bail!("action.dual_dca sideMode must be up, down or all");
        }
    };

    let configured_base_shares = node_config_f64(node, "baseShares");
    let configured_base_usdc = node_config_f64(node, "baseUsdc")
        .or_else(|| node_config_f64(node, "sizeUsdc"))
        .or_else(|| node_config_f64(node, "notionalUsdc"));
    let base_sizing_raw = node_config_string(node, "baseSizing")
        .or_else(|| node_config_string(node, "baseSizeMode"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseSizing (shares/usdc)"))?
        .trim()
        .to_ascii_lowercase();
    let base_sizing = match base_sizing_raw.as_str() {
        "shares" => "shares",
        "usdc" => "usdc",
        _ => {
            anyhow::bail!("action.dual_dca baseSizing must be shares or usdc");
        }
    };

    let base_shares = if base_sizing == "shares" {
        let value = configured_base_shares
            .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseShares"))?;
        anyhow::ensure!(value > 0.0, "action.dual_dca baseShares must be > 0");
        Some(value)
    } else {
        None
    };
    let base_usdc = if base_sizing == "usdc" {
        let value = configured_base_usdc
            .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseUsdc"))?;
        anyhow::ensure!(value > 0.0, "action.dual_dca baseUsdc must be > 0");
        Some(value)
    } else {
        None
    };

    let base_price_usdc = node_config_f64(node, "basePriceUsdc")
        .or_else(|| node_config_f64(node, "basePrice"))
        .or_else(|| node_config_f64(node, "basePriceCent").map(|v| v / 100.0));
    if let Some(base_price) = base_price_usdc {
        anyhow::ensure!(
            (0.01..=0.99).contains(&base_price),
            "action.dual_dca basePriceUsdc must be in [0.01, 0.99]"
        );
    }

    let dca_levels = node_config_i64(node, "dcaLevels")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires dcaLevels"))?;
    anyhow::ensure!(
        (1..=20).contains(&dca_levels),
        "action.dual_dca dcaLevels must be in [1, 20]"
    );

    let near_step = node_config_f64(node, "nearStep")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires nearStep"))?;
    anyhow::ensure!(
        near_step > 0.0 && near_step < 1.0,
        "action.dual_dca nearStep must be in (0, 1)"
    );
    let step_mult = node_config_f64(node, "stepMult")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires stepMult"))?;
    anyhow::ensure!(step_mult >= 1.0, "action.dual_dca stepMult must be >= 1");
    let size_mult = node_config_f64(node, "sizeMult")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sizeMult"))?;
    anyhow::ensure!(size_mult > 0.0, "action.dual_dca sizeMult must be > 0");
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires minPriceDistanceCent"))?;
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.dual_dca minPriceDistanceCent must be > 0"
    );
    let cutoff_min = node_config_i64(node, "cutoffMin")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires cutoffMin"))?;
    anyhow::ensure!(cutoff_min >= 0, "action.dual_dca cutoffMin must be >= 0");

    let tp_profit_usdc = node_config_f64(node, "tpProfitPct")
        .or_else(|| node_config_f64(node, "tpProfit"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires tpProfitPct"))?;
    let sl_loss_usdc = node_config_f64(node, "slLossPct")
        .or_else(|| node_config_f64(node, "slLoss"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires slLossPct"))?;
    let sl_spread_usdc = node_config_f64(node, "slSpreadPct")
        .or_else(|| node_config_f64(node, "slSpread"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires slSpreadPct"))?;
    anyhow::ensure!(
        tp_profit_usdc >= 0.0 && sl_loss_usdc >= 0.0 && sl_spread_usdc >= 0.0,
        "action.dual_dca risk thresholds must be >= 0"
    );

    let scope_def = find_updown_scope_by_asset_timeframe(&asset, timeframe).ok_or_else(|| {
        anyhow::anyhow!(
            "action.dual_dca unsupported asset/timeframe ({asset}/{timeframe}); supported assets: btc, eth, sol, xrp; timeframes: 5m, 15m"
        )
    })?;

    let job_id = repo
        .upsert_trade_flow_dual_dca_job(
            run.id,
            run.definition_id,
            Some(run.version_id),
            &node.key,
            Some(source_trade_id),
            &asset,
            timeframe,
            side_mode,
            base_sizing,
            base_shares,
            base_usdc,
            base_price_usdc,
            dca_levels as i32,
            near_step,
            step_mult,
            size_mult,
            min_price_distance_cent,
            cutoff_min as i32,
            tp_profit_usdc,
            sl_loss_usdc,
            sl_spread_usdc,
        )
        .await?;

    repo.append_trade_flow_dual_dca_event(
        job_id,
        None,
        "job_upserted",
        &json!({
            "flow_run_id": run.id,
            "flow_definition_id": run.definition_id,
            "flow_version_id": run.version_id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_scope": scope_def.scope,
            "asset": asset,
            "timeframe": timeframe,
            "side_mode": side_mode,
            "base_sizing": base_sizing,
            "base_shares": base_shares,
            "base_usdc": base_usdc,
            "base_price_usdc": base_price_usdc,
            "dca_levels": dca_levels,
            "near_step": near_step,
            "step_mult": step_mult,
            "size_mult": size_mult,
            "min_price_distance_cent": min_price_distance_cent,
            "cutoff_min": cutoff_min,
            "tp_profit_usdc": tp_profit_usdc,
            "sl_loss_usdc": sl_loss_usdc,
            "sl_spread_usdc": sl_spread_usdc
        }),
    )
    .await?;

    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    set_flow_ref(context, &node.key, json!(job_id));
    set_flow_ref(context, &ref_key, json!(job_id));
    set_flow_var(context, &format!("{ref_key}_job_id"), json!(job_id));
    set_flow_context(context, "marketScope", json!(scope_def.scope));
    set_flow_context(context, "marketAsset", json!(scope_def.asset));
    set_flow_context(context, "marketTimeframe", json!(scope_def.timeframe));

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "job_id": job_id,
            "ref_key": ref_key,
            "market_scope": scope_def.scope,
            "asset": scope_def.asset,
            "timeframe": scope_def.timeframe,
            "side_mode": side_mode,
            "base_sizing": base_sizing,
            "base_shares": base_shares,
            "base_usdc": base_usdc,
            "base_price_usdc": base_price_usdc,
            "dca_levels": dca_levels
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires sourceTradeId"))?;
    let side = node_config_string(node, "side").unwrap_or_else(|| "buy".to_string());
    anyhow::ensure!(
        matches!(side.as_str(), "buy" | "sell"),
        "action.place_order side must be buy or sell"
    );
    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires marketSlug"))?;
    let token_id = node_config_string(node, "tokenId")
        .or_else(|| flow_context_string(context, "tokenId"))
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires tokenId"))?;
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_else(|| token_id.clone());
    let size_mode = node_config_string(node, "sizeMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(mode) = size_mode.as_deref() {
        anyhow::ensure!(
            matches!(mode, "usdc" | "pct"),
            "action.place_order sizeMode must be usdc or pct"
        );
    }
    let max_triggers = node_config_i64(node, "maxTriggers")
        .unwrap_or(1)
        .clamp(1, 20) as i32;
    let trigger_sizes = if let Some(raw_values) = node.config.get("triggerSizes") {
        let values = raw_values
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("action.place_order triggerSizes must be an array"))?;
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let parsed = value_as_f64(value).ok_or_else(|| {
                anyhow::anyhow!("action.place_order triggerSizes entries must be numeric")
            })?;
            anyhow::ensure!(
                parsed > 0.0 && parsed.is_finite(),
                "action.place_order triggerSizes entries must be > 0"
            );
            out.push(parsed);
        }
        anyhow::ensure!(
            out.len() <= max_triggers as usize,
            "action.place_order triggerSizes length cannot exceed maxTriggers"
        );
        out
    } else {
        Vec::new()
    };
    let trigger_size_for_first_fire = trigger_sizes.first().copied();
    let configured_size_usdc =
        node_config_f64(node, "sizeUsdc").or_else(|| node_config_f64(node, "targetNotionalUsdc"));
    let configured_size_pct =
        node_config_f64(node, "sizePct").or_else(|| node_config_f64(node, "sizePercent"));
    let use_pct_size = if trigger_size_for_first_fire.is_some() {
        if let Some(mode) = size_mode.as_deref() {
            mode == "pct"
        } else {
            configured_size_usdc.is_none() && configured_size_pct.is_some()
        }
    } else {
        matches!(size_mode.as_deref(), Some("pct"))
            || (configured_size_usdc.is_none() && configured_size_pct.is_some())
    };
    if !trigger_sizes.is_empty() && use_pct_size {
        let trigger_pct_total: f64 = trigger_sizes.iter().sum();
        anyhow::ensure!(
            trigger_pct_total <= 100.000001,
            "action.place_order pct triggerSizes total must be <= 100"
        );
    }
    let (size_usdc, resolved_size_mode, resolved_size_pct) = if use_pct_size {
        let size_pct = trigger_size_for_first_fire
            .or(configured_size_pct)
            .ok_or_else(|| {
                anyhow::anyhow!("action.place_order requires sizePct (0, 100] when sizeMode is pct")
            })?;
        anyhow::ensure!(
            size_pct > 0.0 && size_pct <= 100.0,
            "action.place_order sizePct must be in (0, 100]"
        );
        let source_notional = repo
            .trade_notional_usdc(source_trade_id)
            .await?
            .unwrap_or(0.0);
        anyhow::ensure!(
            source_notional > 0.0,
            "action.place_order sizePct requires source trade notional > 0"
        );
        let resolved = source_notional * (size_pct / 100.0);
        anyhow::ensure!(
            resolved > 0.0,
            "action.place_order resolved size must be > 0"
        );
        (resolved, "pct", Some(size_pct))
    } else {
        let resolved = trigger_size_for_first_fire
            .or(configured_size_usdc)
            .ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
            )
        })?;
        anyhow::ensure!(resolved > 0.0, "action.place_order size must be > 0");
        (resolved, "usdc", None)
    };
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent").unwrap_or(1.0);
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.place_order minPriceDistanceCent must be > 0"
    );
    let trigger_condition = node_config_string(node, "triggerCondition");
    let trigger_price = node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
    if let Some(condition) = trigger_condition.as_deref() {
        anyhow::ensure!(
            matches!(condition, "cross_above" | "cross_below"),
            "action.place_order triggerCondition must be cross_above/cross_below"
        );
    }
    let mut kind = node_config_string(node, "kind").unwrap_or_else(|| {
        if trigger_condition.is_some() && trigger_price.is_some() {
            "conditional".to_string()
        } else {
            "immediate".to_string()
        }
    });
    if kind != "conditional" && kind != "immediate" {
        kind = "immediate".to_string();
    }
    let expires_at = node_config_datetime(node, "expiresAt")?;

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        source_trade_id,
        size_usdc,
        limits,
        policy,
    )
    .await?;
    if !matches!(risk, RiskDecision::Allow) {
        let output = json!({
            "node_key": node.key,
            "blocked": true,
            "risk_decision": format!("{risk:?}"),
            "source_trade_id": source_trade_id
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    let builder_order_id = repo
        .create_trade_builder_order(
            source_trade_id,
            &kind,
            "pending",
            &market_slug,
            &token_id,
            &outcome_label,
            &side,
            trigger_condition.as_deref(),
            trigger_price,
            size_usdc,
            min_price_distance_cent,
            expires_at,
            max_triggers,
        )
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_created",
        &json!({
            "flow_run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "size_mode": resolved_size_mode,
            "size_pct": resolved_size_pct,
            "trigger_sizes": trigger_sizes
        }),
    )
    .await?;

    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    set_flow_ref(context, &ref_key, json!(builder_order_id));
    set_flow_ref(context, &node.key, json!(builder_order_id));

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "ref_key": ref_key,
            "kind": kind,
            "side": side,
            "market_slug": market_slug,
            "token_id": token_id,
            "size_mode": resolved_size_mode,
            "size_pct": resolved_size_pct,
            "size_usdc": size_usdc
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_cancel_order(
    repo: &PostgresRepository,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let builder_order_id = resolve_flow_builder_order_id(node, context).ok_or_else(|| {
        anyhow::anyhow!("action.cancel_order requires builderOrderId or targetRef")
    })?;

    repo.set_trade_builder_order_status(builder_order_id, "canceled_requested", None)
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_cancel_requested",
        &json!({ "node_key": node.key }),
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "canceled_requested": true
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_update_order(
    repo: &PostgresRepository,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let builder_order_id = resolve_flow_builder_order_id(node, context).ok_or_else(|| {
        anyhow::anyhow!("action.update_order requires builderOrderId or targetRef")
    })?;
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent");
    let max_triggers = node_config_i64(node, "maxTriggers").map(|v| v.clamp(1, 1000) as i32);

    repo.update_trade_builder_order_params(builder_order_id, min_price_distance_cent, max_triggers)
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_updated",
        &json!({
            "node_key": node.key,
            "min_price_distance_cent": min_price_distance_cent,
            "max_triggers": max_triggers
        }),
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "updated": true
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_action_set_state(
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let state_patch = node
        .config
        .get("statePatch")
        .cloned()
        .or_else(|| node.config.get("state").cloned())
        .unwrap_or_else(|| json!({}));
    let state_patch_obj = state_patch
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("action.set_state statePatch must be object"))?;

    let state = ensure_nested_object(context, "state");
    for (key, value) in state_patch_obj {
        state.insert(key.clone(), value.clone());
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "state_patch": state_patch_obj
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_notify(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let message =
        node_config_string(node, "message").unwrap_or_else(|| "trade flow notify".to_string());
    let channel = node_config_string(node, "channel").unwrap_or_else(|| "ui".to_string());
    let payload = json!({
        "node_key": node.key,
        "message": message,
        "channel": channel,
        "vars": context.get("vars").cloned().unwrap_or(Value::Null)
    });
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "notify",
        &payload,
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: payload,
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn resolve_template_vars(template: &str, context: &Value) -> String {
    let mut result = template.to_string();
    if let Some(vars) = context.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            let placeholder = format!("{{{{vars.{}}}}}", k);
            let value_str = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
    }
    if let Some(state) = context.get("state").and_then(|v| v.as_object()) {
        for (k, v) in state {
            let placeholder = format!("{{{{state.{}}}}}", k);
            let value_str = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
    }
    result
}

async fn execute_action_telegram_notify(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let bot_token = node_config_string(node, "botToken")
        .ok_or_else(|| anyhow::anyhow!("action.telegram_notify requires botToken"))?;
    let chat_id = node_config_string(node, "chatId")
        .ok_or_else(|| anyhow::anyhow!("action.telegram_notify requires chatId"))?;
    let message_template = node_config_string(node, "message")
        .unwrap_or_else(|| "Trade flow notification".to_string());

    let message = resolve_template_vars(&message_template, context);

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML",
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    let (edge_type, output) = match resp {
        Ok(r) if r.status().is_success() => (
            "on_success".to_string(),
            json!({
                "node_key": node.key,
                "status": "sent",
                "chat_id": chat_id,
                "message": message,
            }),
        ),
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            (
                "on_error".to_string(),
                json!({
                    "node_key": node.key,
                    "status": "error",
                    "http_status": status,
                    "error": body,
                }),
            )
        }
        Err(e) => (
            "on_error".to_string(),
            json!({
                "node_key": node.key,
                "status": "error",
                "error": e.to_string(),
            }),
        ),
    };

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "telegram_notify",
        &output,
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output,
        routes: vec![TradeFlowRouteDecision {
            edge_type,
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn fetch_trade_flow_market_price(
    ws: &ClobWsClient,
    client: Option<&dyn OrderExecutor>,
    market_slug: &str,
    token_id: Option<&str>,
) -> Result<f64> {
    let token_id = token_id.filter(|v| !v.trim().is_empty());
    if let Some(token_id) = token_id {
        if let Some(ws_price) = fetch_price_from_market_ws(ws, token_id).await {
            return Ok(clamp_probability(ws_price));
        }
    }
    if let Some(client) = client {
        let token_id = token_id.ok_or_else(|| {
            anyhow::anyhow!(
                "trigger.market_price requires tokenId for REST midpoint fallback (marketSlug={market_slug})"
            )
        })?;
        let fallback = client.midpoint(token_id).await?;
        return Ok(clamp_probability(fallback.price));
    }

    Err(anyhow::anyhow!(
        "trigger.market_price fallback requires live order executor (set LIVE_TRADING_ENABLED=true or provide tokenId websocket price)"
    ))
}

fn node_config_string(node: &TradeFlowNode, key: &str) -> Option<String> {
    node.config
        .get(key)
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            Value::Bool(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|v| !v.is_empty())
}

fn node_config_f64(node: &TradeFlowNode, key: &str) -> Option<f64> {
    node.config.get(key).and_then(value_as_f64)
}

fn node_config_i64(node: &TradeFlowNode, key: &str) -> Option<i64> {
    node.config.get(key).and_then(value_as_i64)
}

fn node_config_bool(node: &TradeFlowNode, key: &str) -> Option<bool> {
    node.config.get(key).and_then(|value| match value {
        Value::Bool(v) => Some(*v),
        Value::Number(v) => v
            .as_i64()
            .map(|n| n != 0)
            .or_else(|| v.as_f64().map(|n| n != 0.0)),
        Value::String(v) => {
            let normalized = v.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "y" | "on" => Some(true),
                "false" | "0" | "no" | "n" | "off" => Some(false),
                _ => None,
            }
        }
        _ => None,
    })
}

fn node_config_datetime(node: &TradeFlowNode, key: &str) -> Result<Option<DateTime<Utc>>> {
    let Some(value) = node.config.get(key) else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(anyhow::anyhow!("{key} must be RFC3339 datetime string"));
    };
    let parsed = DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("invalid RFC3339 datetime for {key}"))?
        .with_timezone(&Utc);
    Ok(Some(parsed))
}

fn flow_context_string(context: &Value, key: &str) -> Option<String> {
    context
        .get("flowContext")
        .and_then(|v| v.get(key))
        .and_then(|v| match v {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|v| !v.is_empty())
}

fn resolve_flow_source_trade_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    node_config_i64(node, "sourceTradeId").or_else(|| {
        context
            .get("flowContext")
            .and_then(|v| v.get("sourceTradeId"))
            .and_then(value_as_i64)
    })
}

fn resolve_flow_builder_order_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    if let Some(id) = node_config_i64(node, "builderOrderId") {
        return Some(id);
    }

    let target_ref = node_config_string(node, "targetRef")?;
    context
        .get("refs")
        .and_then(|v| v.get(&target_ref))
        .and_then(value_as_i64)
}

fn ensure_nested_object<'a>(
    context: &'a mut Value,
    key: &str,
) -> &'a mut serde_json::Map<String, Value> {
    let root = ensure_object_mut(context);
    if !root.get(key).map(Value::is_object).unwrap_or(false) {
        root.insert(key.to_string(), json!({}));
    }
    root.get_mut(key)
        .and_then(Value::as_object_mut)
        .expect("nested object should exist")
}

fn set_flow_var(context: &mut Value, key: &str, value: Value) {
    let vars = ensure_nested_object(context, "vars");
    vars.insert(key.to_string(), value);
}

fn set_flow_context(context: &mut Value, key: &str, value: Value) {
    let flow_context = ensure_nested_object(context, "flowContext");
    if value.is_null() {
        flow_context.remove(key);
    } else {
        flow_context.insert(key.to_string(), value);
    }
}

fn flow_context_value(context: &Value, key: &str) -> Option<Value> {
    context.get("flowContext").and_then(|v| v.get(key)).cloned()
}

fn set_flow_ref(context: &mut Value, key: &str, value: Value) {
    let refs = ensure_nested_object(context, "refs");
    refs.insert(key.to_string(), value);
}

fn set_flow_node_state(context: &mut Value, node_key: &str, state_key: &str, value: Value) {
    let node_state = ensure_nested_object(context, "nodeState");
    if !node_state
        .get(node_key)
        .map(Value::is_object)
        .unwrap_or(false)
    {
        node_state.insert(node_key.to_string(), json!({}));
    }
    if let Some(state_obj) = node_state.get_mut(node_key).and_then(Value::as_object_mut) {
        state_obj.insert(state_key.to_string(), value);
    }
}

fn flow_node_state<'a>(context: &'a Value, node_key: &str, state_key: &str) -> Option<&'a Value> {
    context
        .get("nodeState")
        .and_then(|node_state| node_state.get(node_key))
        .and_then(|node| node.get(state_key))
}

fn build_trade_flow_eval_data(context: &Value) -> Value {
    let mut root = serde_json::Map::new();
    for section in ["flowContext", "state", "vars"] {
        if let Some(obj) = context.get(section).and_then(Value::as_object) {
            for (key, value) in obj {
                root.insert(key.clone(), value.clone());
            }
        }
    }
    for section in ["flowContext", "state", "vars", "refs", "nodeState"] {
        if let Some(value) = context.get(section) {
            root.insert(section.to_string(), value.clone());
        }
    }
    Value::Object(root)
}

fn evaluate_jsonlogic(expression: &Value, data: &Value) -> Value {
    if let Some(object) = expression.as_object() {
        if object.len() != 1 {
            return Value::Null;
        }
        let (operator, args) = object.iter().next().expect("single entry object");
        return match operator.as_str() {
            "var" => resolve_jsonlogic_var(args, data),
            "==" | "!=" => {
                let values = evaluate_jsonlogic_args(args, data);
                if values.len() < 2 {
                    return Value::Bool(false);
                }
                let eq = values_equal(&values[0], &values[1]);
                Value::Bool(if operator == "==" { eq } else { !eq })
            }
            ">" | ">=" | "<" | "<=" => {
                let values = evaluate_jsonlogic_args(args, data);
                if values.len() < 2 {
                    return Value::Bool(false);
                }
                let Some(left) = value_as_f64(&values[0]) else {
                    return Value::Bool(false);
                };
                let Some(right) = value_as_f64(&values[1]) else {
                    return Value::Bool(false);
                };
                let result = match operator.as_str() {
                    ">" => left > right,
                    ">=" => left >= right,
                    "<" => left < right,
                    "<=" => left <= right,
                    _ => false,
                };
                Value::Bool(result)
            }
            "and" => {
                let values = evaluate_jsonlogic_args(args, data);
                Value::Bool(values.iter().all(value_truthy))
            }
            "or" => {
                let values = evaluate_jsonlogic_args(args, data);
                Value::Bool(values.iter().any(value_truthy))
            }
            "!" => {
                let values = evaluate_jsonlogic_args(args, data);
                let value = values.first().cloned().unwrap_or(Value::Bool(false));
                Value::Bool(!value_truthy(&value))
            }
            "+" | "-" | "*" | "/" => {
                let values = evaluate_jsonlogic_args(args, data);
                let numeric_values = values.iter().filter_map(value_as_f64).collect::<Vec<_>>();
                if numeric_values.is_empty() {
                    return Value::Null;
                }
                let computed = match operator.as_str() {
                    "+" => numeric_values.iter().sum::<f64>(),
                    "-" => {
                        if numeric_values.len() == 1 {
                            -numeric_values[0]
                        } else {
                            numeric_values[0] - numeric_values[1..].iter().sum::<f64>()
                        }
                    }
                    "*" => numeric_values.iter().product::<f64>(),
                    "/" => {
                        if numeric_values.len() < 2 || numeric_values[1] == 0.0 {
                            return Value::Null;
                        }
                        numeric_values[0] / numeric_values[1]
                    }
                    _ => return Value::Null,
                };
                serde_json::Number::from_f64(computed)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            "if" => {
                let values = evaluate_jsonlogic_args(args, data);
                let mut idx = 0usize;
                while idx + 1 < values.len() {
                    if value_truthy(&values[idx]) {
                        return values[idx + 1].clone();
                    }
                    idx += 2;
                }
                if values.len() % 2 == 1 {
                    values.last().cloned().unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        };
    }

    if let Some(array) = expression.as_array() {
        return Value::Array(
            array
                .iter()
                .map(|item| evaluate_jsonlogic(item, data))
                .collect(),
        );
    }

    expression.clone()
}

fn evaluate_jsonlogic_args(args: &Value, data: &Value) -> Vec<Value> {
    if let Some(array) = args.as_array() {
        array
            .iter()
            .map(|value| evaluate_jsonlogic(value, data))
            .collect()
    } else {
        vec![evaluate_jsonlogic(args, data)]
    }
}

fn resolve_jsonlogic_var(args: &Value, data: &Value) -> Value {
    if let Some(path) = args.as_str() {
        return lookup_jsonlogic_path(data, path).unwrap_or(Value::Null);
    }
    if let Some(list) = args.as_array() {
        let path = list.first().and_then(Value::as_str).unwrap_or_default();
        let fallback = list.get(1).cloned().unwrap_or(Value::Null);
        return lookup_jsonlogic_path(data, path).unwrap_or(fallback);
    }
    Value::Null
}

fn lookup_jsonlogic_path(data: &Value, path: &str) -> Option<Value> {
    if path.is_empty() {
        return Some(data.clone());
    }
    if let Some(value) = lookup_json_path(data, path) {
        return Some(value.clone());
    }

    if !path.contains('.') {
        for section in ["vars", "state", "flowContext", "refs"] {
            if let Some(value) = data.get(section).and_then(|v| v.get(path)) {
                return Some(value.clone());
            }
        }
    }

    None
}

fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        let key = part.trim();
        if key.is_empty() {
            continue;
        }
        current = current.get(key)?;
    }
    Some(current)
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(v) => {
            let normalized = v.trim().to_lowercase();
            !normalized.is_empty() && normalized != "false" && normalized != "0"
        }
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(v) => v.as_f64(),
        Value::String(v) => v.parse::<f64>().ok(),
        Value::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(v) => v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)),
        Value::String(v) => v.parse::<i64>().ok(),
        _ => None,
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    if let (Some(left_num), Some(right_num)) = (value_as_f64(left), value_as_f64(right)) {
        return (left_num - right_num).abs() <= 0.0000001;
    }
    left == right
}

async fn process_trade_builder_orders(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
) -> Result<()> {
    let orders = repo
        .list_trade_builder_orders_for_processing(MANUAL_ORDER_PROCESS_LIMIT)
        .await?;
    if orders.is_empty() {
        return Ok(());
    }

    if let Err(err) = sync_recent_trade_builder_fills(repo, client).await {
        warn!(
            run_id,
            error = %err,
            "TRADE_BUILDER_FILL_SYNC_ERROR"
        );
    }

    let limits = to_risk_limits(cfg);
    let policy = DefaultRiskPolicy;

    for order in orders {
        if let Err(err) =
            process_trade_builder_order(repo, run_id, cfg, &limits, &policy, client, ws, &order)
                .await
        {
            let _ = repo
                .set_trade_builder_order_status(order.id, "error", Some(&err.to_string()))
                .await;
            let _ = repo
                .append_trade_builder_order_event(
                    order.id,
                    "processing_error",
                    &json!({ "error": err.to_string() }),
                )
                .await;
            warn!(
                run_id,
                builder_order_id = order.id,
                error = %err,
                "TRADE_BUILDER_ORDER_ERROR"
            );
        }
    }

    if let Err(err) = sync_recent_trade_builder_fills(repo, client).await {
        warn!(
            run_id,
            error = %err,
            "TRADE_BUILDER_FILL_SYNC_ERROR"
        );
    }

    Ok(())
}


// DCA functions moved to dca.rs — direct market order approach.


#[cfg(test)]
mod dual_dca_tests {
    use super::*;

    fn runtime_graph(nodes: Vec<(&str, &str)>, edges: Vec<(&str, &str)>) -> TradeFlowGraphRuntime {
        TradeFlowGraphRuntime {
            context: json!({}),
            nodes: nodes
                .into_iter()
                .map(|(key, node_type)| TradeFlowNode {
                    key: key.to_string(),
                    node_type: node_type.to_string(),
                    config: json!({}),
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(source, target)| TradeFlowEdge {
                    source: source.to_string(),
                    target: target.to_string(),
                    edge_type: "default".to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn initial_seed_prefers_triggers_when_present() {
        let graph = runtime_graph(
            vec![
                ("trigger_tick", "trigger.market_price"),
                ("dual_root", "action.dual_dca"),
            ],
            vec![],
        );

        let (mode, start_nodes) =
            select_trade_flow_initial_seed_nodes(&graph).expect("selection should succeed");
        assert_eq!(mode, TradeFlowSeedMode::Trigger);
        assert_eq!(start_nodes.len(), 1);
        assert_eq!(start_nodes[0].key, "trigger_tick");
    }

    #[test]
    fn initial_seed_allows_dual_dca_roots_without_triggers() {
        let graph = runtime_graph(
            vec![
                ("dual_root", "action.dual_dca"),
                ("dual_child", "action.dual_dca"),
            ],
            vec![("dual_root", "dual_child")],
        );

        let (mode, start_nodes) =
            select_trade_flow_initial_seed_nodes(&graph).expect("selection should succeed");
        assert_eq!(mode, TradeFlowSeedMode::DualDcaRoot);
        assert_eq!(start_nodes.len(), 1);
        assert_eq!(start_nodes[0].key, "dual_root");
    }

    #[test]
    fn initial_seed_rejects_non_dual_roots_without_triggers() {
        let graph = runtime_graph(
            vec![
                ("dual_root", "action.dual_dca"),
                ("notify_root", "action.notify"),
            ],
            vec![],
        );

        let err = select_trade_flow_initial_seed_nodes(&graph).expect_err("should fail");
        assert_eq!(err, "flow_invalid_roots_without_trigger");
    }

    #[test]
    fn initial_seed_requires_trigger_when_dual_dca_absent() {
        let graph = runtime_graph(vec![("notify_root", "action.notify")], vec![]);

        let err = select_trade_flow_initial_seed_nodes(&graph).expect_err("should fail");
        assert_eq!(err, "flow_missing_trigger");
    }
    #[test]
    fn candidate_slugs_cover_prev_current_and_future_15m_windows() {
        let scope_def = find_updown_scope_by_scope("btc_15m_updown").expect("scope should exist");
        let now = DateTime::parse_from_rfc3339("2026-02-23T14:55:18Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let slugs = updown_scope_candidate_slugs(scope_def, now);
        assert_eq!(
            slugs,
            vec![
                "btc-updown-15m-1771857000".to_string(),
                "btc-updown-15m-1771857900".to_string(),
                "btc-updown-15m-1771858800".to_string(),
                "btc-updown-15m-1771859700".to_string(),
            ]
        );
    }

    #[test]
    fn candidate_slugs_cover_prev_current_and_future_5m_windows() {
        let scope_def = find_updown_scope_by_scope("btc_5m_updown").expect("scope should exist");
        let now = DateTime::parse_from_rfc3339("2026-02-23T14:55:18Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        let slugs = updown_scope_candidate_slugs(scope_def, now);
        assert_eq!(
            slugs,
            vec![
                "btc-updown-5m-1771858200".to_string(),
                "btc-updown-5m-1771858500".to_string(),
                "btc-updown-5m-1771858800".to_string(),
                "btc-updown-5m-1771859100".to_string(),
            ]
        );
    }

    fn gamma_market_for_test(slug: &str) -> GammaMarket {
        GammaMarket {
            slug: slug.to_string(),
            end_date_iso: None,
            active: true,
            closed: false,
            yes_token_id: Some("yes-token".to_string()),
            no_token_id: Some("no-token".to_string()),
            maker_base_fee: 0,
        }
    }

    #[test]
    fn select_preferred_live_market_prefers_current_window() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:33:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-1771885800"),
            gamma_market_for_test("btc-updown-15m-1771886700"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-1771885800");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::InWindow
        );
    }

    #[test]
    fn select_preferred_live_market_uses_nearest_future_when_no_current() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:29:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-1771885800"),
            gamma_market_for_test("btc-updown-15m-1771886700"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-1771885800");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::NearestFuture
        );
    }

    #[test]
    fn select_preferred_live_market_falls_back_to_latest_slug() {
        let now = DateTime::parse_from_rfc3339("2026-02-23T22:33:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let markets = vec![
            gamma_market_for_test("btc-updown-15m-alpha"),
            gamma_market_for_test("btc-updown-15m-beta"),
        ];

        let selected =
            select_preferred_live_market(markets, now).expect("market should be selected");
        assert_eq!(selected.slug, "btc-updown-15m-beta");
        assert_eq!(
            selected.selection_reason,
            LiveMarketSelectionReason::LatestBySlugFallback
        );
    }
    fn make_leg(levels_filled: u32, last_fill_price: Option<f64>) -> DualLegRuntime {
        DualLegRuntime {
            side: LegSide::Yes,
            token_id: "tok".to_string(),
            qty: 10.0,
            avg_entry: 0.50,
            levels_filled,
            last_fill_price,
            last_dca_at: None,
        }
    }

    #[test]
    fn apply_fill_buy_increments_when_flag_true() {
        let mut leg = make_leg(0, None);
        apply_fill_to_leg(&mut leg, "buy", 0.50, 10.0, true);
        assert_eq!(leg.levels_filled, 1);
        assert_eq!(leg.last_fill_price, Some(0.50));
    }

    #[test]
    fn apply_fill_buy_no_increment_when_flag_false() {
        let mut leg = make_leg(2, Some(0.50));
        apply_fill_to_leg(&mut leg, "buy", 0.48, 5.0, false);
        assert_eq!(leg.levels_filled, 2);
        assert_eq!(leg.last_fill_price, Some(0.48));
    }

    #[test]
    fn apply_fill_sell_does_not_increment_levels() {
        let mut leg = make_leg(3, Some(0.60));
        apply_fill_to_leg(&mut leg, "sell", 0.65, 5.0, true);
        assert_eq!(leg.levels_filled, 3);
    }

    #[test]
    fn apply_fill_buy_avg_entry_correct() {
        let mut leg = make_leg(0, None);
        leg.qty = 10.0;
        leg.avg_entry = 0.50;
        apply_fill_to_leg(&mut leg, "buy", 0.40, 10.0, false);
        assert!((leg.avg_entry - 0.45).abs() < 1e-9);
        assert_eq!(leg.qty, 20.0);
    }

    #[test]
    fn dca_cap_simulation_max_3_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        let max = 3u32;
        let steps = [0.50f64, 0.48, 0.46];
        for price in steps {
            assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
            leg.levels_filled += 1;
            leg.last_fill_price = Some(price);
        }
        assert!(!strat.should_dca_leg(0.44, leg.last_fill_price, 0.02, leg.levels_filled, max));
        assert!(!strat.should_dca_leg(0.01, leg.last_fill_price, 0.02, leg.levels_filled, max));
    }

    #[test]
    fn dca_cap_simulation_max_1_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        assert!(strat.should_dca_leg(0.50, leg.last_fill_price, 0.02, leg.levels_filled, 1));
        leg.levels_filled += 1;
        leg.last_fill_price = Some(0.50);
        assert!(!strat.should_dca_leg(0.45, leg.last_fill_price, 0.02, leg.levels_filled, 1));
    }

    #[test]
    fn dca_cap_simulation_max_5_with_leg() {
        use bot_core::DualSideStrategy;
        let strat = bot_core::SymmetricDualDcaStrategy;
        let mut leg = make_leg(0, None);
        let max = 5u32;
        let prices = [0.50f64, 0.48, 0.46, 0.44, 0.42];
        for price in prices {
            assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
            leg.levels_filled += 1;
            leg.last_fill_price = Some(price);
        }
        assert!(!strat.should_dca_leg(0.40, leg.last_fill_price, 0.02, leg.levels_filled, max));
    }
}

fn parse_rfc3339_utc(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|value| value.with_timezone(&Utc))
        .ok()
}

pub(crate) fn dual_dca_timeframe_duration(timeframe: &str) -> ChronoDuration {
    match timeframe.trim().to_ascii_lowercase().as_str() {
        "15m" => ChronoDuration::minutes(15),
        _ => ChronoDuration::minutes(5),
    }
}

async fn process_trade_builder_workflows(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
) -> Result<()> {
    let workflows = repo
        .list_trade_builder_workflows_for_processing(WORKFLOW_PROCESS_LIMIT)
        .await?;
    if workflows.is_empty() {
        return Ok(());
    }

    if let Err(err) = sync_recent_trade_builder_fills(repo, client).await {
        warn!(
            run_id,
            error = %err,
            "TRADE_BUILDER_FILL_SYNC_ERROR"
        );
    }

    let limits = to_risk_limits(cfg);
    let policy = DefaultRiskPolicy;

    for workflow in workflows {
        if let Err(err) = process_trade_builder_workflow(
            repo, run_id, cfg, &limits, &policy, client, ws, &workflow,
        )
        .await
        {
            let _ = repo
                .set_trade_builder_workflow_status(workflow.id, "error", Some(&err.to_string()))
                .await;
            let _ = repo
                .append_trade_builder_workflow_event(
                    workflow.id,
                    None,
                    "processing_error",
                    &json!({ "error": err.to_string() }),
                )
                .await;
            warn!(
                run_id,
                workflow_id = workflow.id,
                error = %err,
                "TRADE_BUILDER_WORKFLOW_ERROR"
            );
        }
    }

    Ok(())
}

async fn process_trade_builder_workflow(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    workflow: &TradeBuilderWorkflow,
) -> Result<()> {
    if let Some(expires_at) = workflow.expires_at {
        if expires_at <= Utc::now() {
            if workflow.status != "expired" {
                repo.set_trade_builder_workflow_status(workflow.id, "expired", None)
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    None,
                    "expired",
                    &json!({ "expires_at": expires_at }),
                )
                .await?;
            }
            return Ok(());
        }
    }

    let legs = repo.load_trade_builder_workflow_legs(workflow.id).await?;
    let mut sell_leg = legs
        .iter()
        .find(|leg| leg.leg_type == "sell")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("workflow missing sell leg"))?;
    let mut buy_leg = legs
        .iter()
        .find(|leg| leg.leg_type == "buy")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("workflow missing buy leg"))?;

    let _ = ensure_sell_leg_order(repo, workflow, &sell_leg).await?;
    refresh_workflow_leg_fill_metrics(repo, &mut sell_leg).await?;
    refresh_workflow_leg_fill_metrics(repo, &mut buy_leg).await?;

    let sell_progress_pct = workflow_leg_progress_pct(&sell_leg);

    if workflow.status == "armed" && sell_progress_pct > 0.0 {
        repo.set_trade_builder_workflow_status(workflow.id, "running", None)
            .await?;
    }

    let mut buy_child_transitioned = false;
    if let Some(active_buy_order_id) = buy_leg.builder_order_id {
        if let Some(active_buy_order) = repo.get_trade_builder_order(active_buy_order_id).await? {
            if active_buy_order.status == "completed" {
                repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    Some(buy_leg.id),
                    "buy_child_completed",
                    &json!({
                        "builder_order_id": active_buy_order.id,
                        "filled_notional_usdc": buy_leg.filled_notional_usdc,
                        "filled_qty": buy_leg.filled_qty
                    }),
                )
                .await?;
                buy_leg.builder_order_id = None;
                buy_child_transitioned = true;
            } else if matches!(
                active_buy_order.status.as_str(),
                "canceled" | "expired" | "blocked" | "error"
            ) {
                repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    Some(buy_leg.id),
                    "buy_child_terminal",
                    &json!({
                        "builder_order_id": active_buy_order.id,
                        "status": active_buy_order.status
                    }),
                )
                .await?;
                buy_leg.builder_order_id = None;
                buy_child_transitioned = true;
            }
        } else {
            repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                .await?;
            buy_leg.builder_order_id = None;
            buy_child_transitioned = true;
        }
    }

    refresh_workflow_leg_fill_metrics(repo, &mut buy_leg).await?;
    if buy_child_transitioned {
        return Ok(());
    }

    let threshold_ok = sell_progress_pct >= workflow.buy_start_after_sell_progress_pct;
    let (price_ok, workflow_current_price, workflow_previous_price) =
        evaluate_workflow_buy_price_condition(repo, ws, client, &buy_leg).await?;
    let should_buy = workflow_should_activate_buy(workflow, threshold_ok, price_ok);

    let desired_buy_notional = (buy_leg.target_notional_usdc * (sell_progress_pct / 100.0))
        .clamp(0.0, buy_leg.target_notional_usdc.max(0.0));
    let mut delta_notional = (desired_buy_notional - buy_leg.filled_notional_usdc).max(0.0);

    if !should_buy {
        let (reason_code, reason_message) = if !threshold_ok && !price_ok {
            (
                "workflow_condition_not_met",
                "Sell progress and price condition are both not met.",
            )
        } else if !threshold_ok {
            (
                "workflow_waiting_sell_progress",
                "Waiting for required sell progress threshold.",
            )
        } else {
            (
                "workflow_waiting_price_condition",
                "Waiting for price trigger condition.",
            )
        };
        info!(
            run_id,
            workflow_id = workflow.id,
            buy_leg_id = buy_leg.id,
            reason_code,
            sell_progress_pct,
            sell_progress_threshold = workflow.buy_start_after_sell_progress_pct,
            price_condition_passed = price_ok,
            current_price = ?workflow_current_price,
            previous_price = ?workflow_previous_price,
            trigger_condition = ?buy_leg.trigger_condition,
            trigger_price = ?buy_leg.trigger_price,
            "TRADE_BUILDER_WORKFLOW_NOT_BUY_DECISION"
        );
        repo.set_trade_builder_workflow_leg_status(buy_leg.id, "waiting_sell_progress")
            .await?;
        repo.append_trade_builder_workflow_event(
            workflow.id,
            Some(buy_leg.id),
            "not_buy_decision",
            &json!({
                "reason_code": reason_code,
                "reason_message": reason_message,
                "mode": workflow.buy_trigger_mode,
                "sell_progress_pct": sell_progress_pct,
                "sell_progress_threshold_pct": workflow.buy_start_after_sell_progress_pct,
                "threshold_ok": threshold_ok,
                "price_ok": price_ok,
                "market_slug": &buy_leg.market_slug,
                "token_id": &buy_leg.token_id,
                "trigger_condition": buy_leg.trigger_condition.as_deref(),
                "trigger_price": buy_leg.trigger_price,
                "previous_price": workflow_previous_price,
                "current_price": workflow_current_price
            }),
        )
        .await?;
        delta_notional = 0.0;
    }

    let can_place_buy_child = buy_leg.builder_order_id.is_none();
    if should_buy
        && can_place_buy_child
        && delta_notional >= WORKFLOW_MIN_BUY_INCREMENT_USDC
        && buy_leg.filled_notional_usdc < buy_leg.target_notional_usdc
    {
        let risk = risk_gate_manual_order(
            repo,
            run_id,
            cfg,
            workflow.source_trade_id,
            delta_notional,
            limits,
            policy,
        )
        .await?;

        if matches!(risk, RiskDecision::Allow) {
            let buy_order_id = repo
                .create_trade_builder_order(
                    workflow.source_trade_id,
                    "immediate",
                    "pending",
                    &buy_leg.market_slug,
                    &buy_leg.token_id,
                    &buy_leg.outcome_label,
                    &buy_leg.side,
                    None,
                    None,
                    delta_notional,
                    buy_leg.min_price_distance_cent,
                    workflow.expires_at,
                    1,
                )
                .await?;
            repo.set_trade_builder_workflow_leg_builder_order(
                buy_leg.id,
                Some(buy_order_id),
                "open",
            )
            .await?;
            repo.set_trade_builder_workflow_status(workflow.id, "running", None)
                .await?;
            repo.append_trade_builder_workflow_event(
                workflow.id,
                Some(buy_leg.id),
                "buy_child_created",
                &json!({
                    "builder_order_id": buy_order_id,
                    "size_usdc": delta_notional,
                    "sell_progress_pct": sell_progress_pct,
                    "mode": workflow.buy_trigger_mode
                }),
            )
            .await?;
        } else {
            repo.set_trade_builder_workflow_leg_status(buy_leg.id, "blocked")
                .await?;
            repo.append_trade_builder_workflow_event(
                workflow.id,
                Some(buy_leg.id),
                "buy_blocked_by_risk",
                &json!({
                    "reason_code": "risk_blocked",
                    "reason_message": "Buy leg order blocked by risk policy.",
                    "decision": format!("{risk:?}"),
                    "size_usdc": delta_notional,
                    "sell_progress_pct": sell_progress_pct
                }),
            )
            .await?;
            warn!(
                run_id,
                workflow_id = workflow.id,
                buy_leg_id = buy_leg.id,
                reason_code = "risk_blocked",
                decision = %format!("{risk:?}"),
                size_usdc = delta_notional,
                "TRADE_BUILDER_WORKFLOW_NOT_BUY_DECISION"
            );
        }
    }

    let is_sell_done = sell_progress_pct >= 99.999;
    let no_active_buy_order = buy_leg.builder_order_id.is_none();
    let is_buy_target_done = buy_leg.filled_notional_usdc + 0.0001 >= buy_leg.target_notional_usdc;

    if is_sell_done && no_active_buy_order && is_buy_target_done {
        repo.set_trade_builder_workflow_status(workflow.id, "completed", None)
            .await?;
        repo.set_trade_builder_workflow_leg_status(sell_leg.id, "completed")
            .await?;
        repo.set_trade_builder_workflow_leg_status(buy_leg.id, "completed")
            .await?;
        repo.append_trade_builder_workflow_event(
            workflow.id,
            None,
            "completed",
            &json!({
                "sell_progress_pct": sell_progress_pct,
                "buy_filled_usdc": buy_leg.filled_notional_usdc,
                "buy_target_usdc": buy_leg.target_notional_usdc
            }),
        )
        .await?;
    }

    Ok(())
}

async fn ensure_sell_leg_order(
    repo: &PostgresRepository,
    workflow: &TradeBuilderWorkflow,
    sell_leg: &TradeBuilderWorkflowLeg,
) -> Result<Option<TradeBuilderOrder>> {
    if let Some(builder_order_id) = sell_leg.builder_order_id {
        if let Some(existing) = repo.get_trade_builder_order(builder_order_id).await? {
            return Ok(Some(existing));
        }
        repo.set_trade_builder_workflow_leg_builder_order(sell_leg.id, None, "pending")
            .await?;
    }

    anyhow::ensure!(
        sell_leg.target_notional_usdc > 0.0,
        "sell leg target_notional_usdc must be > 0"
    );

    let kind = if sell_leg.trigger_condition.is_some() && sell_leg.trigger_price.is_some() {
        "conditional"
    } else {
        "immediate"
    };

    let sell_order_id = repo
        .create_trade_builder_order(
            workflow.source_trade_id,
            kind,
            "pending",
            &sell_leg.market_slug,
            &sell_leg.token_id,
            &sell_leg.outcome_label,
            &sell_leg.side,
            sell_leg.trigger_condition.as_deref(),
            sell_leg.trigger_price,
            sell_leg.target_notional_usdc,
            sell_leg.min_price_distance_cent,
            workflow.expires_at,
            1,
        )
        .await?;
    repo.set_trade_builder_workflow_leg_builder_order(sell_leg.id, Some(sell_order_id), "open")
        .await?;
    repo.append_trade_builder_workflow_event(
        workflow.id,
        Some(sell_leg.id),
        "sell_child_created",
        &json!({
            "builder_order_id": sell_order_id,
            "kind": kind,
            "target_notional_usdc": sell_leg.target_notional_usdc
        }),
    )
    .await?;

    repo.get_trade_builder_order(sell_order_id).await
}

fn workflow_leg_progress_pct(leg: &TradeBuilderWorkflowLeg) -> f64 {
    if leg.target_notional_usdc <= 0.0 {
        return 0.0;
    }

    ((leg.filled_notional_usdc.max(0.0) / leg.target_notional_usdc) * 100.0).clamp(0.0, 100.0)
}

fn workflow_should_activate_buy(
    workflow: &TradeBuilderWorkflow,
    threshold_ok: bool,
    price_ok: bool,
) -> bool {
    match workflow.buy_trigger_mode.as_str() {
        "sell_progress_only" => threshold_ok,
        "price_only" => price_ok,
        "sell_progress_and_price" => threshold_ok && price_ok,
        _ => threshold_ok && price_ok,
    }
}

async fn evaluate_workflow_buy_price_condition(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    buy_leg: &TradeBuilderWorkflowLeg,
) -> Result<(bool, Option<f64>, Option<f64>)> {
    let Some(trigger_price) = buy_leg.trigger_price else {
        return Ok((true, None, buy_leg.last_seen_price));
    };
    let Some(trigger_condition) = buy_leg.trigger_condition.as_deref() else {
        return Ok((true, None, buy_leg.last_seen_price));
    };

    let current_price =
        fetch_current_token_price_for_leg(ws, client, &buy_leg.market_slug, &buy_leg.token_id)
            .await?;
    repo.set_trade_builder_workflow_leg_last_seen_price(buy_leg.id, current_price)
        .await?;

    let previous_price = buy_leg.last_seen_price;
    let pass = match trigger_condition {
        "cross_above" => previous_price
            .map(|prev| prev < trigger_price && current_price >= trigger_price)
            .unwrap_or(current_price >= trigger_price),
        "cross_below" => previous_price
            .map(|prev| prev > trigger_price && current_price <= trigger_price)
            .unwrap_or(current_price <= trigger_price),
        _ => true,
    };
    Ok((pass, Some(current_price), previous_price))
}

async fn fetch_current_token_price_for_leg(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    _market_slug: &str,
    token_id: &str,
) -> Result<f64> {
    if let Some(ws_price) = fetch_price_from_market_ws(ws, token_id).await {
        return Ok(clamp_probability(ws_price));
    }
    let fallback = client.midpoint(token_id).await?;
    Ok(clamp_probability(fallback.price))
}

async fn refresh_workflow_leg_fill_metrics(
    repo: &PostgresRepository,
    leg: &mut TradeBuilderWorkflowLeg,
) -> Result<()> {
    let (filled_notional_usdc, filled_qty) = repo
        .aggregate_trade_builder_workflow_leg_fills(leg.id)
        .await?;

    if (leg.filled_notional_usdc - filled_notional_usdc).abs() > 0.0001
        || (leg.filled_qty - filled_qty).abs() > 0.000001
    {
        repo.set_trade_builder_workflow_leg_filled_metrics(
            leg.id,
            filled_notional_usdc,
            filled_qty,
        )
        .await?;
    }

    leg.filled_notional_usdc = filled_notional_usdc;
    leg.filled_qty = filled_qty;
    Ok(())
}

pub(crate) async fn sync_recent_trade_builder_fills(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
) -> Result<usize> {
    let fills = client.list_fills(None).await?;
    let mut synced = 0usize;

    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            continue;
        }
        let Some(internal_order_id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        else {
            continue;
        };

        let raw_fill = json!({
            "fill_id": fill.fill_id,
            "order_id": fill.order_id,
            "price": fill.price,
            "size": fill.size,
            "fee": fill.fee,
            "timestamp": fill.ts
        });

        repo.upsert_fill_by_exchange_fill_id(
            internal_order_id,
            &fill.fill_id,
            fill.price,
            fill.size,
            fill.fee.unwrap_or_default(),
            fill.ts,
            &raw_fill,
        )
        .await?;

        synced = synced.saturating_add(1);
    }

    Ok(synced)
}

async fn process_trade_builder_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
) -> Result<()> {
    if order.triggers_fired >= order.max_triggers && order.status != "completed" {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "max_trigger_reached",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers
            }),
        )
        .await?;
        if let Ok(Some(unblocked_id)) = repo
            .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
            .await
        {
            info!(
                builder_order_id = order.id,
                unblocked_order_id = unblocked_id,
                "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
            );
        }
        return Ok(());
    }

    if let Some(expires_at) = order.expires_at {
        if expires_at <= Utc::now() {
            if order.status != "expired" && order.status != "completed" {
                repo.clear_trade_builder_active_exchange_order(order.id, "expired")
                    .await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "expired",
                    &json!({ "expires_at": expires_at }),
                )
                .await?;
            }
            return Ok(());
        }
    }

    let current_price = fetch_current_token_price(ws, client, order).await?;
    repo.set_trade_builder_last_seen_price(order.id, current_price)
        .await?;

    if let Some(exchange_order_id) = order.active_exchange_order_id.as_deref() {
        reconcile_trade_builder_open_order(
            repo,
            run_id,
            client,
            order,
            exchange_order_id,
            current_price,
        )
        .await?;
        return Ok(());
    }

    if order.status == "canceled_requested" {
        repo.set_trade_builder_order_status(order.id, "canceled", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "canceled_without_open_order",
            &json!({ "reason": "user_request" }),
        )
        .await?;
        return Ok(());
    }

    let should_trigger = should_trigger_builder_order(order, current_price);
    if !should_trigger {
        if order.kind == "conditional" {
            info!(
                run_id,
                builder_order_id = order.id,
                market = %order.market_slug,
                token_id = %order.token_id,
                trigger_condition = ?order.trigger_condition,
                trigger_price = ?order.trigger_price,
                previous_price = ?order.last_seen_price,
                current_price,
                order_status = %order.status,
                reason_code = "trigger_not_crossed",
                "TRADE_BUILDER_NOT_BUY_DECISION"
            );
        }
        if order.kind == "conditional" && order.status == "pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "not_buy_decision",
                &json!({
                    "reason_code": "trigger_not_crossed",
                    "reason_message": "Trigger condition has not crossed yet.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": order.last_seen_price,
                    "current_price": current_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
        }
        return Ok(());
    }
    info!(
        run_id,
        builder_order_id = order.id,
        market = %order.market_slug,
        token_id = %order.token_id,
        trigger_condition = ?order.trigger_condition,
        trigger_price = ?order.trigger_price,
        previous_price = ?order.last_seen_price,
        current_price,
        order_status = %order.status,
        "TRADE_BUILDER_TRIGGER_CONDITION_MET"
    );

    let (
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        exhausted_trigger_sizes,
        trigger_size_index,
    ) = resolve_trade_builder_next_trigger_size_usdc(repo, order).await?;
    if exhausted_trigger_sizes {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "trigger_size_exhausted",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers,
                "next_trigger_index": trigger_size_index + 1
            }),
        )
        .await?;
        return Ok(());
    }

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        order.trade_id,
        resolved_size_usdc,
        limits,
        policy,
    )
    .await?;
    if !matches!(risk, RiskDecision::Allow) {
        repo.set_trade_builder_order_status(order.id, "blocked", Some("risk_block"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "blocked_by_risk",
            &json!({
                "reason_code": "risk_blocked",
                "reason_message": "Order blocked by risk policy.",
                "decision": format!("{risk:?}"),
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "current_price": current_price
            }),
        )
        .await?;
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            reason_code = "risk_blocked",
            decision = %format!("{risk:?}"),
            current_price,
            "TRADE_BUILDER_NOT_BUY_DECISION"
        );
        return Ok(());
    }

    let desired_price =
        aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent);
    let remaining_usdc = order.remaining_size.unwrap_or(resolved_size_usdc);
    let size = calc_level_size(remaining_usdc, desired_price);
    anyhow::ensure!(size > 0.0, "computed builder order size is zero");

    let intent = if order.kind == "immediate" {
        "manual_immediate"
    } else {
        "manual_trigger"
    };
    let client_order_id = format!("tb-{}", Uuid::new_v4());
    let req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size,
        intent: intent.to_string(),
        client_order_id: client_order_id.clone(),
        leg_side: None,
        fee_rate_bps: 1000,
    };

    let ack = client.place(&req).await?;
    let exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let normalized_status = normalize_exchange_status(&ack.status);
    let raw = json!({
        "builder_order_id": order.id,
        "client_order_id": ack.client_order_id,
        "exchange_order_id": exchange_order_id,
        "status": ack.status,
        "normalized_status": normalized_status,
        "trigger_price": order.trigger_price,
        "current_price": current_price,
        "execution_price": desired_price,
        "size": size,
        "size_mode": trigger_size_mode,
        "trigger_size_value": trigger_size_value,
        "trigger_size_index": trigger_size_index + 1,
        "resolved_size_usdc": resolved_size_usdc,
        "remaining_usdc": remaining_usdc,
        "reject_reason": ack.reject_reason,
        "raw_status": ack.raw_status,
        "exchange_ts": ack.exchange_ts
    });

    repo.upsert_order_by_exchange_id(
        order.trade_id,
        &exchange_order_id,
        Some(&client_order_id),
        intent,
        &order.side,
        desired_price,
        size,
        normalized_status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
    )
    .await?;
    repo.set_trade_builder_order_working_state(
        order.id,
        Some(&exchange_order_id),
        Some(desired_price),
        Some(remaining_usdc),
        normalized_status,
    )
    .await?;
    repo.append_trade_builder_order_event(order.id, "submitted", &raw)
        .await?;

    if normalized_status == "filled" {
        finalize_builder_fill(repo, order, &exchange_order_id, size, desired_price).await?;
    }

    Ok(())
}

async fn resolve_trade_builder_next_trigger_size_usdc(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<(f64, Option<String>, Option<f64>, bool, usize)> {
    let next_index = order.triggers_fired.max(0) as usize;
    let Some((size_mode, trigger_sizes)) =
        repo.load_trade_builder_order_trigger_plan(order.id).await?
    else {
        return Ok((order.size_usdc, None, None, false, next_index));
    };
    if trigger_sizes.is_empty() {
        return Ok((order.size_usdc, size_mode, None, false, next_index));
    }
    if next_index >= trigger_sizes.len() {
        return Ok((0.0, size_mode, None, true, next_index));
    }

    let trigger_size_value = trigger_sizes[next_index];
    let normalized_mode = size_mode.unwrap_or_else(|| "usdc".to_string());
    let resolved_size_usdc = if normalized_mode == "pct" {
        let source_notional = repo
            .trade_notional_usdc(order.trade_id)
            .await?
            .unwrap_or(0.0);
        anyhow::ensure!(
            source_notional > 0.0,
            "trade_builder pct trigger size requires source trade notional > 0"
        );
        source_notional * (trigger_size_value / 100.0)
    } else {
        trigger_size_value
    };
    anyhow::ensure!(
        resolved_size_usdc > 0.0 && resolved_size_usdc.is_finite(),
        "trade_builder resolved trigger size must be > 0"
    );

    Ok((
        resolved_size_usdc,
        Some(normalized_mode),
        Some(trigger_size_value),
        false,
        next_index,
    ))
}

async fn reconcile_trade_builder_open_order(
    repo: &PostgresRepository,
    run_id: i64,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    current_price: f64,
) -> Result<()> {
    if order.status == "canceled_requested" {
        client.cancel(exchange_order_id).await?;
        repo.mark_order_status(exchange_order_id, "canceled")
            .await?;
        repo.clear_trade_builder_active_exchange_order(order.id, "canceled")
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "cancel_requested",
            &json!({ "exchange_order_id": exchange_order_id }),
        )
        .await?;
        return Ok(());
    }

    let order_info = client.status(exchange_order_id).await?;
    let normalized = normalize_exchange_status(&order_info.status);
    repo.mark_order_status(exchange_order_id, normalized)
        .await?;

    if normalized == "filled" {
        let filled_size = order_info
            .filled_size
            .or(order_info.size)
            .unwrap_or_default();
        let price = order_info.price.unwrap_or(current_price);
        finalize_builder_fill(repo, order, exchange_order_id, filled_size, price).await?;
        return Ok(());
    }

    if matches!(normalized, "canceled" | "rejected" | "expired") {
        let next_status =
            if order.kind == "conditional" && order.triggers_fired < order.max_triggers {
                "armed"
            } else {
                "completed"
            };
        repo.clear_trade_builder_active_exchange_order(order.id, next_status)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "terminal_exchange_status",
            &json!({
                "exchange_order_id": exchange_order_id,
                "status": normalized
            }),
        )
        .await?;
        return Ok(());
    }

    let remaining_usdc = estimate_remaining_usdc(order, &order_info, current_price);
    let desired_price =
        aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent);
    let price_distance = min_price_distance_to_probability(order.min_price_distance_cent);
    let should_reprice = order.working_price.map_or(true, |working_price| {
        (working_price - desired_price).abs() >= price_distance
    });

    if !should_reprice {
        repo.set_trade_builder_order_working_state(
            order.id,
            Some(exchange_order_id),
            order.working_price,
            Some(remaining_usdc),
            normalized,
        )
        .await?;
        return Ok(());
    }

    let size = calc_level_size(remaining_usdc, desired_price);
    if size <= 0.0 {
        repo.clear_trade_builder_active_exchange_order(order.id, "completed")
            .await?;
        return Ok(());
    }

    let replace_req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size,
        intent: "manual_reprice".to_string(),
        client_order_id: format!("tb-reprice-{}", Uuid::new_v4()),
        leg_side: None,
        fee_rate_bps: 1000,
    };

    let ack = client.replace(exchange_order_id, &replace_req).await?;
    repo.mark_order_status(exchange_order_id, "canceled")
        .await?;

    let new_exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let normalized_status = normalize_exchange_status(&ack.status);
    let raw = json!({
        "prev_exchange_order_id": exchange_order_id,
        "new_exchange_order_id": new_exchange_order_id,
        "status": ack.status,
        "normalized_status": normalized_status,
        "target_price": desired_price,
        "remaining_usdc": remaining_usdc,
        "size": size
    });

    repo.upsert_order_by_exchange_id(
        order.trade_id,
        &new_exchange_order_id,
        Some(&ack.client_order_id),
        "manual_reprice",
        &order.side,
        desired_price,
        size,
        normalized_status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
    )
    .await?;
    repo.set_trade_builder_order_working_state(
        order.id,
        Some(&new_exchange_order_id),
        Some(desired_price),
        Some(remaining_usdc),
        normalized_status,
    )
    .await?;
    repo.append_trade_builder_order_event(order.id, "reprice", &raw)
        .await?;
    info!(
        run_id,
        builder_order_id = order.id,
        old_exchange_order_id = exchange_order_id,
        new_exchange_order_id = %new_exchange_order_id,
        "TRADE_BUILDER_ORDER_REPRICED"
    );

    if normalized_status == "filled" {
        finalize_builder_fill(repo, order, &new_exchange_order_id, size, desired_price).await?;
    }

    Ok(())
}

async fn finalize_builder_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    filled_size: f64,
    execution_price: f64,
) -> Result<()> {
    repo.increment_trade_builder_trigger_count(order.id).await?;

    let next_trigger_count = order.triggers_fired + 1;
    let reached_limit = next_trigger_count >= order.max_triggers;
    let next_status = if order.kind == "immediate" || reached_limit {
        "completed"
    } else {
        "armed"
    };

    repo.clear_trade_builder_active_exchange_order(order.id, next_status)
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "filled",
        &json!({
            "exchange_order_id": exchange_order_id,
            "filled_size": filled_size,
            "execution_price": execution_price,
            "triggers_fired": next_trigger_count,
            "max_triggers": order.max_triggers,
            "next_status": next_status
        }),
    )
    .await?;

    // Unblock next DCA level for the same trade + token
    if let Ok(Some(unblocked_id)) = repo
        .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
        .await
    {
        info!(
            builder_order_id = order.id,
            unblocked_order_id = unblocked_id,
            trade_id = order.trade_id,
            "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
        );
    }

    Ok(())
}

fn should_trigger_builder_order(order: &TradeBuilderOrder, current_price: f64) -> bool {
    if order.kind == "immediate" {
        return matches!(
            order.status.as_str(),
            "pending" | "armed" | "triggered" | "blocked"
        );
    }

    let Some(trigger_price) = order.trigger_price else {
        return false;
    };
    let Some(trigger_condition) = order.trigger_condition.as_deref() else {
        return false;
    };

    let previous_price = order.last_seen_price;
    match trigger_condition {
        "cross_above" if order.status == "blocked" => current_price >= trigger_price,
        "cross_below" if order.status == "blocked" => current_price <= trigger_price,
        "cross_above" => previous_price
            .map(|prev| prev < trigger_price && current_price >= trigger_price)
            .unwrap_or(current_price >= trigger_price),
        "cross_below" => previous_price
            .map(|prev| prev > trigger_price && current_price <= trigger_price)
            .unwrap_or(current_price <= trigger_price),
        _ => false,
    }
}

async fn fetch_current_token_price(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
) -> Result<f64> {
    if let Some(ws_price) = fetch_price_from_market_ws(ws, &order.token_id).await {
        return Ok(clamp_probability(ws_price));
    }
    let fallback = client.midpoint(&order.token_id).await?;
    Ok(clamp_probability(fallback.price))
}

pub(crate) async fn fetch_price_from_market_ws(ws: &ClobWsClient, token_id: &str) -> Option<f64> {
    let events = ws
        .subscribe_once(WsChannel::Market, &[token_id.to_string()])
        .await
        .ok()?;
    extract_price_from_market_events(&events, token_id).map(|(price, _)| price)
}

fn extract_price_from_market_events(
    events: &[WsEvent],
    token_id: &str,
) -> Option<(f64, Option<i64>)> {
    for event in events.iter().rev() {
        if let Some(price) = event.price {
            return Some((price, event.ts));
        }

        if let Some(price) = parse_json_number(event.payload.get("price")) {
            return Some((price, event.ts));
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                if let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) {
                    if asset_id != token_id {
                        continue;
                    }
                }
                if let Some(price) = parse_json_number(change.get("price")) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some((price, ts));
                }
            }
        }

        let bid = parse_json_number(event.payload.get("best_bid"));
        let ask = parse_json_number(event.payload.get("best_ask"));
        if let (Some(bid), Some(ask)) = (bid, ask) {
            return Some((((bid + ask) / 2.0), event.ts));
        }
    }

    None
}

fn parse_json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value {
        Some(serde_json::Value::Number(v)) => v.as_f64(),
        Some(serde_json::Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn normalize_exchange_status(status: &str) -> &'static str {
    let normalized = status.to_lowercase();
    if normalized.contains("partial") {
        return "partially_filled";
    }
    if normalized.contains("fill") {
        return "filled";
    }
    if normalized.contains("cancel") {
        return "canceled";
    }
    if normalized.contains("reject") {
        return "rejected";
    }
    if normalized.contains("expir") {
        return "expired";
    }
    if normalized.contains("open") || normalized.contains("live") || normalized.contains("book") {
        return "open";
    }
    "open"
}

pub(crate) fn min_price_distance_to_probability(min_price_distance_cent: f64) -> f64 {
    (min_price_distance_cent / 100.0).max(0.0001)
}

pub(crate) fn aggressive_price_for_side(side: &str, current_price: f64, min_price_distance_cent: f64) -> f64 {
    let distance = min_price_distance_to_probability(min_price_distance_cent);
    if side == "sell" {
        return clamp_probability(current_price - distance);
    }
    clamp_probability(current_price + distance)
}

fn estimate_remaining_usdc(
    order: &TradeBuilderOrder,
    order_info: &OrderInfo,
    fallback_price: f64,
) -> f64 {
    if let (Some(order_size), Some(filled_size)) = (order_info.size, order_info.filled_size) {
        let remaining_size = (order_size - filled_size).max(0.0);
        let price = order_info
            .price
            .or(order.working_price)
            .unwrap_or(fallback_price);
        return (remaining_size * price).max(0.0);
    }
    order.remaining_size.unwrap_or(order.size_usdc).max(0.0)
}

pub(crate) async fn risk_gate_manual_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade_id: i64,
    proposed_notional_usdc: f64,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let open_orders = repo.open_order_count().await?;
    let daily_pnl = repo.daily_realized_pnl().await?;
    let consec_losses = repo
        .consecutive_losses(cfg.risk.max_consecutive_losses as i64)
        .await?;

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc,
            open_orders,
            stale_data_ms: 0,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    StateRepository::record_risk_event(
        repo,
        Some(trade_id),
        "risk_check_manual_order",
        &format!("{:?}", risk.decision).to_lowercase(),
        risk.reason,
    )
    .await?;

    if !matches!(risk.decision, RiskDecision::Allow) {
        warn!(
            run_id,
            trade_id,
            reason = risk.reason,
            decision = ?risk.decision,
            "RISK_MANUAL_ORDER_BLOCKED"
        );
    }

    Ok(risk.decision)
}

async fn risk_gate_dual(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade_id: i64,
    limits: &RiskLimits,
    stale_data_ms: u64,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let open_orders = repo.open_order_count().await?;
    let daily_pnl = repo.daily_realized_pnl().await?;
    let consec_losses = repo
        .consecutive_losses(cfg.risk.max_consecutive_losses as i64)
        .await?;

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc: cfg.strategy.total_notional_usdc,
            open_orders,
            stale_data_ms,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    StateRepository::record_risk_event(
        repo,
        Some(trade_id),
        "risk_check_dual",
        &format!("{:?}", risk.decision).to_lowercase(),
        risk.reason,
    )
    .await?;

    match risk.decision {
        RiskDecision::Halt => {
            warn!(run_id, trade_id, reason = risk.reason, "RISK_HALT_DUAL");
            Ok(RiskDecision::Halt)
        }
        RiskDecision::Block => {
            warn!(run_id, trade_id, reason = risk.reason, "RISK_BLOCK_DUAL");
            Ok(RiskDecision::Block)
        }
        RiskDecision::Allow => Ok(RiskDecision::Allow),
    }
}

async fn process_trade_step(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade: &mut TradeRuntime,
    current_price: f64,
    auto_fill: bool,
    strategy: &impl Strategy,
) -> Result<()> {
    if trade.state == TradeState::WaitingEntry
        && strategy.entry_signal(current_price, trade.entry_price)
    {
        transition(repo, trade, TradeState::EntryPlaced, "entry-threshold-hit").await?;
        let entry_status = if auto_fill { "filled" } else { "open" };
        let entry_order_id = repo
            .append_order_event(
                trade.trade_id,
                "entry",
                "buy",
                trade.entry_price,
                trade.position_size,
                entry_status,
            )
            .await?;

        if auto_fill {
            repo.append_fill_event(entry_order_id, trade.entry_price, trade.position_size, 0.0)
                .await?;
            transition(repo, trade, TradeState::EntryFilled, "entry-filled").await?;
            transition(repo, trade, TradeState::TpPlaced, "tp-placed").await?;
            info!(
                run_id,
                trade_id = trade.trade_id,
                tp_price = trade.tp_price,
                "ENTRY_FILLED"
            );
        }
    }

    if trade.state == TradeState::TpPlaced {
        if current_price >= trade.tp_price {
            let key = format!("trade:{}:tp:{}", trade.trade_id, trade.tp_price);
            if repo.try_record_idempotency_key(&key).await? {
                let exit_order_id = repo
                    .append_order_event(
                        trade.trade_id,
                        "tp",
                        "sell",
                        trade.tp_price,
                        trade.position_size,
                        "filled",
                    )
                    .await?;
                repo.append_fill_event(exit_order_id, trade.tp_price, trade.position_size, 0.0)
                    .await?;
                transition(repo, trade, TradeState::ExitFilled, "tp-hit").await?;
                let pnl = (trade.tp_price - trade.entry_price) * trade.position_size;
                repo.close_trade(trade.trade_id, trade.tp_price, pnl)
                    .await?;
                info!(run_id, trade_id = trade.trade_id, pnl, "TRADE_COMPLETED");
            }
            return Ok(());
        }

        let aggressive_sl =
            strategy.aggressive_stop_price(trade.entry_price, cfg.strategy.aggressive_sl_pct);
        if current_price <= aggressive_sl {
            let key = format!("trade:{}:sl:{:.4}", trade.trade_id, aggressive_sl);
            if repo.try_record_idempotency_key(&key).await? {
                transition(repo, trade, TradeState::SlArmed, "aggressive-sl-armed").await?;
                let sl_order_id = repo
                    .append_order_event(
                        trade.trade_id,
                        "sl",
                        "sell",
                        aggressive_sl,
                        trade.position_size,
                        "filled",
                    )
                    .await?;
                repo.append_fill_event(sl_order_id, aggressive_sl, trade.position_size, 0.0)
                    .await?;
                transition(repo, trade, TradeState::ExitFilled, "sl-hit").await?;
                let pnl = (aggressive_sl - trade.entry_price) * trade.position_size;
                repo.close_trade(trade.trade_id, aggressive_sl, pnl).await?;
                warn!(
                    run_id,
                    trade_id = trade.trade_id,
                    pnl,
                    "TRADE_COMPLETED_WITH_SL"
                );
            }
        }
    }

    Ok(())
}

async fn reconcile_live(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &mut TradeRuntime,
    client: &dyn OrderExecutor,
    user_events: &[WsEvent],
) -> Result<ReconcileOutcome> {
    let open_orders = match client.list_open(Some(&trade.market_slug)).await {
        Ok(v) => v,
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &e.to_string(),
            )
            .await?;
            return Ok(ReconcileOutcome::Error);
        }
    };
    let fills = match client.list_fills(None).await {
        Ok(v) => v,
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &e.to_string(),
            )
            .await?;
            return Ok(ReconcileOutcome::Error);
        }
    };

    apply_order_reconcile(repo, trade.trade_id, &open_orders).await?;
    let (applied_fills, duplicate_fills, skipped_fills) =
        apply_fill_reconcile(repo, trade.trade_id, &fills).await?;
    let user_applied = apply_user_stream_events(repo, trade.trade_id, user_events).await?;

    if matches!(
        trade.state,
        TradeState::EntryPlaced | TradeState::EntryPartiallyFilled
    ) && (applied_fills > 0 || user_applied > 0)
    {
        transition(
            repo,
            trade,
            TradeState::EntryFilled,
            "reconcile-entry-filled",
        )
        .await?;
        transition(repo, trade, TradeState::TpPlaced, "reconcile-tp-placed").await?;
    }

    let status = if skipped_fills > 0 { "warning" } else { "ok" };
    repo.record_reconcile_run(
        run_id,
        &trade.market_slug,
        status,
        &json!({
            "open_orders_count": open_orders.len(),
            "fills_count": fills.len(),
            "applied_fills": applied_fills,
            "duplicate_fills": duplicate_fills,
            "skipped_fills": skipped_fills,
            "user_events_applied": user_applied
        })
        .to_string(),
    )
    .await?;

    Ok(if skipped_fills > 0 {
        ReconcileOutcome::Warning
    } else {
        ReconcileOutcome::Ok
    })
}

async fn apply_order_reconcile(
    repo: &PostgresRepository,
    trade_id: i64,
    open_orders: &[OrderInfo],
) -> Result<()> {
    for order in open_orders {
        if order.order_id.is_empty() {
            continue;
        }
        let raw = json!({
            "order_id": order.order_id,
            "client_order_id": order.client_order_id,
            "status": order.status,
            "price": order.price,
            "size": order.size,
            "filled_size": order.filled_size
        });
        repo.upsert_order_by_exchange_id(
            trade_id,
            &order.order_id,
            order.client_order_id.as_deref(),
            "reconcile",
            "unknown",
            order.price.unwrap_or_default(),
            order.size.unwrap_or_default(),
            &order.status,
            None,
            None,
            &raw,
        )
        .await?;
    }
    Ok(())
}

async fn apply_fill_reconcile(
    repo: &PostgresRepository,
    trade_id: i64,
    fills: &[FillInfo],
) -> Result<(usize, usize, usize)> {
    let mut applied = 0usize;
    let mut duplicates = 0usize;
    let mut skipped = 0usize;

    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            skipped += 1;
            continue;
        }
        let key = format!("fill:{}", fill.fill_id);
        if !repo.try_record_idempotency_key(&key).await? {
            duplicates += 1;
            continue;
        }
        let internal_order_id = if let Some(id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        {
            id
        } else {
            let raw_order = json!({"from_fill": true, "exchange_order_id": fill.order_id, "trade_id": trade_id});
            repo.upsert_order_by_exchange_id(
                trade_id,
                &fill.order_id,
                None,
                "reconcile",
                "unknown",
                fill.price,
                fill.size,
                "filled",
                fill.ts,
                None,
                &raw_order,
            )
            .await?
        };
        let raw_fill = json!({
            "fill_id": fill.fill_id,
            "order_id": fill.order_id,
            "price": fill.price,
            "size": fill.size,
            "fee": fill.fee,
            "timestamp": fill.ts
        });
        repo.upsert_fill_by_exchange_fill_id(
            internal_order_id,
            &fill.fill_id,
            fill.price,
            fill.size,
            fill.fee.unwrap_or_default(),
            fill.ts,
            &raw_fill,
        )
        .await?;
        applied += 1;
    }
    Ok((applied, duplicates, skipped))
}

async fn apply_user_stream_events(
    repo: &PostgresRepository,
    trade_id: i64,
    events: &[WsEvent],
) -> Result<usize> {
    let mut applied = 0usize;
    for event in events {
        if !matches!(event.channel, WsChannel::User) {
            continue;
        }
        if !matches!(event.event_type, WsEventType::Fill | WsEventType::Order) {
            continue;
        }
        let key = format!(
            "ws:{}:{:?}",
            event
                .fill_id
                .as_deref()
                .or(event.order_id.as_deref())
                .unwrap_or("na"),
            event.event_type
        );
        if !repo.try_record_idempotency_key(&key).await? {
            continue;
        }
        repo.record_risk_event(
            Some(trade_id),
            "user_stream_event",
            "allow",
            &event.payload.to_string(),
        )
        .await?;
        applied += 1;
    }
    Ok(applied)
}

async fn record_reconcile_error(
    repo: &PostgresRepository,
    run_id: i64,
    market_slug: &str,
    kind: ReconcileErrorKind,
    details: &str,
) -> Result<()> {
    repo.record_reconcile_run(
        run_id,
        market_slug,
        "error",
        &json!({
            "kind": format!("{kind:?}"),
            "details": details
        })
        .to_string(),
    )
    .await
}

async fn halt_trade(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &mut TradeRuntime,
    reason: &str,
    order_executor: Option<&dyn OrderExecutor>,
) -> Result<()> {
    if let Some(executor) = order_executor {
        enforce_halt_open_order_safety(repo, run_id, trade, executor).await?;
    }
    transition(repo, trade, TradeState::Halted, reason).await
}

async fn enforce_halt_open_order_safety(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &TradeRuntime,
    order_executor: &dyn OrderExecutor,
) -> Result<()> {
    let db_open_order_ids: BTreeSet<String> = repo
        .open_exchange_order_ids_for_trade(trade.trade_id)
        .await?
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect();
    let mut open_order_ids: BTreeSet<String> = BTreeSet::new();

    match order_executor.list_open(Some(&trade.market_slug)).await {
        Ok(open_orders) => {
            for order in open_orders {
                if !order.order_id.is_empty() {
                    open_order_ids.insert(order.order_id);
                }
            }
            if open_order_ids.is_empty() {
                open_order_ids.extend(db_open_order_ids.iter().cloned());
            }
        }
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &format!("halt-cancel list_open failed: {e}"),
            )
            .await?;
            open_order_ids.extend(db_open_order_ids.iter().cloned());
        }
    }

    let mut canceled = 0usize;
    let mut failed = 0usize;
    for exchange_order_id in &open_order_ids {
        match order_executor.cancel(exchange_order_id).await {
            Ok(()) => {
                repo.mark_order_status(exchange_order_id, "canceled")
                    .await?;
                repo.record_risk_event(
                    Some(trade.trade_id),
                    "halt_cancel_order",
                    "allow",
                    &format!("exchange_order_id={exchange_order_id}"),
                )
                .await?;
                canceled += 1;
            }
            Err(e) => {
                record_reconcile_error(
                    repo,
                    run_id,
                    &trade.market_slug,
                    ReconcileErrorKind::Network,
                    &format!("halt-cancel failed order_id={exchange_order_id} err={e}"),
                )
                .await?;
                repo.record_risk_event(
                    Some(trade.trade_id),
                    "halt_cancel_order",
                    "block",
                    &format!("exchange_order_id={exchange_order_id} err={e}"),
                )
                .await?;
                failed += 1;
            }
        }
    }

    repo.record_reconcile_run(
        run_id,
        &trade.market_slug,
        if failed > 0 { "error" } else { "ok" },
        &json!({
            "reason": "halt_open_order_safety",
            "trade_id": trade.trade_id,
            "candidates": open_order_ids.len(),
            "canceled": canceled,
            "failed": failed
        })
        .to_string(),
    )
    .await?;

    info!(
        run_id,
        trade_id = trade.trade_id,
        candidates = open_order_ids.len(),
        canceled,
        failed,
        "HALT_OPEN_ORDER_SAFETY_COMPLETED"
    );
    Ok(())
}

async fn create_runtime(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    market_id: i64,
    market_slug: String,
    strategy: &impl Strategy,
) -> Result<TradeRuntime> {
    let trade_id = repo
        .create_trade_stub(
            market_id,
            cfg.strategy.entry_price,
            cfg.risk.max_notional_per_market_usdc,
        )
        .await?;

    let size =
        (cfg.risk.max_notional_per_market_usdc / cfg.strategy.entry_price * 100.0).round() / 100.0;
    Ok(TradeRuntime {
        trade_id,
        market_slug,
        entry_price: cfg.strategy.entry_price,
        tp_price: strategy.take_profit_price(cfg.strategy.entry_price, cfg.strategy.tp_pct),
        position_size: size,
        state: TradeState::Idle,
    })
}

pub(crate) fn to_risk_limits(cfg: &AppConfig) -> RiskLimits {
    RiskLimits {
        max_daily_loss_usdc: cfg.risk.max_daily_loss_usdc,
        max_consecutive_losses: cfg.risk.max_consecutive_losses,
        max_notional_per_market_usdc: cfg.risk.max_notional_per_market_usdc,
        max_open_orders: cfg.risk.max_open_orders,
        max_stale_data_ms: cfg.risk.max_stale_data_ms,
        kill_switch_mode: cfg.risk.kill_switch_mode,
    }
}

async fn risk_gate(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade: &TradeRuntime,
    limits: &RiskLimits,
    stale_data_ms: u64,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let open_orders = repo.open_order_count().await?;
    let daily_pnl = repo.daily_realized_pnl().await?;
    let consec_losses = repo
        .consecutive_losses(cfg.risk.max_consecutive_losses as i64)
        .await?;

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc: cfg.risk.max_notional_per_market_usdc,
            open_orders,
            stale_data_ms,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    StateRepository::record_risk_event(
        repo,
        Some(trade.trade_id),
        "risk_check",
        &format!("{:?}", risk.decision).to_lowercase(),
        risk.reason,
    )
    .await?;

    match risk.decision {
        RiskDecision::Halt => {
            warn!(
                run_id,
                trade_id = trade.trade_id,
                reason = risk.reason,
                "RISK_HALT"
            );
            return Ok(RiskDecision::Halt);
        }
        RiskDecision::Block => {
            warn!(
                run_id,
                trade_id = trade.trade_id,
                reason = risk.reason,
                "RISK_BLOCK"
            );
            return Ok(RiskDecision::Block);
        }
        RiskDecision::Allow => {}
    }

    Ok(RiskDecision::Allow)
}

async fn transition(
    repo: &PostgresRepository,
    trade: &mut TradeRuntime,
    to: TradeState,
    reason: &str,
) -> Result<()> {
    let from = trade.state;
    StateRepository::transition_trade_state(repo, trade.trade_id, from, to, reason).await?;
    trade.state = to;
    Ok(())
}

fn init_tracing() {
    let subscriber = tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(false)
        .json()
        .finish();

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        error!(error = %e, "failed to init tracing");
    }
}
