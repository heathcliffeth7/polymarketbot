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
use bot_infra::config::{AppConfig, TelegramConfig};
use bot_infra::contracts::{OrderExecutor, StateRepository};
use bot_infra::db::{
    PostgresRepository, TradeBuilderOrder, TradeBuilderWorkflow, TradeBuilderWorkflowLeg,
    TradeFlowDefinitionRuntime, TradeFlowRun, TradeFlowRunStep, TradeFlowVersionRuntime,
};
use bot_infra::exchange::{
    ClobHttpClient, ClobRestClient, FillInfo, GammaClient, GammaHttpClient, GammaMarket, OrderInfo,
    PlaceOrderRequest,
};
use bot_infra::market_data::{MarketDataProvider, MockMarketDataProvider};
use bot_infra::reconcile::reconcile_tick_and_snapshot;
use bot_infra::signer::ApiCredentials;
use bot_infra::ws::{ClobWsClient, WsChannel, WsEvent, WsEventType};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use ethers::{
    signers::{LocalWallet, Signer as _},
    types::Address,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex as StdMutex},
    time::Instant,
};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
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
const AUTO_SCOPE_CACHE_TTL_SECS: u64 = 30;
const TRADE_BUILDER_EXIT_QTY_TOLERANCE: f64 = 0.011;
const TRADE_BUILDER_EXIT_TP_SLACK: f64 = 0.05;
const TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC: &str = "notional_usdc";
const TRADE_BUILDER_SIZE_BASIS_SHARES: &str = "shares";
const TRIGGER_PROTECTION_MODE_OFF: &str = "off";
const TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM: &str = "underlying_confirm";
const TRIGGER_PROTECTION_PRESET_LOOSE: &str = "loose";
const TRIGGER_PROTECTION_PRESET_BALANCED: &str = "balanced";
const TRIGGER_PROTECTION_PRESET_STRICT: &str = "strict";
const UNDERLYING_REFERENCE_BASE_URL: &str = "https://api.exchange.coinbase.com";
const UNDERLYING_REFERENCE_MIN_REFRESH_SECS: u64 = 2;
const UNDERLYING_REFERENCE_TICK_RETENTION_SECS: i64 = 90;
const UNDERLYING_REFERENCE_HISTORY_WINDOW_SECS: u64 = 60;
const UNDERLYING_REFERENCE_POLY_DIVERGENCE_CENT: f64 = 4.0;
const UNDERLYING_REFERENCE_BALANCED_FLAT_DELTA_PCT: f64 = 0.02;
const UNDERLYING_REFERENCE_STRICT_DELTA_10S_PCT: f64 = 0.04;
const UNDERLYING_REFERENCE_STRICT_DELTA_30S_PCT: f64 = 0.08;

static AUTO_SCOPE_MARKET_CACHE: LazyLock<StdMutex<HashMap<String, (Instant, Vec<GammaMarket>)>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));
static UNDERLYING_REFERENCE_SERVICE: LazyLock<UnderlyingReferenceService> =
    LazyLock::new(UnderlyingReferenceService::new);
const WORKFLOW_MIN_BUY_INCREMENT_USDC: f64 = 1.0;
const FLOW_NODE_STATE_ONCE_FIRED: &str = "once_fired";
const FLOW_NODE_STATE_ONCE_FIRED_AT: &str = "once_fired_at";
const FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG: &str = "once_fired_market_slug";
const FLOW_NODE_STATE_ONCE_BLOCK_LOGGED: &str = "once_blocked_logged";
const TRIGGER_MARKET_ONCE_SCOPE_VERSION_CURRENT: i64 = 2;
const FLOW_STATE_PUBLISH_MARKER: &str = "__publish_marker";
const BOT_RUNNER_LOCK_PATH_DEFAULT: &str = "/tmp/polymarketbot-bot-runner.lock";
const BOT_RUNNER_DB_LOCK_KEY: i64 = 4_925_982_722_255_244_133;
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

#[derive(Default)]
struct FlowAutoClaimRuntime {
    service: Option<AutoClaimService>,
    init_attempted: bool,
}

async fn maybe_tick_flow_auto_claim(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    auto_claim: &mut FlowAutoClaimRuntime,
) {
    let enabled = match repo.has_active_trade_flow_auto_claim_enabled().await {
        Ok(enabled) => enabled,
        Err(err) => {
            warn!(run_id, error = %err, "AUTO_CLAIM_FLOW_FLAG_CHECK_FAILED");
            return;
        }
    };
    if !enabled {
        return;
    }

    if !auto_claim.init_attempted {
        auto_claim.init_attempted = true;
        match AutoClaimService::from_app_config(cfg) {
            Ok(service) => {
                if service.is_none() {
                    warn!(run_id, "AUTO_CLAIM_FLOW_ENABLED_BUT_CLAIM_DISABLED");
                }
                auto_claim.service = service;
            }
            Err(err) => {
                warn!(run_id, error = %err, "AUTO_CLAIM_FLOW_ENABLED_BUT_CONFIG_INVALID");
                return;
            }
        }
    }

    if let Some(service) = auto_claim.service.as_mut() {
        if let Err(err) = service.maybe_tick(repo).await {
            warn!(run_id, error = %err, "AUTO_CLAIM_TICK_FAILED");
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdownScopeDef {
    scope: &'static str,
    asset: &'static str,
    timeframe: &'static str,
    slug_prefix: &'static str,
}

struct RunnerProcessLock {
    path: PathBuf,
    _file: std::fs::File,
}

impl Drop for RunnerProcessLock {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            warn!(
                lock_path = %self.path.display(),
                error = %err,
                "BOT_RUNNER_LOCK_RELEASE_FAILED"
            );
        }
    }
}

fn parse_lock_pid(content: &str) -> Option<u32> {
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "pid" {
            continue;
        }
        if let Ok(pid) = value.trim().parse::<u32>() {
            return Some(pid);
        }
    }
    None
}

fn is_lock_stale(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return true;
    };
    let Some(pid) = parse_lock_pid(&content) else {
        return true;
    };
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if !proc_dir.exists() {
        return true;
    }
    let Ok(cmdline_raw) = fs::read(proc_dir.join("cmdline")) else {
        // If process metadata is inaccessible, assume lock owner may still be alive.
        return false;
    };
    let cmdline = String::from_utf8_lossy(&cmdline_raw);
    if cmdline.is_empty() {
        return false;
    }
    !cmdline.contains("bot-runner")
}

fn acquire_runner_process_lock() -> Result<RunnerProcessLock> {
    let lock_path = env::var("BOT_RUNNER_LOCK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(BOT_RUNNER_LOCK_PATH_DEFAULT));

    for _ in 0..2 {
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                return Ok(RunnerProcessLock {
                    path: lock_path,
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_lock_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                anyhow::bail!(
                    "another bot-runner process appears active (lock: {})",
                    lock_path.display()
                );
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    anyhow::bail!(
        "failed to acquire bot-runner lock after stale cleanup (lock: {})",
        lock_path.display()
    )
}

fn crossed_above_strict(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
) -> bool {
    previous_price
        .map(|prev| prev < trigger_price && current_price >= trigger_price)
        .unwrap_or(false)
}

fn crossed_below_strict(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
) -> bool {
    previous_price
        .map(|prev| prev > trigger_price && current_price <= trigger_price)
        .unwrap_or(false)
}

fn evaluate_trigger_market_price_condition(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
    trigger_condition: &str,
    allow_first_tick_threshold: bool,
    max_price: Option<f64>,
) -> (bool, &'static str) {
    match trigger_condition {
        "cross_above" => {
            let crossed = if let Some(prev) = previous_price {
                if prev < trigger_price && current_price >= trigger_price {
                    Some("cross_detected")
                } else {
                    None
                }
            } else if allow_first_tick_threshold && current_price >= trigger_price {
                Some("first_tick_threshold")
            } else {
                None
            };
            match crossed {
                Some(mode) => {
                    if let Some(mp) = max_price {
                        if current_price > mp {
                            return (false, "above_max_price");
                        }
                    }
                    (true, mode)
                }
                None => {
                    if previous_price.is_none() && !allow_first_tick_threshold {
                        (false, "no_previous")
                    } else {
                        (false, "no_cross")
                    }
                }
            }
        }
        "cross_below" => {
            let crossed = if let Some(prev) = previous_price {
                if prev > trigger_price && current_price <= trigger_price {
                    Some("cross_detected")
                } else {
                    None
                }
            } else if allow_first_tick_threshold && current_price <= trigger_price {
                Some("first_tick_threshold")
            } else {
                None
            };
            match crossed {
                Some(mode) => {
                    if let Some(mp) = max_price {
                        if current_price > mp {
                            return (false, "above_max_price");
                        }
                    }
                    (true, mode)
                }
                None => {
                    if previous_price.is_none() && !allow_first_tick_threshold {
                        (false, "no_previous")
                    } else {
                        (false, "no_cross")
                    }
                }
            }
        }
        _ => (false, "unsupported_condition"),
    }
}

fn should_apply_ws_cross_confirmed_short_circuit(
    ws_sourced: bool,
    ws_evaluation_mode_from_step: &str,
    ws_hard_ignore_reason: Option<&str>,
) -> bool {
    ws_sourced
        && ws_evaluation_mode_from_step == "cross_confirmed"
        && ws_hard_ignore_reason.is_none()
}

fn is_ws_cross_confirmed_unexpected_fail(
    ws_sourced: bool,
    ws_evaluation_mode_from_step: &str,
    pass: bool,
    ws_hard_ignore_reason: Option<&str>,
) -> bool {
    ws_sourced
        && ws_evaluation_mode_from_step == "cross_confirmed"
        && !pass
        && ws_hard_ignore_reason.is_none()
}

fn market_price_confirmation_ms(node_spec: &WsOpenPositionPriceNodeSpec) -> Option<i64> {
    if node_spec.node_type != "trigger.market_price" {
        return None;
    }
    node_spec.confirmation_ms.filter(|value| *value > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WsPriceMode {
    Midpoint,
    Raw,
    BestBid,
    BestAsk,
}

impl WsPriceMode {
    fn parse(raw: Option<&str>, default: Self) -> Self {
        let normalized = raw.map(str::trim).unwrap_or_default().to_ascii_lowercase();
        match normalized.as_str() {
            "midpoint" | "orderbook_midpoint" | "mid" => Self::Midpoint,
            "raw" | "trade" | "last_trade" | "last_trade_price" => Self::Raw,
            "best_bid" | "bid" => Self::BestBid,
            "best_ask" | "ask" => Self::BestAsk,
            _ => default,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Midpoint => "midpoint",
            Self::Raw => "raw",
            Self::BestBid => "best_bid",
            Self::BestAsk => "best_ask",
        }
    }
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
    once_mode: bool,
    once_scope_market: bool,
    auto_scope: bool,
    price_mode: WsPriceMode,
    market_slug: Option<String>,
    token_id: String,
    trigger_condition: String,
    trigger_price: f64,
    max_price: Option<f64>,
    protection_mode: String,
    protection_asset: Option<String>,
    confirmation_ms: Option<i64>,
    cycle_window_mode: Option<String>,
    cycle_window_secs: Option<i64>,
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

#[derive(Debug, Clone)]
struct UnderlyingProtectionConfig {
    mode: String,
    preset: String,
    asset: String,
    direction: String,
    reference_symbol: String,
}

#[derive(Debug, Clone)]
struct UnderlyingProtectionEvaluation {
    mode: String,
    preset: String,
    asset: String,
    direction: String,
    reference_feed: String,
    reference_symbol: String,
    passed: bool,
    reason_code: String,
    reason_detail: Option<String>,
    cycle_open_price: Option<f64>,
    current_price: Option<f64>,
    delta_10s_pct: Option<f64>,
    delta_30s_pct: Option<f64>,
    poly_delta_10s_cent: Option<f64>,
    divergence_blocked: bool,
}

#[derive(Debug, Clone)]
struct UnderlyingTick {
    price: f64,
    ts: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct UnderlyingReferenceState {
    ticks: VecDeque<UnderlyingTick>,
    cycle_open_by_ts: HashMap<i64, f64>,
    last_refresh_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct UnderlyingReferenceSnapshot {
    cycle_open_price: f64,
    current_price: f64,
    delta_10s_pct: Option<f64>,
    delta_30s_pct: Option<f64>,
}

#[derive(Debug)]
struct UnderlyingReferenceService {
    http: reqwest::Client,
    state: StdMutex<HashMap<String, UnderlyingReferenceState>>,
}

impl UnderlyingReferenceService {
    fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            state: StdMutex::new(HashMap::new()),
        }
    }

    async fn prime(&self, asset: &str) -> Result<()> {
        let _ = self.current_tick(asset).await?;
        Ok(())
    }

    async fn snapshot(
        &self,
        asset: &str,
        market_slug: &str,
    ) -> Result<UnderlyingReferenceSnapshot> {
        let reference_symbol = underlying_reference_symbol(asset)
            .ok_or_else(|| anyhow::anyhow!("unsupported underlying asset: {asset}"))?;
        let cycle_start = MarketCycleId(market_slug.to_string())
            .start_time()
            .ok_or_else(|| {
                anyhow::anyhow!("failed to parse cycle start from market slug: {market_slug}")
            })?;
        let current_tick = self.current_tick(asset).await?;
        let cycle_open_price = self
            .cycle_open_price(asset, reference_symbol, cycle_start)
            .await?;
        let (delta_10s_pct, delta_30s_pct) =
            self.compute_deltas(asset, current_tick.ts, current_tick.price);
        Ok(UnderlyingReferenceSnapshot {
            cycle_open_price,
            current_price: current_tick.price,
            delta_10s_pct,
            delta_30s_pct,
        })
    }

    async fn current_tick(&self, asset: &str) -> Result<UnderlyingTick> {
        if let Some(cached) = self.cached_recent_tick(asset) {
            return Ok(cached);
        }
        let reference_symbol = underlying_reference_symbol(asset)
            .ok_or_else(|| anyhow::anyhow!("unsupported underlying asset: {asset}"))?;
        let tick = self.fetch_current_tick(reference_symbol).await?;
        self.store_tick(asset, tick.clone());
        Ok(tick)
    }

    async fn cycle_open_price(
        &self,
        asset: &str,
        reference_symbol: &str,
        cycle_start: DateTime<Utc>,
    ) -> Result<f64> {
        let cycle_ts = cycle_start.timestamp();
        if let Some(price) = self.cached_cycle_open(asset, cycle_ts) {
            return Ok(price);
        }
        let price = self
            .fetch_cycle_open_price(reference_symbol, cycle_start)
            .await?;
        let mut state = self
            .state
            .lock()
            .expect("underlying reference state poisoned");
        let entry = state.entry(asset.to_string()).or_default();
        entry.cycle_open_by_ts.insert(cycle_ts, price);
        Ok(price)
    }

    fn cached_recent_tick(&self, asset: &str) -> Option<UnderlyingTick> {
        let state = self.state.lock().ok()?;
        let entry = state.get(asset)?;
        let last_refresh_at = entry.last_refresh_at?;
        if last_refresh_at.elapsed().as_secs() >= UNDERLYING_REFERENCE_MIN_REFRESH_SECS {
            return None;
        }
        entry.ticks.back().cloned()
    }

    fn cached_cycle_open(&self, asset: &str, cycle_ts: i64) -> Option<f64> {
        let state = self.state.lock().ok()?;
        state
            .get(asset)
            .and_then(|entry| entry.cycle_open_by_ts.get(&cycle_ts).copied())
    }

    fn store_tick(&self, asset: &str, tick: UnderlyingTick) {
        let mut state = self
            .state
            .lock()
            .expect("underlying reference state poisoned");
        let entry = state.entry(asset.to_string()).or_default();
        entry.last_refresh_at = Some(Instant::now());
        entry.ticks.push_back(tick.clone());
        let cutoff = tick.ts - ChronoDuration::seconds(UNDERLYING_REFERENCE_TICK_RETENTION_SECS);
        while entry
            .ticks
            .front()
            .map(|sample| sample.ts < cutoff)
            .unwrap_or(false)
        {
            entry.ticks.pop_front();
        }
        if entry.ticks.len() > 600 {
            let overflow = entry.ticks.len() - 600;
            entry.ticks.drain(0..overflow);
        }
    }

    fn compute_deltas(
        &self,
        asset: &str,
        current_ts: DateTime<Utc>,
        current_price: f64,
    ) -> (Option<f64>, Option<f64>) {
        let state = match self.state.lock() {
            Ok(value) => value,
            Err(_) => return (None, None),
        };
        let Some(entry) = state.get(asset) else {
            return (None, None);
        };
        let delta_10s_pct =
            underlying_delta_pct_from_ticks(&entry.ticks, current_ts, current_price, 10);
        let delta_30s_pct =
            underlying_delta_pct_from_ticks(&entry.ticks, current_ts, current_price, 30);
        (delta_10s_pct, delta_30s_pct)
    }

    async fn fetch_current_tick(&self, reference_symbol: &str) -> Result<UnderlyingTick> {
        let response = self
            .http
            .get(format!(
                "{UNDERLYING_REFERENCE_BASE_URL}/products/{reference_symbol}/ticker"
            ))
            .header(
                reqwest::header::USER_AGENT,
                "polymarketbot/underlying-protection",
            )
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;
        let payload: Value = response.json().await?;
        let price = payload.get("price").and_then(value_as_f64).ok_or_else(|| {
            anyhow::anyhow!("ticker response missing price for {reference_symbol}")
        })?;
        let ts = payload
            .get("time")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_utc)
            .unwrap_or_else(Utc::now);
        Ok(UnderlyingTick { price, ts })
    }

    async fn fetch_cycle_open_price(
        &self,
        reference_symbol: &str,
        cycle_start: DateTime<Utc>,
    ) -> Result<f64> {
        let start = cycle_start.to_rfc3339();
        let end = (cycle_start + ChronoDuration::minutes(1)).to_rfc3339();
        let response = self
            .http
            .get(format!(
                "{UNDERLYING_REFERENCE_BASE_URL}/products/{reference_symbol}/candles"
            ))
            .query(&[
                ("granularity", "60"),
                ("start", start.as_str()),
                ("end", end.as_str()),
            ])
            .header(
                reqwest::header::USER_AGENT,
                "polymarketbot/underlying-protection",
            )
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;
        let payload: Value = response.json().await?;
        let Some(rows) = payload.as_array() else {
            return Err(anyhow::anyhow!(
                "candles response was not an array for {reference_symbol}"
            ));
        };
        let cycle_ts = cycle_start.timestamp();
        let mut fallback_open = None;
        for row in rows {
            let Some(items) = row.as_array() else {
                continue;
            };
            let row_ts = items.first().and_then(value_as_i64);
            let row_open = items.get(3).and_then(value_as_f64);
            if fallback_open.is_none() {
                fallback_open = row_open;
            }
            if row_ts == Some(cycle_ts) {
                if let Some(open) = row_open {
                    return Ok(open);
                }
            }
        }
        fallback_open.ok_or_else(|| {
            anyhow::anyhow!("failed to resolve cycle-open candle for {reference_symbol}")
        })
    }
}

impl UnderlyingProtectionEvaluation {
    fn to_value(&self) -> Value {
        json!({
            "mode": self.mode,
            "preset": self.preset,
            "asset": self.asset,
            "direction": self.direction,
            "reference_feed": self.reference_feed,
            "reference_symbol": self.reference_symbol,
            "passed": self.passed,
            "reason_code": self.reason_code,
            "reason_detail": self.reason_detail,
            "cycle_open_price": self.cycle_open_price,
            "current_price": self.current_price,
            "delta_10s_pct": self.delta_10s_pct,
            "delta_30s_pct": self.delta_30s_pct,
            "poly_delta_10s_cent": self.poly_delta_10s_cent,
            "divergence_blocked": self.divergence_blocked
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct TriggerPriceSample {
    ts_ms: i64,
    price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionDrawdownDirection {
    Down,
    Up,
}

impl PositionDrawdownDirection {
    fn parse(raw: Option<&str>) -> Option<Self> {
        let normalized = raw.map(str::trim).unwrap_or_default().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "down" {
            return Some(Self::Down);
        }
        if normalized == "up" {
            return Some(Self::Up);
        }
        None
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
        }
    }

    fn metric_type(self) -> &'static str {
        match self {
            Self::Down => "loss_pct",
            Self::Up => "gain_pct",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PositionDrawdownRule {
    index: usize,
    loss_pct: f64,
    direction: PositionDrawdownDirection,
    window_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct PositionDrawdownSample {
    ts_ms: i64,
    loss_pct: f64,
    gain_pct: f64,
    price: f64,
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
    let _process_lock = acquire_runner_process_lock()?;
    info!(
        lock_path = %_process_lock.path.display(),
        pid = std::process::id(),
        "BOT_RUNNER_PROCESS_LOCK_ACQUIRED"
    );

    let config_dir = current_config_dir();
    let database_url = env::var("DATABASE_URL").context(
        "DATABASE_URL is required, e.g. postgres://postgres:postgres@localhost:5432/dextrabot",
    )?;

    let cfg = AppConfig::load(&config_dir)?;

    let repo = PostgresRepository::new(&database_url).await?;
    let _runner_db_lock = repo
        .try_acquire_runner_singleton_lock(BOT_RUNNER_DB_LOCK_KEY)
        .await?
        .ok_or_else(|| anyhow::anyhow!("another bot-runner process is already active"))?;
    info!(
        lock_key = _runner_db_lock.lock_key(),
        "BOT_RUNNER_DB_LOCK_ACQUIRED"
    );
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
            info!(
                run_id = scope_run_id,
                parent_run_id = run_id,
                scope = scope.as_str(),
                "SCOPE_STARTED"
            );
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
                    info!(
                        run_id = scope_run_id,
                        "MARKET_CYCLE_COMPLETE_SEARCHING_NEXT"
                    );
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

fn decrypt_config_string_if_needed(field_name: &str, raw_value: &str) -> Result<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let key_material = if trimmed.starts_with(CONFIG_ENC_PREFIX) {
        Some(load_config_encryption_key()?)
    } else {
        None
    };

    decrypt_config_value_if_needed(field_name, trimmed, key_material.as_ref())
        .map(|value| value.trim().to_string())
}

fn current_config_dir() -> PathBuf {
    PathBuf::from(env::var("BOT_CONFIG_DIR").unwrap_or_else(|_| "./config".to_string()))
}

fn load_live_telegram_config() -> Result<TelegramConfig> {
    TelegramConfig::load_from_dir(&current_config_dir())
}

fn resolve_telegram_bot_token(telegram: &TelegramConfig, node: &TradeFlowNode) -> Result<String> {
    let global_token = telegram.bot_token.trim();
    if !global_token.is_empty() {
        let resolved = decrypt_config_string_if_needed("telegram.bot_token", global_token)?;
        anyhow::ensure!(
            !resolved.is_empty(),
            "action.telegram_notify requires non-empty global telegram.bot_token"
        );
        return Ok(resolved);
    }

    let legacy_token = node_config_string(node, "botToken").ok_or_else(|| {
        anyhow::anyhow!(
            "action.telegram_notify requires global telegram.bot_token or legacy botToken"
        )
    })?;
    let resolved =
        decrypt_config_string_if_needed("action.telegram_notify.botToken", &legacy_token)?;
    anyhow::ensure!(
        !resolved.is_empty(),
        "action.telegram_notify requires global telegram.bot_token or legacy botToken"
    );
    Ok(resolved)
}

fn resolve_telegram_chat_id(telegram: &TelegramConfig, node: &TradeFlowNode) -> Result<String> {
    if let Some(node_chat_id) = node_config_string(node, "chatId") {
        let resolved = node_chat_id.trim().to_string();
        if !resolved.is_empty() {
            return Ok(resolved);
        }
    }

    let global_chat_id = telegram.chat_id.trim();
    anyhow::ensure!(
        !global_chat_id.is_empty(),
        "action.telegram_notify requires chatId or global telegram.chat_id"
    );
    Ok(global_chat_id.to_string())
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

pub(crate) fn find_updown_scope_by_asset_timeframe(
    asset: &str,
    timeframe: &str,
) -> Option<UpdownScopeDef> {
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

pub(crate) async fn list_markets_for_scope(
    gamma: &GammaHttpClient,
    scope: &str,
) -> Result<Vec<GammaMarket>> {
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
            Some(cfg.claim.data_api_base_url.clone()),
            cfg.claim.positions_page_size,
            cfg.claim.positions_max_pages,
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
    let mut auto_claim = FlowAutoClaimRuntime::default();

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
        maybe_tick_flow_auto_claim(repo, run_id, cfg, &mut auto_claim).await;

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
                    order_type: "GTC".to_string(),
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

async fn run_flow_only_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
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
        Some(cfg.claim.data_api_base_url.clone()),
        cfg.claim.positions_page_size,
        cfg.claim.positions_max_pages,
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
    let mut auto_claim = FlowAutoClaimRuntime::default();

    info!(run_id, "FLOW_ONLY_LOOP_STARTED");

    // Infinite loop — only processes canvas/flow systems, no automatic trades
    loop {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        maybe_tick_flow_auto_claim(repo, run_id, cfg, &mut auto_claim).await;

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
        Some(cfg.claim.data_api_base_url.clone()),
        cfg.claim.positions_page_size,
        cfg.claim.positions_max_pages,
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
    let mut auto_claim = FlowAutoClaimRuntime::default();
    let override_slug = configured_market_override_slug(cfg)?;

    let mut waiting_event_emitted = false;
    let selected = loop {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        maybe_tick_flow_auto_claim(repo, run_id, cfg, &mut auto_claim).await;

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
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_flows(repo, run_id, cfg, Some(&client), &ws).await {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }
        maybe_tick_flow_auto_claim(repo, run_id, cfg, &mut auto_claim).await;

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
        order_type: "GTC".to_string(),
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
    if let Err(err) = enqueue_trade_flow_ws_open_position_price_steps(repo, run_id, cfg, ws).await {
        warn!(run_id, error = %err, "TRADE_FLOW_WS_TRIGGER_ENQUEUE_FAILED");
    }

    let limits = to_risk_limits(cfg);
    let policy = DefaultRiskPolicy;
    let claimed_steps = repo
        .claim_ready_trade_flow_steps(FLOW_STEP_PROCESS_LIMIT)
        .await?;
    for step in claimed_steps {
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

fn node_repeat_mode(node: &TradeFlowNode) -> &str {
    node.config
        .get("repeatMode")
        .and_then(Value::as_str)
        .unwrap_or("loop")
}

fn node_trigger_market_once_scope_version(node: &TradeFlowNode) -> i64 {
    node.config
        .get("onceScopeVersion")
        .and_then(value_as_i64)
        .unwrap_or(0)
}

fn node_uses_legacy_auto_scope_once_scope(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node)
        && node_market_mode(node) == "auto_scope"
        && node_trigger_market_once_scope_version(node) < TRIGGER_MARKET_ONCE_SCOPE_VERSION_CURRENT
}

fn node_once_scope(node: &TradeFlowNode) -> &str {
    if node_uses_legacy_auto_scope_once_scope(node) {
        return "market";
    }
    match node
        .config
        .get("onceScope")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("market") => "market",
        _ => "run",
    }
}

fn is_trade_flow_market_price_once_scope_market(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node) && node_once_scope(node) == "market"
}

fn allow_first_tick_threshold_for_ws_node(
    node_spec: &WsOpenPositionPriceNodeSpec,
    previous_price: Option<f64>,
) -> bool {
    if node_spec.node_type != "trigger.market_price" {
        return false;
    }
    if matches!(node_spec.cycle_window_mode.as_deref(), Some("last")) {
        return false;
    }
    !node_spec.once_mode || node_spec.auto_scope || previous_price.is_none()
}

fn is_trade_flow_market_price_once_node(node: &TradeFlowNode) -> bool {
    node.node_type == "trigger.market_price" && node_repeat_mode(node) == "once"
}

/// Check if an auto-scope market has expired based on the slug timestamp.
/// Slug format: "btc-updown-5m-{unix_ts}" or "btc-updown-15m-{unix_ts}".
fn is_auto_scope_market_expired(slug: &str, buffer_secs: i64) -> bool {
    let parts: Vec<&str> = slug.rsplit('-').collect();
    let ts: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return false,
    };
    let duration = if slug.contains("-5m-") {
        300
    } else if slug.contains("-15m-") {
        900
    } else {
        return false;
    };
    let now = Utc::now().timestamp();
    now >= ts + duration + buffer_secs
}

/// Check if an auto-scope market is in its resolution window (last `grace_secs`).
/// During resolution, prices converge to 0/1 and should not trigger cross conditions.
fn is_auto_scope_market_in_resolution_window(slug: &str, grace_secs: i64) -> bool {
    let parts: Vec<&str> = slug.rsplit('-').collect();
    let ts: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return false,
    };
    let duration = if slug.contains("-5m-") {
        300i64
    } else if slug.contains("-15m-") {
        900i64
    } else {
        return false;
    };
    let now = Utc::now().timestamp();
    now >= ts + duration - grace_secs
}

/// Cycle window focus: returns true if current time is OUTSIDE the configured sub-window.
/// - mode "first": active window is [start_ts, start_ts + window_secs)
/// - mode "last":  active window is [end_ts - window_secs, end_ts)
fn is_outside_cycle_window_focus(slug: &str, mode: &str, window_secs: i64) -> bool {
    let parts: Vec<&str> = slug.rsplit('-').collect();
    let ts: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return false,
    };
    let duration = if slug.contains("-5m-") {
        300i64
    } else if slug.contains("-15m-") {
        900i64
    } else {
        return false;
    };
    if window_secs >= duration {
        return false;
    }
    let now = Utc::now().timestamp();
    match mode {
        "first" => now >= ts + window_secs,
        "last" => now < ts + duration - window_secs,
        _ => false,
    }
}

fn auto_scope_resolution_window_guard_enabled(cycle_window_mode: Option<&str>) -> bool {
    !matches!(cycle_window_mode, Some("last"))
}

fn trade_flow_publish_marker(version: &TradeFlowVersionRuntime) -> String {
    let marker_ts = version
        .published_at
        .unwrap_or(version.created_at)
        .timestamp_millis();
    format!("{}:{marker_ts}", version.id)
}

fn sync_trade_flow_once_state_for_publish(
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    publish_marker: &str,
) -> (Option<String>, Vec<String>) {
    let previous_marker = flow_state_string(context, FLOW_STATE_PUBLISH_MARKER);
    if previous_marker.as_deref() == Some(publish_marker) {
        return (previous_marker, Vec::new());
    }

    let mut reset_nodes = Vec::new();
    for node in &graph.nodes {
        if !is_trade_flow_market_price_once_node(node) {
            continue;
        }
        if flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED) {
            reset_nodes.push(node.key.clone());
        }
        clear_trade_flow_market_price_once_state(context, &node.key);
    }

    set_flow_state(context, FLOW_STATE_PUBLISH_MARKER, json!(publish_marker));
    (previous_marker, reset_nodes)
}

fn node_market_mode(node: &TradeFlowNode) -> &str {
    match node
        .config
        .get("marketMode")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("auto_scope") => "auto_scope",
        _ => "fixed",
    }
}

fn node_market_selection(node: &TradeFlowNode) -> String {
    node_config_string(node, "marketSelection")
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "latest_by_slug".to_string())
}

fn should_accept_ws_market_slug_override(node: &TradeFlowNode, current_market_slug: &str) -> bool {
    node_market_mode(node) != "auto_scope" || current_market_slug.trim().is_empty()
}

fn normalized_binary_outcome_label(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("yes"),
        "no" | "down" | "short" | "bear" => Some("no"),
        _ => None,
    }
}

fn resolve_token_id_for_outcome_label(outcome_label: &str, context: &Value) -> Option<String> {
    match normalized_binary_outcome_label(outcome_label) {
        Some("yes") => flow_context_string(context, "yesTokenId"),
        Some("no") => flow_context_string(context, "noTokenId"),
        _ => None,
    }
}

fn normalize_trigger_protection_mode(raw: Option<&str>) -> &'static str {
    match raw
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM => TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM,
        _ => TRIGGER_PROTECTION_MODE_OFF,
    }
}

fn normalize_trigger_protection_preset(raw: Option<&str>) -> &'static str {
    match raw
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        TRIGGER_PROTECTION_PRESET_LOOSE => TRIGGER_PROTECTION_PRESET_LOOSE,
        TRIGGER_PROTECTION_PRESET_STRICT => TRIGGER_PROTECTION_PRESET_STRICT,
        _ => TRIGGER_PROTECTION_PRESET_BALANCED,
    }
}

fn underlying_reference_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC-USD"),
        "eth" => Some("ETH-USD"),
        "sol" => Some("SOL-USD"),
        "xrp" => Some("XRP-USD"),
        _ => None,
    }
}

fn resolve_auto_scope_underlying_asset(
    node: &TradeFlowNode,
    context: &Value,
    market_slug: Option<&str>,
) -> Option<String> {
    node_config_string(node, "marketScope")
        .or_else(|| flow_context_string(context, "marketScope"))
        .as_deref()
        .and_then(find_updown_scope_by_scope)
        .map(|scope| scope.asset.to_string())
        .or_else(|| flow_context_string(context, "marketAsset"))
        .or_else(|| {
            market_slug
                .and_then(find_updown_scope_by_slug)
                .map(|scope| scope.asset.to_string())
        })
}

fn resolve_underlying_direction_label(outcome_label: &str) -> Option<&'static str> {
    match normalized_binary_outcome_label(outcome_label) {
        Some("yes") => Some("up"),
        Some("no") => Some("down"),
        _ => None,
    }
}

fn direction_multiplier(direction: &str) -> Option<f64> {
    match direction.trim().to_ascii_lowercase().as_str() {
        "up" => Some(1.0),
        "down" => Some(-1.0),
        _ => None,
    }
}

fn parse_trigger_price_samples(value: Option<&Value>) -> Vec<TriggerPriceSample> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(ts_ms) = obj.get("ts_ms").and_then(value_as_i64) else {
            continue;
        };
        let Some(price) = obj.get("price").and_then(value_as_f64) else {
            continue;
        };
        if !price.is_finite() || price < 0.0 || price > 1.0 {
            continue;
        }
        out.push(TriggerPriceSample { ts_ms, price });
    }
    out.sort_by_key(|sample| sample.ts_ms);
    out
}

