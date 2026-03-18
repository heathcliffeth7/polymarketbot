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
    PendingTradeBuilderFirstVisibleInventoryObservation, PostgresRepository,
    TradeBuilderInventoryObservationInput, TradeBuilderOrder, TradeBuilderWorkflow,
    TradeBuilderWorkflowLeg, TradeFlowDefinitionRuntime, TradeFlowRun, TradeFlowRunStep,
    TradeFlowVersionRuntime,
};
use bot_infra::exchange::{
    ClobHttpClient, ClobRestClient, FillInfo, GammaClient, GammaHttpClient, GammaMarket, OrderInfo,
    PlaceOrderRequest,
};
use bot_infra::market_data::{MarketDataProvider, MockMarketDataProvider};
use bot_infra::reconcile::reconcile_tick_and_snapshot;
use bot_infra::signer::ApiCredentials;
use bot_infra::ws::{
    ClobWsClient, MarketDataSnapshot, MarketTickCallback, WsChannel, WsEvent, WsEventType,
};
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
    sync::{Arc, LazyLock, Mutex as StdMutex},
    time::Instant,
};
use tokio::{
    sync::{Notify, RwLock},
    time::{sleep, Duration},
};
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
const FLOW_HOUSEKEEPING_INTERVAL_MS: u64 = 1_000;
const FLOW_BOUNDARY_REFRESH_RETRY_MS: u64 = 1_000;
const FLOW_WS_FAST_PATH_DEBOUNCE_MS: u64 = 1;
const FLOW_RUNTIME_CACHE_TTL_SECS: u64 = 60;
const TRADE_BUILDER_EXIT_QTY_TOLERANCE: f64 = 0.011;
const TRADE_BUILDER_EXIT_TP_SLACK: f64 = 0.05;
const TRADE_BUILDER_EXIT_TRIGGER_BUFFER: f64 = 0.05;
const TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER: f64 = 0.03;
const TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER_RATE: f64 = 0.01;
const TRADE_BUILDER_EXIT_RETRY_MIN_DECREMENT: f64 = 0.01;
const TRADE_BUILDER_EXIT_STAGE_MARKER_PREFIX: &str = "[exit_submit_stage=";
const TRADE_BUILDER_INVENTORY_OBSERVATION_LIMIT: i64 = 250;
const TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC: &str = "notional_usdc";
const TRADE_BUILDER_SIZE_BASIS_SHARES: &str = "shares";
const TRADE_BUILDER_OBSERVATION_KIND_BASELINE: &str = "buy_inventory_baseline";
const TRADE_BUILDER_OBSERVATION_KIND_SUBMIT: &str = "buy_submit_dynamic_qty";
const TRADE_BUILDER_OBSERVATION_KIND_FILL: &str = "buy_fill_resolution";
const TRADE_BUILDER_OBSERVATION_KIND_FIRST_VISIBLE: &str = "first_visible_inventory";
const DEFAULT_TRADE_BUILDER_FEE_RATE_BPS: u64 = 1000;
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
static TRADE_FLOW_WS_FAST_PATH_CACHE: LazyLock<RwLock<TradeFlowWsFastPathCache>> =
    LazyLock::new(|| RwLock::new(TradeFlowWsFastPathCache::default()));
static UNDERLYING_REFERENCE_SERVICE: LazyLock<UnderlyingReferenceService> =
    LazyLock::new(UnderlyingReferenceService::new);
static FLOW_PROCESS_NOTIFY: LazyLock<Notify> = LazyLock::new(Notify::new);
static TELEGRAM_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .pool_max_idle_per_host(8)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .build()
        .expect("telegram http client")
});
const WORKFLOW_MIN_BUY_INCREMENT_USDC: f64 = 1.0;
const FLOW_NODE_STATE_ONCE_FIRED: &str = "once_fired";
const FLOW_NODE_STATE_ONCE_FIRED_AT: &str = "once_fired_at";
const FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG: &str = "once_fired_market_slug";
const FLOW_NODE_STATE_ONCE_BLOCK_LOGGED: &str = "once_blocked_logged";
const FLOW_NODE_STATE_REENTRY_GENERATION: &str = "reentry_generation";
const FLOW_NODE_STATE_REENTRY_ATTEMPTS_USED: &str = "reentry_attempts_used";
const FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX: &str = "cycle_window_boundary_marker_";
const FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG: &str =
    "publish_auto_scope_locked_market_slug";
const TRADE_BUILDER_GUARD_BLOCKED_STATUS: &str = "guard_blocked";
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