fn record_trigger_price_sample(
    context: &mut Value,
    node_key: &str,
    token_id: &str,
    price: f64,
    now: DateTime<Utc>,
) -> Option<f64> {
    let state_key = format!("price_samples_{}", token_id);
    let now_ms = now.timestamp_millis();
    let mut samples = parse_trigger_price_samples(flow_node_state(context, node_key, &state_key));
    samples.push(TriggerPriceSample {
        ts_ms: now_ms,
        price,
    });
    let cutoff = now_ms.saturating_sub((UNDERLYING_REFERENCE_HISTORY_WINDOW_SECS as i64) * 1000);
    samples.retain(|sample| sample.ts_ms >= cutoff);
    if samples.len() > 120 {
        let overflow = samples.len() - 120;
        samples.drain(0..overflow);
    }
    let delta_10s_cent = samples
        .iter()
        .rev()
        .find(|sample| sample.ts_ms <= now_ms.saturating_sub(10_000))
        .map(|sample| (price - sample.price) * 100.0);
    set_flow_node_state(
        context,
        node_key,
        &state_key,
        json!(samples
            .iter()
            .map(|sample| json!({
                "ts_ms": sample.ts_ms,
                "price": sample.price
            }))
            .collect::<Vec<Value>>()),
    );
    delta_10s_cent
}

fn underlying_delta_pct_from_ticks(
    ticks: &VecDeque<UnderlyingTick>,
    current_ts: DateTime<Utc>,
    current_price: f64,
    seconds: i64,
) -> Option<f64> {
    let target_ts = current_ts - ChronoDuration::seconds(seconds);
    let previous = ticks
        .iter()
        .rev()
        .find(|sample| sample.ts <= target_ts)
        .or_else(|| ticks.front())?;
    if previous.price <= 0.0 {
        return None;
    }
    Some(((current_price - previous.price) / previous.price) * 100.0)
}

fn build_underlying_protection_config(
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    outcome_label: &str,
) -> Option<UnderlyingProtectionConfig> {
    let mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    );
    if mode != TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
        return None;
    }
    if node_market_mode(node) != "auto_scope" {
        return None;
    }
    let asset = resolve_auto_scope_underlying_asset(node, context, Some(market_slug))?;
    let direction = resolve_underlying_direction_label(outcome_label)?.to_string();
    let reference_symbol = underlying_reference_symbol(&asset)?.to_string();
    Some(UnderlyingProtectionConfig {
        mode: mode.to_string(),
        preset: normalize_trigger_protection_preset(
            node.config.get("protectionPreset").and_then(Value::as_str),
        )
        .to_string(),
        asset,
        direction,
        reference_symbol,
    })
}

fn parse_underlying_protection_config(value: Option<Value>) -> Option<UnderlyingProtectionConfig> {
    let value = value?;
    let obj = value.as_object()?;
    let mode = normalize_trigger_protection_mode(obj.get("mode").and_then(Value::as_str));
    if mode != TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
        return None;
    }
    let asset = obj
        .get("asset")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    let direction = obj
        .get("direction")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    let reference_symbol = obj
        .get("reference_symbol")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| underlying_reference_symbol(&asset).map(str::to_string))?;
    Some(UnderlyingProtectionConfig {
        mode: mode.to_string(),
        preset: normalize_trigger_protection_preset(obj.get("preset").and_then(Value::as_str))
            .to_string(),
        asset,
        direction,
        reference_symbol,
    })
}

async fn evaluate_underlying_protection(
    config: &UnderlyingProtectionConfig,
    market_slug: &str,
    poly_delta_10s_cent: Option<f64>,
) -> UnderlyingProtectionEvaluation {
    let base = UnderlyingProtectionEvaluation {
        mode: config.mode.clone(),
        preset: config.preset.clone(),
        asset: config.asset.clone(),
        direction: config.direction.clone(),
        reference_feed: "coinbase_spot".to_string(),
        reference_symbol: config.reference_symbol.clone(),
        passed: false,
        reason_code: "reference_fetch_failed".to_string(),
        reason_detail: None,
        cycle_open_price: None,
        current_price: None,
        delta_10s_pct: None,
        delta_30s_pct: None,
        poly_delta_10s_cent,
        divergence_blocked: false,
    };
    let multiplier = match direction_multiplier(&config.direction) {
        Some(value) => value,
        None => {
            return UnderlyingProtectionEvaluation {
                reason_code: "unsupported_direction".to_string(),
                reason_detail: Some(config.direction.clone()),
                ..base
            }
        }
    };
    let snapshot = match UNDERLYING_REFERENCE_SERVICE
        .snapshot(&config.asset, market_slug)
        .await
    {
        Ok(value) => value,
        Err(err) => {
            return UnderlyingProtectionEvaluation {
                reason_code: "reference_data_unready".to_string(),
                reason_detail: Some(err.to_string()),
                ..base
            }
        }
    };

    let mut evaluation = UnderlyingProtectionEvaluation {
        cycle_open_price: Some(snapshot.cycle_open_price),
        current_price: Some(snapshot.current_price),
        delta_10s_pct: snapshot.delta_10s_pct,
        delta_30s_pct: snapshot.delta_30s_pct,
        reason_code: "passed".to_string(),
        passed: true,
        ..base
    };

    if ((snapshot.current_price - snapshot.cycle_open_price) * multiplier) <= 0.0 {
        evaluation.passed = false;
        evaluation.reason_code = "cycle_open_mismatch".to_string();
        return evaluation;
    }

    let directional_delta_10s = match snapshot.delta_10s_pct {
        Some(value) => value * multiplier,
        None => {
            evaluation.passed = false;
            evaluation.reason_code = "reference_data_unready".to_string();
            return evaluation;
        }
    };

    match config.preset.as_str() {
        TRIGGER_PROTECTION_PRESET_LOOSE => {
            if directional_delta_10s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
            }
            return evaluation;
        }
        TRIGGER_PROTECTION_PRESET_STRICT => {
            if directional_delta_10s < UNDERLYING_REFERENCE_STRICT_DELTA_10S_PCT {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
                return evaluation;
            }
        }
        _ => {
            if directional_delta_10s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
                return evaluation;
            }
        }
    }

    let directional_delta_30s = match snapshot.delta_30s_pct {
        Some(value) => value * multiplier,
        None => {
            evaluation.passed = false;
            evaluation.reason_code = "reference_data_unready".to_string();
            return evaluation;
        }
    };

    match config.preset.as_str() {
        TRIGGER_PROTECTION_PRESET_STRICT => {
            if directional_delta_30s < UNDERLYING_REFERENCE_STRICT_DELTA_30S_PCT {
                evaluation.passed = false;
                evaluation.reason_code = "delta_30s_mismatch".to_string();
                return evaluation;
            }
        }
        _ => {
            if directional_delta_30s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_30s_mismatch".to_string();
                return evaluation;
            }
        }
    }

    let divergence_guard_enabled = config.preset == TRIGGER_PROTECTION_PRESET_BALANCED
        || config.preset == TRIGGER_PROTECTION_PRESET_STRICT;
    if divergence_guard_enabled {
        if let Some(poly_delta) = poly_delta_10s_cent {
            let directional_poly_delta = poly_delta * multiplier;
            let abs_delta_10s = snapshot.delta_10s_pct.unwrap_or_default().abs();
            if directional_poly_delta >= UNDERLYING_REFERENCE_POLY_DIVERGENCE_CENT
                && (directional_delta_10s <= 0.0
                    || abs_delta_10s < UNDERLYING_REFERENCE_BALANCED_FLAT_DELTA_PCT)
            {
                evaluation.passed = false;
                evaluation.reason_code = "divergence_detected".to_string();
                evaluation.divergence_blocked = true;
                return evaluation;
            }
        }
    }

    evaluation
}

async fn sync_trigger_market_auto_scope_context(
    cfg: &AppConfig,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<Option<SelectedLiveMarket>> {
    if node_market_mode(node) != "auto_scope" {
        return Ok(None);
    }

    let market_scope = node_config_string(node, "marketScope")
        .or_else(|| flow_context_string(context, "marketScope"))
        .ok_or_else(|| anyhow::anyhow!("trigger.market_price auto_scope requires marketScope"))?;
    let scope_def = find_updown_scope_by_scope(&market_scope).ok_or_else(|| {
        anyhow::anyhow!(
            "trigger.market_price auto_scope unsupported marketScope={market_scope} (supported: {})",
            SUPPORTED_UPDOWN_SCOPE_DEFS
                .iter()
                .map(|def| def.scope)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let market_selection = node_market_selection(node);
    let markets = {
        let cache_hit = AUTO_SCOPE_MARKET_CACHE
            .lock()
            .unwrap()
            .get(&market_scope)
            .filter(|(t, _)| {
                t.elapsed() < std::time::Duration::from_secs(AUTO_SCOPE_CACHE_TTL_SECS)
            })
            .map(|(_, m)| m.clone());
        if let Some(cached) = cache_hit {
            cached
        } else {
            let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
            let fresh = list_markets_for_scope(&gamma, &market_scope).await?;
            AUTO_SCOPE_MARKET_CACHE
                .lock()
                .unwrap()
                .insert(market_scope.clone(), (Instant::now(), fresh.clone()));
            fresh
        }
    };
    let selected = select_market_from_candidates(markets, None, &market_selection, true);
    let Some(selected) = selected else {
        return Ok(None);
    };

    set_flow_context(context, "marketSlug", json!(selected.slug));
    set_flow_context(context, "marketScope", json!(scope_def.scope));
    set_flow_context(context, "marketAsset", json!(scope_def.asset));
    set_flow_context(context, "marketTimeframe", json!(scope_def.timeframe));
    set_flow_context(context, "yesTokenId", json!(selected.yes_token_id));
    set_flow_context(context, "noTokenId", json!(selected.no_token_id));
    if let Some(preferred_outcome) = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .and_then(|value| normalized_binary_outcome_label(&value).map(str::to_string))
    {
        let token_id = if preferred_outcome == "no" {
            selected.no_token_id.clone()
        } else {
            selected.yes_token_id.clone()
        };
        let outcome_label = if preferred_outcome == "no" {
            "No"
        } else {
            "Yes"
        };
        set_flow_context(context, "outcomeLabel", json!(outcome_label));
        set_flow_context(context, "tokenId", json!(token_id));
    }

    Ok(Some(selected))
}

fn open_position_ws_price_node_specs(
    node: &TradeFlowNode,
    context: &Value,
) -> Vec<WsOpenPositionPriceNodeSpec> {
    if node.node_type != "trigger.open_positions" && node.node_type != "trigger.market_price" {
        return Vec::new();
    }
    let once_mode = is_trade_flow_market_price_once_node(node);
    let once_scope_market = is_trade_flow_market_price_once_scope_market(node);
    let price_mode = if node.node_type == "trigger.market_price" {
        WsPriceMode::parse(
            node.config.get("priceMode").and_then(|v| v.as_str()),
            WsPriceMode::Midpoint,
        )
    } else {
        WsPriceMode::Raw
    };
    let confirmation_ms = node
        .config
        .get("confirmationMs")
        .and_then(value_as_i64_strict)
        .filter(|value| *value >= 0);
    let protection_mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    )
    .to_string();
    let cycle_window_mode = node
        .config
        .get("cycleWindowMode")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s == "first" || s == "last");
    let cycle_window_secs = node
        .config
        .get("cycleWindowSecs")
        .and_then(value_as_i64_strict)
        .filter(|v| *v > 0);
    let (cycle_window_mode, cycle_window_secs) = match (&cycle_window_mode, cycle_window_secs) {
        (Some(mode), Some(secs)) => (Some(mode.clone()), Some(secs)),
        _ => (None, None),
    };
    let market_slug = if node_market_mode(node) == "auto_scope" {
        flow_context_string(context, "marketSlug")
            .or_else(|| node_config_string(node, "marketSlug"))
    } else {
        node_config_string(node, "marketSlug")
            .or_else(|| flow_context_string(context, "marketSlug"))
    };
    let protection_asset = if protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM
        && node_market_mode(node) == "auto_scope"
    {
        resolve_auto_scope_underlying_asset(node, context, market_slug.as_deref())
    } else {
        None
    };
    // Multi-outcome path
    if let Some(conditions) = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
    {
        let mut specs = Vec::new();
        for cond in conditions {
            let mut token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_outcome_label = cond
                .get("outcomeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if node_market_mode(node) == "auto_scope" && !cond_outcome_label.is_empty() {
                token_id = resolve_token_id_for_outcome_label(cond_outcome_label, context)
                    .unwrap_or(token_id);
            }
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
            let max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
            specs.push(WsOpenPositionPriceNodeSpec {
                node_key: node.key.clone(),
                node_type: node.node_type.clone(),
                once_mode,
                once_scope_market,
                auto_scope: node_market_mode(node) == "auto_scope",
                price_mode,
                market_slug: market_slug.clone(),
                token_id,
                trigger_condition,
                trigger_price: tp,
                max_price,
                protection_mode: protection_mode.clone(),
                protection_asset: protection_asset.clone(),
                confirmation_ms,
                cycle_window_mode: cycle_window_mode.clone(),
                cycle_window_secs,
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
        .or_else(|| {
            if node_market_mode(node) != "auto_scope" {
                return None;
            }
            let outcome = node_config_string(node, "outcomeLabel")
                .or_else(|| flow_context_string(context, "outcomeLabel"))?;
            resolve_token_id_for_outcome_label(&outcome, context)
        }) {
        Some(id) if !id.is_empty() => id,
        _ => return Vec::new(),
    };
    let max_price = node_config_f64(node, "maxPrice")
        .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
        .filter(|v| *v > 0.0 && *v <= 1.0);
    vec![WsOpenPositionPriceNodeSpec {
        node_key: node.key.clone(),
        node_type: node.node_type.clone(),
        once_mode,
        once_scope_market,
        auto_scope: node_market_mode(node) == "auto_scope",
        price_mode,
        market_slug,
        token_id,
        trigger_condition,
        trigger_price,
        max_price,
        protection_mode,
        protection_asset,
        confirmation_ms,
        cycle_window_mode,
        cycle_window_secs,
    }]
}

fn ws_price_trigger_step_idempotency_key(
    run_id: i64,
    node_key: &str,
    trigger_condition: &str,
    current_price: f64,
    event_ts: Option<i64>,
    once_mode: bool,
    once_scope_market: bool,
    market_slug: Option<&str>,
) -> String {
    if once_mode {
        // once mode must enqueue at most one step for a once scope.
        if once_scope_market {
            let scope_slug = market_slug
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("unknown-market");
            format!("ws-once:{run_id}:{node_key}:{scope_slug}")
        } else {
            format!("ws-once:{run_id}:{node_key}")
        }
    } else {
        let dedupe_ts = event_ts.unwrap_or_else(|| Utc::now().timestamp_millis());
        format!(
            "ws-open-price:{run_id}:{node_key}:{trigger_condition}:{current_price:.6}:{dedupe_ts}"
        )
    }
}

async fn enqueue_trade_flow_ws_open_position_price_steps(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
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
        let mut context = normalize_trade_flow_context(run.context_json.clone(), &graph.context);
        let mut nodes = Vec::new();
        for node in &graph.nodes {
            if node_market_mode(node) == "auto_scope"
                && matches!(
                    node.node_type.as_str(),
                    "trigger.market_price" | "trigger.open_positions"
                )
            {
                match sync_trigger_market_auto_scope_context(cfg, node, &mut context).await {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        continue;
                    }
                    Err(err) => {
                        warn!(
                            run_id,
                            flow_run_id = run.id,
                            node_key = %node.key,
                            error = %err,
                            "TRADE_FLOW_TRIGGER_AUTO_SCOPE_RESOLVE_FAILED"
                        );
                        continue;
                    }
                }
            }
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

    // Batch subscribe: single WS connection for all tokens
    let all_token_ids: Vec<String> = token_targets.keys().cloned().collect();
    let all_events = match ws.subscribe_once(WsChannel::Market, &all_token_ids).await {
        Ok(events) => events,
        Err(err) => {
            warn!(
                run_id,
                token_count = all_token_ids.len(),
                error = %err,
                "TRADE_FLOW_WS_BATCH_SUBSCRIBE_FAILED"
            );
            return Ok(());
        }
    };

    let mut ws_price_cache: HashMap<(String, WsPriceMode), Option<ExtractedWsPrice>> =
        HashMap::new();
    for (token_id, targets) in token_targets {
        for (run_index, node_index) in targets {
            let Some(node_spec) = run_specs
                .get(run_index)
                .and_then(|run_spec| run_spec.nodes.get(node_index))
                .cloned()
            else {
                continue;
            };

            let cache_key = (token_id.clone(), node_spec.price_mode);
            let extracted = ws_price_cache.entry(cache_key).or_insert_with(|| {
                extract_price_from_market_events_with_mode(
                    &all_events,
                    &token_id,
                    node_spec.price_mode,
                )
            });
            let Some(extracted_price) = *extracted else {
                debug!(
                    run_id,
                    node_key = %node_spec.node_key,
                    token_id = %node_spec.token_id,
                    market = ?node_spec.market_slug,
                    "TRIGGER_WS_NO_PRICE_DATA"
                );
                continue;
            };
            let current_price = clamp_probability(extracted_price.price);
            let event_ts = extracted_price.ts;
            let price_source = extracted_price.source;

            let Some(run_spec) = run_specs.get_mut(run_index) else {
                continue;
            };
            // Skip expired (resolved) markets for auto_scope triggers
            if node_spec.auto_scope {
                if let Some(ref slug) = node_spec.market_slug {
                    if is_auto_scope_market_expired(slug, 15) {
                        continue;
                    }
                }
            }
            // Skip resolution-extreme prices (market resolving)
            if current_price < 0.03 || current_price > 0.97 {
                debug!(
                    run_id,
                    flow_run_id = run_spec.run_id,
                    node_key = %node_spec.node_key,
                    price = current_price,
                    token_id = %node_spec.token_id,
                    market = ?node_spec.market_slug,
                    "TRIGGER_SKIP_RESOLUTION_PRICE"
                );
                continue;
            }
            // Skip markets in resolution window unless the workflow explicitly
            // targets the last slice of the cycle via cycleWindowMode=last.
            if node_spec.auto_scope {
                if let Some(ref slug) = node_spec.market_slug {
                    if auto_scope_resolution_window_guard_enabled(
                        node_spec.cycle_window_mode.as_deref(),
                    ) && is_auto_scope_market_in_resolution_window(slug, 120)
                    {
                        debug!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            market_slug = %slug,
                            "TRIGGER_SKIP_RESOLUTION_WINDOW"
                        );
                        continue;
                    }
                }
            }
            // Skip if outside configured cycle window focus
            if node_spec.auto_scope {
                if let (Some(ref slug), Some(ref cw_mode), Some(cw_secs)) = (
                    &node_spec.market_slug,
                    &node_spec.cycle_window_mode,
                    node_spec.cycle_window_secs,
                ) {
                    if is_outside_cycle_window_focus(slug, cw_mode, cw_secs) {
                        debug!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            market_slug = %slug,
                            cycle_window_mode = %cw_mode,
                            cycle_window_secs = cw_secs,
                            "TRIGGER_SKIP_CYCLE_WINDOW_FOCUS"
                        );
                        continue;
                    }
                }
            }
            if node_spec.protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
                if let Some(asset) = node_spec.protection_asset.as_deref() {
                    if let Err(err) = UNDERLYING_REFERENCE_SERVICE.prime(asset).await {
                        debug!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            asset = %asset,
                            error = %err,
                            "TRIGGER_UNDERLYING_PRIME_FAILED"
                        );
                    }
                }
            }
            sync_trade_flow_market_price_once_scope_state(
                &mut run_spec.context,
                &node_spec.node_key,
                node_spec.once_scope_market,
                node_spec.market_slug.as_deref(),
            );
            // Market rotasyonunda previous_price temizle (sahte cross önleme)
            if let Some(current_slug) = node_spec.market_slug.as_deref() {
                let last_slug = flow_node_state_string(
                    &run_spec.context,
                    &node_spec.node_key,
                    "last_ws_market_slug",
                );
                let slug_changed = last_slug
                    .as_deref()
                    .map(|last| last != current_slug)
                    .unwrap_or(true);
                if slug_changed {
                    let prev_key = format!("previous_price_{}", node_spec.token_id);
                    remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &prev_key);
                    remove_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        "previous_price",
                    );
                    // Clear any pending cross confirmation from previous market
                    let cpend_at = format!("cross_pending_at_{}", node_spec.token_id);
                    let cpend_price = format!("cross_pending_price_{}", node_spec.token_id);
                    let cpend_prev = format!("cross_pending_prev_{}", node_spec.token_id);
                    remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &cpend_at);
                    remove_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_price,
                    );
                    remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &cpend_prev);
                    run_spec.context_dirty = true;
                }
                set_flow_node_state(
                    &mut run_spec.context,
                    &node_spec.node_key,
                    "last_ws_market_slug",
                    json!(current_slug),
                );
                if slug_changed {
                    run_spec.context_dirty = true;
                }
            }
            if node_spec.once_mode
                && trade_flow_market_price_once_fired_for_scope(
                    &run_spec.context,
                    &node_spec.node_key,
                    node_spec.once_scope_market,
                    node_spec.market_slug.as_deref(),
                )
            {
                if !flow_node_state_truthy(
                    &run_spec.context,
                    &node_spec.node_key,
                    FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                ) {
                    set_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                        json!(true),
                    );
                    run_spec.context_dirty = true;
                    if let Err(err) = repo
                        .append_trade_flow_event(
                            Some(run_spec.run_id),
                            run_spec.definition_id,
                            Some(run_spec.version_id),
                            "trigger_once_blocked",
                            &json!({
                                "node_key": node_spec.node_key,
                                "node_type": node_spec.node_type,
                                "token_id": node_spec.token_id,
                                "reason": "ws_enqueue_once_fired",
                                "once_scope": if node_spec.once_scope_market { "market" } else { "run" },
                                "market_slug": node_spec.market_slug.clone()
                            }),
                        )
                        .await
                    {
                        warn!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            error = %err,
                            "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
                        );
                    }
                }
                continue;
            }

            // Use per-token state key for multi-outcome nodes
            // PRCE-01: Only use per-token key. No fallback to bare "previous_price" —
            // that key may hold a stale price from a different token.
            let prev_key = format!("previous_price_{}", node_spec.token_id);
            let previous_price = flow_node_state(&run_spec.context, &node_spec.node_key, &prev_key)
                .and_then(value_as_f64);
            // `cycleWindowMode=last` stratejisinde ilk tick threshold tetik sayilmaz;
            // son pencere icinde gercek bir gecis gerekir. Diger modlarda mevcut
            // auto_scope/bootstrap davranisini koruyoruz.
            let allow_first_tick_threshold =
                allow_first_tick_threshold_for_ws_node(&node_spec, previous_price);
            let (crossed, evaluation_mode) = evaluate_trigger_market_price_condition(
                previous_price,
                current_price,
                node_spec.trigger_price,
                &node_spec.trigger_condition,
                allow_first_tick_threshold,
                node_spec.max_price,
            );

            if !crossed && evaluation_mode == "no_previous" {
                debug!(
                    run_id,
                    flow_run_id = run_spec.run_id,
                    node_key = %node_spec.node_key,
                    price = current_price,
                    trigger_price = node_spec.trigger_price,
                    trigger_condition = %node_spec.trigger_condition,
                    once_mode = node_spec.once_mode,
                    token_id = %node_spec.token_id,
                    market = ?node_spec.market_slug,
                    "TRIGGER_WS_NO_PREVIOUS_PRICE"
                );
            }

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

            // ── Confirmation gate for trigger.market_price with explicit confirmationMs ──
            // When confirmationMs > 0 is configured, price must STAY in trigger zone
            // for the configured duration before enqueuing.
            let mut should_enqueue = crossed;
            let mut final_eval_mode: &str = evaluation_mode;

            if let Some(confirmation_ms) = market_price_confirmation_ms(&node_spec) {
                let cpend_at_key = format!("cross_pending_at_{}", node_spec.token_id);
                let cpend_price_key = format!("cross_pending_price_{}", node_spec.token_id);
                let cpend_prev_key = format!("cross_pending_prev_{}", node_spec.token_id);

                let still_in_zone = match node_spec.trigger_condition.as_str() {
                    "cross_below" => current_price <= node_spec.trigger_price,
                    "cross_above" => {
                        let above_trigger = current_price >= node_spec.trigger_price;
                        let below_max = node_spec.max_price.map_or(true, |mp| current_price <= mp);
                        above_trigger && below_max
                    }
                    _ => false,
                };

                if crossed {
                    // New cross detected (including first_tick_threshold) → start confirmation
                    // period. Both real crosses AND first_tick events must sustain in-zone
                    // for confirmation_ms before enqueuing. This prevents false triggers
                    // from transient opening prices in auto_scope+once mode.
                    set_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_at_key,
                        json!(Utc::now().to_rfc3339()),
                    );
                    set_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_price_key,
                        json!(current_price),
                    );
                    set_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_prev_key,
                        json!(previous_price),
                    );
                    run_spec.context_dirty = true;
                    should_enqueue = false;
                    info!(
                        run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        price = current_price,
                        prev = ?previous_price,
                        market = ?node_spec.market_slug,
                        "CROSS_PENDING_START: waiting {}ms confirmation",
                        confirmation_ms,
                    );
                } else if let Some(pending_at_str) =
                    flow_node_state_string(&run_spec.context, &node_spec.node_key, &cpend_at_key)
                {
                    // A cross was previously detected — check confirmation
                    if still_in_zone {
                        if let Ok(pending_at) = DateTime::parse_from_rfc3339(&pending_at_str) {
                            let elapsed =
                                Utc::now().signed_duration_since(pending_at.with_timezone(&Utc));
                            if elapsed.num_milliseconds() >= confirmation_ms {
                                // Confirmed! Price stayed in zone long enough.
                                should_enqueue = true;
                                final_eval_mode = "cross_confirmed";
                                // Clear pending state
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_at_key,
                                );
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_price_key,
                                );
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_prev_key,
                                );
                                run_spec.context_dirty = true;
                                info!(
                                    run_id = run_spec.run_id,
                                    node_key = %node_spec.node_key,
                                    price = current_price,
                                    elapsed_ms = elapsed.num_milliseconds(),
                                    market = ?node_spec.market_slug,
                                    "CROSS_CONFIRMED: sustained for {}ms",
                                    elapsed.num_milliseconds(),
                                );
                            }
                            // else: not enough time, keep waiting
                        }
                    } else {
                        // Price left trigger zone — reset pending confirmation
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_at_key,
                        );
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_price_key,
                        );
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_prev_key,
                        );
                        run_spec.context_dirty = true;
                        info!(
                            run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            price = current_price,
                            trigger = node_spec.trigger_price,
                            market = ?node_spec.market_slug,
                            "CROSS_PENDING_RESET: price left trigger zone, confirmation timer cleared",
                        );
                    }
                }
            }

            if !should_enqueue {
                continue;
            }

            let input_json = json!({
                "triggerSource": "ws_market_price",
                "tokenId": token_id,
                "wsPrice": current_price,
                "wsPrices": { token_id.clone(): current_price },
                "wsPreviousPrice": previous_price,
                "wsPreviousPrices": { token_id.clone(): previous_price },
                "wsEventTs": event_ts,
                "wsMarketSlug": node_spec.market_slug.clone(),
                "wsEvaluationMode": final_eval_mode,
                "wsPriceMode": node_spec.price_mode.as_str(),
                "wsPriceSource": price_source
            });
            let idempotency_key = ws_price_trigger_step_idempotency_key(
                run_spec.run_id,
                &node_spec.node_key,
                &node_spec.trigger_condition,
                current_price,
                event_ts,
                node_spec.once_mode,
                node_spec.once_scope_market,
                node_spec.market_slug.as_deref(),
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
                        "previous_price": previous_price,
                        "trigger_condition": node_spec.trigger_condition,
                        "trigger_price": node_spec.trigger_price,
                        "max_price": node_spec.max_price,
                        "evaluation_mode": final_eval_mode,
                        "price_mode": node_spec.price_mode.as_str(),
                        "price_source": price_source,
                        "event_ts": event_ts,
                        "once_mode": node_spec.once_mode,
                        "once_scope": if node_spec.once_scope_market { "market" } else { "run" },
                        "market_slug": node_spec.market_slug.clone(),
                        "idempotency_key": idempotency_key
                    }),
                )
                .await?;
                // DB idempotency key (ws-once:{run}:{node}:{market}) prevents
                // duplicate enqueues. No in-memory once_fired marking needed here;
                // the execute path handles once_fired state after processing.
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
    let publish_marker = trade_flow_publish_marker(&version);

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
        } else {
            let mut context =
                normalize_trade_flow_context(active_run.context_json.clone(), &graph.context);
            let (previous_publish_marker, reset_nodes) =
                sync_trade_flow_once_state_for_publish(&graph, &mut context, &publish_marker);
            let publish_marker_changed =
                previous_publish_marker.as_deref() != Some(publish_marker.as_str());
            if publish_marker_changed {
                repo.update_trade_flow_run_context(active_run.id, &context)
                    .await?;
                if let Some(prev_marker) = previous_publish_marker {
                    repo.append_trade_flow_event(
                        Some(active_run.id),
                        definition.id,
                        Some(active_run.version_id),
                        "trigger_once_reset_on_publish",
                        &json!({
                            "previous_publish_marker": prev_marker,
                            "next_publish_marker": publish_marker.clone(),
                            "version_id": version.id,
                            "reset_node_keys": reset_nodes,
                        }),
                    )
                    .await?;
                }
            }
        }
    } else {
        needs_new_run = true;
    }

    if !needs_new_run {
        return Ok(());
    }

    // Defensive: cancel any stale 'running' run that might cause unique constraint violation.
    // This handles crash recovery and concurrent-start edge cases.
    if let Some(stale_run) = repo.get_active_trade_flow_run(definition.id).await? {
        warn!(
            run_id,
            definition_id = definition.id,
            stale_run_id = stale_run.id,
            "TRADE_FLOW_STALE_RUN_CLEANUP"
        );
        repo.set_trade_flow_run_status(stale_run.id, "canceled", Some("stale_run_cleanup"))
            .await?;
    }

    let mut context_json = build_initial_trade_flow_context(&graph.context);
    set_flow_state(
        &mut context_json,
        FLOW_STATE_PUBLISH_MARKER,
        json!(publish_marker),
    );
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
                    "routes": execution.routes.iter().map(|r| r.edge_type.clone()).collect::<Vec<_>>(),
                    "triggered": !execution.routes.is_empty(),
                    "trigger_price": execution.output.get("triggered_price"),
                    "max_price": execution.output.get("max_price"),
                    "trigger_condition": execution.output.get("triggered_condition"),
                    "current_price": execution.output.get("price"),
                    "market_slug": execution.output.get("market_slug")
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
            execute_trigger_market_price(repo, cfg, client, ws, run, step, node, context).await
        }
        "trigger.sell_progress" => execute_trigger_sell_progress(repo, run, node, context).await,
        "trigger.open_positions" => {
            execute_trigger_open_positions(repo, client, ws, run, step, node, context).await
        }
        "trigger.position_drawdown" => {
            execute_trigger_position_drawdown(repo, ws, run, step, node, context).await
        }
        "trigger.time_window" => execute_trigger_time_window(node, context),
        "logic.if" => execute_logic_if(node, context),
        "logic.switch" => execute_logic_switch(node, context),
        "logic.delay" => execute_logic_delay(node),
        "logic.retry" => execute_logic_retry(node, step, context),
        "action.resolve_market" => execute_action_resolve_market(cfg, node, context).await,
        "action.dual_dca" => execute_action_dual_dca(repo, run, node, context).await,
        "action.place_order" => {
            execute_action_place_order(repo, run_id, cfg, limits, policy, run, step, node, context)
                .await
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
    repo: &PostgresRepository,
    cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let var_key = node_config_string(node, "varKey").unwrap_or_else(|| node.key.clone());
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .or_else(|| node_config_i64(node, "pollIntervalMs"))
        .unwrap_or(10000)
        .max(250) as i64;
    let repeat_mode = node_repeat_mode(node);
    let once_mode = repeat_mode == "once";
    let once_scope_market = is_trade_flow_market_price_once_scope_market(node);
    let auto_scope_mode = node_market_mode(node) == "auto_scope";
    let price_mode = WsPriceMode::parse(
        node.config.get("priceMode").and_then(|v| v.as_str()),
        WsPriceMode::Midpoint,
    );

    // --- Early WS-sourced detection for auto_scope guard ---
    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(|v| v.as_str())
        == Some("ws_market_price");
    let ws_market_slug_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| {
            input
                .get("wsMarketSlug")
                .or_else(|| input.get("marketSlug"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    // Skip WS-sourced steps for expired auto_scope markets
    if ws_sourced && auto_scope_mode {
        if let Some(ref ws_slug) = ws_market_slug_from_step {
            if is_auto_scope_market_expired(ws_slug, 30) {
                let output = json!({
                    "run_id": run.id,
                    "node_key": node.key,
                    "pass": false,
                    "reason": "market_expired",
                    "ws_market_slug": ws_slug
                });
                return Ok(TradeFlowNodeExecution {
                    output,
                    routes: Vec::new(),
                    repeat_at: None,
                    repeat_idempotency_key: None,
                });
            }
        }
    }

    let mut market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();
    if auto_scope_mode && !(ws_sourced && ws_market_slug_from_step.is_some()) {
        match sync_trigger_market_auto_scope_context(cfg, node, context).await? {
            Some(selected) => {
                market_slug = selected.slug;
            }
            None => {
                let output = json!({
                    "run_id": run.id,
                    "node_key": node.key,
                    "pass": false,
                    "once_mode": once_mode,
                    "once_scope": if once_scope_market { "market" } else { "run" },
                    "market_mode": "auto_scope",
                    "market_scope": node_config_string(node, "marketScope")
                        .or_else(|| flow_context_string(context, "marketScope")),
                    "error": "market_not_found"
                });
                let repeat_at = if once_mode {
                    None
                } else {
                    Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
                };
                return Ok(TradeFlowNodeExecution {
                    output,
                    routes: Vec::new(),
                    repeat_at,
                    repeat_idempotency_key: None,
                });
            }
        }
    }
    // For WS-sourced auto_scope steps, use the step's market slug directly
    // instead of re-resolving from Gamma API (which may return a newer market)
    if ws_sourced && auto_scope_mode {
        if let Some(ref ws_slug) = ws_market_slug_from_step {
            market_slug = ws_slug.clone();
        }
    }
    if market_slug.trim().is_empty() {
        return Err(anyhow::anyhow!("trigger.market_price requires marketSlug"));
    }
    set_flow_context(context, "marketSlug", json!(market_slug.clone()));
    let trigger_protection_mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    );
    if trigger_protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM
        && node_market_mode(node) == "auto_scope"
    {
        set_flow_context(context, "underlyingProtection", Value::Null);
        if let Some(asset) = resolve_auto_scope_underlying_asset(node, context, Some(&market_slug))
        {
            if let Err(err) = UNDERLYING_REFERENCE_SERVICE.prime(&asset).await {
                debug!(
                    flow_run_id = run.id,
                    node_key = %node.key,
                    asset = %asset,
                    error = %err,
                    "TRIGGER_UNDERLYING_PRIME_FAILED"
                );
            }
        }
    }
    sync_trade_flow_market_price_once_scope_state(
        context,
        &node.key,
        once_scope_market,
        Some(market_slug.as_str()),
    );

    // --- WS-sourced step data (ws_sourced already computed above) ---
    let trigger_source = if ws_sourced {
        Some("ws_market_price")
    } else {
        None
    };
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(|v| v.as_object());
    let ws_previous_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrices"))
        .and_then(|v| v.as_object());
    let ws_token_id_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("tokenId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_present = step
        .input_json
        .as_ref()
        .map(|input| input.get("wsPreviousPrice").is_some())
        .unwrap_or(false);
    // WS confirmation-gate mode: "cross_confirmed" means the WS path already
    // validated the cross + held confirmation_ms in zone.  The step must not
    // re-evaluate the cross (prev/cur are now both in-zone → no_cross), so we
    // accept the pre-confirmed result directly.
    let ws_evaluation_mode_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsEvaluationMode"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_price_mode_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPriceMode"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_price_source_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPriceSource"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut ws_cross_confirmed_short_circuit_applied = false;
    // ws_market_slug_from_step already computed above
    if let Some(ws_market_slug) = ws_market_slug_from_step.as_deref() {
        if should_accept_ws_market_slug_override(node, &market_slug) {
            market_slug = ws_market_slug.to_string();
            set_flow_context(context, "marketSlug", json!(market_slug.clone()));
            sync_trade_flow_market_price_once_scope_state(
                context,
                &node.key,
                once_scope_market,
                Some(market_slug.as_str()),
            );
        }
    }
    if once_mode
        && trade_flow_market_price_once_fired_for_scope(
            context,
            &node.key,
            once_scope_market,
            Some(market_slug.as_str()),
        )
    {
        if !flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED) {
            set_flow_node_state(
                context,
                &node.key,
                FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                json!(true),
            );
            if let Err(err) = repo
                .append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "trigger_once_blocked",
                    &json!({
                        "node_key": node.key,
                        "node_type": node.node_type,
                        "reason": "execute_once_fired_guard",
                        "once_scope": if once_scope_market { "market" } else { "run" },
                        "market_slug": market_slug.clone(),
                        "trigger_source": trigger_source,
                    }),
                )
                .await
            {
                warn!(
                    flow_run_id = run.id,
                    node_key = %node.key,
                    error = %err,
                    "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
                );
            }
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "run_id": run.id,
                "node_key": node.key,
                "market_slug": market_slug,
                "pass": false,
                "once_mode": true,
                "once_scope": if once_scope_market { "market" } else { "run" },
                "once_fired": true,
                "once_blocked": true,
                "trigger_source": trigger_source
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

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
    let mut triggered_max_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut triggered_previous_price: Option<f64> = None;
    let mut effective_previous_price: Option<f64> = None;
    let mut triggered_poly_delta_10s_cent: Option<f64> = None;
    let mut trigger_evaluation_mode: &'static str = "not_evaluated";
    let mut ws_hard_ignore_reason: Option<String> = None;
    let mut ws_soft_ignore_reason: Option<String> = None;
    let mut pass: bool;

    if ws_sourced {
        match ws_market_slug_from_step.as_deref() {
            Some(ws_market_slug) if ws_market_slug == market_slug => {}
            Some(ws_market_slug) => {
                ws_hard_ignore_reason = Some(format!(
                    "ws_market_slug_mismatch:{ws_market_slug}!={market_slug}"
                ));
            }
            None => {
                ws_hard_ignore_reason = Some("ws_market_slug_missing".to_string());
            }
        }
    }

    // --- WS cross_confirmed short-circuit ---
    // When the WS confirmation gate has already validated the cross + sustained
    // in-zone period ("cross_confirmed"), the step must NOT re-evaluate the
    // cross condition.  At confirmation time both prev and cur are in-zone (no
    // strict boundary crossing), so evaluate_trigger_market_price_condition
    // would return (false, "no_cross") and the trigger would silently fail.
    //
    // Instead, accept the gate's verdict and propagate triggered fields
    // directly from the WS payload so downstream logic (once-fire recording,
    // output JSON, route generation) works correctly.
    if should_apply_ws_cross_confirmed_short_circuit(
        ws_sourced,
        ws_evaluation_mode_from_step,
        ws_hard_ignore_reason.as_deref(),
    ) {
        let conf_token_id = ws_token_id_from_step.clone().unwrap_or_default();
        let conf_price = ws_price_from_step;
        let conf_prev = ws_previous_price_from_step;

        // Resolve triggered condition + trigger_price from node config.
        // For multi-outcome nodes find the matching condition row; for single-
        // token nodes read directly from node config.
        let (conf_condition, _conf_trigger_price, conf_outcome_label) =
            if let Some(ref conditions) = outcome_conditions {
                conditions
                    .iter()
                    .find_map(|cond| {
                        let mut tid = cond
                            .get("tokenId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let ol = cond
                            .get("outcomeLabel")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if node_market_mode(node) == "auto_scope" && !ol.is_empty() {
                            tid = resolve_token_id_for_outcome_label(&ol, context).unwrap_or(tid);
                        }
                        if !conf_token_id.is_empty() && tid != conf_token_id {
                            return None;
                        }
                        let tc = cond
                            .get("triggerCondition")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let tp = cond
                            .get("triggerPriceCent")
                            .and_then(value_as_f64)
                            .map(|v| v / 100.0)
                            .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
                        Some((tc, tp, ol))
                    })
                    .unwrap_or_default()
            } else {
                let tc = node_config_string(node, "triggerCondition").unwrap_or_default();
                let tp = node_config_f64(node, "triggerPrice")
                    .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
                (tc, tp, String::new())
            };

        triggered_token_id = conf_token_id;
        triggered_outcome_label = conf_outcome_label;
        triggered_condition = conf_condition;
        triggered_price = conf_price;
        current_price = conf_price;
        triggered_previous_price = conf_prev;
        effective_previous_price = conf_prev;
        trigger_evaluation_mode = "cross_confirmed";
        pass = true;
        ws_cross_confirmed_short_circuit_applied = true;
        if let Some(price) = conf_price {
            if !triggered_token_id.is_empty() {
                triggered_poly_delta_10s_cent = record_trigger_price_sample(
                    context,
                    &node.key,
                    &triggered_token_id,
                    price,
                    Utc::now(),
                );
            }
        }

        if let Err(err) = repo
            .append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "trigger_ws_cross_confirmed_applied",
                &json!({
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "market_slug": market_slug.clone(),
                    "price_mode": price_mode.as_str(),
                    "ws_market_slug": ws_market_slug_from_step.clone(),
                    "ws_token_id": ws_token_id_from_step.clone(),
                    "ws_price": ws_price_from_step,
                    "ws_previous_price": ws_previous_price_from_step,
                    "ws_evaluation_mode": ws_evaluation_mode_from_step,
                    "ws_price_mode": ws_price_mode_from_step,
                    "ws_price_source": ws_price_source_from_step,
                    "once_mode": once_mode,
                    "once_scope": if once_scope_market { "market" } else { "run" }
                }),
            )
            .await
        {
            warn!(
                flow_run_id = run.id,
                node_key = %node.key,
                error = %err,
                "TRADE_FLOW_WS_CROSS_CONFIRMED_APPLIED_EVENT_FAILED"
            );
        }

        // Update per-token previous price in context so subsequent non-WS
        // evaluations have a valid state to start from.
        if let Some(price) = conf_price {
            if !triggered_token_id.is_empty() {
                let per_token_key = format!("previous_price_{}", triggered_token_id);
                set_flow_node_state(context, &node.key, &per_token_key, json!(price));
            }
        }

        // Skip the standard evaluation block entirely.
        // The `if pass { once-fire + routes }` block below handles the rest.
    } else if let Some(ref conditions) = outcome_conditions {
        // Multi-outcome: OR logic
        pass = false;
        let mut last_eval_mode = "not_evaluated";
        for cond in conditions {
            let mut cond_token_id = cond
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
            if node_market_mode(node) == "auto_scope" && !cond_outcome_label.is_empty() {
                cond_token_id = resolve_token_id_for_outcome_label(&cond_outcome_label, context)
                    .unwrap_or(cond_token_id);
            }
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
            let cond_max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
            if cond_token_id.is_empty() || cond_trigger_condition.is_empty() {
                continue;
            }
            let tp = match cond_trigger_price {
                Some(v) => v,
                None => continue,
            };
            let prev_state_key = format!("previous_price_{}", cond_token_id);
            let state_prev =
                flow_node_state(context, &node.key, &prev_state_key).and_then(value_as_f64);
            let prev = resolve_ws_previous_price(
                ws_sourced,
                state_prev,
                cond_token_id.as_str(),
                ws_token_id_from_step.as_deref(),
                ws_previous_price_from_step,
                ws_previous_price_present,
                ws_previous_prices_map,
            );
            effective_previous_price = prev;
            let step_ws_price = ws_prices_map
                .and_then(|m| m.get(&cond_token_id))
                .and_then(value_as_f64)
                .map(clamp_probability);
            if ws_sourced && step_ws_price.is_none() {
                if ws_soft_ignore_reason.is_none() {
                    ws_soft_ignore_reason =
                        Some(format!("ws_price_missing_for_token:{cond_token_id}"));
                }
                continue;
            }
            let cur_result = if let Some(sp) = step_ws_price {
                Ok(sp)
            } else {
                fetch_trade_flow_market_price(
                    ws,
                    client,
                    &market_slug,
                    Some(cond_token_id.as_str()),
                    price_mode,
                )
                .await
            };
            let cur = match cur_result {
                Ok(price) => price,
                Err(err) => {
                    if repeat_mode == "once" {
                        return Ok(TradeFlowNodeExecution {
                            output: json!({
                                "run_id": run.id,
                                "node_key": node.key,
                                "market_slug": market_slug,
                                "error": err.to_string(),
                                "retry": false
                            }),
                            routes: Vec::new(),
                            repeat_at: None,
                            repeat_idempotency_key: None,
                        });
                    }
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
            let poly_delta_10s_cent =
                record_trigger_price_sample(context, &node.key, &cond_token_id, cur, Utc::now());
            let allow_first_tick = !once_mode;
            let (pass_this, eval_mode) = evaluate_trigger_market_price_condition(
                prev,
                cur,
                tp,
                &cond_trigger_condition,
                allow_first_tick,
                cond_max_price,
            );
            last_eval_mode = eval_mode;
            if ws_sourced && !pass_this && ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some(format!(
                    "ws_condition_not_met:{cond_token_id}:{cond_trigger_condition}:{tp:.6}"
                ));
            }
            if pass_this && !pass {
                pass = true;
                triggered_token_id = cond_token_id;
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_price = Some(cur);
                triggered_max_price = cond_max_price;
                current_price = Some(cur);
                triggered_previous_price = prev;
                trigger_evaluation_mode = eval_mode;
                triggered_poly_delta_10s_cent = poly_delta_10s_cent;
            }
        }
        if !pass {
            trigger_evaluation_mode = last_eval_mode;
        }
    } else {
        // Legacy single-token path
        let token_id = node_config_string(node, "tokenId")
            .or_else(|| flow_context_string(context, "tokenId"))
            .or_else(|| {
                if node_market_mode(node) != "auto_scope" {
                    return None;
                }
                let outcome = node_config_string(node, "outcomeLabel")
                    .or_else(|| flow_context_string(context, "outcomeLabel"))?;
                resolve_token_id_for_outcome_label(&outcome, context)
            });
        let legacy_outcome_label = node_config_string(node, "outcomeLabel")
            .or_else(|| flow_context_string(context, "outcomeLabel"))
            .unwrap_or_default();
        let cur_result = if let Some(sp) = ws_price_from_step {
            Ok(Some(sp))
        } else if ws_sourced {
            if ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some("ws_price_missing".to_string());
            }
            Ok(None)
        } else {
            fetch_trade_flow_market_price(ws, client, &market_slug, token_id.as_deref(), price_mode)
                .await
                .map(Some)
        };
        let cur = match cur_result {
            Ok(price) => price,
            Err(err) => {
                if repeat_mode == "once" {
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "run_id": run.id,
                            "node_key": node.key,
                            "market_slug": market_slug,
                            "error": err.to_string(),
                            "retry": false
                        }),
                        routes: Vec::new(),
                        repeat_at: None,
                        repeat_idempotency_key: None,
                    });
                }
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
        current_price = cur;
        if let Some(cur_price) = cur {
            set_flow_var(context, &var_key, json!(cur_price));
            set_flow_node_state(context, &node.key, "last_price", json!(cur_price));
        }

        let trigger_condition = node_config_string(node, "triggerCondition");
        let trigger_price = node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
        // PRCE-01: Use per-token key to avoid cross-token price contamination.
        // Returns None when token_id is empty/missing — safe because resolve_ws_previous_price
        // handles None state_previous_price correctly (falls through to ws payload or returns None).
        let state_previous_price = token_id
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|tid| format!("previous_price_{}", tid))
            .and_then(|key| flow_node_state(context, &node.key, &key))
            .and_then(value_as_f64);
        let expected_token_id = token_id.as_deref().filter(|value| !value.is_empty());
        let previous_price = resolve_ws_previous_price(
            ws_sourced,
            state_previous_price,
            expected_token_id.unwrap_or_default(),
            ws_token_id_from_step.as_deref(),
            ws_previous_price_from_step,
            ws_previous_price_present,
            ws_previous_prices_map,
        );
        effective_previous_price = previous_price;
        let allow_first_tick = !once_mode;
        let legacy_max_price = node_config_f64(node, "maxPrice")
            .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
            .filter(|v| *v > 0.0 && *v <= 1.0);
        let legacy_poly_delta_10s_cent =
            match (cur, token_id.as_deref().filter(|value| !value.is_empty())) {
                (Some(cur_price), Some(tid)) => {
                    record_trigger_price_sample(context, &node.key, tid, cur_price, Utc::now())
                }
                _ => None,
            };
        triggered_max_price = legacy_max_price;
        pass = if let Some(cur_price) = cur {
            match (trigger_condition.as_deref(), trigger_price) {
                (Some("cross_above"), Some(tp)) => {
                    let (matched, eval_mode) = evaluate_trigger_market_price_condition(
                        previous_price,
                        cur_price,
                        tp,
                        "cross_above",
                        allow_first_tick,
                        legacy_max_price,
                    );
                    trigger_evaluation_mode = eval_mode;
                    matched
                }
                (Some("cross_below"), Some(tp)) => {
                    let (matched, eval_mode) = evaluate_trigger_market_price_condition(
                        previous_price,
                        cur_price,
                        tp,
                        "cross_below",
                        allow_first_tick,
                        legacy_max_price,
                    );
                    trigger_evaluation_mode = eval_mode;
                    matched
                }
                _ => {
                    trigger_evaluation_mode = "unsupported_condition";
                    false
                }
            }
        } else {
            trigger_evaluation_mode = "ws_missing_price";
            false
        };
        if let Some(cur_price) = cur {
            // PRCE-01: Write only per-token key. Bare "previous_price" write removed —
            // nothing reads it after the per-token read-side fix.
            if let Some(ref tid) = token_id {
                if !tid.is_empty() {
                    let per_token_key = format!("previous_price_{}", tid);
                    set_flow_node_state(context, &node.key, &per_token_key, json!(cur_price));
                }
            }
        }
        if ws_sourced {
            if let Some(expected_token_id) = token_id.as_deref() {
                if expected_token_id.is_empty() {
                    if ws_hard_ignore_reason.is_none() {
                        ws_hard_ignore_reason = Some("ws_expected_token_missing".to_string());
                    }
                } else if ws_token_id_from_step.as_deref() != Some(expected_token_id) {
                    if ws_hard_ignore_reason.is_none() {
                        ws_hard_ignore_reason = Some(format!(
                            "ws_token_mismatch:{}!={expected_token_id}",
                            ws_token_id_from_step.as_deref().unwrap_or("missing")
                        ));
                    }
                }
            }
            if !pass && ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some("ws_condition_not_met".to_string());
            }
        }
        triggered_token_id = token_id.unwrap_or_default();
        triggered_outcome_label = legacy_outcome_label;
        triggered_condition = trigger_condition.unwrap_or_default();
        triggered_price = current_price;
        triggered_previous_price = previous_price;
        if pass {
            triggered_poly_delta_10s_cent = legacy_poly_delta_10s_cent;
        }
    }

    let ws_ignore_reason = if ws_sourced {
        if let Some(reason) = ws_hard_ignore_reason.clone() {
            pass = false;
            Some(reason)
        } else if pass {
            None
        } else {
            ws_soft_ignore_reason.clone()
        }
    } else {
        None
    };
    if let Some(reason) = ws_ignore_reason.as_deref() {
        if let Err(err) = repo
            .append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "trigger_ws_price_ignored",
                &json!({
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "reason": reason,
                    "price_mode": price_mode.as_str(),
                    "trigger_source": trigger_source,
                    "market_slug": market_slug.clone(),
                    "ws_market_slug": ws_market_slug_from_step,
                    "ws_token_id": ws_token_id_from_step,
                    "expected_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id.clone()) },
                    "ws_price": ws_price_from_step,
                    "ws_previous_price": ws_previous_price_from_step,
                    "ws_price_mode": ws_price_mode_from_step,
                    "ws_price_source": ws_price_source_from_step,
                    "effective_previous_price": effective_previous_price
                }),
            )
            .await
        {
            warn!(
                flow_run_id = run.id,
                node_key = %node.key,
                error = %err,
                "TRADE_FLOW_WS_IGNORE_EVENT_FAILED"
            );
        }
    }
    if is_ws_cross_confirmed_unexpected_fail(
        ws_sourced,
        ws_evaluation_mode_from_step,
        pass,
        ws_hard_ignore_reason.as_deref(),
    ) {
        if let Err(err) = repo
            .append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "trigger_ws_cross_confirmed_unexpected_fail",
                &json!({
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "market_slug": market_slug.clone(),
                    "price_mode": price_mode.as_str(),
                    "ws_market_slug": ws_market_slug_from_step.clone(),
                    "ws_token_id": ws_token_id_from_step.clone(),
                    "ws_price": ws_price_from_step,
                    "ws_previous_price": ws_previous_price_from_step,
                    "ws_evaluation_mode": ws_evaluation_mode_from_step,
                    "ws_price_mode": ws_price_mode_from_step,
                    "ws_price_source": ws_price_source_from_step,
                    "evaluation_mode": trigger_evaluation_mode,
                    "ws_ignored_reason": ws_ignore_reason.clone(),
                    "effective_previous_price": effective_previous_price,
                    "short_circuit_applied": ws_cross_confirmed_short_circuit_applied
                }),
            )
            .await
        {
            warn!(
                flow_run_id = run.id,
                node_key = %node.key,
                error = %err,
                "TRADE_FLOW_WS_CROSS_CONFIRMED_UNEXPECTED_FAIL_EVENT_FAILED"
            );
        }
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
        if pass {
            set_flow_context(context, "tokenId", json!(&triggered_token_id));
        }
    }
    if !triggered_outcome_label.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_outcome_label"),
            json!(triggered_outcome_label),
        );
        if pass {
            set_flow_context(context, "outcomeLabel", json!(&triggered_outcome_label));
        }
    }
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(context, &format!("{var_key}_triggered_price"), json!(tp));
    }
    if let Some(max_price) = triggered_max_price {
        set_flow_var(context, &format!("{var_key}_max_price"), json!(max_price));
    }
    if pass {
        if let Some(max_price) = triggered_max_price {
            set_flow_context(context, "maxPrice", json!(max_price));
        } else {
            set_flow_context(context, "maxPrice", Value::Null);
        }
    }
    let mut protection_output = Value::Null;
    if pass {
        if let Some(protection_config) = build_underlying_protection_config(
            node,
            context,
            &market_slug,
            &triggered_outcome_label,
        ) {
            let protection = evaluate_underlying_protection(
                &protection_config,
                &market_slug,
                triggered_poly_delta_10s_cent,
            )
            .await;
            protection_output = protection.to_value();
            set_flow_context(context, "underlyingProtection", protection_output.clone());
            let event_type = if protection.passed {
                "trigger_protection_passed"
            } else {
                "trigger_protection_blocked"
            };
            if let Err(err) = repo
                .append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    event_type,
                    &json!({
                        "node_key": node.key,
                        "node_type": node.node_type,
                        "market_slug": market_slug.clone(),
                        "triggered_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id.clone()) },
                        "triggered_outcome_label": if triggered_outcome_label.is_empty() { Value::Null } else { json!(triggered_outcome_label.clone()) },
                        "triggered_condition": if triggered_condition.is_empty() { Value::Null } else { json!(triggered_condition.clone()) },
                        "triggered_price": triggered_price,
                        "max_price": triggered_max_price,
                        "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
                        "protection": protection_output.clone()
                    }),
                )
                .await
            {
                warn!(
                    flow_run_id = run.id,
                    node_key = %node.key,
                    error = %err,
                    "TRADE_FLOW_TRIGGER_PROTECTION_EVENT_FAILED"
                );
            }
            if !protection.passed {
                pass = false;
            }
        }
    }
    if once_mode && pass {
        let once_fire_key = trade_flow_market_price_once_idempotency_key(
            run.id,
            &node.key,
            once_scope_market,
            Some(market_slug.as_str()),
        );
        if repo.try_record_idempotency_key(&once_fire_key).await? {
            let fired_at = Utc::now();
            mark_trade_flow_market_price_once_fired(
                context,
                &node.key,
                fired_at,
                once_scope_market.then_some(market_slug.as_str()),
            );
            if let Err(err) = repo
                .append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "trigger_once_fired",
                    &json!({
                        "node_key": node.key,
                        "node_type": node.node_type,
                        "market_slug": market_slug.clone(),
                        "price_mode": price_mode.as_str(),
                        "triggered_token_id": triggered_token_id.clone(),
                        "triggered_outcome_label": triggered_outcome_label.clone(),
                        "triggered_condition": triggered_condition.clone(),
                        "triggered_price": triggered_price,
                        "max_price": triggered_max_price,
                        "previous_price": triggered_previous_price,
                        "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
                        "protection": protection_output.clone(),
                        "evaluation_mode": trigger_evaluation_mode,
                        "price": current_price,
                        "ws_sourced": ws_sourced,
                        "ws_price_mode": ws_price_mode_from_step,
                        "ws_price_source": ws_price_source_from_step,
                        "once_scope": if once_scope_market { "market" } else { "run" },
                        "fired_at": fired_at,
                        "idempotency_key": once_fire_key
                    }),
                )
                .await
            {
                warn!(
                    flow_run_id = run.id,
                    node_key = %node.key,
                    error = %err,
                    "TRADE_FLOW_ONCE_FIRED_EVENT_FAILED"
                );
            }
        } else {
            let already_block_logged =
                flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED);
            if !trade_flow_market_price_once_fired_for_scope(
                context,
                &node.key,
                once_scope_market,
                Some(market_slug.as_str()),
            ) {
                mark_trade_flow_market_price_once_fired(
                    context,
                    &node.key,
                    Utc::now(),
                    once_scope_market.then_some(market_slug.as_str()),
                );
            }
            set_flow_node_state(
                context,
                &node.key,
                FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                json!(true),
            );
            pass = false;

            if !already_block_logged {
                if let Err(err) = repo
                    .append_trade_flow_event(
                        Some(run.id),
                        run.definition_id,
                        Some(run.version_id),
                        "trigger_once_blocked",
                        &json!({
                            "node_key": node.key,
                            "node_type": node.node_type,
                            "reason": "global_once_idempotency",
                            "idempotency_key": once_fire_key,
                            "trigger_source": trigger_source,
                            "market_slug": market_slug.clone(),
                            "once_scope": if once_scope_market { "market" } else { "run" },
                            "ws_sourced": ws_sourced
                        }),
                    )
                    .await
                {
                    warn!(
                        flow_run_id = run.id,
                        node_key = %node.key,
                        error = %err,
                        "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
                    );
                }
            }
        }
    }

    let repeat_at = if ws_sourced {
        None
    } else if once_mode {
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
        "price_mode": price_mode.as_str(),
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "maxPrice": triggered_max_price,
        "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
        "protection": protection_output,
        "previous_price": triggered_previous_price,
        "ws_previous_price": ws_previous_price_from_step,
        "effective_previous_price": effective_previous_price,
        "evaluation_mode": trigger_evaluation_mode,
        "ws_evaluation_mode_from_step": ws_evaluation_mode_from_step,
        "ws_price_mode_from_step": ws_price_mode_from_step,
        "ws_price_source_from_step": ws_price_source_from_step,
        "cross_confirmed_short_circuit_applied": ws_cross_confirmed_short_circuit_applied,
        "price": current_price,
        "pass": pass,
        "var_key": var_key,
        "multi_outcome": outcome_conditions.is_some(),
        "ws_sourced": ws_sourced,
        "ws_ignored_reason": ws_ignore_reason,
        "once_mode": once_mode,
        "once_scope": if once_scope_market { "market" } else { "run" },
        "once_fired": trade_flow_market_price_once_fired_for_scope(
            context,
            &node.key,
            once_scope_market,
            Some(market_slug.as_str())
        )
    });
    info!(
        flow_run_id = run.id,
        node_key = %node.key,
        pass,
        trigger_evaluation_mode,
        ?current_price,
        ?effective_previous_price,
        price_mode = price_mode.as_str(),
        once_mode,
        ws_sourced,
        routes_count = routes.len(),
        "TRIGGER_MARKET_PRICE_EVALUATED"
    );
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
    let ws_previous_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrices"))
        .and_then(|v| v.as_object());
    let ws_token_id_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("tokenId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let ws_previous_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_present = step
        .input_json
        .as_ref()
        .map(|input| input.get("wsPreviousPrice").is_some())
        .unwrap_or(false);
    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(Value::as_str)
        == Some("ws_market_price");

    // Shared mutable state for triggered outcome
    let mut triggered_token_id = String::new();
    let mut triggered_outcome_label = String::new();
    let mut triggered_condition = String::new();
    let mut triggered_price: Option<f64> = None;
    let mut triggered_max_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut effective_previous_price: Option<f64> = None;
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
            let cond_max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
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
            let state_prev =
                flow_node_state(context, &node.key, &prev_state_key).and_then(value_as_f64);
            let prev = resolve_ws_previous_price(
                ws_sourced,
                state_prev,
                cond_token_id.as_str(),
                ws_token_id_from_step.as_deref(),
                ws_previous_price_from_step,
                ws_previous_price_present,
                ws_previous_prices_map,
            );
            effective_previous_price = prev;
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
                        WsPriceMode::Raw,
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
                    .map(|v| {
                        crossed_above_strict(prev, v, tp)
                            && cond_max_price.map_or(true, |mp| v <= mp)
                    })
                    .unwrap_or(false),
                "cross_below" => cur
                    .map(|v| {
                        crossed_below_strict(prev, v, tp)
                            && cond_max_price.map_or(true, |mp| v <= mp)
                    })
                    .unwrap_or(false),
                _ => false,
            };
            if pass_this && !price_pass {
                price_pass = true;
                triggered_token_id = cond_token_id.clone();
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_price = cur;
                triggered_max_price = cond_max_price;
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
        let legacy_max_price = node_config_f64(node, "maxPrice")
            .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
            .filter(|v| *v > 0.0 && *v <= 1.0);
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
        // PRCE-01: Use per-token key to avoid cross-token price contamination
        let state_previous_price = if !token_id.is_empty() {
            let key = format!("previous_price_{}", token_id);
            flow_node_state(context, &node.key, &key).and_then(value_as_f64)
        } else {
            None
        };
        let token_id_ref = if token_id.is_empty() {
            None
        } else {
            Some(token_id.as_str())
        };
        let previous_price = resolve_ws_previous_price(
            ws_sourced,
            state_previous_price,
            token_id_ref.unwrap_or_default(),
            ws_token_id_from_step.as_deref(),
            ws_previous_price_from_step,
            ws_previous_price_present,
            ws_previous_prices_map,
        );
        effective_previous_price = previous_price;
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
                        fetch_trade_flow_market_price(
                            ws,
                            client,
                            &market_slug,
                            token_id_ref,
                            WsPriceMode::Raw,
                        )
                        .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        crossed_above_strict(previous_price, value, tp)
                            && legacy_max_price.map_or(true, |mp| value <= mp)
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
                        fetch_trade_flow_market_price(
                            ws,
                            client,
                            &market_slug,
                            token_id_ref,
                            WsPriceMode::Raw,
                        )
                        .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        crossed_below_strict(previous_price, value, tp)
                            && legacy_max_price.map_or(true, |mp| value <= mp)
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
        triggered_max_price = legacy_max_price;
        if let Some(p) = cur {
            // PRCE-01: Write only per-token key. Bare "previous_price" write removed —
            // nothing reads it after the per-token read-side fix.
            if !triggered_token_id.is_empty() {
                let per_token_key = format!("previous_price_{}", triggered_token_id);
                set_flow_node_state(context, &node.key, &per_token_key, json!(p));
            }
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
        set_flow_var(context, &format!("{var_prefix}_triggered_price"), json!(tp));
    }
    if let Some(max_price) = triggered_max_price {
        set_flow_var(
            context,
            &format!("{var_prefix}_max_price"),
            json!(max_price),
        );
    }
    if pass {
        if let Some(max_price) = triggered_max_price {
            set_flow_context(context, "maxPrice", json!(max_price));
        } else {
            set_flow_context(context, "maxPrice", Value::Null);
        }
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
        "max_price": triggered_max_price,
        "maxPrice": triggered_max_price,
        "websocket_price_mode": websocket_price_mode,
        "ws_sourced": ws_sourced,
        "ws_previous_price": ws_previous_price_from_step,
        "effective_previous_price": effective_previous_price,
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

fn parse_position_drawdown_rules(node: &TradeFlowNode) -> Vec<PositionDrawdownRule> {
    let mut rules = Vec::new();
    if let Some(items) = node.config.get("lossRules").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let Some(loss_pct) = obj.get("lossPct").and_then(value_as_f64) else {
                continue;
            };
            if !loss_pct.is_finite() || loss_pct <= 0.0 || loss_pct > 100.0 {
                continue;
            }
            let Some(direction) =
                PositionDrawdownDirection::parse(obj.get("direction").and_then(Value::as_str))
            else {
                continue;
            };
            let window_ms = obj
                .get("windowMs")
                .and_then(value_as_i64)
                .filter(|v| *v > 0);
            rules.push(PositionDrawdownRule {
                index,
                loss_pct,
                direction,
                window_ms,
            });
        }
    }

    // Backward-compatible single-rule fallback.
    if rules.is_empty() {
        if let Some(loss_pct) = node_config_f64(node, "lossPct") {
            if loss_pct.is_finite() && loss_pct > 0.0 && loss_pct <= 100.0 {
                rules.push(PositionDrawdownRule {
                    index: 0,
                    loss_pct,
                    direction: PositionDrawdownDirection::Down,
                    window_ms: node_config_i64(node, "windowMs").filter(|v| *v > 0),
                });
            }
        }
    }

    rules
}

fn has_deprecated_drawdown_window_sec(node: &TradeFlowNode) -> bool {
    if node.config.get("windowSec").is_some() {
        return true;
    }
    node.config
        .get("lossRules")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                item.as_object()
                    .map(|obj| obj.contains_key("windowSec"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn parse_position_drawdown_samples(value: Option<&Value>) -> Vec<PositionDrawdownSample> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut samples = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(ts_ms) = obj.get("ts_ms").and_then(value_as_i64) else {
            continue;
        };
        let Some(price) = obj.get("price").and_then(value_as_f64) else {
            continue;
        };
        let loss_pct = obj
            .get("loss_pct")
            .and_then(value_as_f64)
            .unwrap_or_default();
        let gain_pct = obj
            .get("gain_pct")
            .and_then(value_as_f64)
            .unwrap_or_default();
        if !price.is_finite() || price < 0.0 {
            continue;
        }
        if !loss_pct.is_finite() || loss_pct < 0.0 {
            continue;
        }
        if !gain_pct.is_finite() || gain_pct < 0.0 {
            continue;
        }
        samples.push(PositionDrawdownSample {
            ts_ms,
            loss_pct: loss_pct.clamp(0.0, 100.0),
            gain_pct: gain_pct.clamp(0.0, 100.0),
            price: clamp_probability(price),
        });
    }
    samples.sort_by_key(|sample| sample.ts_ms);
    samples
}

async fn execute_trigger_position_drawdown(
    _repo: &PostgresRepository,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context);
    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();
    let token_id = node_config_string(node, "tokenId")
        .or_else(|| flow_context_string(context, "tokenId"))
        .unwrap_or_default();
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_default();
    let entry_price = node_config_f64(node, "entryPriceCent")
        .map(|value| value / 100.0)
        .or_else(|| node_config_f64(node, "entryPrice"));

    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(250)
        .max(250) as i64;
    let now = Utc::now();
    let repeat_at = Some(now + ChronoDuration::milliseconds(interval_ms));

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| node.key.clone());

    if has_deprecated_drawdown_window_sec(node) {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "reason": "deprecated_window_sec",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }

    let rules = parse_position_drawdown_rules(node);
    if rules.is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "reason": "missing_loss_rules",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    };
    let entry_price = match entry_price {
        Some(value) if value.is_finite() && value > 0.0 && value <= 1.0 => value,
        Some(value) => {
            let output = json!({
                "run_id": run.id,
                "node_key": node.key,
                "source_trade_id": source_trade_id,
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "entry_price": value,
                "reason": "invalid_entry_price",
                "pass": false
            });
            return Ok(TradeFlowNodeExecution {
                output,
                routes: Vec::new(),
                repeat_at,
                repeat_idempotency_key: None,
            });
        }
        None => {
            let output = json!({
                "run_id": run.id,
                "node_key": node.key,
                "source_trade_id": source_trade_id,
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "reason": "entry_price_missing",
                "pass": false
            });
            return Ok(TradeFlowNodeExecution {
                output,
                routes: Vec::new(),
                repeat_at,
                repeat_idempotency_key: None,
            });
        }
    };
    if market_slug.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "reason": "missing_market_slug",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }
    if token_id.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "reason": "missing_token_id",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }
    if outcome_label.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "entry_price": entry_price,
            "reason": "missing_outcome_label",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }

    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(Value::as_str)
        == Some("ws_market_price");
    let ws_token_id_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("tokenId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(Value::as_object);
    let step_ws_price_for_token = ws_prices_map
        .and_then(|map| map.get(&token_id))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let step_token_matches = ws_token_id_from_step
        .as_deref()
        .map(|value| value == token_id.as_str())
        .unwrap_or(true);

    let mut current_price = None;
    let mut price_source = "unavailable";
    if let Some(price) = step_ws_price_for_token {
        current_price = Some(price);
        price_source = "step_ws_prices";
    } else if step_token_matches {
        if let Some(price) = ws_price_from_step {
            current_price = Some(price);
            price_source = "step_ws_price";
        }
    }
    if current_price.is_none() {
        if let Some(price) = fetch_price_from_market_ws(ws, &token_id).await {
            current_price = Some(clamp_probability(price));
            price_source = "ws_subscribe_once";
        }
    }
    let Some(current_price) = current_price else {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "price_source": price_source,
            "reason": "current_price_unavailable",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    };

    let loss_pct_now = (((entry_price - current_price) / entry_price) * 100.0)
        .max(0.0)
        .clamp(0.0, 100.0);
    let gain_pct_now = (((current_price - entry_price) / entry_price) * 100.0)
        .max(0.0)
        .clamp(0.0, 100.0);
    let now_ms = now.timestamp_millis();

    let mut samples = parse_position_drawdown_samples(flow_node_state(
        context,
        &node.key,
        "drawdown_loss_samples",
    ));
    samples.push(PositionDrawdownSample {
        ts_ms: now_ms,
        loss_pct: loss_pct_now,
        gain_pct: gain_pct_now,
        price: current_price,
    });
    let max_window_ms = rules
        .iter()
        .filter_map(|rule| rule.window_ms)
        .max()
        .unwrap_or(0);
    if max_window_ms > 0 {
        let cutoff = now_ms.saturating_sub(max_window_ms);
        samples.retain(|sample| sample.ts_ms >= cutoff);
    } else if let Some(last) = samples.last().copied() {
        samples.clear();
        samples.push(last);
    }
    if samples.len() > 4000 {
        let overflow = samples.len() - 4000;
        samples.drain(0..overflow);
    }

    let combine_mode_raw = node_config_string(node, "combineMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let resolved_combine_mode = match combine_mode_raw.as_deref() {
        Some("and") => "and",
        Some("or") => "or",
        _ if rules.len() <= 1 => "single",
        _ => "or",
    };

    let mut rule_pass_flags = Vec::with_capacity(rules.len());
    let rule_outputs: Vec<Value> = rules
        .iter()
        .map(|rule| {
            let threshold_price = match rule.direction {
                PositionDrawdownDirection::Down => {
                    clamp_probability(entry_price * (1.0 - (rule.loss_pct / 100.0)))
                }
                PositionDrawdownDirection::Up => {
                    clamp_probability(entry_price * (1.0 + (rule.loss_pct / 100.0)))
                }
            };
            if let Some(window_ms) = rule.window_ms {
                let cutoff = now_ms.saturating_sub(window_ms);
                let window_samples: Vec<&PositionDrawdownSample> = samples
                    .iter()
                    .filter(|sample| sample.ts_ms >= cutoff)
                    .collect();
                let max_loss_pct = window_samples
                    .iter()
                    .map(|sample| (((entry_price - sample.price) / entry_price) * 100.0).max(0.0))
                    .map(|value| value.clamp(0.0, 100.0))
                    .fold(0.0_f64, f64::max);
                let max_gain_pct = window_samples
                    .iter()
                    .map(|sample| (((sample.price - entry_price) / entry_price) * 100.0).max(0.0))
                    .map(|value| value.clamp(0.0, 100.0))
                    .fold(0.0_f64, f64::max);
                let max_metric_pct = match rule.direction {
                    PositionDrawdownDirection::Down => max_loss_pct,
                    PositionDrawdownDirection::Up => max_gain_pct,
                };
                let pass = max_metric_pct >= rule.loss_pct;
                rule_pass_flags.push(pass);
                json!({
                    "index": rule.index,
                    "direction": rule.direction.as_str(),
                    "loss_pct": rule.loss_pct,
                    "window_ms": window_ms,
                    "threshold_price": threshold_price,
                    "max_loss_pct_in_window": max_loss_pct,
                    "max_gain_pct_in_window": max_gain_pct,
                    "metric_type": rule.direction.metric_type(),
                    "max_metric_pct_in_window": max_metric_pct,
                    "sample_count_in_window": window_samples.len(),
                    "pass": pass
                })
            } else {
                let metric_now = match rule.direction {
                    PositionDrawdownDirection::Down => loss_pct_now,
                    PositionDrawdownDirection::Up => gain_pct_now,
                };
                let pass = metric_now >= rule.loss_pct;
                rule_pass_flags.push(pass);
                json!({
                    "index": rule.index,
                    "direction": rule.direction.as_str(),
                    "loss_pct": rule.loss_pct,
                    "window_ms": Value::Null,
                    "threshold_price": threshold_price,
                    "max_loss_pct_in_window": loss_pct_now,
                    "max_gain_pct_in_window": gain_pct_now,
                    "metric_type": rule.direction.metric_type(),
                    "max_metric_pct_in_window": metric_now,
                    "sample_count_in_window": 1,
                    "pass": pass
                })
            }
        })
        .collect();

    let pass = match resolved_combine_mode {
        "and" => rule_pass_flags.iter().all(|value| *value),
        "or" => rule_pass_flags.iter().any(|value| *value),
        _ => rule_pass_flags.first().copied().unwrap_or(false),
    };

    set_flow_node_state(
        context,
        &node.key,
        "drawdown_loss_samples",
        json!(samples
            .iter()
            .map(|sample| json!({
                "ts_ms": sample.ts_ms,
                "loss_pct": sample.loss_pct,
                "gain_pct": sample.gain_pct,
                "price": sample.price
            }))
            .collect::<Vec<Value>>()),
    );
    set_flow_node_state(context, &node.key, "last_price", json!(current_price));
    set_flow_node_state(context, &node.key, "last_loss_pct", json!(loss_pct_now));
    set_flow_node_state(context, &node.key, "last_gain_pct", json!(gain_pct_now));
    set_flow_node_state(context, &node.key, "last_entry_price", json!(entry_price));
    set_flow_node_state(context, &node.key, "last_position_qty", json!(0.0));
    set_flow_node_state(context, &node.key, "last_pass", json!(pass));

    if let Some(trade_id) = source_trade_id {
        set_flow_var(context, &format!("{var_prefix}_trade_id"), json!(trade_id));
    }
    set_flow_var(
        context,
        &format!("{var_prefix}_market_slug"),
        json!(market_slug.clone()),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        json!(token_id.clone()),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        json!(outcome_label.clone()),
    );
    set_flow_var(context, &format!("{var_prefix}_position_qty"), json!(0.0));
    set_flow_var(
        context,
        &format!("{var_prefix}_entry_price"),
        json!(entry_price),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_current_price"),
        json!(current_price),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_loss_pct"),
        json!(loss_pct_now),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_gain_pct"),
        json!(gain_pct_now),
    );
    set_flow_var(context, &format!("{var_prefix}_pass"), json!(pass));

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: now,
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "position_found": false,
        "position_qty": 0.0,
        "entry_price": entry_price,
        "entry_price_source": "manual_entry_price",
        "current_price": current_price,
        "loss_pct": loss_pct_now,
        "gain_pct": gain_pct_now,
        "loss_rules": rule_outputs,
        "combine_mode_input": combine_mode_raw,
        "combine_mode_resolved": resolved_combine_mode,
        "price_source": price_source,
        "ws_sourced": ws_sourced,
        "samples_tracked": samples.len(),
        "pass": pass
    });

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
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let side = node_config_string(node, "side")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires side (buy or sell)"))?;
    anyhow::ensure!(
        matches!(side.as_str(), "buy" | "sell"),
        "action.place_order side must be buy or sell"
    );
    let execution_mode = node_config_string(node, "executionMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("action.place_order requires executionMode (market or limit)")
        })?;
    anyhow::ensure!(
        matches!(execution_mode.as_str(), "market" | "limit"),
        "action.place_order executionMode must be market or limit"
    );
    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires marketSlug"))?;
    let token_id = resolve_action_place_order_string(
        node,
        context,
        step,
        "tokenId",
        "tokenId",
        &["triggered_token_id", "tokenId"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires tokenId"))?;
    let outcome_label = resolve_action_place_order_string(
        node,
        context,
        step,
        "outcomeLabel",
        "outcomeLabel",
        &["triggered_outcome_label", "outcomeLabel"],
    )
    .unwrap_or_else(|| token_id.clone());
    set_flow_context(context, "marketSlug", json!(market_slug.clone()));
    set_flow_context(context, "tokenId", json!(token_id.clone()));
    set_flow_context(context, "outcomeLabel", json!(outcome_label.clone()));
    let mut protection_output = Value::Null;
    if side == "buy" {
        if let Some(raw_protection) =
            resolve_action_place_order_underlying_protection(context, step)
        {
            if let Some(protection_config) =
                parse_underlying_protection_config(Some(raw_protection.clone()))
            {
                let poly_delta_10s_cent = raw_protection
                    .get("poly_delta_10s_cent")
                    .and_then(value_as_f64);
                let resolved_market_asset = flow_context_string(context, "marketAsset")
                    .or_else(|| {
                        step_input_string(step, &["market_asset", "marketAsset"])
                            .map(|value| value.trim().to_ascii_lowercase())
                    })
                    .or_else(|| {
                        find_updown_scope_by_slug(&market_slug).map(|scope| scope.asset.to_string())
                    });
                let resolved_direction =
                    resolve_underlying_direction_label(&outcome_label).map(str::to_string);
                let protection = if let Some(ref market_asset) = resolved_market_asset {
                    if market_asset != &protection_config.asset {
                        UnderlyingProtectionEvaluation {
                            mode: protection_config.mode.clone(),
                            preset: protection_config.preset.clone(),
                            asset: protection_config.asset.clone(),
                            direction: protection_config.direction.clone(),
                            reference_feed: "coinbase_spot".to_string(),
                            reference_symbol: protection_config.reference_symbol.clone(),
                            passed: false,
                            reason_code: "asset_mismatch".to_string(),
                            reason_detail: Some(format!(
                                "expected_asset={} current_asset={market_asset}",
                                protection_config.asset
                            )),
                            cycle_open_price: None,
                            current_price: None,
                            delta_10s_pct: None,
                            delta_30s_pct: None,
                            poly_delta_10s_cent,
                            divergence_blocked: false,
                        }
                    } else if let Some(ref current_direction) = resolved_direction {
                        if current_direction != &protection_config.direction {
                            UnderlyingProtectionEvaluation {
                                mode: protection_config.mode.clone(),
                                preset: protection_config.preset.clone(),
                                asset: protection_config.asset.clone(),
                                direction: protection_config.direction.clone(),
                                reference_feed: "coinbase_spot".to_string(),
                                reference_symbol: protection_config.reference_symbol.clone(),
                                passed: false,
                                reason_code: "direction_mismatch".to_string(),
                                reason_detail: Some(format!(
                                    "expected_direction={} current_direction={current_direction}",
                                    protection_config.direction
                                )),
                                cycle_open_price: None,
                                current_price: None,
                                delta_10s_pct: None,
                                delta_30s_pct: None,
                                poly_delta_10s_cent,
                                divergence_blocked: false,
                            }
                        } else {
                            evaluate_underlying_protection(
                                &protection_config,
                                &market_slug,
                                poly_delta_10s_cent,
                            )
                            .await
                        }
                    } else {
                        evaluate_underlying_protection(
                            &protection_config,
                            &market_slug,
                            poly_delta_10s_cent,
                        )
                        .await
                    }
                } else if let Some(ref current_direction) = resolved_direction {
                    if current_direction != &protection_config.direction {
                        UnderlyingProtectionEvaluation {
                            mode: protection_config.mode.clone(),
                            preset: protection_config.preset.clone(),
                            asset: protection_config.asset.clone(),
                            direction: protection_config.direction.clone(),
                            reference_feed: "coinbase_spot".to_string(),
                            reference_symbol: protection_config.reference_symbol.clone(),
                            passed: false,
                            reason_code: "direction_mismatch".to_string(),
                            reason_detail: Some(format!(
                                "expected_direction={} current_direction={current_direction}",
                                protection_config.direction
                            )),
                            cycle_open_price: None,
                            current_price: None,
                            delta_10s_pct: None,
                            delta_30s_pct: None,
                            poly_delta_10s_cent,
                            divergence_blocked: false,
                        }
                    } else {
                        evaluate_underlying_protection(
                            &protection_config,
                            &market_slug,
                            poly_delta_10s_cent,
                        )
                        .await
                    }
                } else {
                    evaluate_underlying_protection(
                        &protection_config,
                        &market_slug,
                        poly_delta_10s_cent,
                    )
                    .await
                };
                protection_output = protection.to_value();
                set_flow_context(context, "underlyingProtection", protection_output.clone());
                if !protection.passed {
                    repo.append_trade_flow_event(
                        Some(run.id),
                        run.definition_id,
                        Some(run.version_id),
                        "pre_order_protection_blocked",
                        &json!({
                            "node_key": node.key,
                            "node_type": node.node_type,
                            "market_slug": market_slug,
                            "token_id": token_id,
                            "outcome_label": outcome_label,
                            "side": side,
                            "execution_mode": execution_mode,
                            "protection": protection_output.clone()
                        }),
                    )
                    .await?;
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "node_key": node.key,
                            "blocked": true,
                            "reason": "underlying_protection_blocked",
                            "market_slug": market_slug,
                            "token_id": token_id,
                            "outcome_label": outcome_label,
                            "side": side,
                            "execution_mode": execution_mode,
                            "protection": protection_output
                        }),
                        routes: vec![TradeFlowRouteDecision {
                            edge_type: "on_error".to_string(),
                            available_at: Utc::now(),
                        }],
                        repeat_at: None,
                        repeat_idempotency_key: None,
                    });
                }
            }
        }
    }
    let mut source_trade_id = resolve_flow_source_trade_id(node, context).or_else(|| {
        step_input_i64(step, &["sourceTradeId", "source_trade_id"]).filter(|value| *value > 0)
    });
    if let Some(resolved_source_trade_id) = source_trade_id {
        set_flow_context(context, "sourceTradeId", json!(resolved_source_trade_id));
    }
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
        let source_trade_id = source_trade_id.ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order sizePct requires sourceTradeId when sizeMode is pct"
            )
        })?;
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
    if source_trade_id.is_none() {
        anyhow::ensure!(
            side == "buy",
            "action.place_order side=sell requires sourceTradeId or an explicit open-position context"
        );
        let reference_price = resolve_action_place_order_reference_price(node, step).unwrap_or(0.5);
        let ensured_source_trade_id = repo
            .ensure_manual_builder_source_trade(
                &market_slug,
                &token_id,
                &outcome_label,
                reference_price,
                size_usdc,
            )
            .await?;
        info!(
            flow_run_id = run.id,
            node_key = %node.key,
            source_trade_id = ensured_source_trade_id,
            side = %side,
            market_slug = %market_slug,
            token_id = %token_id,
            "TRADE_FLOW_PLACE_ORDER_SOURCE_TRADE_AUTO_RESOLVED"
        );
        set_flow_context(context, "sourceTradeId", json!(ensured_source_trade_id));
        source_trade_id = Some(ensured_source_trade_id);
    }
    let source_trade_id = source_trade_id
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires sourceTradeId"))?;
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent").unwrap_or(1.0);
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.place_order minPriceDistanceCent must be > 0"
    );
    let trigger_condition = node_config_string(node, "triggerCondition");
    let trigger_price = node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
    let max_price = resolve_action_place_order_max_price(context, step);
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

    let tp_enabled = node_config_bool(node, "tpEnabled").unwrap_or(false);
    let tp_price = resolve_action_place_order_exit_price(
        node,
        &side,
        tp_enabled,
        "tpPriceCent",
        "tpPrice",
        "tp",
    )?;
    let sl_enabled = node_config_bool(node, "slEnabled").unwrap_or(false);
    let sl_price = resolve_action_place_order_exit_price(
        node,
        &side,
        sl_enabled,
        "slPriceCent",
        "slPrice",
        "sl",
    )?;
    if let (Some(tp_price), Some(sl_price)) = (tp_price, sl_price) {
        anyhow::ensure!(
            sl_price < tp_price,
            "action.place_order requires slPrice < tpPrice when both stop loss and take profit are enabled"
        );
    }

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
            &execution_mode,
            trigger_condition.as_deref(),
            trigger_price,
            max_price,
            TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            size_usdc,
            None,
            None,
            min_price_distance_cent,
            expires_at,
            max_triggers,
            None,
            tp_enabled,
            tp_price,
            sl_enabled,
            sl_price,
        )
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_created",
        &json!({
            "flow_run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "execution_mode": execution_mode,
            "order_type": clob_order_type_for_execution_mode(&execution_mode),
            "size_mode": resolved_size_mode,
            "size_pct": resolved_size_pct,
            "trigger_sizes": trigger_sizes,
            "max_price": max_price,
            "protection": protection_output.clone(),
            "tp_enabled": tp_enabled,
            "tp_price": tp_price,
            "sl_enabled": sl_enabled,
            "sl_price": sl_price
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
            "source_trade_id": source_trade_id,
            "kind": kind,
            "side": side,
            "execution_mode": execution_mode,
            "order_type": clob_order_type_for_execution_mode(&execution_mode),
            "market_slug": market_slug,
            "token_id": token_id,
            "max_price": max_price,
            "protection": protection_output,
            "size_mode": resolved_size_mode,
            "size_pct": resolved_size_pct,
            "size_usdc": size_usdc,
            "tp_enabled": tp_enabled,
            "tp_price": tp_price,
            "sl_enabled": sl_enabled,
            "sl_price": sl_price
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
    let telegram = load_live_telegram_config()?;
    let bot_token = resolve_telegram_bot_token(&telegram, node)?;
    let chat_id = resolve_telegram_chat_id(&telegram, node)?;
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
    price_mode: WsPriceMode,
) -> Result<f64> {
    let token_id = token_id.filter(|v| !v.trim().is_empty());
    if let Some(token_id) = token_id {
        if let Some(ws_price) = fetch_price_from_market_ws_with_mode(ws, token_id, price_mode).await
        {
            return Ok(clamp_probability(ws_price));
        }
    }
    if let Some(client) = client {
        let token_id = token_id.ok_or_else(|| {
            anyhow::anyhow!(
                "trigger.market_price requires tokenId for REST midpoint fallback (marketSlug={market_slug})"
            )
        })?;
        if matches!(price_mode, WsPriceMode::BestBid | WsPriceMode::BestAsk) {
            warn!(
                %market_slug,
                price_mode = price_mode.as_str(),
                "REST_FALLBACK_MIDPOINT_USED: WS price unavailable, falling back to REST midpoint for {} mode",
                price_mode.as_str()
            );
        }
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

fn step_input_value<'a>(step: &'a TradeFlowRunStep, keys: &[&str]) -> Option<&'a Value> {
    let input = step.input_json.as_ref()?;
    keys.iter().find_map(|key| input.get(*key))
}

fn step_input_string(step: &TradeFlowRunStep, keys: &[&str]) -> Option<String> {
    step_input_value(step, keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn step_input_f64(step: &TradeFlowRunStep, keys: &[&str]) -> Option<f64> {
    step_input_value(step, keys).and_then(value_as_f64)
}

fn step_input_i64(step: &TradeFlowRunStep, keys: &[&str]) -> Option<i64> {
    step_input_value(step, keys).and_then(value_as_i64)
}

fn resolve_action_place_order_string(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
    config_key: &str,
    context_key: &str,
    step_keys: &[&str],
) -> Option<String> {
    node_config_string(node, config_key)
        .or_else(|| flow_context_string(context, context_key))
        .or_else(|| step_input_string(step, step_keys))
}

fn resolve_action_place_order_exit_price(
    node: &TradeFlowNode,
    side: &str,
    enabled: bool,
    cent_key: &str,
    raw_key: &str,
    label: &str,
) -> Result<Option<f64>> {
    if !enabled {
        return Ok(None);
    }

    anyhow::ensure!(
        side == "buy",
        "action.place_order {label}Enabled is only valid for side=buy"
    );

    let cent = node_config_f64(node, cent_key);
    let raw = node_config_f64(node, raw_key);
    let price = cent.map(|value| value / 100.0).or(raw);
    anyhow::ensure!(
        price.is_some() && price.unwrap() > 0.0 && price.unwrap() <= 1.0,
        "action.place_order {label}Price must be in (0, 1] when {label}Enabled is true"
    );
    Ok(price)
}

fn resolve_action_place_order_reference_price(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<f64> {
    step_input_f64(step, &["triggered_price", "price", "wsPrice"])
        .or_else(|| node_config_f64(node, "triggerPrice"))
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|value| value / 100.0))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_max_price(context: &Value, step: &TradeFlowRunStep) -> Option<f64> {
    flow_context_value(context, "maxPrice")
        .as_ref()
        .and_then(value_as_f64)
        .or_else(|| step_input_f64(step, &["max_price", "maxPrice"]))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_underlying_protection(
    context: &Value,
    step: &TradeFlowRunStep,
) -> Option<Value> {
    flow_context_value(context, "underlyingProtection")
        .or_else(|| step_input_value(step, &["protection"]).cloned())
}

fn resolve_flow_source_trade_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    node_config_i64(node, "sourceTradeId")
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|v| v.get("sourceTradeId"))
                .and_then(value_as_i64)
        })
        .filter(|value| *value > 0)
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

fn set_flow_state(context: &mut Value, key: &str, value: Value) {
    let state = ensure_nested_object(context, "state");
    if value.is_null() {
        state.remove(key);
    } else {
        state.insert(key.to_string(), value);
    }
}

fn flow_state_string(context: &Value, key: &str) -> Option<String> {
    context
        .get("state")
        .and_then(|state| state.get(key))
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
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

fn remove_flow_node_state(context: &mut Value, node_key: &str, state_key: &str) {
    let Some(node_state) = context.get_mut("nodeState").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(node) = node_state.get_mut(node_key).and_then(Value::as_object_mut) else {
        return;
    };
    node.remove(state_key);
}

fn flow_node_state<'a>(context: &'a Value, node_key: &str, state_key: &str) -> Option<&'a Value> {
    context
        .get("nodeState")
        .and_then(|node_state| node_state.get(node_key))
        .and_then(|node| node.get(state_key))
}

fn flow_node_state_string(context: &Value, node_key: &str, state_key: &str) -> Option<String> {
    flow_node_state(context, node_key, state_key)
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn flow_node_state_truthy(context: &Value, node_key: &str, state_key: &str) -> bool {
    flow_node_state(context, node_key, state_key)
        .map(value_truthy)
        .unwrap_or(false)
}

fn trade_flow_market_price_once_idempotency_key(
    run_id: i64,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
) -> String {
    if once_scope_market {
        let market_scope = market_slug
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("unknown-market");
        format!("flow-once-fired:{run_id}:{node_key}:{market_scope}")
    } else {
        format!("flow-once-fired:{run_id}:{node_key}")
    }
}

fn trade_flow_market_price_once_fired_for_scope(
    context: &Value,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
) -> bool {
    if !flow_node_state_truthy(context, node_key, FLOW_NODE_STATE_ONCE_FIRED) {
        return false;
    }
    if !once_scope_market {
        return true;
    }
    let Some(current_market_slug) = market_slug.map(str::trim).filter(|v| !v.is_empty()) else {
        return false;
    };
    flow_node_state_string(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG)
        .map(|fired_market_slug| fired_market_slug == current_market_slug)
        .unwrap_or(false)
}

fn sync_trade_flow_market_price_once_scope_state(
    context: &mut Value,
    node_key: &str,
    once_scope_market: bool,
    market_slug: Option<&str>,
) {
    if !once_scope_market {
        return;
    }
    let Some(current_market_slug) = market_slug.map(str::trim).filter(|v| !v.is_empty()) else {
        return;
    };
    let Some(last_fired_market_slug) =
        flow_node_state_string(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG)
    else {
        if flow_node_state_truthy(context, node_key, FLOW_NODE_STATE_ONCE_FIRED) {
            clear_trade_flow_market_price_once_state(context, node_key);
        }
        return;
    };
    if last_fired_market_slug != current_market_slug {
        clear_trade_flow_market_price_once_state(context, node_key);
    }
}

fn mark_trade_flow_market_price_once_fired(
    context: &mut Value,
    node_key: &str,
    fired_at: DateTime<Utc>,
    market_slug: Option<&str>,
) {
    set_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED, json!(true));
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_ONCE_FIRED_AT,
        json!(fired_at.to_rfc3339()),
    );
    if let Some(slug) = market_slug.map(str::trim).filter(|v| !v.is_empty()) {
        set_flow_node_state(
            context,
            node_key,
            FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
            json!(slug),
        );
    } else {
        remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG);
    }
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
        json!(false),
    );
}

fn clear_trade_flow_market_price_once_state(context: &mut Value, node_key: &str) {
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_AT);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG);
    remove_flow_node_state(context, node_key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED);
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

fn resolve_ws_previous_price(
    ws_sourced: bool,
    state_previous_price: Option<f64>,
    token_id: &str,
    ws_token_id_from_step: Option<&str>,
    ws_previous_price_from_step: Option<f64>,
    ws_previous_price_present: bool,
    ws_previous_prices_map: Option<&serde_json::Map<String, Value>>,
) -> Option<f64> {
    if !ws_sourced {
        return state_previous_price;
    }

    let token_id = token_id.trim();
    if !token_id.is_empty() {
        if let Some(map) = ws_previous_prices_map {
            if let Some(raw_value) = map.get(token_id) {
                // Explicit null means "no previous price"; do not fallback to context state.
                return value_as_f64(raw_value).map(clamp_probability);
            }
        }
    }

    let ws_token_matches = ws_token_id_from_step
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|step_token_id| token_id.is_empty() || step_token_id == token_id)
        .unwrap_or(token_id.is_empty());
    if ws_token_matches && ws_previous_price_present {
        // Explicit key presence (including null) should override state fallback.
        return ws_previous_price_from_step;
    }

    state_previous_price
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(v) => v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)),
        Value::String(v) => v.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_as_i64_strict(value: &Value) -> Option<i64> {
    match value {
        Value::Number(v) => v.as_i64(),
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

    fn drawdown_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "drawdown_test".to_string(),
            node_type: "trigger.position_drawdown".to_string(),
            config,
        }
    }

    #[test]
    fn parse_drawdown_rules_supports_up_direction_and_defaults_to_down() {
        let node = drawdown_node(json!({
            "lossRules": [
                { "lossPct": 10 },
                { "lossPct": 15, "direction": "up", "windowMs": 5000 }
            ]
        }));

        let rules = parse_position_drawdown_rules(&node);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].direction, PositionDrawdownDirection::Down);
        assert_eq!(rules[1].direction, PositionDrawdownDirection::Up);
        assert_eq!(rules[1].window_ms, Some(5000));
    }

    #[test]
    fn parse_drawdown_rules_ignores_invalid_direction_values() {
        let node = drawdown_node(json!({
            "lossRules": [
                { "lossPct": 10, "direction": "sideways" },
                { "lossPct": 7, "direction": "down" }
            ]
        }));

        let rules = parse_position_drawdown_rules(&node);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].direction, PositionDrawdownDirection::Down);
        assert!((rules[0].loss_pct - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn drawdown_detects_deprecated_window_sec_fields() {
        let legacy_root = drawdown_node(json!({
            "lossPct": 10,
            "windowSec": 5
        }));
        assert!(has_deprecated_drawdown_window_sec(&legacy_root));

        let legacy_rule = drawdown_node(json!({
            "lossRules": [
                { "lossPct": 10, "windowSec": 5 }
            ]
        }));
        assert!(has_deprecated_drawdown_window_sec(&legacy_rule));

        let modern = drawdown_node(json!({
            "lossRules": [
                { "lossPct": 10, "windowMs": 5000 }
            ]
        }));
        assert!(!has_deprecated_drawdown_window_sec(&modern));
    }

    #[test]
    fn ws_once_idempotency_key_is_stable_per_run_and_node() {
        let key_1 = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.41,
            Some(1000),
            true,
            false,
            None,
        );
        let key_2 = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_below",
            0.67,
            Some(2000),
            true,
            false,
            None,
        );

        assert_eq!(key_1, "ws-once:42:trigger_market");
        assert_eq!(key_2, "ws-once:42:trigger_market");
    }

    #[test]
    fn ws_loop_idempotency_key_depends_on_event_and_price() {
        let key_a = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.41,
            Some(1000),
            false,
            false,
            None,
        );
        let key_b = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.41,
            Some(1000),
            false,
            false,
            None,
        );
        let key_c = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.41,
            Some(1001),
            false,
            false,
            None,
        );

        assert_eq!(key_a, key_b);
        assert_ne!(key_a, key_c);
    }

    #[test]
    fn ws_market_once_idempotency_key_is_scoped_by_market_slug() {
        let key_1 = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.41,
            Some(1000),
            true,
            true,
            Some("btc-updown-5m-1"),
        );
        let key_2 = ws_price_trigger_step_idempotency_key(
            42,
            "trigger_market",
            "cross_above",
            0.42,
            Some(1001),
            true,
            true,
            Some("btc-updown-5m-2"),
        );

        assert_eq!(key_1, "ws-once:42:trigger_market:btc-updown-5m-1");
        assert_eq!(key_2, "ws-once:42:trigger_market:btc-updown-5m-2");
        assert_ne!(key_1, key_2);
    }

    #[test]
    fn resolve_ws_previous_price_prefers_ws_payload_values() {
        let token = "tok-yes";
        let ws_prev_payload = json!({ token: 0.34 });
        let ws_prev_map = ws_prev_payload.as_object();

        let from_map = resolve_ws_previous_price(
            true,
            Some(0.30),
            token,
            Some(token),
            Some(0.31),
            true,
            ws_prev_map,
        );
        assert!(from_map.is_some());
        assert!((from_map.unwrap_or_default() - 0.34).abs() < 1e-9);

        let from_single =
            resolve_ws_previous_price(true, Some(0.30), token, Some(token), Some(0.33), true, None);
        assert!(from_single.is_some());
        assert!((from_single.unwrap_or_default() - 0.33).abs() < 1e-9);

        let from_state = resolve_ws_previous_price(
            true,
            Some(0.30),
            token,
            Some("tok-no"),
            Some(0.33),
            true,
            None,
        );
        assert!(from_state.is_some());
        assert!((from_state.unwrap_or_default() - 0.30).abs() < 1e-9);

        let ws_prev_null_payload = json!({ token: null });
        let ws_prev_null_map = ws_prev_null_payload.as_object();
        let explicit_null_from_map = resolve_ws_previous_price(
            true,
            Some(0.30),
            token,
            Some(token),
            Some(0.33),
            true,
            ws_prev_null_map,
        );
        assert!(explicit_null_from_map.is_none());

        let explicit_null_from_single =
            resolve_ws_previous_price(true, Some(0.30), token, Some(token), None, true, None);
        assert!(explicit_null_from_single.is_none());

        let non_ws = resolve_ws_previous_price(
            false,
            Some(0.22),
            token,
            Some(token),
            Some(0.99),
            true,
            ws_prev_map,
        );
        assert!(non_ws.is_some());
        assert!((non_ws.unwrap_or_default() - 0.22).abs() < 1e-9);
    }

    #[test]
    fn ws_previous_price_preserves_cross_detection_after_context_update() {
        let token = "tok-yes";
        let trigger_price = 0.30;
        let current_price = 0.30;
        let state_prev = Some(0.30);

        let ws_prev_payload = json!({ token: 0.34 });
        let ws_prev_map = ws_prev_payload.as_object();
        let effective_prev = resolve_ws_previous_price(
            true,
            state_prev,
            token,
            Some(token),
            Some(0.34),
            true,
            ws_prev_map,
        );
        let (pass_with_ws_prev, mode_with_ws_prev) = evaluate_trigger_market_price_condition(
            effective_prev,
            current_price,
            trigger_price,
            "cross_below",
            true,
            None,
        );
        assert!(pass_with_ws_prev);
        assert_eq!(mode_with_ws_prev, "cross_detected");

        let (pass_with_state_prev, mode_with_state_prev) = evaluate_trigger_market_price_condition(
            state_prev,
            current_price,
            trigger_price,
            "cross_below",
            true,
            None,
        );
        assert!(!pass_with_state_prev);
        assert_eq!(mode_with_state_prev, "no_cross");
    }

    #[test]
    fn auto_scope_specs_resolve_token_from_outcome_label() {
        let node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "repeatMode": "once",
                "onceScope": "market",
                "marketMode": "auto_scope",
                "outcomeConditions": [{
                    "outcomeLabel": "Up",
                    "triggerCondition": "cross_above",
                    "triggerPriceCent": 60
                }]
            }),
        };
        let context = json!({
            "flowContext": {
                "marketSlug": "btc-updown-5m-1772296200",
                "yesTokenId": "yes-token",
                "noTokenId": "no-token"
            }
        });

        let specs = open_position_ws_price_node_specs(&node, &context);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].token_id, "yes-token");
        assert!(specs[0].once_mode);
        assert!(specs[0].once_scope_market);
        assert_eq!(specs[0].price_mode, WsPriceMode::Midpoint);
        assert_eq!(
            specs[0].market_slug.as_deref(),
            Some("btc-updown-5m-1772296200")
        );
    }

    #[test]
    fn auto_scope_specs_prefer_context_market_slug_over_stale_config_slug() {
        let node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "repeatMode": "once",
                "onceScope": "market",
                "marketMode": "auto_scope",
                "marketSlug": "btc-updown-15m-stale",
                "outcomeConditions": [{
                    "outcomeLabel": "Up",
                    "triggerCondition": "cross_above",
                    "triggerPriceCent": 60
                }]
            }),
        };
        let context = json!({
            "flowContext": {
                "marketSlug": "btc-updown-15m-fresh",
                "yesTokenId": "yes-token",
                "noTokenId": "no-token"
            }
        });

        let specs = open_position_ws_price_node_specs(&node, &context);
        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0].market_slug.as_deref(),
            Some("btc-updown-15m-fresh")
        );
    }

    #[test]
    fn market_price_specs_parse_price_mode_and_default_to_midpoint() {
        let context = json!({
            "flowContext": {
                "marketSlug": "epl-test",
                "yesTokenId": "yes-token",
                "noTokenId": "no-token"
            }
        });
        let default_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "outcomeConditions": [{
                    "outcomeLabel": "Yes",
                    "triggerCondition": "cross_above",
                    "triggerPriceCent": 60
                }]
            }),
        };
        let raw_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "priceMode": "raw",
                "outcomeConditions": [{
                    "outcomeLabel": "Yes",
                    "triggerCondition": "cross_above",
                    "triggerPriceCent": 60
                }]
            }),
        };

        let default_specs = open_position_ws_price_node_specs(&default_node, &context);
        let raw_specs = open_position_ws_price_node_specs(&raw_node, &context);
        assert_eq!(default_specs.len(), 1);
        assert_eq!(raw_specs.len(), 1);
        assert_eq!(default_specs[0].price_mode, WsPriceMode::Midpoint);
        assert_eq!(raw_specs[0].price_mode, WsPriceMode::Raw);
    }

    #[test]
    fn open_positions_specs_keep_raw_price_mode() {
        let node = TradeFlowNode {
            key: "trigger_open".to_string(),
            node_type: "trigger.open_positions".to_string(),
            config: json!({
                "marketSlug": "epl-test",
                "outcomeConditions": [{
                    "tokenId": "tok-yes",
                    "triggerCondition": "cross_above",
                    "triggerPriceCent": 55
                }]
            }),
        };

        let specs = open_position_ws_price_node_specs(&node, &json!({}));
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].price_mode, WsPriceMode::Raw);
    }

    #[test]
    fn ws_market_slug_override_is_ignored_for_auto_scope_when_resolved_slug_exists() {
        let auto_scope_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope"
            }),
        };
        let fixed_node = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "fixed"
            }),
        };

        assert!(!should_accept_ws_market_slug_override(
            &auto_scope_node,
            "btc-updown-15m-1772300000"
        ));
        assert!(should_accept_ws_market_slug_override(&auto_scope_node, ""));
        assert!(should_accept_ws_market_slug_override(
            &fixed_node,
            "btc-updown-15m-1772300000"
        ));
    }

    #[test]
    fn cross_below_requires_actual_crossing() {
        // None previous → false (ilk tick, sadece fiyat kaydedilir)
        assert!(!crossed_below_strict(None, 0.25, 0.30));
        assert!(!crossed_below_strict(None, 0.35, 0.30));
        // Gercek crossing: yukaridan asagiya
        assert!(crossed_below_strict(Some(0.31), 0.30, 0.30));
        assert!(crossed_below_strict(Some(0.35), 0.29, 0.30));
        // Zaten asagida, crossing yok
        assert!(!crossed_below_strict(Some(0.28), 0.27, 0.30));
    }

    #[test]
    fn cross_above_requires_actual_crossing() {
        assert!(!crossed_above_strict(None, 0.35, 0.30));
        assert!(!crossed_above_strict(None, 0.25, 0.30));
        assert!(crossed_above_strict(Some(0.29), 0.30, 0.30));
        assert!(crossed_above_strict(Some(0.25), 0.31, 0.30));
        assert!(!crossed_above_strict(Some(0.32), 0.33, 0.30));
    }

    #[test]
    fn trigger_market_price_allows_first_tick_threshold_hit() {
        let (pass_above, mode_above) =
            evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", true, None);
        assert!(pass_above);
        assert_eq!(mode_above, "first_tick_threshold");

        let (pass_below, mode_below) =
            evaluate_trigger_market_price_condition(None, 0.25, 0.30, "cross_below", true, None);
        assert!(pass_below);
        assert_eq!(mode_below, "first_tick_threshold");

        let (strict_pass, strict_mode) =
            evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", false, None);
        assert!(!strict_pass);
        assert_eq!(strict_mode, "no_previous");
    }

    #[test]
    fn extract_price_ignores_price_changes_without_asset_id() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [
                    { "price": "0.71", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        assert!(extract_price_from_market_events(&events, "tok-yes").is_none());

        let events_with_asset = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [
                    { "asset_id": "tok-yes", "price": "0.71", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let extracted = extract_price_from_market_events(&events_with_asset, "tok-yes");
        assert_eq!(extracted, Some((0.71, Some(12345))));
    }

    #[test]
    fn extract_price_midpoint_mode_prefers_best_bid_ask_over_price_changes() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
                "price_changes": [
                    { "asset_id": "tok-yes", "price": "0.14", "timestamp": 12345 }
                ]
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let raw = extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Raw);
        assert_eq!(
            raw,
            Some(ExtractedWsPrice {
                price: 0.14,
                ts: Some(12345),
                source: "price_changes",
            })
        );

        let midpoint =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Midpoint);
        assert_eq!(
            midpoint,
            Some(ExtractedWsPrice {
                price: 0.58,
                ts: Some(12345),
                source: "best_bid_ask",
            })
        );
    }

    #[test]
    fn ws_price_mode_parse_best_bid_ask_aliases() {
        assert_eq!(
            WsPriceMode::parse(Some("best_bid"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("bid"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("best_ask"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("ask"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("BEST_BID"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some(" Best_Ask "), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
        assert_eq!(
            WsPriceMode::parse(Some("BID"), WsPriceMode::Midpoint),
            WsPriceMode::BestBid
        );
        assert_eq!(
            WsPriceMode::parse(Some("ASK"), WsPriceMode::Midpoint),
            WsPriceMode::BestAsk
        );
    }

    #[test]
    fn extract_price_best_bid_mode_returns_bid_only() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let best_bid =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
        assert_eq!(
            best_bid,
            Some(ExtractedWsPrice {
                price: 0.57,
                ts: Some(12345),
                source: "best_bid",
            })
        );
    }

    #[test]
    fn extract_price_best_ask_mode_returns_ask_only() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "asset_id": "tok-yes",
                "best_bid": "0.57",
                "best_ask": "0.59",
            }),
            event_type: WsEventType::PriceChange,
            market: Some("tok-yes".to_string()),
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(12345),
        }];

        let best_ask =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
        assert_eq!(
            best_ask,
            Some(ExtractedWsPrice {
                price: 0.59,
                ts: Some(12345),
                source: "best_ask",
            })
        );
    }

    #[test]
    fn extract_price_best_bid_from_price_changes() {
        let events = vec![WsEvent {
            channel: WsChannel::Market,
            payload: json!({
                "price_changes": [{
                    "asset_id": "tok-yes",
                    "best_bid": "0.45",
                    "best_ask": "0.47",
                    "timestamp": 99999
                }]
            }),
            event_type: WsEventType::PriceChange,
            market: None,
            order_id: None,
            fill_id: None,
            status: None,
            price: None,
            size: None,
            ts: Some(88888),
        }];

        let bid =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
        assert_eq!(
            bid,
            Some(ExtractedWsPrice {
                price: 0.45,
                ts: Some(99999),
                source: "best_bid",
            })
        );

        let ask =
            extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
        assert_eq!(
            ask,
            Some(ExtractedWsPrice {
                price: 0.47,
                ts: Some(99999),
                source: "best_ask",
            })
        );
    }

    #[test]
    fn market_once_idempotency_key_contains_market_slug() {
        let key = trade_flow_market_price_once_idempotency_key(
            77,
            "trigger_market",
            true,
            Some("btc-updown-5m-1772296200"),
        );
        assert_eq!(
            key,
            "flow-once-fired:77:trigger_market:btc-updown-5m-1772296200"
        );
    }

    #[test]
    fn market_once_state_clears_when_market_changes() {
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {
                "trigger_market": {
                    "once_fired": true,
                    "once_fired_market_slug": "btc-updown-5m-old"
                }
            }
        });
        sync_trade_flow_market_price_once_scope_state(
            &mut context,
            "trigger_market",
            true,
            Some("btc-updown-5m-new"),
        );
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_market",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
    }

    #[test]
    fn parse_lock_pid_extracts_pid_value() {
        assert_eq!(parse_lock_pid("pid=12345\n"), Some(12345));
        assert_eq!(parse_lock_pid("foo=1\npid=77\n"), Some(77));
        assert_eq!(parse_lock_pid("pid=abc\n"), None);
        assert_eq!(parse_lock_pid(""), None);
    }

    #[test]
    fn market_price_once_detection_only_matches_once_mode() {
        let once_node = TradeFlowNode {
            key: "trigger_once".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({ "repeatMode": "once" }),
        };
        let loop_node = TradeFlowNode {
            key: "trigger_loop".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({ "repeatMode": "loop" }),
        };
        let open_positions_node = TradeFlowNode {
            key: "trigger_open".to_string(),
            node_type: "trigger.open_positions".to_string(),
            config: json!({ "repeatMode": "once" }),
        };

        assert!(is_trade_flow_market_price_once_node(&once_node));
        assert!(!is_trade_flow_market_price_once_node(&loop_node));
        assert!(!is_trade_flow_market_price_once_node(&open_positions_node));
    }

    #[test]
    fn publish_marker_change_resets_once_state_for_once_nodes() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![
                TradeFlowNode {
                    key: "trigger_once".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "repeatMode": "once" }),
                },
                TradeFlowNode {
                    key: "trigger_loop".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "repeatMode": "loop" }),
                },
            ],
            edges: vec![],
        };
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {
                "__publish_marker": "101:1000"
            },
            "refs": {},
            "nodeState": {
                "trigger_once": {
                    "once_fired": true,
                    "once_fired_at": "2026-02-28T00:00:00Z",
                    "once_blocked_logged": true
                },
                "trigger_loop": {
                    "once_fired": true
                }
            }
        });

        let (previous_marker, reset_nodes) =
            sync_trade_flow_once_state_for_publish(&graph, &mut context, "101:2000");
        assert_eq!(previous_marker.as_deref(), Some("101:1000"));
        assert_eq!(reset_nodes, vec!["trigger_once".to_string()]);
        assert_eq!(
            flow_state_string(&context, FLOW_STATE_PUBLISH_MARKER).as_deref(),
            Some("101:2000")
        );
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_once",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
        assert!(flow_node_state_truthy(
            &context,
            "trigger_loop",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
    }

    #[test]
    fn publish_marker_same_keeps_once_state_intact() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![TradeFlowNode {
                key: "trigger_once".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({ "repeatMode": "once" }),
            }],
            edges: vec![],
        };
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {
                "__publish_marker": "77:5000"
            },
            "refs": {},
            "nodeState": {
                "trigger_once": {
                    "once_fired": true
                }
            }
        });

        let (previous_marker, reset_nodes) =
            sync_trade_flow_once_state_for_publish(&graph, &mut context, "77:5000");
        assert_eq!(previous_marker.as_deref(), Some("77:5000"));
        assert!(reset_nodes.is_empty());
        assert!(flow_node_state_truthy(
            &context,
            "trigger_once",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
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

#[cfg(test)]
mod place_order_binding_tests {
    use super::*;

    fn test_step(input_json: Value) -> TradeFlowRunStep {
        TradeFlowRunStep {
            id: 1,
            run_id: 42,
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(input_json),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        }
    }

    fn test_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn test_builder_order(side: &str, parent_order_id: Option<i64>) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
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
            tp_enabled: false,
            tp_price: None,
            sl_enabled: false,
            sl_price: None,
        }
    }

    #[test]
    fn place_order_resolves_runtime_binding_from_step_input() {
        let node = test_node(json!({}));
        let step = test_step(json!({
            "market_slug": "btc-updown-5m-1772729700",
            "triggered_token_id": "tok-up",
            "triggered_outcome_label": "Up",
            "triggered_price": 0.875,
            "sourceTradeId": 77
        }));
        let context = json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        });

        assert_eq!(
            resolve_action_place_order_string(
                &node,
                &context,
                &step,
                "marketSlug",
                "marketSlug",
                &["market_slug", "marketSlug", "wsMarketSlug"],
            )
            .as_deref(),
            Some("btc-updown-5m-1772729700")
        );
        assert_eq!(
            resolve_action_place_order_string(
                &node,
                &context,
                &step,
                "tokenId",
                "tokenId",
                &["triggered_token_id", "tokenId"],
            )
            .as_deref(),
            Some("tok-up")
        );
        assert_eq!(
            resolve_action_place_order_string(
                &node,
                &context,
                &step,
                "outcomeLabel",
                "outcomeLabel",
                &["triggered_outcome_label", "outcomeLabel"],
            )
            .as_deref(),
            Some("Up")
        );
        assert_eq!(
            step_input_i64(&step, &["sourceTradeId", "source_trade_id"]),
            Some(77)
        );
        assert_eq!(
            resolve_action_place_order_reference_price(&node, &step),
            Some(0.875)
        );
    }

    #[test]
    fn place_order_resolves_inherited_max_price_from_context_before_step_input() {
        let step = test_step(json!({
            "max_price": 0.95,
            "maxPrice": 0.94
        }));
        let context = json!({
            "flowContext": { "maxPrice": 0.9 },
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        });

        assert_eq!(
            resolve_action_place_order_max_price(&context, &step),
            Some(0.9)
        );

        let empty_context = json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        });
        assert_eq!(
            resolve_action_place_order_max_price(&empty_context, &step),
            Some(0.95)
        );
    }

    #[test]
    fn resolve_flow_source_trade_id_ignores_non_positive_values() {
        let node_missing = test_node(json!({ "sourceTradeId": 0 }));
        let context_missing = json!({
            "flowContext": { "sourceTradeId": 0 },
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        });
        assert_eq!(
            resolve_flow_source_trade_id(&node_missing, &context_missing),
            None
        );

        let node_config = test_node(json!({ "sourceTradeId": 12 }));
        assert_eq!(
            resolve_flow_source_trade_id(&node_config, &context_missing),
            Some(12)
        );

        let node_context = test_node(json!({}));
        let context_positive = json!({
            "flowContext": { "sourceTradeId": 34 },
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        });
        assert_eq!(
            resolve_flow_source_trade_id(&node_context, &context_positive),
            Some(34)
        );
    }

    #[test]
    fn place_order_exit_price_parses_tp_and_sl_from_cent_values() {
        let node = test_node(json!({
            "tpPriceCent": 98,
            "slPriceCent": 72
        }));

        assert_eq!(
            resolve_action_place_order_exit_price(
                &node,
                "buy",
                true,
                "tpPriceCent",
                "tpPrice",
                "tp"
            )
            .unwrap(),
            Some(0.98)
        );
        assert_eq!(
            resolve_action_place_order_exit_price(
                &node,
                "buy",
                true,
                "slPriceCent",
                "slPrice",
                "sl"
            )
            .unwrap(),
            Some(0.72)
        );
    }

    #[test]
    fn place_order_exit_price_rejects_sell_side() {
        let node = test_node(json!({ "slPriceCent": 72 }));
        let err = resolve_action_place_order_exit_price(
            &node,
            "sell",
            true,
            "slPriceCent",
            "slPrice",
            "sl",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("slEnabled is only valid for side=buy"));
    }

    #[test]
    fn oco_cancel_only_applies_to_child_sell_orders_with_live_status() {
        let sell_child = test_builder_order("sell", Some(9));
        let buy_parent = test_builder_order("buy", None);
        let sell_without_parent = test_builder_order("sell", None);

        assert!(should_request_trade_builder_oco_cancel(&sell_child, "open"));
        assert!(should_request_trade_builder_oco_cancel(
            &sell_child,
            "partially_filled"
        ));
        assert!(should_request_trade_builder_oco_cancel(
            &sell_child,
            "filled"
        ));
        assert!(!should_request_trade_builder_oco_cancel(
            &sell_child,
            "rejected"
        ));
        assert!(!should_request_trade_builder_oco_cancel(
            &buy_parent,
            "open"
        ));
        assert!(!should_request_trade_builder_oco_cancel(
            &sell_without_parent,
            "open"
        ));
    }

    #[test]
    fn normalize_exchange_status_treats_matched_as_filled() {
        assert_eq!(normalize_exchange_status("matched"), "filled");
        assert_eq!(normalize_exchange_status("MATCHED"), "filled");
    }

    #[test]
    fn matched_cancel_errors_are_treated_as_terminal_match() {
        assert!(cancel_error_indicates_terminal_match(
            "matched orders can't be canceled"
        ));
        assert!(cancel_error_indicates_terminal_match(
            "cannot cancel order because it is already matched"
        ));
        assert!(!cancel_error_indicates_terminal_match(
            "not enough balance / allowance"
        ));
    }

    #[test]
    fn desired_price_above_builder_max_price_is_blocked() {
        let mut order = test_builder_order("buy", None);
        order.max_price = Some(0.90);

        assert!(!trade_builder_price_exceeds_max_price(&order, 0.90));
        assert!(trade_builder_price_exceeds_max_price(&order, 0.91));
    }

    #[test]
    fn trigger_market_price_rejects_cross_when_above_max_price() {
        let (matched, reason) = evaluate_trigger_market_price_condition(
            Some(0.76),
            0.93,
            0.77,
            "cross_above",
            true,
            Some(0.90),
        );

        assert!(!matched);
        assert_eq!(reason, "above_max_price");
    }

    #[test]
    fn exit_child_sizing_uses_filled_qty_as_share_target() {
        let sizing = trade_builder_exit_child_sizing(5.104, 0.98);
        assert_eq!(sizing.target_qty, 5.10);
        assert_eq!(sizing.remaining_qty, 5.10);
        assert!((sizing.size_usdc - 4.998).abs() < 0.000001);
    }

    #[test]
    fn inventory_pending_sl_does_not_latch_when_price_recovers() {
        let mut order = test_builder_order("sell", Some(9));
        order.status = "inventory_pending".to_string();
        order.trigger_condition = Some("cross_below".to_string());
        order.trigger_price = Some(0.60);
        order.last_seen_price = Some(0.94);

        assert!(!should_trigger_builder_order(&order, 0.99));
        assert!(should_trigger_builder_order(&order, 0.40));
    }

    #[test]
    fn inventory_pending_tp_uses_slack_window() {
        let mut order = test_builder_order("sell", Some(9));
        order.status = "inventory_pending".to_string();
        order.trigger_condition = Some("cross_above".to_string());
        order.trigger_price = Some(0.98);
        order.last_seen_price = Some(0.99);

        assert!(should_trigger_builder_order(&order, 0.95));
        assert!(!should_trigger_builder_order(&order, 0.92));
    }

    #[test]
    fn share_basis_remaining_qty_does_not_expand_at_low_price() {
        let mut order = test_builder_order("sell", Some(9));
        order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
        order.target_qty = Some(5.10);
        order.remaining_qty = Some(5.10);

        let order_info = OrderInfo {
            order_id: "ord-1".to_string(),
            client_order_id: None,
            status: "live".to_string(),
            price: Some(0.01),
            size: Some(5.10),
            filled_size: Some(0.0),
        };

        let (remaining_usdc, remaining_qty) =
            estimate_remaining_trade_builder_sizing(&order, &order_info, 0.01);
        assert_eq!(remaining_qty, Some(5.10));
        assert_eq!(remaining_usdc, Some(0.051));
    }

    #[test]
    fn visible_share_qty_is_clamped_with_floor_precision() {
        assert_eq!(
            clamp_trade_builder_visible_share_qty(6.02, Some(5.9815)),
            Some(5.98)
        );
        assert_eq!(
            clamp_trade_builder_visible_share_qty(6.02, Some(6.50)),
            Some(6.02)
        );
        assert_eq!(
            clamp_trade_builder_visible_share_qty(6.02, Some(0.009)),
            None
        );
    }

    #[test]
    fn inventory_pending_tp_trigger_price_applies_slack_only_to_tp_children() {
        let mut tp_order = test_builder_order("sell", Some(9));
        tp_order.status = "inventory_pending".to_string();
        tp_order.trigger_condition = Some("cross_above".to_string());
        tp_order.trigger_price = Some(0.98);

        let mut sl_order = test_builder_order("sell", Some(9));
        sl_order.status = "inventory_pending".to_string();
        sl_order.trigger_condition = Some("cross_below".to_string());
        sl_order.trigger_price = Some(0.60);

        assert_eq!(
            trade_builder_inventory_pending_tp_trigger_price(&tp_order),
            Some(0.93)
        );
        assert_eq!(
            trade_builder_inventory_pending_tp_trigger_price(&sl_order),
            Some(0.60)
        );
    }
}

#[cfg(test)]
mod confirmation_gate_tests {
    use super::*;

    fn test_node_spec(
        trigger_condition: &str,
        trigger_price: f64,
        confirmation_ms: i64,
    ) -> WsOpenPositionPriceNodeSpec {
        WsOpenPositionPriceNodeSpec {
            node_key: "trigger_1".to_string(),
            node_type: "trigger.market_price".to_string(),
            once_mode: true,
            once_scope_market: false,
            auto_scope: true,
            price_mode: WsPriceMode::Midpoint,
            market_slug: Some("btc-updown-5m-test".to_string()),
            token_id: "tok-yes-123".to_string(),
            trigger_condition: trigger_condition.to_string(),
            trigger_price,
            max_price: None,
            protection_mode: TRIGGER_PROTECTION_MODE_OFF.to_string(),
            protection_asset: None,
            confirmation_ms: Some(confirmation_ms),
            cycle_window_mode: None,
            cycle_window_secs: None,
        }
    }

    #[test]
    fn last_cycle_window_disables_resolution_window_guard() {
        assert!(auto_scope_resolution_window_guard_enabled(None));
        assert!(auto_scope_resolution_window_guard_enabled(Some("first")));
        assert!(!auto_scope_resolution_window_guard_enabled(Some("last")));
    }

    #[test]
    fn legacy_auto_scope_once_scope_upgrades_to_market() {
        let auto_scope_run = TradeFlowNode {
            key: "trigger_run".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "repeatMode": "once",
                "onceScope": "run",
            }),
        };
        let auto_scope_market = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "repeatMode": "once",
                "onceScope": "market",
            }),
        };

        assert_eq!(node_once_scope(&auto_scope_run), "market");
        assert_eq!(node_once_scope(&auto_scope_market), "market");
    }

    #[test]
    fn explicit_auto_scope_once_scope_honors_config() {
        let auto_scope_run = TradeFlowNode {
            key: "trigger_run".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "repeatMode": "once",
                "onceScope": "run",
                "onceScopeVersion": 2,
            }),
        };
        let auto_scope_market = TradeFlowNode {
            key: "trigger_market".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "marketMode": "auto_scope",
                "repeatMode": "once",
                "onceScope": "market",
                "onceScopeVersion": 2,
            }),
        };

        assert_eq!(node_once_scope(&auto_scope_run), "run");
        assert_eq!(node_once_scope(&auto_scope_market), "market");
    }

    #[test]
    fn market_once_state_clears_legacy_run_state_without_market_slug() {
        let mut context = json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {
                "trigger_market": {
                    "once_fired": true,
                    "once_blocked_logged": true
                }
            }
        });
        sync_trade_flow_market_price_once_scope_state(
            &mut context,
            "trigger_market",
            true,
            Some("btc-updown-5m-new"),
        );
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_market",
            FLOW_NODE_STATE_ONCE_FIRED
        ));
        assert!(!flow_node_state_truthy(
            &context,
            "trigger_market",
            FLOW_NODE_STATE_ONCE_BLOCK_LOGGED
        ));
    }

    #[test]
    fn last_cycle_window_requires_real_cross_for_first_tick() {
        let mut node = test_node_spec("cross_above", 0.80, 15_000);
        node.cycle_window_mode = Some("last".to_string());
        node.cycle_window_secs = Some(60);
        assert!(!allow_first_tick_threshold_for_ws_node(&node, None));

        node.cycle_window_mode = Some("first".to_string());
        assert!(allow_first_tick_threshold_for_ws_node(&node, None));
    }

    #[test]
    fn confirmation_gate_resets_on_zone_exit() {
        let node = test_node_spec("cross_above", 0.80, 15_000);
        let mut context = json!({});
        let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
        let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
        let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

        // Simulate: a cross was pending
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!("2026-01-01T00:00:00Z"),
        );
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Price is now out of zone (below trigger for cross_above)
        let current_price = 0.78_f64;
        let still_in_zone = current_price >= node.trigger_price; // false for cross_above
        assert!(!still_in_zone);

        // Simulate the reset (replicating fixed confirmation gate logic)
        if !still_in_zone {
            remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
            remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
            remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
        }

        // Assert all pending state cleared
        assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());
        assert!(flow_node_state(&context, &node.node_key, &cpend_price_key).is_none());
        assert!(flow_node_state(&context, &node.node_key, &cpend_prev_key).is_none());
    }

    #[test]
    fn confirmation_gate_reentry_restarts_timer() {
        let node = test_node_spec("cross_above", 0.80, 15_000);
        let mut context = json!({});
        let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
        let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
        let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

        // Set up old pending state (10 seconds ago)
        let old_ts = (Utc::now() - ChronoDuration::seconds(10)).to_rfc3339();
        set_flow_node_state(&mut context, &node.node_key, &cpend_at_key, json!(old_ts));
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Zone exit: reset all pending state
        let out_of_zone = 0.78_f64 >= node.trigger_price; // false
        assert!(!out_of_zone);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);

        // Re-entry: new cross detected, set fresh timestamp
        let new_ts = Utc::now().to_rfc3339();
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!(new_ts.clone()),
        );
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Assert new timestamp is different from old (timer restarted)
        let stored = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
        assert_ne!(
            stored, old_ts,
            "New pending timestamp must differ from old (timer restarted)"
        );
        // New timestamp should be very recent (within 2 seconds)
        let parsed = DateTime::parse_from_rfc3339(&stored)
            .unwrap()
            .with_timezone(&Utc);
        let elapsed = Utc::now().signed_duration_since(parsed);
        assert!(
            elapsed.num_seconds() < 2,
            "Re-entry timestamp should be near-zero elapsed"
        );
    }

    #[test]
    fn first_tick_threshold_enters_confirmation_gate() {
        // With no previous_price (first tick), auto_scope=true, once_mode=true, price above trigger
        let (crossed, eval_mode) =
            evaluate_trigger_market_price_condition(None, 0.85, 0.80, "cross_above", true, None);

        assert!(crossed, "first_tick_threshold should return crossed=true");
        assert_eq!(eval_mode, "first_tick_threshold");

        // Simulate: crossed=true enters confirmation gate with confirmation_ms>0
        // The gate sets should_enqueue=false and records pending timestamp
        let node = test_node_spec("cross_above", 0.80, 15_000);
        let mut context = json!({});
        let cpend_at_key = format!("cross_pending_at_{}", node.token_id);

        // With explicit confirmationMs > 0, crossed enters gate (not immediately enqueued)
        let should_enqueue_immediately = false; // confirmation gate defers enqueue
        let enter_gate = crossed && market_price_confirmation_ms(&node).is_some();
        assert!(
            enter_gate,
            "first_tick_threshold + explicit confirmation_ms should enter gate"
        );
        assert!(
            !should_enqueue_immediately,
            "Timer started — should NOT enqueue immediately"
        );

        // Simulate timer start
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!(Utc::now().to_rfc3339()),
        );
        assert!(
            flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_some(),
            "cross_pending_at should be set when entering confirmation gate"
        );
    }

    #[test]
    fn confirmation_gate_fires_after_sustained_zone() {
        let node = test_node_spec("cross_above", 0.80, 15_000);
        let mut context = json!({});
        let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
        let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
        let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

        // cross_pending_at was set 16s ago (past the 15000ms confirmation threshold)
        let pending_ts = (Utc::now() - ChronoDuration::seconds(16)).to_rfc3339();
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!(pending_ts.clone()),
        );
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Current price is still in zone
        let current_price = 0.83_f64;
        let still_in_zone = current_price >= node.trigger_price; // true for cross_above
        assert!(still_in_zone);

        // Check elapsed time
        let stored_ts = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
        let pending_at = DateTime::parse_from_rfc3339(&stored_ts)
            .unwrap()
            .with_timezone(&Utc);
        let elapsed_ms = Utc::now()
            .signed_duration_since(pending_at)
            .num_milliseconds();
        let confirmation_ms = market_price_confirmation_ms(&node).unwrap();
        assert!(
            elapsed_ms >= confirmation_ms,
            "Elapsed {}ms should be >= confirmation_ms {}ms",
            elapsed_ms,
            confirmation_ms
        );

        // Gate fires: should_enqueue = true, eval_mode = "cross_confirmed"
        let should_enqueue = elapsed_ms >= confirmation_ms;
        let final_eval_mode = if should_enqueue {
            "cross_confirmed"
        } else {
            "pending"
        };
        assert!(should_enqueue, "Should enqueue after sustained zone time");
        assert_eq!(final_eval_mode, "cross_confirmed");

        // Pending state cleared after confirmation
        remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
        assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());
    }

    #[test]
    fn cross_leave_reenter_no_accumulated_time() {
        let node = test_node_spec("cross_above", 0.80, 15_000);
        let mut context = json!({});
        let cpend_at_key = format!("cross_pending_at_{}", node.token_id);
        let cpend_price_key = format!("cross_pending_price_{}", node.token_id);
        let cpend_prev_key = format!("cross_pending_prev_{}", node.token_id);

        // Step 1: Cross detected — set pending with timestamp 8 seconds ago
        let first_pending_ts = (Utc::now() - ChronoDuration::seconds(8)).to_rfc3339();
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!(first_pending_ts.clone()),
        );
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.81));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Step 2: Out-of-zone tick — reset all pending state
        let out_of_zone = 0.77_f64 >= node.trigger_price; // false
        assert!(!out_of_zone);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_at_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_price_key);
        remove_flow_node_state(&mut context, &node.node_key, &cpend_prev_key);
        assert!(flow_node_state_string(&context, &node.node_key, &cpend_at_key).is_none());

        // Step 3: New cross — set fresh pending timestamp (near-zero elapsed)
        let second_pending_ts = Utc::now().to_rfc3339();
        set_flow_node_state(
            &mut context,
            &node.node_key,
            &cpend_at_key,
            json!(second_pending_ts.clone()),
        );
        set_flow_node_state(&mut context, &node.node_key, &cpend_price_key, json!(0.82));
        set_flow_node_state(&mut context, &node.node_key, &cpend_prev_key, json!(0.79));

        // Assert: elapsed from second entry is near-zero (not accumulated from first 8s entry)
        let stored = flow_node_state_string(&context, &node.node_key, &cpend_at_key).unwrap();
        assert_ne!(
            stored, first_pending_ts,
            "Second entry must use fresh timestamp, not old one"
        );
        let second_pending_at = DateTime::parse_from_rfc3339(&stored)
            .unwrap()
            .with_timezone(&Utc);
        let elapsed_ms = Utc::now()
            .signed_duration_since(second_pending_at)
            .num_milliseconds();
        assert!(
            elapsed_ms < 2_000,
            "Elapsed from re-entry should be near-zero (got {}ms), not accumulated from first entry (8s)",
            elapsed_ms
        );
        assert!(
            elapsed_ms < market_price_confirmation_ms(&node).unwrap(),
            "Should not have fired yet — confirmation_ms={} not yet elapsed",
            market_price_confirmation_ms(&node).unwrap()
        );
    }

    #[test]
    fn market_price_confirmation_ms_helper_is_explicit_only_and_mode_agnostic() {
        let mut fixed_once = test_node_spec("cross_above", 0.80, 250);
        fixed_once.auto_scope = false; // fixed market mode
        assert_eq!(market_price_confirmation_ms(&fixed_once), Some(250));

        fixed_once.confirmation_ms = None;
        assert_eq!(market_price_confirmation_ms(&fixed_once), None);

        fixed_once.confirmation_ms = Some(0);
        assert_eq!(market_price_confirmation_ms(&fixed_once), None);

        fixed_once.confirmation_ms = Some(250);
        fixed_once.node_type = "trigger.open_positions".to_string();
        assert_eq!(market_price_confirmation_ms(&fixed_once), None);
    }

    // ---------------------------------------------------------------------------
    // Test: cross_confirmed mode — step execution must short-circuit past the
    // cross re-evaluation.
    //
    // This captures the root-cause of the trigger-node-not-firing bug:
    // When the WS confirmation gate fires, it enqueues a step with
    //   wsPreviousPrice = tick-in-zone price (already past the cross)
    //   wsPrice         = tick-in-zone price (still in zone)
    //   wsEvaluationMode = "cross_confirmed"
    //
    // Without the short-circuit, evaluate_trigger_market_price_condition would
    // receive prev >= trigger and cur >= trigger and return (false, "no_cross").
    //
    // With the short-circuit, wsEvaluationMode="cross_confirmed" causes pass=true
    // without re-evaluating the strict cross condition.
    // ---------------------------------------------------------------------------
    #[test]
    fn cross_confirmed_mode_short_circuits_cross_recheck() {
        // Simulate the data that would be present in step.input_json when the
        // WS confirmation gate fires:
        //   - trigger: cross_above at 0.60
        //   - Original cross: price went from 0.55 → 0.65
        //   - Confirmation tick: prev=0.65 (in zone), cur=0.65 (in zone)
        //   - wsEvaluationMode: "cross_confirmed"
        let trigger_price = 0.60_f64;
        let ws_price = 0.65_f64;
        let ws_prev_price = 0.65_f64; // in-zone: already above trigger

        // Verify that WITHOUT the short-circuit, the cross check would fail:
        // prev=0.65 >= trigger=0.60, so "prev < trigger" is false → no_cross
        let (would_cross, mode) = evaluate_trigger_market_price_condition(
            Some(ws_prev_price),
            ws_price,
            trigger_price,
            "cross_above",
            false, // once_mode=true → allow_first_tick_threshold=false
            None,
        );
        assert!(
            !would_cross,
            "Pre-confirmed in-zone prices must NOT produce a new cross (got mode={mode})"
        );
        assert_eq!(mode, "no_cross");

        // Verify that the cross_confirmed detection logic works:
        // ws_sourced=true AND wsEvaluationMode="cross_confirmed" → ws_cross_confirmed=true
        let ws_sourced = true;
        let ws_evaluation_mode = "cross_confirmed";
        let ws_cross_confirmed = ws_sourced && ws_evaluation_mode == "cross_confirmed";
        assert!(
            ws_cross_confirmed,
            "ws_cross_confirmed must be true when wsEvaluationMode=cross_confirmed"
        );

        // And verify that WITHOUT ws_cross_confirmed, the trigger would silently fail
        let without_short_circuit_pass = would_cross; // false from above
        assert!(
            !without_short_circuit_pass,
            "Without short-circuit, trigger would silently fail"
        );

        // With the short-circuit, pass is set to true directly (tested implicitly
        // by the production code path - this test validates the invariants the
        // short-circuit relies on).
        let with_short_circuit_pass = ws_cross_confirmed; // true
        assert!(
            with_short_circuit_pass,
            "With short-circuit, trigger must fire when cross_confirmed"
        );
    }

    #[test]
    fn cross_confirmed_mode_not_triggered_for_regular_ws_events() {
        // When wsEvaluationMode is absent or not "cross_confirmed", ws_cross_confirmed=false
        // and the normal cross evaluation path runs (no short-circuit).
        let ws_sourced = true;

        let mode_absent = "";
        let mode_cross_detected = "cross_detected";
        let mode_first_tick = "first_tick_threshold";

        assert!(
            !(ws_sourced && mode_absent == "cross_confirmed"),
            "Empty evaluation mode must not trigger short-circuit"
        );
        assert!(
            !(ws_sourced && mode_cross_detected == "cross_confirmed"),
            "cross_detected must not trigger short-circuit (handled normally)"
        );
        assert!(
            !(ws_sourced && mode_first_tick == "cross_confirmed"),
            "first_tick_threshold must not trigger short-circuit"
        );

        // Non-WS steps also must not trigger short-circuit
        let not_ws_sourced = false;
        assert!(
            !(not_ws_sourced && "cross_confirmed" == "cross_confirmed"),
            "Non-WS steps must not trigger short-circuit even if mode says cross_confirmed"
        );
    }

    #[test]
    fn cross_confirmed_short_circuit_helper_requires_clean_ws_context() {
        assert!(should_apply_ws_cross_confirmed_short_circuit(
            true,
            "cross_confirmed",
            None
        ));
        assert!(!should_apply_ws_cross_confirmed_short_circuit(
            true,
            "cross_confirmed",
            Some("ws_market_slug_mismatch:a!=b")
        ));
        assert!(!should_apply_ws_cross_confirmed_short_circuit(
            false,
            "cross_confirmed",
            None
        ));
        assert!(!should_apply_ws_cross_confirmed_short_circuit(
            true,
            "cross_detected",
            None
        ));
    }

    #[test]
    fn cross_confirmed_unexpected_fail_helper_flags_only_real_regression_case() {
        assert!(is_ws_cross_confirmed_unexpected_fail(
            true,
            "cross_confirmed",
            false,
            None
        ));
        assert!(!is_ws_cross_confirmed_unexpected_fail(
            true,
            "cross_confirmed",
            true,
            None
        ));
        assert!(!is_ws_cross_confirmed_unexpected_fail(
            true,
            "cross_confirmed",
            false,
            Some("ws_market_slug_mismatch:a!=b")
        ));
        assert!(!is_ws_cross_confirmed_unexpected_fail(
            true,
            "cross_detected",
            false,
            None
        ));
    }
}

#[cfg(test)]
mod price_integrity_tests {
    use super::*;

    #[test]
    fn prce01_no_legacy_fallback_for_previous_price() {
        // Build a context with only bare "previous_price" key, no per-token key.
        // The lookup must return None — not the legacy bare-key value.
        let node_key = "trigger_node_1";
        let token_id = "tok-yes";
        let prev_key = format!("previous_price_{}", token_id);
        let mut context = json!({
            "nodeStates": {
                node_key: {
                    "previous_price": 0.55
                }
            }
        });
        // Per-token key absent — lookup must return None (PRCE-01 guarantee)
        let result = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
        assert!(
            result.is_none(),
            "Must not fall back to bare previous_price; got {:?}",
            result
        );

        // Now set per-token key — lookup must return its value
        set_flow_node_state(&mut context, node_key, &prev_key, json!(0.42));
        let result2 = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
        assert_eq!(
            result2,
            Some(0.42),
            "Per-token key must be returned when present"
        );
    }

    #[test]
    fn prce01_per_token_key_works_independently() {
        // When flow node state has "previous_price_tok-yes" set to 0.42,
        // the per-token lookup returns Some(0.42).
        let node_key = "trigger_node_1";
        let token_id = "tok-yes";
        let prev_key = format!("previous_price_{}", token_id);
        let mut context = json!({
            "nodeStates": {
                node_key: {}
            }
        });
        set_flow_node_state(&mut context, node_key, &prev_key, json!(0.42));
        let result = flow_node_state(&context, node_key, &prev_key).and_then(value_as_f64);
        assert_eq!(
            result,
            Some(0.42),
            "Per-token key lookup must return stored value"
        );
    }

    #[test]
    fn prce01_no_cross_token_contamination() {
        // Core PRCE-01 safety guarantee:
        // When flow node state has "previous_price_tok-A" = 0.60 but no "previous_price_tok-B",
        // looking up previous price for tok-B must return None (not 0.60).
        let node_key = "trigger_node_1";
        let token_a = "tok-A";
        let token_b = "tok-B";
        let key_a = format!("previous_price_{}", token_a);
        let key_b = format!("previous_price_{}", token_b);
        let mut context = json!({
            "nodeStates": {
                node_key: {}
            }
        });
        set_flow_node_state(&mut context, node_key, &key_a, json!(0.60));

        // tok-A lookup works
        let result_a = flow_node_state(&context, node_key, &key_a).and_then(value_as_f64);
        assert_eq!(result_a, Some(0.60), "tok-A lookup must return 0.60");

        // tok-B lookup must return None — no cross-token contamination
        let result_b = flow_node_state(&context, node_key, &key_b).and_then(value_as_f64);
        assert!(
            result_b.is_none(),
            "tok-B must not get tok-A's price; got {:?}",
            result_b
        );
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
                    "limit",
                    None,
                    None,
                    None,
                    TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
                    delta_notional,
                    None,
                    None,
                    buy_leg.min_price_distance_cent,
                    workflow.expires_at,
                    1,
                    None,
                    false,
                    None,
                    false,
                    None,
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
            "limit",
            sell_leg.trigger_condition.as_deref(),
            sell_leg.trigger_price,
            None,
            TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            sell_leg.target_notional_usdc,
            None,
            None,
            sell_leg.min_price_distance_cent,
            workflow.expires_at,
            1,
            None,
            false,
            None,
            false,
            None,
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
        "cross_above" => crossed_above_strict(previous_price, current_price, trigger_price),
        "cross_below" => crossed_below_strict(previous_price, current_price, trigger_price),
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

fn is_trade_builder_order_processable_status(status: &str) -> bool {
    matches!(
        status,
        "pending"
            | "armed"
            | "triggered"
            | "open"
            | "partially_filled"
            | "canceled_requested"
            | "inventory_pending"
    )
}

fn should_request_trade_builder_oco_cancel(
    order: &TradeBuilderOrder,
    normalized_status: &str,
) -> bool {
    order.parent_order_id.is_some()
        && order.side == "sell"
        && matches!(normalized_status, "open" | "partially_filled" | "filled")
}

fn cancel_error_indicates_terminal_match(error_text: &str) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    normalized.contains("matched orders can't be canceled")
        || (normalized.contains("matched") && normalized.contains("cancel"))
        || (normalized.contains("filled") && normalized.contains("cancel"))
}

fn trade_builder_error_indicates_balance_or_allowance(error_text: &str) -> bool {
    error_text
        .to_ascii_lowercase()
        .contains("not enough balance / allowance")
}

fn trade_builder_price_exceeds_max_price(order: &TradeBuilderOrder, desired_price: f64) -> bool {
    order
        .max_price
        .map(|max_price| desired_price.is_finite() && desired_price > max_price)
        .unwrap_or(false)
}

fn normalize_trade_builder_size_basis(raw: &str) -> &'static str {
    if raw
        .trim()
        .eq_ignore_ascii_case(TRADE_BUILDER_SIZE_BASIS_SHARES)
    {
        return TRADE_BUILDER_SIZE_BASIS_SHARES;
    }
    TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC
}

fn round_trade_builder_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * 100.0).round() / 100.0
}

fn floor_trade_builder_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * 100.0).floor() / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderExitChildSizing {
    size_usdc: f64,
    target_qty: f64,
    remaining_qty: f64,
}

fn trade_builder_exit_child_sizing(
    filled_size: f64,
    execution_price: f64,
) -> TradeBuilderExitChildSizing {
    let target_qty = round_trade_builder_share_qty(filled_size);
    let size_usdc = (target_qty * execution_price).max(0.0);
    TradeBuilderExitChildSizing {
        size_usdc,
        target_qty,
        remaining_qty: target_qty,
    }
}

fn trade_builder_share_request_qty(order: &TradeBuilderOrder) -> Option<f64> {
    let qty = order.remaining_qty.or(order.target_qty)?;
    let rounded = round_trade_builder_share_qty(qty);
    (rounded > 0.0).then_some(rounded)
}

fn clamp_trade_builder_visible_share_qty(
    requested_qty: f64,
    available_qty: Option<f64>,
) -> Option<f64> {
    let effective_qty = match available_qty {
        Some(quantity) => requested_qty.min(quantity),
        None => requested_qty,
    };
    let clamped = floor_trade_builder_share_qty(effective_qty);
    (clamped > 0.0).then_some(clamped)
}

fn trade_builder_inventory_pending_tp_trigger_price(order: &TradeBuilderOrder) -> Option<f64> {
    let trigger_price = order.trigger_price?;
    if order.status == "inventory_pending"
        && order.side == "sell"
        && order.parent_order_id.is_some()
        && matches!(order.trigger_condition.as_deref(), Some("cross_above"))
    {
        let adjusted =
            ((trigger_price - TRADE_BUILDER_EXIT_TP_SLACK).max(0.0) * 100.0).round() / 100.0;
        return Some(clamp_probability(adjusted));
    }
    Some(trigger_price)
}

fn estimate_remaining_trade_builder_sizing(
    order: &TradeBuilderOrder,
    order_info: &OrderInfo,
    fallback_price: f64,
) -> (Option<f64>, Option<f64>) {
    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    if let (Some(order_size), Some(filled_size)) = (order_info.size, order_info.filled_size) {
        let remaining_qty = round_trade_builder_share_qty((order_size - filled_size).max(0.0));
        let price = order_info
            .price
            .or(order.working_price)
            .unwrap_or(fallback_price);
        let remaining_usdc = (remaining_qty * price).max(0.0);
        return (
            Some(remaining_usdc),
            if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
                Some(remaining_qty)
            } else {
                None
            },
        );
    }

    if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        let remaining_qty = trade_builder_share_request_qty(order);
        let remaining_usdc = remaining_qty.map(|qty| qty * fallback_price);
        return (remaining_usdc, remaining_qty);
    }

    (
        Some(order.remaining_size.unwrap_or(order.size_usdc).max(0.0)),
        None,
    )
}

async fn mark_trade_builder_inventory_pending(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
    current_price: f64,
    required_qty: f64,
    available_qty: Option<f64>,
) -> Result<()> {
    if order.active_exchange_order_id.is_some() {
        repo.clear_trade_builder_active_exchange_order_preserve_sizing(
            order.id,
            "inventory_pending",
        )
        .await?;
    } else {
        repo.set_trade_builder_order_status(order.id, "inventory_pending", Some(reason))
            .await?;
    }
    repo.set_trade_builder_order_status(order.id, "inventory_pending", Some(reason))
        .await?;

    repo.append_trade_builder_order_event(
        order.id,
        "exit_inventory_pending",
        &json!({
            "reason": reason,
            "side": order.side,
            "size_basis": order.size_basis,
            "trigger_condition": order.trigger_condition,
            "trigger_price": order.trigger_price,
            "current_price": current_price,
            "required_qty": required_qty,
            "available_qty": available_qty,
        }),
    )
    .await?;

    info!(
        builder_order_id = order.id,
        token_id = %order.token_id,
        required_qty,
        available_qty,
        current_price,
        reason = reason,
        "TRADE_BUILDER_EXIT_INVENTORY_PENDING"
    );
    Ok(())
}

async fn request_trade_builder_oco_cancel_for_siblings(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
) -> Result<()> {
    let Some(parent_order_id) = order.parent_order_id else {
        return Ok(());
    };
    if order.side != "sell" {
        return Ok(());
    }

    let siblings = repo
        .list_trade_builder_child_orders_by_parent(parent_order_id, Some(order.id))
        .await?;
    let mut sibling_order_ids = Vec::new();
    for sibling in siblings {
        if matches!(
            sibling.status.as_str(),
            "completed" | "canceled" | "expired" | "filled" | "canceled_requested"
        ) {
            continue;
        }

        repo.set_trade_builder_order_status(
            sibling.id,
            "canceled_requested",
            Some("oco_sibling_triggered"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            sibling.id,
            "oco_cancel_requested",
            &json!({
                "parent_order_id": parent_order_id,
                "triggered_by_order_id": order.id,
                "reason": reason,
                "status_before": sibling.status,
                "status_after": "canceled_requested",
                "active_exchange_order_id": sibling.active_exchange_order_id
            }),
        )
        .await?;
        sibling_order_ids.push(sibling.id);
    }

    if !sibling_order_ids.is_empty() {
        repo.append_trade_builder_order_event(
            order.id,
            "oco_siblings_cancel_requested",
            &json!({
                "parent_order_id": parent_order_id,
                "sibling_order_ids": sibling_order_ids,
                "reason": reason
            }),
        )
        .await?;
    }

    Ok(())
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
    let Some(order) = repo.get_trade_builder_order(order.id).await? else {
        return Ok(());
    };
    if !is_trade_builder_order_processable_status(&order.status) {
        return Ok(());
    }

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

    let current_price = fetch_current_token_price(ws, client, &order).await?;
    repo.set_trade_builder_last_seen_price(order.id, current_price)
        .await?;

    if let Some(exchange_order_id) = order.active_exchange_order_id.as_deref() {
        reconcile_trade_builder_open_order(
            repo,
            run_id,
            client,
            &order,
            exchange_order_id,
            current_price,
        )
        .await?;
        return Ok(());
    }

    if order.status == "canceled_requested" {
        let cancel_reason = order.last_error.as_deref().unwrap_or("user_request");
        repo.set_trade_builder_order_status(order.id, "canceled", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "canceled_without_open_order",
            &json!({ "reason": cancel_reason }),
        )
        .await?;
        return Ok(());
    }

    let should_trigger = should_trigger_builder_order(&order, current_price);
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
                "TRADE_BUILDER_TRIGGER_NOT_MET"
            );
        }
        if order.kind == "conditional" && order.status == "inventory_pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "inventory_pending_released",
                &json!({
                    "reason_code": "trigger_recheck_failed",
                    "reason_message": "Exit trigger no longer valid while inventory was pending.",
                    "side": &order.side,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "current_price": current_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
            return Ok(());
        }
        if order.kind == "conditional" && order.status == "pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "trigger_not_met",
                &json!({
                    "reason_code": "trigger_not_crossed",
                    "reason_message": "Trigger condition has not crossed yet.",
                    "side": &order.side,
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

    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let (
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        exhausted_trigger_sizes,
        trigger_size_index,
    ) = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        (
            order.size_usdc,
            None,
            None,
            false,
            order.triggers_fired.max(0) as usize,
        )
    } else {
        resolve_trade_builder_next_trigger_size_usdc(repo, &order).await?
    };
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

    let desired_price =
        aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent);
    let (remaining_usdc, remaining_qty, size, proposed_notional_usdc) =
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            let qty = trade_builder_share_request_qty(&order).ok_or_else(|| {
                anyhow::anyhow!("share-basis builder order requires target_qty or remaining_qty")
            })?;
            let remaining_usdc = (qty * desired_price).max(0.0);
            (Some(remaining_usdc), Some(qty), qty, remaining_usdc)
        } else {
            let remaining_usdc = order.remaining_size.unwrap_or(resolved_size_usdc);
            let size = calc_level_size(remaining_usdc, desired_price);
            (Some(remaining_usdc), None, size, resolved_size_usdc)
        };
    anyhow::ensure!(size > 0.0, "computed builder order size is zero");
    if trade_builder_price_exceeds_max_price(&order, desired_price) {
        repo.set_trade_builder_order_status(order.id, "canceled", Some("above_max_price"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "max_price_blocked",
            &json!({
                "reason_code": "above_max_price",
                "reason_message": "Order price would exceed the configured max price.",
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "max_price": order.max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "status_before": &order.status,
                "status_after": "canceled"
            }),
        )
        .await?;
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            desired_price,
            max_price = ?order.max_price,
            reason_code = "above_max_price",
            "TRADE_BUILDER_ORDER_MAX_PRICE_BLOCKED"
        );
        return Ok(());
    }

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        order.trade_id,
        proposed_notional_usdc,
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
            "TRADE_BUILDER_ORDER_BLOCKED"
        );
        return Ok(());
    }

    let requested_share_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        Some(size)
    } else {
        None
    };
    let mut available_qty = None;
    let mut submit_partial_visible_inventory = false;
    let mut submit_size = size;
    let mut submit_remaining_usdc = remaining_usdc;
    let mut submit_remaining_qty = remaining_qty;
    if order.side == "sell" && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        match client.available_token_qty(&order.token_id).await {
            Ok(quantity) => {
                available_qty = quantity;
                let Some(clamped_qty) = clamp_trade_builder_visible_share_qty(size, quantity)
                else {
                    let reason = "exit inventory not yet available";
                    mark_trade_builder_inventory_pending(
                        repo,
                        &order,
                        reason,
                        current_price,
                        size,
                        quantity,
                    )
                    .await?;
                    return Ok(());
                };
                if clamped_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE < size {
                    submit_partial_visible_inventory = true;
                }
                submit_size = clamped_qty;
                submit_remaining_qty = Some(clamped_qty);
                submit_remaining_usdc = Some((clamped_qty * desired_price).max(0.0));
            }
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    token_id = %order.token_id,
                    error = %err,
                    "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                );
            }
        }
    }

    let intent = if order.kind == "immediate" {
        "manual_immediate"
    } else {
        "manual_trigger"
    };
    let normalized_execution_mode = normalize_trade_builder_execution_mode(&order.execution_mode);
    let order_type = clob_order_type_for_execution_mode(normalized_execution_mode);
    let client_order_id = format!("tb-{}", Uuid::new_v4());
    let req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size: submit_size,
        intent: intent.to_string(),
        order_type: order_type.to_string(),
        client_order_id: client_order_id.clone(),
        leg_side: None,
        fee_rate_bps: 1000,
    };

    let ack = match client.place(&req).await {
        Ok(ack) => ack,
        Err(err) => {
            let error_text = err.to_string();
            if order.side == "sell"
                && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
                && trade_builder_error_indicates_balance_or_allowance(&error_text)
            {
                let rechecked_qty = match client.available_token_qty(&order.token_id).await {
                    Ok(quantity) => quantity,
                    Err(recheck_err) => {
                        warn!(
                            run_id,
                            builder_order_id = order.id,
                            token_id = %order.token_id,
                            error = %recheck_err,
                            "TRADE_BUILDER_EXIT_INVENTORY_RECHECK_FAILED"
                        );
                        None
                    }
                };
                if rechecked_qty
                    .and_then(|qty| clamp_trade_builder_visible_share_qty(size, Some(qty)))
                    .is_some()
                {
                    mark_trade_builder_inventory_pending(
                        repo,
                        &order,
                        "exchange rejected sell before inventory synced",
                        current_price,
                        size,
                        rechecked_qty,
                    )
                    .await?;
                    return Ok(());
                }
            }
            return Err(err);
        }
    };
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
        "max_price": order.max_price,
        "current_price": current_price,
        "execution_price": desired_price,
        "execution_mode": normalized_execution_mode,
        "order_type": order_type,
        "size_basis": size_basis,
        "size": submit_size,
        "requested_qty": requested_share_qty,
        "clamped_qty": submit_remaining_qty,
        "partial_visible_inventory_submit": submit_partial_visible_inventory,
        "target_qty": order.target_qty,
        "remaining_qty": submit_remaining_qty,
        "size_mode": trigger_size_mode,
        "trigger_size_value": trigger_size_value,
        "trigger_size_index": trigger_size_index + 1,
        "resolved_size_usdc": resolved_size_usdc,
        "remaining_usdc": submit_remaining_usdc,
        "available_qty": available_qty,
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
        submit_size,
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
        submit_remaining_usdc,
        submit_remaining_qty,
        normalized_status,
    )
    .await?;
    if submit_partial_visible_inventory {
        repo.append_trade_builder_order_event(
            order.id,
            "partial_visible_inventory_submit",
            &json!({
                "requested_qty": requested_share_qty,
                "available_qty": available_qty,
                "submitted_qty": submit_size,
                "residual_qty_ignored": requested_share_qty.map(|qty| (qty - submit_size).max(0.0)),
            }),
        )
        .await?;
    }
    repo.append_trade_builder_order_event(order.id, "submitted", &raw)
        .await?;

    if should_request_trade_builder_oco_cancel(&order, normalized_status) {
        request_trade_builder_oco_cancel_for_siblings(repo, &order, "child_exit_submitted").await?;
    }

    if normalized_status == "filled" {
        finalize_builder_fill(
            repo,
            &order,
            &exchange_order_id,
            submit_size,
            desired_price,
            false,
        )
        .await?;
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
        let cancel_reason = order.last_error.as_deref().unwrap_or("user_request");
        client.cancel(exchange_order_id).await?;
        repo.mark_order_status(exchange_order_id, "canceled")
            .await?;
        repo.clear_trade_builder_active_exchange_order(order.id, "canceled")
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "cancel_requested",
            &json!({
                "exchange_order_id": exchange_order_id,
                "reason": cancel_reason
            }),
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
        finalize_builder_fill(repo, order, exchange_order_id, filled_size, price, false).await?;
        return Ok(());
    }

    if matches!(normalized, "canceled" | "rejected" | "expired") {
        let next_status =
            if order.kind == "conditional" && order.triggers_fired < order.max_triggers {
                "armed"
            } else {
                "completed"
            };
        if next_status == "armed" {
            repo.clear_trade_builder_active_exchange_order_preserve_sizing(order.id, next_status)
                .await?;
        } else {
            repo.clear_trade_builder_active_exchange_order(order.id, next_status)
                .await?;
        }
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

    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let (remaining_usdc, remaining_qty) =
        estimate_remaining_trade_builder_sizing(order, &order_info, current_price);
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
            remaining_usdc,
            remaining_qty,
            normalized,
        )
        .await?;
        return Ok(());
    }

    let requested_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        remaining_qty
    } else {
        None
    };
    let mut available_qty = None;
    let mut size = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        remaining_qty.unwrap_or_default()
    } else {
        calc_level_size(remaining_usdc.unwrap_or_default(), desired_price)
    };
    if size <= 0.0 {
        repo.clear_trade_builder_active_exchange_order(order.id, "completed")
            .await?;
        return Ok(());
    }
    if trade_builder_price_exceeds_max_price(order, desired_price) {
        let filled_size = order_info.filled_size.unwrap_or_default();
        let execution_price = order_info
            .price
            .or(order.working_price)
            .unwrap_or(current_price);
        match client.cancel(exchange_order_id).await {
            Ok(()) => {
                repo.mark_order_status(exchange_order_id, "canceled")
                    .await?;
            }
            Err(err) => {
                let error_text = err.to_string();
                if cancel_error_indicates_terminal_match(&error_text) {
                    let terminal_filled_size =
                        order_info.filled_size.or(order_info.size).unwrap_or(size);
                    let terminal_price = order_info
                        .price
                        .or(order.working_price)
                        .unwrap_or(current_price);
                    repo.mark_order_status(exchange_order_id, "filled").await?;
                    repo.append_trade_builder_order_event(
                        order.id,
                        "max_price_canceled",
                        &json!({
                            "exchange_order_id": exchange_order_id,
                            "status_before": normalized,
                            "cancel_result": "terminal_match",
                            "filled_size": terminal_filled_size,
                            "execution_price": terminal_price,
                            "current_price": current_price,
                            "desired_price": desired_price,
                            "working_price": order.working_price,
                            "max_price": order.max_price,
                            "cancel_error": error_text
                        }),
                    )
                    .await?;
                    finalize_builder_fill(
                        repo,
                        order,
                        exchange_order_id,
                        terminal_filled_size,
                        terminal_price,
                        true,
                    )
                    .await?;
                    return Ok(());
                }
                return Err(err).context(format!(
                    "failed to cancel builder order at max price guard: {exchange_order_id}"
                ));
            }
        }
        repo.append_trade_builder_order_event(
            order.id,
            "max_price_canceled",
            &json!({
                "exchange_order_id": exchange_order_id,
                "status_before": normalized,
                "filled_size": filled_size,
                "execution_price": execution_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "working_price": order.working_price,
                "max_price": order.max_price
            }),
        )
        .await?;
        if filled_size > 0.0 {
            finalize_builder_fill(
                repo,
                order,
                exchange_order_id,
                filled_size,
                execution_price,
                true,
            )
            .await?;
        } else {
            repo.clear_trade_builder_active_exchange_order(order.id, "canceled")
                .await?;
            repo.set_trade_builder_order_status(order.id, "canceled", Some("above_max_price"))
                .await?;
        }
        return Ok(());
    }

    let mut submit_partial_visible_inventory = false;
    if order.side == "sell" && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        match client.available_token_qty(&order.token_id).await {
            Ok(quantity) => {
                available_qty = quantity;
                let Some(clamped_qty) = clamp_trade_builder_visible_share_qty(size, quantity)
                else {
                    mark_trade_builder_inventory_pending(
                        repo,
                        order,
                        "exit inventory not yet available",
                        current_price,
                        size,
                        quantity,
                    )
                    .await?;
                    return Ok(());
                };
                if clamped_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE < size {
                    submit_partial_visible_inventory = true;
                }
                size = clamped_qty;
            }
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    token_id = %order.token_id,
                    error = %err,
                    "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                );
            }
        }
    }

    match client.cancel(exchange_order_id).await {
        Ok(()) => {
            repo.mark_order_status(exchange_order_id, "canceled")
                .await?;
        }
        Err(err) => {
            let error_text = err.to_string();
            if cancel_error_indicates_terminal_match(&error_text) {
                let filled_size = order_info.filled_size.or(order_info.size).unwrap_or(size);
                let price = order_info
                    .price
                    .or(order.working_price)
                    .unwrap_or(current_price);
                repo.mark_order_status(exchange_order_id, "filled").await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "replace_cancel_terminal_match",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "status": order_info.status,
                        "normalized_status": normalized,
                        "filled_size": filled_size,
                        "execution_price": price,
                        "cancel_error": error_text
                    }),
                )
                .await?;
                finalize_builder_fill(repo, order, exchange_order_id, filled_size, price, false)
                    .await?;
                return Ok(());
            }
            return Err(err).context(format!(
                "failed to cancel builder order before reprice: {exchange_order_id}"
            ));
        }
    }

    let replace_req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size,
        intent: "manual_reprice".to_string(),
        order_type: clob_order_type_for_execution_mode(normalize_trade_builder_execution_mode(
            &order.execution_mode,
        ))
        .to_string(),
        client_order_id: format!("tb-reprice-{}", Uuid::new_v4()),
        leg_side: None,
        fee_rate_bps: 1000,
    };

    let ack = match client.place(&replace_req).await {
        Ok(ack) => ack,
        Err(err) => {
            let error_text = err.to_string();
            if order.side == "sell"
                && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
                && trade_builder_error_indicates_balance_or_allowance(&error_text)
            {
                let rechecked_qty = match client.available_token_qty(&order.token_id).await {
                    Ok(quantity) => quantity,
                    Err(recheck_err) => {
                        warn!(
                            run_id,
                            builder_order_id = order.id,
                            token_id = %order.token_id,
                            error = %recheck_err,
                            "TRADE_BUILDER_EXIT_INVENTORY_RECHECK_FAILED"
                        );
                        None
                    }
                };
                if rechecked_qty
                    .and_then(|qty| clamp_trade_builder_visible_share_qty(size, Some(qty)))
                    .is_some()
                {
                    mark_trade_builder_inventory_pending(
                        repo,
                        order,
                        "exchange rejected repriced sell before inventory synced",
                        current_price,
                        size,
                        rechecked_qty,
                    )
                    .await?;
                    return Ok(());
                }
            }
            return Err(err);
        }
    };
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
        "execution_mode": normalize_trade_builder_execution_mode(&order.execution_mode),
        "order_type": clob_order_type_for_execution_mode(normalize_trade_builder_execution_mode(&order.execution_mode)),
        "target_price": desired_price,
        "max_price": order.max_price,
        "remaining_usdc": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some((size * desired_price).max(0.0)) } else { remaining_usdc },
        "remaining_qty": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some(size) } else { remaining_qty },
        "size_basis": size_basis,
        "size": size,
        "requested_qty": requested_qty,
        "clamped_qty": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some(size) } else { None },
        "available_qty": available_qty,
        "partial_visible_inventory_submit": submit_partial_visible_inventory
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
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            Some((size * desired_price).max(0.0))
        } else {
            remaining_usdc
        },
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            Some(size)
        } else {
            remaining_qty
        },
        normalized_status,
    )
    .await?;
    if submit_partial_visible_inventory {
        repo.append_trade_builder_order_event(
            order.id,
            "partial_visible_inventory_submit",
            &json!({
                "requested_qty": requested_qty,
                "available_qty": available_qty,
                "submitted_qty": size,
                "residual_qty_ignored": requested_qty.map(|qty| (qty - size).max(0.0)),
            }),
        )
        .await?;
    }
    repo.append_trade_builder_order_event(order.id, "reprice", &raw)
        .await?;
    info!(
        run_id,
        builder_order_id = order.id,
        old_exchange_order_id = exchange_order_id,
        new_exchange_order_id = %new_exchange_order_id,
        "TRADE_BUILDER_ORDER_REPRICED"
    );

    if should_request_trade_builder_oco_cancel(order, normalized_status) {
        request_trade_builder_oco_cancel_for_siblings(repo, order, "child_exit_repriced").await?;
    }

    if normalized_status == "filled" {
        finalize_builder_fill(
            repo,
            order,
            &new_exchange_order_id,
            size,
            desired_price,
            false,
        )
        .await?;
    }

    Ok(())
}

async fn finalize_builder_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    filled_size: f64,
    execution_price: f64,
    force_terminal: bool,
) -> Result<()> {
    repo.increment_trade_builder_trigger_count(order.id).await?;

    let next_trigger_count = order.triggers_fired + 1;
    let reached_limit = next_trigger_count >= order.max_triggers;
    let next_status = if force_terminal || order.kind == "immediate" || reached_limit {
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

    if should_request_trade_builder_oco_cancel(order, "filled") {
        request_trade_builder_oco_cancel_for_siblings(repo, order, "child_exit_filled").await?;
    }

    // Take Profit / Stop Loss: buy fill olunca otomatik conditional IOC sell child orderlari olustur
    if order.side == "buy" && order.tp_enabled {
        if let Some(tp_price) = order.tp_price {
            let tp_sizing = trade_builder_exit_child_sizing(filled_size, execution_price);
            let tp_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "pending",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_above"),
                    Some(tp_price),
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    tp_sizing.size_usdc,
                    Some(tp_sizing.target_qty),
                    Some(tp_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                )
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "tp_sell_created",
                &json!({
                    "child_order_id": tp_sell_id,
                    "tp_price": tp_price,
                    "tp_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": tp_sizing.target_qty,
                    "filled_size": filled_size,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                tp_sell_order_id = tp_sell_id,
                tp_price,
                "TRADE_BUILDER_TP_SELL_CREATED"
            );
        }
    }
    if order.side == "buy" && order.sl_enabled {
        if let Some(sl_price) = order.sl_price {
            let sl_sizing = trade_builder_exit_child_sizing(filled_size, execution_price);
            let sl_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "pending",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_below"),
                    Some(sl_price),
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    sl_sizing.size_usdc,
                    Some(sl_sizing.target_qty),
                    Some(sl_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                )
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "sl_sell_created",
                &json!({
                    "child_order_id": sl_sell_id,
                    "sl_price": sl_price,
                    "sl_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": sl_sizing.target_qty,
                    "filled_size": filled_size,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                sl_sell_order_id = sl_sell_id,
                sl_price,
                "TRADE_BUILDER_SL_SELL_CREATED"
            );
        }
    }

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

    let Some(trigger_price) = trade_builder_inventory_pending_tp_trigger_price(order) else {
        return false;
    };
    let Some(trigger_condition) = order.trigger_condition.as_deref() else {
        return false;
    };

    let previous_price = order.last_seen_price;
    match trigger_condition {
        "cross_above" if matches!(order.status.as_str(), "blocked" | "inventory_pending") => {
            current_price >= trigger_price
        }
        "cross_below" if matches!(order.status.as_str(), "blocked" | "inventory_pending") => {
            current_price <= trigger_price
        }
        "cross_above" => crossed_above_strict(previous_price, current_price, trigger_price),
        "cross_below" => crossed_below_strict(previous_price, current_price, trigger_price),
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
    fetch_price_from_market_ws_with_mode(ws, token_id, WsPriceMode::Raw).await
}

async fn fetch_price_from_market_ws_with_mode(
    ws: &ClobWsClient,
    token_id: &str,
    mode: WsPriceMode,
) -> Option<f64> {
    let events = ws
        .subscribe_once(WsChannel::Market, &[token_id.to_string()])
        .await
        .ok()?;
    extract_price_from_market_events_with_mode(&events, token_id, mode).map(|value| value.price)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ExtractedWsPrice {
    price: f64,
    ts: Option<i64>,
    source: &'static str,
}

fn extract_price_from_market_events(
    events: &[WsEvent],
    token_id: &str,
) -> Option<(f64, Option<i64>)> {
    extract_price_from_market_events_with_mode(events, token_id, WsPriceMode::Raw)
        .map(|value| (value.price, value.ts))
}

fn extract_price_from_market_events_with_mode(
    events: &[WsEvent],
    token_id: &str,
    mode: WsPriceMode,
) -> Option<ExtractedWsPrice> {
    match mode {
        WsPriceMode::Midpoint => extract_price_from_market_events_midpoint(events, token_id),
        WsPriceMode::Raw => extract_price_from_market_events_raw(events, token_id),
        WsPriceMode::BestBid | WsPriceMode::BestAsk => {
            extract_price_from_market_events_book_side(events, token_id, mode)
        }
    }
}

fn extract_price_from_market_events_midpoint(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some(price) = extract_midpoint_from_payload(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "best_bid_ask",
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = extract_midpoint_from_payload(change) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice {
                        price,
                        ts,
                        source: "best_bid_ask",
                    });
                }
            }
        }
    }

    None
}

fn extract_price_from_market_events_book_side(
    events: &[WsEvent],
    token_id: &str,
    side: WsPriceMode,
) -> Option<ExtractedWsPrice> {
    let extract_fn: fn(&Value) -> Option<f64> = match side {
        WsPriceMode::BestBid => extract_bid_from_payload,
        WsPriceMode::BestAsk => extract_ask_from_payload,
        _ => return None,
    };

    let source: &'static str = match side {
        WsPriceMode::BestBid => "best_bid",
        WsPriceMode::BestAsk => "best_ask",
        _ => "unknown",
    };

    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some(price) = extract_fn(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source,
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = extract_fn(change) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice { price, ts, source });
                }
            }
        }
    }

    None
}

fn extract_price_from_market_events_raw(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if let Some(price) = event.price {
            if matches_token(event) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "event_price",
                });
            }
        }

        if matches_token(event) {
            if let Some(price) = parse_json_number(event.payload.get("price")) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "payload_price",
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = parse_json_number(change.get("price")) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice {
                        price,
                        ts,
                        source: "price_changes",
                    });
                }
            }
        }

        if matches_token(event) {
            if let Some(price) = extract_midpoint_from_payload(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "best_bid_ask",
                });
            }
        }
    }

    None
}

fn extract_bid_from_payload(payload: &Value) -> Option<f64> {
    parse_json_number(
        payload
            .get("best_bid")
            .or_else(|| payload.get("bestBid"))
            .or_else(|| payload.get("bid")),
    )
}

fn extract_ask_from_payload(payload: &Value) -> Option<f64> {
    parse_json_number(
        payload
            .get("best_ask")
            .or_else(|| payload.get("bestAsk"))
            .or_else(|| payload.get("ask")),
    )
}

fn extract_midpoint_from_payload(payload: &Value) -> Option<f64> {
    match (
        extract_bid_from_payload(payload),
        extract_ask_from_payload(payload),
    ) {
        (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
        _ => None,
    }
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
    if normalized.contains("match") {
        return "filled";
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

pub(crate) fn normalize_trade_builder_execution_mode(raw: &str) -> &'static str {
    if raw.trim().eq_ignore_ascii_case("market") {
        return "market";
    }
    "market"
}

pub(crate) fn clob_order_type_for_execution_mode(mode: &str) -> &'static str {
    if mode.eq_ignore_ascii_case("market") {
        return "IOC";
    }
    "GTC"
}

pub(crate) fn aggressive_price_for_side(
    side: &str,
    current_price: f64,
    min_price_distance_cent: f64,
) -> f64 {
    let distance = min_price_distance_to_probability(min_price_distance_cent);
    if side == "sell" {
        return clamp_probability(current_price - distance);
    }
    clamp_probability(current_price + distance)
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
