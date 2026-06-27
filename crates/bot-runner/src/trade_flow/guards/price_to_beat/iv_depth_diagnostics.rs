use super::iv_mismatch_edge_config::PriceToBeatIvMismatchEdgeConfig;
use super::iv_mismatch_protection::PriceToBeatIvBookQuotes;
use super::runtime::PriceToBeatGuardRuntimeContext;
use super::{normalize_outcome_direction, PriceToBeatSignalFormulaMarketInput};
use bot_infra::exchange::OrderBookSnapshot;
use serde_json::{json, Map, Value};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    future::Future,
    sync::{LazyLock, Mutex as StdMutex},
    time::{Duration, Instant},
};

const SOURCE_SELECTED_ENTRY_SIZE_USDC: &str = "selected_entry_size_usdc";
const SOURCE_SIZE_USDC: &str = "size_usdc";
const SOURCE_TARGET_NOTIONAL_USDC: &str = "target_notional_usdc";
const SOURCE_TARGET_QTY: &str = "target_qty";
const SOURCE_NONE: &str = "none";
const TOKEN_SOURCE_ACTION_IDENTITY: &str = "action_identity";
const TOKEN_SOURCE_MISSING: &str = "missing";
const OPPOSITE_TOKEN_SOURCE_PAIRED_STEP_TOKENS: &str = "paired_step_tokens";
const OPPOSITE_TOKEN_SOURCE_MISSING: &str = "missing";
const OPPOSITE_TOKEN_SOURCE_UNTRUSTED_PAIR: &str = "untrusted_pair";
const ORDER_BOOK_CACHE_SCOPE_HOUSEKEEPING_PASS: &str = "housekeeping_pass";
const ORDER_BOOK_CACHE_SCOPE_PROCESS_TTL: &str = "process_ttl";
const ORDER_BOOK_NOT_REQUESTED_EARLY_BLOCK: &str = "not_requested_early_block";
const ORDER_BOOK_SUCCESS_TTL: Duration = Duration::from_millis(250);
const ORDER_BOOK_ERROR_TTL: Duration = Duration::from_millis(100);
const ORDER_BOOK_GUARD_FETCH_TIMEOUT: Duration = Duration::from_millis(750);
const ORDER_BOOK_TTL_CACHE_MAX_ENTRIES: usize = 512;

tokio::task_local! {
    static IV_DEPTH_ORDER_BOOK_PASS_STATE: RefCell<IvDepthOrderBookPassState>;
}

static IV_DEPTH_ORDER_BOOK_TTL_CACHE: LazyLock<
    StdMutex<HashMap<String, IvDepthOrderBookTtlEntry>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

pub(crate) async fn with_price_to_beat_iv_depth_order_book_pass_cache<F>(future: F) -> F::Output
where
    F: Future,
{
    IV_DEPTH_ORDER_BOOK_PASS_STATE
        .scope(RefCell::new(IvDepthOrderBookPassState::default()), future)
        .await
}

#[derive(Debug, Clone, Default)]
pub(crate) struct IvDepthOrderBookPassStats {
    pub(crate) cache_hits: u64,
    pub(crate) pass_cache_hits: u64,
    pub(crate) process_ttl_cache_hits: u64,
    pub(crate) cache_misses: u64,
    pub(crate) fetch_errors: u64,
    pub(crate) unique_tokens_fetched: usize,
}

#[derive(Default)]
struct IvDepthOrderBookPassState {
    cache: HashMap<String, OrderBookFetchDiagnostics>,
    stats: IvDepthOrderBookPassStats,
    fetched_token_keys: HashSet<String>,
}

struct IvDepthOrderBookTtlEntry {
    diagnostics: OrderBookFetchDiagnostics,
    expires_at: Instant,
}

pub(crate) fn price_to_beat_iv_depth_order_book_pass_stats() -> IvDepthOrderBookPassStats {
    IV_DEPTH_ORDER_BOOK_PASS_STATE
        .try_with(|state| {
            let state = state.borrow();
            let mut stats = state.stats.clone();
            stats.cache_hits = stats
                .pass_cache_hits
                .saturating_add(stats.process_ttl_cache_hits);
            stats.unique_tokens_fetched = state.fetched_token_keys.len();
            stats
        })
        .unwrap_or_default()
}

pub(crate) fn price_to_beat_iv_mismatch_needs_book_hydration(
    config: &PriceToBeatIvMismatchEdgeConfig,
) -> bool {
    config.depth_guard_enabled
        || config.execution_vwap_guard.enabled
        || (config.protection_mode.is_active() && config.book_lead_guard_enabled)
}

pub(crate) fn price_to_beat_iv_early_block_can_skip_book(reason_code: &str) -> bool {
    matches!(
        reason_code,
        "open_gap_opposite_venue_detected"
            | "open_gap_no_clean_pair"
            | "open_gap_chainlink_sanity_fail"
            | "chainlink_provider_stale_global"
            | "chainlink_provider_stale_entry_quality"
            | "blocked_no_matching_time_rule"
            | "blocked_time_rule_max_price"
            | "blocked_price_above_effective_max"
    )
}

pub(crate) fn annotate_price_to_beat_iv_book_not_requested_for_early_block(
    evaluation: &mut super::PriceToBeatGuardEvaluation,
) {
    annotate_iv_value_book_fetch_status(
        evaluation.iv_mismatch_edge.as_mut(),
        ORDER_BOOK_NOT_REQUESTED_EARLY_BLOCK,
    );
    annotate_entry_current_source_debug_book_fetch_status(
        evaluation.entry_current_source_debug.as_mut(),
        ORDER_BOOK_NOT_REQUESTED_EARLY_BLOCK,
    );
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvDepthRuntimeDiagnostics {
    pub(crate) market_slug: Option<String>,
    pub(crate) action_token_id: Option<String>,
    pub(crate) selected_outcome_label: Option<String>,
    pub(crate) selected_token_id: Option<String>,
    pub(crate) selected_token_source: &'static str,
    pub(crate) token_matches_action_token: Option<bool>,
    pub(crate) selected_pair_side: Option<&'static str>,
    pub(crate) opposite_outcome_label: Option<String>,
    pub(crate) opposite_token_id: Option<String>,
    pub(crate) opposite_token_source: &'static str,
    pub(crate) size_mode: Option<String>,
    pub(crate) selected_entry_size_usdc: Option<f64>,
    pub(crate) size_usdc: Option<f64>,
    pub(crate) target_notional_usdc: Option<f64>,
    pub(crate) target_qty: Option<f64>,
    pub(crate) sizing_source: &'static str,
    pub(crate) sizing_best_ask: Option<f64>,
    pub(crate) computed_intended_qty: Option<f64>,
    pub(crate) sizing_missing_reason: Option<&'static str>,
    pub(crate) order_book_fetch_status: Option<&'static str>,
    pub(crate) order_book_fetch_error_kind: Option<String>,
    pub(crate) order_book_fetch_error_code: Option<u16>,
    pub(crate) order_book_fetch_error_reason: Option<String>,
    pub(crate) order_book_cache_hit: Option<bool>,
    pub(crate) order_book_cache_scope: Option<&'static str>,
    pub(crate) book_confirmation_missing_reason: Option<&'static str>,
}

impl Default for PriceToBeatIvDepthRuntimeDiagnostics {
    fn default() -> Self {
        Self {
            market_slug: None,
            action_token_id: None,
            selected_outcome_label: None,
            selected_token_id: None,
            selected_token_source: TOKEN_SOURCE_MISSING,
            token_matches_action_token: None,
            selected_pair_side: None,
            opposite_outcome_label: None,
            opposite_token_id: None,
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_MISSING,
            size_mode: None,
            selected_entry_size_usdc: None,
            size_usdc: None,
            target_notional_usdc: None,
            target_qty: None,
            sizing_source: SOURCE_NONE,
            sizing_best_ask: None,
            computed_intended_qty: None,
            sizing_missing_reason: None,
            order_book_fetch_status: None,
            order_book_fetch_error_kind: None,
            order_book_fetch_error_code: None,
            order_book_fetch_error_reason: None,
            order_book_cache_hit: None,
            order_book_cache_scope: None,
            book_confirmation_missing_reason: None,
        }
    }
}

impl PriceToBeatIvDepthRuntimeDiagnostics {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("depth_market_slug".to_string(), json!(self.market_slug));
        obj.insert(
            "depth_action_token_id".to_string(),
            json!(self.action_token_id),
        );
        obj.insert(
            "depth_selected_outcome_label".to_string(),
            json!(self.selected_outcome_label),
        );
        obj.insert(
            "depth_selected_token_id".to_string(),
            json!(self.selected_token_id),
        );
        obj.insert(
            "depth_selected_token_source".to_string(),
            json!(self.selected_token_source),
        );
        obj.insert(
            "depth_token_matches_action_token".to_string(),
            json!(self.token_matches_action_token),
        );
        obj.insert(
            "depth_selected_pair_side".to_string(),
            json!(self.selected_pair_side),
        );
        obj.insert(
            "depth_opposite_outcome_label".to_string(),
            json!(self.opposite_outcome_label),
        );
        obj.insert(
            "depth_opposite_token_id".to_string(),
            json!(self.opposite_token_id),
        );
        obj.insert(
            "depth_opposite_token_source".to_string(),
            json!(self.opposite_token_source),
        );
        obj.insert("depth_size_mode".to_string(), json!(self.size_mode));
        obj.insert(
            "depth_selected_entry_size_usdc".to_string(),
            json!(self.selected_entry_size_usdc),
        );
        obj.insert("depth_size_usdc".to_string(), json!(self.size_usdc));
        obj.insert(
            "depth_target_notional_usdc".to_string(),
            json!(self.target_notional_usdc),
        );
        obj.insert("depth_target_qty".to_string(), json!(self.target_qty));
        obj.insert("depth_sizing_source".to_string(), json!(self.sizing_source));
        obj.insert(
            "depth_sizing_best_ask".to_string(),
            json!(self.sizing_best_ask),
        );
        obj.insert(
            "depth_computed_intended_qty".to_string(),
            json!(self.computed_intended_qty),
        );
        obj.insert(
            "depth_sizing_missing_reason".to_string(),
            json!(self.sizing_missing_reason),
        );
        obj.insert(
            "depth_order_book_fetch_status".to_string(),
            json!(self.order_book_fetch_status),
        );
        obj.insert(
            "depth_order_book_fetch_error_kind".to_string(),
            json!(self.order_book_fetch_error_kind),
        );
        obj.insert(
            "depth_order_book_fetch_error_code".to_string(),
            json!(self.order_book_fetch_error_code),
        );
        obj.insert(
            "depth_order_book_fetch_error_reason".to_string(),
            json!(self.order_book_fetch_error_reason),
        );
        obj.insert(
            "depth_order_book_cache_hit".to_string(),
            json!(self.order_book_cache_hit),
        );
        obj.insert(
            "depth_order_book_cache_scope".to_string(),
            json!(self.order_book_cache_scope),
        );
        obj.insert(
            "book_confirmation_missing_reason".to_string(),
            json!(self.book_confirmation_missing_reason),
        );
    }
}

fn annotate_iv_value_book_fetch_status(value: Option<&mut Value>, status: &'static str) {
    let Some(obj) = value.and_then(Value::as_object_mut) else {
        return;
    };
    obj.insert("depth_order_book_fetch_status".to_string(), json!(status));
    obj.insert("depth_order_book_cache_hit".to_string(), json!(false));
    obj.insert(
        "depth_order_book_cache_scope".to_string(),
        json!("not_requested"),
    );
}

fn annotate_entry_current_source_debug_book_fetch_status(
    value: Option<&mut Value>,
    status: &'static str,
) {
    let Some(evaluations) = value
        .and_then(Value::as_object_mut)
        .and_then(|obj| obj.get_mut("entry_current_source_evaluations"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for candidate in evaluations {
        annotate_iv_value_book_fetch_status(candidate.get_mut("iv_mismatch_edge"), status);
    }
}

struct IntendedQtyDiagnostics {
    size_mode: String,
    selected_entry_size_usdc: Option<f64>,
    size_usdc: Option<f64>,
    target_notional_usdc: Option<f64>,
    target_qty: Option<f64>,
    sizing_source: &'static str,
    best_ask: Option<f64>,
    computed_intended_qty: Option<f64>,
    missing_reason: Option<&'static str>,
}

#[derive(Clone)]
struct OrderBookFetchDiagnostics {
    snapshot: Option<OrderBookSnapshot>,
    status: &'static str,
    error_kind: Option<String>,
    error_code: Option<u16>,
    error_reason: Option<String>,
    cache_hit: bool,
    cache_scope: Option<&'static str>,
}

struct OppositeTokenResolution {
    opposite_token_id: Option<String>,
    selected_pair_side: Option<&'static str>,
    opposite_token_source: &'static str,
}

pub(crate) async fn hydrate_action_place_order_iv_mismatch_book_quotes(
    runtime: Option<&PriceToBeatGuardRuntimeContext<'_>>,
    context: &Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
    action_token_id: Option<&str>,
    action_yes_token_id: Option<&str>,
    action_no_token_id: Option<&str>,
    outcome_label: &str,
    signal_market: Option<PriceToBeatSignalFormulaMarketInput>,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    let needs_depth = config.depth_guard_enabled || config.execution_vwap_guard.enabled;
    let needs_book_confirmation =
        config.protection_mode.is_active() && config.book_lead_guard_enabled;
    if !needs_depth && !needs_book_confirmation {
        return;
    }

    let mut diagnostics = PriceToBeatIvDepthRuntimeDiagnostics {
        market_slug: Some(market_slug.to_string()),
        selected_outcome_label: Some(outcome_label.to_string()),
        ..Default::default()
    };
    let action_token_id = normalize_optional_token_id(action_token_id);
    diagnostics.action_token_id = action_token_id.clone();
    diagnostics.selected_token_id = action_token_id.clone();
    diagnostics.selected_token_source = if action_token_id.is_some() {
        TOKEN_SOURCE_ACTION_IDENTITY
    } else {
        TOKEN_SOURCE_MISSING
    };
    diagnostics.token_matches_action_token = action_token_id
        .as_ref()
        .map(|action_token_id| diagnostics.selected_token_id.as_ref() == Some(action_token_id));
    let Some((_, selected_direction)) = normalize_outcome_direction(outcome_label) else {
        diagnostics.book_confirmation_missing_reason = Some("outcome_direction_unavailable");
        config.depth_runtime_diagnostics = diagnostics;
        return;
    };
    let opposite_outcome = if selected_direction == "up" {
        "Down"
    } else {
        "Up"
    };
    diagnostics.opposite_outcome_label = Some(opposite_outcome.to_string());
    let pair_resolution = resolve_opposite_token_from_step_pair(
        action_token_id.as_deref(),
        action_yes_token_id,
        action_no_token_id,
    );
    let opposite_token_id = pair_resolution.opposite_token_id;
    diagnostics.selected_pair_side = pair_resolution.selected_pair_side;
    diagnostics.opposite_token_source = pair_resolution.opposite_token_source;
    diagnostics.opposite_token_id = opposite_token_id.clone();

    let protection_active = config.protection_mode.is_active();
    let (selected_order_book, prefetched_opposite_quote) = if protection_active {
        let selected_book = fetch_iv_order_book(runtime, action_token_id.as_deref());
        let opposite_quote = fetch_iv_book_quote(runtime, opposite_token_id.as_deref());
        let (selected_book, opposite_quote) = tokio::join!(selected_book, opposite_quote);
        (selected_book, Some(opposite_quote))
    } else {
        (
            fetch_iv_order_book(runtime, action_token_id.as_deref()).await,
            None,
        )
    };
    diagnostics.order_book_fetch_status = Some(selected_order_book.status);
    diagnostics.order_book_fetch_error_kind = selected_order_book.error_kind.clone();
    diagnostics.order_book_fetch_error_code = selected_order_book.error_code;
    diagnostics.order_book_fetch_error_reason = selected_order_book.error_reason.clone();
    diagnostics.order_book_cache_hit = Some(selected_order_book.cache_hit);
    diagnostics.order_book_cache_scope = selected_order_book.cache_scope;
    if needs_depth {
        let sizing = resolve_action_place_order_iv_mismatch_intended_qty(
            node,
            context,
            config.market.best_ask,
        );
        diagnostics.size_mode = Some(sizing.size_mode);
        diagnostics.selected_entry_size_usdc = sizing.selected_entry_size_usdc;
        diagnostics.size_usdc = sizing.size_usdc;
        diagnostics.target_notional_usdc = sizing.target_notional_usdc;
        diagnostics.target_qty = sizing.target_qty;
        diagnostics.sizing_source = sizing.sizing_source;
        diagnostics.sizing_best_ask = sizing.best_ask;
        diagnostics.computed_intended_qty = sizing.computed_intended_qty;
        diagnostics.sizing_missing_reason = sizing.missing_reason;
        config.depth_intended_qty = sizing.computed_intended_qty;
        config.depth_order_book = selected_order_book.snapshot.clone();
    }
    if !protection_active {
        diagnostics.book_confirmation_missing_reason = Some("not_requested_protection_off");
        config.depth_runtime_diagnostics = diagnostics;
        return;
    }

    let selected_quote = signal_market
        .and_then(|market| normalize_iv_book_quote(market.best_bid, market.best_ask))
        .or_else(|| {
            selected_order_book
                .snapshot
                .as_ref()
                .and_then(iv_order_book_best_quote)
        })
        .or_else(|| config.market.best_bid.zip(config.market.best_ask))
        .and_then(|(bid, ask)| normalize_iv_book_quote(Some(bid), Some(ask)));
    let selected_quote = match selected_quote {
        Some(quote) => Some(quote),
        None => fetch_iv_book_quote(runtime, action_token_id.as_deref()).await,
    };
    let opposite_quote = prefetched_opposite_quote.unwrap_or(None);
    diagnostics.book_confirmation_missing_reason =
        book_confirmation_missing_reason(selected_quote, opposite_quote);
    config.book_quotes = match (selected_direction, selected_quote, opposite_quote) {
        ("up", Some((up_bid, up_ask)), Some((down_bid, down_ask))) => {
            Some(PriceToBeatIvBookQuotes {
                up_bid: Some(up_bid),
                up_ask: Some(up_ask),
                down_bid: Some(down_bid),
                down_ask: Some(down_ask),
            })
        }
        ("down", Some((down_bid, down_ask)), Some((up_bid, up_ask))) => {
            Some(PriceToBeatIvBookQuotes {
                up_bid: Some(up_bid),
                up_ask: Some(up_ask),
                down_bid: Some(down_bid),
                down_ask: Some(down_ask),
            })
        }
        _ => None,
    };
    config.depth_runtime_diagnostics = diagnostics;
}

async fn fetch_iv_order_book(
    runtime: Option<&PriceToBeatGuardRuntimeContext<'_>>,
    token_id: Option<&str>,
) -> OrderBookFetchDiagnostics {
    let normalized_token_id = normalize_optional_token_id(token_id);
    let cache_key = runtime
        .zip(normalized_token_id.as_deref())
        .map(|(runtime, token_id)| {
            format!(
                "{}|{}",
                runtime.cfg.exchange.clob_base_url.trim_end_matches('/'),
                token_id
            )
        });
    if let Some(cache_key) = cache_key.as_deref() {
        if let Some(mut cached) = cached_iv_order_book_fetch(cache_key) {
            record_iv_order_book_cache_hit(ORDER_BOOK_CACHE_SCOPE_HOUSEKEEPING_PASS);
            cached.cache_hit = true;
            cached.cache_scope = Some(ORDER_BOOK_CACHE_SCOPE_HOUSEKEEPING_PASS);
            return cached;
        }
        if let Some(mut cached) = cached_iv_order_book_fetch_process_ttl(cache_key) {
            record_iv_order_book_cache_hit(ORDER_BOOK_CACHE_SCOPE_PROCESS_TTL);
            cached.cache_hit = true;
            cached.cache_scope = Some(ORDER_BOOK_CACHE_SCOPE_PROCESS_TTL);
            return cached;
        }
    }

    let client = runtime.and_then(|runtime| runtime.client);
    if client.is_some() && normalized_token_id.is_some() {
        record_iv_order_book_fetch_attempt(cache_key.as_deref());
    }
    let mut fetched = fetch_order_book_with_client(client, normalized_token_id.as_deref()).await;
    record_iv_order_book_fetch_error(fetched.status);
    if let Some(cache_key) = cache_key.as_deref() {
        if iv_order_book_pass_cache_available() {
            fetched.cache_scope = Some(ORDER_BOOK_CACHE_SCOPE_HOUSEKEEPING_PASS);
            remember_iv_order_book_fetch(cache_key, fetched.clone());
        }
        remember_iv_order_book_fetch_process_ttl(cache_key, fetched.clone());
    }
    fetched
}

fn cached_iv_order_book_fetch(cache_key: &str) -> Option<OrderBookFetchDiagnostics> {
    IV_DEPTH_ORDER_BOOK_PASS_STATE
        .try_with(|state| state.borrow().cache.get(cache_key).cloned())
        .ok()
        .flatten()
}

fn remember_iv_order_book_fetch(cache_key: &str, diagnostics: OrderBookFetchDiagnostics) {
    let _ = IV_DEPTH_ORDER_BOOK_PASS_STATE.try_with(|state| {
        state
            .borrow_mut()
            .cache
            .insert(cache_key.to_string(), diagnostics);
    });
}

fn cached_iv_order_book_fetch_process_ttl(cache_key: &str) -> Option<OrderBookFetchDiagnostics> {
    let now = Instant::now();
    let mut cache = IV_DEPTH_ORDER_BOOK_TTL_CACHE.lock().ok()?;
    let entry = cache.get(cache_key)?;
    if entry.expires_at <= now {
        cache.remove(cache_key);
        return None;
    }
    Some(entry.diagnostics.clone())
}

fn remember_iv_order_book_fetch_process_ttl(
    cache_key: &str,
    diagnostics: OrderBookFetchDiagnostics,
) {
    let ttl = if diagnostics.status == "fetch_failed" {
        ORDER_BOOK_ERROR_TTL
    } else {
        ORDER_BOOK_SUCCESS_TTL
    };
    let expires_at = Instant::now() + ttl;
    let Ok(mut cache) = IV_DEPTH_ORDER_BOOK_TTL_CACHE.lock() else {
        return;
    };
    if cache.len() >= ORDER_BOOK_TTL_CACHE_MAX_ENTRIES {
        let now = Instant::now();
        cache.retain(|_, entry| entry.expires_at > now);
        if cache.len() >= ORDER_BOOK_TTL_CACHE_MAX_ENTRIES {
            cache.clear();
        }
    }
    cache.insert(
        cache_key.to_string(),
        IvDepthOrderBookTtlEntry {
            diagnostics,
            expires_at,
        },
    );
}

fn iv_order_book_pass_cache_available() -> bool {
    IV_DEPTH_ORDER_BOOK_PASS_STATE.try_with(|_| ()).is_ok()
}

fn record_iv_order_book_cache_hit(cache_scope: &'static str) {
    let _ = IV_DEPTH_ORDER_BOOK_PASS_STATE.try_with(|state| {
        let mut state = state.borrow_mut();
        match cache_scope {
            ORDER_BOOK_CACHE_SCOPE_PROCESS_TTL => state.stats.process_ttl_cache_hits += 1,
            _ => state.stats.pass_cache_hits += 1,
        }
    });
}

fn record_iv_order_book_fetch_attempt(cache_key: Option<&str>) {
    let _ = IV_DEPTH_ORDER_BOOK_PASS_STATE.try_with(|state| {
        let mut state = state.borrow_mut();
        state.stats.cache_misses += 1;
        if let Some(cache_key) = cache_key {
            state.fetched_token_keys.insert(cache_key.to_string());
        }
    });
}

fn record_iv_order_book_fetch_error(status: &str) {
    if status != "fetch_failed" {
        return;
    }
    let _ = IV_DEPTH_ORDER_BOOK_PASS_STATE.try_with(|state| {
        state.borrow_mut().stats.fetch_errors += 1;
    });
}

async fn fetch_order_book_with_client(
    client: Option<&dyn crate::OrderExecutor>,
    token_id: Option<&str>,
) -> OrderBookFetchDiagnostics {
    let Some(token_id) = token_id
        .map(str::trim)
        .filter(|token_id| !token_id.is_empty())
    else {
        return OrderBookFetchDiagnostics {
            snapshot: None,
            status: "selected_token_id_missing",
            error_kind: None,
            error_code: None,
            error_reason: None,
            cache_hit: false,
            cache_scope: None,
        };
    };
    let Some(client) = client else {
        return OrderBookFetchDiagnostics {
            snapshot: None,
            status: "runtime_client_missing",
            error_kind: None,
            error_code: None,
            error_reason: None,
            cache_hit: false,
            cache_scope: None,
        };
    };
    match tokio::time::timeout(
        ORDER_BOOK_GUARD_FETCH_TIMEOUT,
        client.order_book_with_diagnostics(token_id),
    )
    .await
    {
        Ok(Ok(result)) => match result.snapshot {
            Some(snapshot) => {
                let status = classify_order_book_snapshot(&snapshot);
                OrderBookFetchDiagnostics {
                    snapshot: Some(snapshot),
                    status,
                    error_kind: None,
                    error_code: None,
                    error_reason: None,
                    cache_hit: false,
                    cache_scope: None,
                }
            }
            None => OrderBookFetchDiagnostics {
                snapshot: None,
                status: "fetch_failed",
                error_kind: sanitize_fetch_error_kind(result.error_kind)
                    .or_else(|| Some("not_found_or_non_success".to_string())),
                error_code: result.error_code,
                error_reason: sanitize_fetch_error_reason(
                    result
                        .error_reason
                        .as_deref()
                        .unwrap_or("order_book_unavailable"),
                ),
                cache_hit: false,
                cache_scope: None,
            },
        },
        Ok(Err(err)) => {
            let error = err.to_string();
            let lower = error.to_ascii_lowercase();
            let error_kind = if lower.contains("decode") || lower.contains("json") {
                "parse_error"
            } else if lower.contains("timeout") {
                "timeout"
            } else {
                "client_error"
            };
            OrderBookFetchDiagnostics {
                snapshot: None,
                status: "fetch_failed",
                error_kind: Some(error_kind.to_string()),
                error_code: None,
                error_reason: sanitize_fetch_error_reason(&error),
                cache_hit: false,
                cache_scope: None,
            }
        }
        Err(_) => OrderBookFetchDiagnostics {
            snapshot: None,
            status: "fetch_failed",
            error_kind: Some("timeout".to_string()),
            error_code: None,
            error_reason: Some(format!(
                "order_book_guard_fetch_timeout_{}ms",
                ORDER_BOOK_GUARD_FETCH_TIMEOUT.as_millis()
            )),
            cache_hit: false,
            cache_scope: None,
        },
    }
}

fn normalize_optional_token_id(token_id: Option<&str>) -> Option<String> {
    token_id
        .map(str::trim)
        .filter(|token_id| !token_id.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_opposite_token_from_step_pair(
    action_token_id: Option<&str>,
    yes_token_id: Option<&str>,
    no_token_id: Option<&str>,
) -> OppositeTokenResolution {
    let action_token_id = normalize_optional_token_id(action_token_id);
    let yes_token_id = normalize_optional_token_id(yes_token_id);
    let no_token_id = normalize_optional_token_id(no_token_id);
    let Some(action_token_id) = action_token_id.as_deref() else {
        return OppositeTokenResolution {
            opposite_token_id: None,
            selected_pair_side: None,
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_MISSING,
        };
    };

    match (yes_token_id.as_deref(), no_token_id.as_deref()) {
        (Some(yes), Some(no)) if action_token_id == yes => OppositeTokenResolution {
            opposite_token_id: Some(no.to_string()),
            selected_pair_side: Some("yes"),
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_PAIRED_STEP_TOKENS,
        },
        (Some(yes), Some(no)) if action_token_id == no => OppositeTokenResolution {
            opposite_token_id: Some(yes.to_string()),
            selected_pair_side: Some("no"),
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_PAIRED_STEP_TOKENS,
        },
        (Some(yes), None) if action_token_id == yes => OppositeTokenResolution {
            opposite_token_id: None,
            selected_pair_side: Some("yes"),
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_MISSING,
        },
        (None, Some(no)) if action_token_id == no => OppositeTokenResolution {
            opposite_token_id: None,
            selected_pair_side: Some("no"),
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_MISSING,
        },
        (Some(_), Some(_)) => OppositeTokenResolution {
            opposite_token_id: None,
            selected_pair_side: None,
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_UNTRUSTED_PAIR,
        },
        _ => OppositeTokenResolution {
            opposite_token_id: None,
            selected_pair_side: None,
            opposite_token_source: OPPOSITE_TOKEN_SOURCE_MISSING,
        },
    }
}

fn sanitize_fetch_error_kind(error_kind: Option<String>) -> Option<String> {
    let sanitized: String = error_kind?
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .take(64)
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn sanitize_fetch_error_reason(reason: &str) -> Option<String> {
    let sanitized: String = reason
        .trim()
        .chars()
        .filter_map(|ch| {
            if ch == '\n' || ch == '\r' || ch == '\t' {
                Some(' ')
            } else if ch.is_control() {
                None
            } else {
                Some(ch)
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

async fn fetch_iv_book_quote(
    runtime: Option<&PriceToBeatGuardRuntimeContext<'_>>,
    token_id: Option<&str>,
) -> Option<(f64, f64)> {
    let client = runtime.and_then(|runtime| runtime.client)?;
    let token_id = token_id?.trim();
    if token_id.is_empty() {
        return None;
    }
    let (bid, ask) = tokio::time::timeout(
        ORDER_BOOK_GUARD_FETCH_TIMEOUT,
        client.best_bid_ask(token_id),
    )
    .await
    .ok()?
    .ok()?;
    normalize_iv_book_quote(bid, ask)
}

fn iv_order_book_best_quote(snapshot: &OrderBookSnapshot) -> Option<(f64, f64)> {
    let best_bid = snapshot
        .bids
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.price < 1.0)
        .map(|level| level.price)
        .max_by(f64::total_cmp);
    let best_ask = snapshot
        .asks
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.price < 1.0)
        .map(|level| level.price)
        .min_by(f64::total_cmp);
    normalize_iv_book_quote(best_bid, best_ask)
}

fn resolve_action_place_order_iv_mismatch_intended_qty(
    node: &crate::TradeFlowNode,
    context: &Value,
    best_ask: Option<f64>,
) -> IntendedQtyDiagnostics {
    let raw_size_mode = crate::node_config_string(node, "sizeMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "usdc".to_string());
    let selected_entry_size_usdc = crate::flow_context_f64(context, "selectedEntrySizeUsdc")
        .filter(|value| value.is_finite() && *value > 0.0);
    let size_usdc =
        crate::node_config_f64(node, "sizeUsdc").filter(|value| value.is_finite() && *value > 0.0);
    let target_notional_usdc = crate::node_config_f64(node, "targetNotionalUsdc")
        .filter(|value| value.is_finite() && *value > 0.0);
    let target_qty = crate::node_config_f64(node, "targetQty")
        .or_else(|| crate::node_config_f64(node, "target_qty"))
        .filter(|value| value.is_finite() && *value > 0.0);
    let best_ask = best_ask.filter(|value| value.is_finite() && *value > 0.0);

    if raw_size_mode == "shares" {
        return IntendedQtyDiagnostics {
            size_mode: raw_size_mode,
            selected_entry_size_usdc,
            size_usdc,
            target_notional_usdc,
            target_qty,
            sizing_source: target_qty.map(|_| SOURCE_TARGET_QTY).unwrap_or(SOURCE_NONE),
            best_ask,
            computed_intended_qty: target_qty,
            missing_reason: target_qty.is_none().then_some("target_qty_unavailable"),
        };
    }

    let size_input = selected_entry_size_usdc
        .map(|value| (SOURCE_SELECTED_ENTRY_SIZE_USDC, value))
        .or_else(|| size_usdc.map(|value| (SOURCE_SIZE_USDC, value)))
        .or_else(|| target_notional_usdc.map(|value| (SOURCE_TARGET_NOTIONAL_USDC, value)));
    let unsupported_size_mode = raw_size_mode != "usdc" && !raw_size_mode.is_empty();
    let computed_intended_qty = size_input
        .zip(best_ask)
        .map(|((_, size_usdc), best_ask)| size_usdc / best_ask);
    let missing_reason = if computed_intended_qty.is_some() {
        None
    } else if unsupported_size_mode {
        Some("unsupported_size_mode")
    } else if size_input.is_none() {
        Some("size_usdc_unavailable")
    } else {
        Some("best_ask_unavailable")
    };

    IntendedQtyDiagnostics {
        size_mode: raw_size_mode,
        selected_entry_size_usdc,
        size_usdc,
        target_notional_usdc,
        target_qty,
        sizing_source: size_input.map(|(source, _)| source).unwrap_or(SOURCE_NONE),
        best_ask,
        computed_intended_qty,
        missing_reason,
    }
}

fn normalize_iv_book_quote(bid: Option<f64>, ask: Option<f64>) -> Option<(f64, f64)> {
    let bid = bid.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)?;
    let ask = ask.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)?;
    (ask >= bid).then_some((bid, ask))
}

fn classify_order_book_snapshot(snapshot: &OrderBookSnapshot) -> &'static str {
    if snapshot.asks.is_empty() && snapshot.bids.is_empty() {
        "empty_book"
    } else {
        "ok"
    }
}

fn book_confirmation_missing_reason(
    selected_quote: Option<(f64, f64)>,
    opposite_quote: Option<(f64, f64)>,
) -> Option<&'static str> {
    match (selected_quote.is_some(), opposite_quote.is_some()) {
        (true, true) => None,
        (false, false) => Some("selected_and_opposite_quote_missing"),
        (false, true) => Some("selected_quote_missing"),
        (true, false) => Some("opposite_quote_missing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bot_infra::exchange::OrderBookLevel;

    fn node(config: Value) -> crate::TradeFlowNode {
        crate::TradeFlowNode {
            key: "node_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn sizing_diagnostics_reports_usdc_precedence_source() {
        let node = node(json!({
            "sizeMode": "usdc",
            "sizeUsdc": 25,
            "targetNotionalUsdc": 50
        }));
        let diagnostics = resolve_action_place_order_iv_mismatch_intended_qty(
            &node,
            &json!({ "flowContext": { "selectedEntrySizeUsdc": 10 } }),
            Some(0.5),
        );

        assert_eq!(diagnostics.sizing_source, SOURCE_SELECTED_ENTRY_SIZE_USDC);
        assert_eq!(diagnostics.computed_intended_qty, Some(20.0));
        assert_eq!(diagnostics.missing_reason, None);
    }

    #[test]
    fn sizing_diagnostics_reports_missing_share_qty() {
        let diagnostics = resolve_action_place_order_iv_mismatch_intended_qty(
            &node(json!({ "sizeMode": "shares" })),
            &json!({}),
            Some(0.5),
        );

        assert_eq!(diagnostics.sizing_source, SOURCE_NONE);
        assert_eq!(diagnostics.computed_intended_qty, None);
        assert_eq!(diagnostics.missing_reason, Some("target_qty_unavailable"));
    }

    #[test]
    fn sizing_diagnostics_reports_unsupported_mode_when_no_qty_can_be_computed() {
        let diagnostics = resolve_action_place_order_iv_mismatch_intended_qty(
            &node(json!({ "sizeMode": "usd" })),
            &json!({}),
            Some(0.5),
        );

        assert_eq!(diagnostics.computed_intended_qty, None);
        assert_eq!(diagnostics.missing_reason, Some("unsupported_size_mode"));
    }

    #[test]
    fn order_book_status_separates_empty_snapshot_from_ok_snapshot() {
        let empty = OrderBookSnapshot {
            bids: Vec::new(),
            asks: Vec::new(),
        };
        assert_eq!(classify_order_book_snapshot(&empty), "empty_book");

        let with_bid = OrderBookSnapshot {
            bids: vec![OrderBookLevel {
                price: 0.45,
                size: 5.0,
            }],
            asks: Vec::new(),
        };
        assert_eq!(classify_order_book_snapshot(&with_bid), "ok");
    }

    #[test]
    fn book_confirmation_reason_separates_selected_and_opposite_missing() {
        assert_eq!(
            book_confirmation_missing_reason(None, None),
            Some("selected_and_opposite_quote_missing")
        );
        assert_eq!(
            book_confirmation_missing_reason(None, Some((0.4, 0.5))),
            Some("selected_quote_missing")
        );
        assert_eq!(
            book_confirmation_missing_reason(Some((0.4, 0.5)), None),
            Some("opposite_quote_missing")
        );
        assert_eq!(
            book_confirmation_missing_reason(Some((0.4, 0.5)), Some((0.4, 0.5))),
            None
        );
    }

    #[test]
    fn early_open_gap_and_chainlink_blocks_can_skip_book_recheck() {
        assert!(price_to_beat_iv_early_block_can_skip_book(
            "open_gap_opposite_venue_detected"
        ));
        assert!(price_to_beat_iv_early_block_can_skip_book(
            "open_gap_no_clean_pair"
        ));
        assert!(price_to_beat_iv_early_block_can_skip_book(
            "chainlink_provider_stale_global"
        ));
        assert!(!price_to_beat_iv_early_block_can_skip_book(
            "blocked_execution_vwap_above_max_price"
        ));
        assert!(!price_to_beat_iv_early_block_can_skip_book(
            "blocked_depth_qty_insufficient"
        ));
        assert!(!price_to_beat_iv_early_block_can_skip_book(
            "blocked_depth_slippage_too_high"
        ));
    }

    #[test]
    fn annotate_book_not_requested_marks_top_level_and_candidate_iv_debug() {
        let mut evaluation = super::super::PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "open_gap_opposite_venue_detected".to_string(),
            reason_detail: None,
            normalized_outcome_label: None,
            direction: None,
            market_slug: "btc-updown-5m-1".to_string(),
            event_url: String::new(),
            timeframe: None,
            asset: None,
            price_to_beat: None,
            price_to_beat_status: None,
            price_to_beat_source: None,
            price_to_beat_source_latency_ms: None,
            current_price: None,
            current_price_source: "chainlink",
            directional_gap: None,
            gap_abs: None,
            threshold_mode: "iv_mismatch_edge".to_string(),
            configured_threshold_mode: None,
            base_threshold_value: None,
            base_threshold_unit: None,
            base_threshold_usd: None,
            current_effective_ptb_usd: None,
            threshold_value: 5.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 5.0,
            stop_loss_bump_count: 0,
            stop_loss_bump_applied_count: 0,
            stop_loss_bump_amount: None,
            stop_loss_bump_max_value: None,
            stop_loss_bump_unit: None,
            stop_loss_bump_raw_usd: 0.0,
            stop_loss_bump_usd: 0.0,
            stop_loss_bump_capped: false,
            stop_loss_bump_max_reached: false,
            stop_loss_bump_current_market_excluded: false,
            stop_loss_bump_increment_usd: 0.0,
            reentry_generation: 0,
            reentry_override_active: false,
            reentry_override_value: None,
            reentry_override_unit: None,
            max_price_relax: None,
            auto_threshold_usd: None,
            lookback_windows_used: None,
            current_windows_used: None,
            avg_up_excursion_usd: None,
            avg_down_excursion_usd: None,
            lookback_market_slugs: None,
            lookback_window_snapshots: None,
            baseline_pct: None,
            current_pct: None,
            vol_factor: None,
            threshold_pct: None,
            base_pct: None,
            floor_usd: None,
            ceiling_usd: None,
            threshold_was_clamped: None,
            signal_formula: None,
            iv_mismatch_edge: Some(json!({ "reason": "open_gap_opposite_venue_detected" })),
            early_stale_side: None,
            cex_direction_guard: None,
            entry_current_source_debug: Some(json!({
                "entry_current_source_evaluations": [
                    { "iv_mismatch_edge": { "reason": "open_gap_opposite_venue_detected" } }
                ]
            })),
        };

        annotate_price_to_beat_iv_book_not_requested_for_early_block(&mut evaluation);

        assert_eq!(
            evaluation
                .iv_mismatch_edge
                .as_ref()
                .and_then(|value| value.get("depth_order_book_fetch_status"))
                .and_then(Value::as_str),
            Some(ORDER_BOOK_NOT_REQUESTED_EARLY_BLOCK)
        );
        assert_eq!(
            evaluation
                .entry_current_source_debug
                .as_ref()
                .and_then(|value| value.pointer(
                    "/entry_current_source_evaluations/0/iv_mismatch_edge/depth_order_book_fetch_status"
                ))
                .and_then(Value::as_str),
            Some(ORDER_BOOK_NOT_REQUESTED_EARLY_BLOCK)
        );
    }

    #[test]
    fn process_ttl_cache_returns_recent_order_book_fetch() {
        IV_DEPTH_ORDER_BOOK_TTL_CACHE
            .lock()
            .expect("ttl cache")
            .clear();
        remember_iv_order_book_fetch_process_ttl(
            "clob|TOKEN",
            OrderBookFetchDiagnostics {
                snapshot: Some(OrderBookSnapshot {
                    bids: Vec::new(),
                    asks: Vec::new(),
                }),
                status: "empty_book",
                error_kind: None,
                error_code: None,
                error_reason: None,
                cache_hit: false,
                cache_scope: None,
            },
        );

        let cached = cached_iv_order_book_fetch_process_ttl("clob|TOKEN")
            .expect("cached order book diagnostics");
        assert_eq!(cached.status, "empty_book");
        assert!(cached.snapshot.is_some());
    }

    #[tokio::test]
    async fn selected_depth_uses_action_token_and_not_global_resolver() {
        let mut config =
            PriceToBeatIvMismatchEdgeConfig::crypto_defaults(PriceToBeatSignalFormulaMarketInput {
                best_bid: Some(0.49),
                best_ask: Some(0.51),
            });
        hydrate_action_place_order_iv_mismatch_book_quotes(
            None,
            &json!({
                "flowContext": {
                    "yesTokenId": "GLOBAL_YES",
                    "noTokenId": "GLOBAL_NO"
                }
            }),
            &node(json!({ "sizeMode": "shares", "targetQty": 10 })),
            "btc-updown-5m-1",
            Some("ACTION_YES"),
            Some("ACTION_YES"),
            Some("ACTION_NO"),
            "Up",
            None,
            &mut config,
        )
        .await;

        let diagnostics = config.depth_runtime_diagnostics;
        assert_eq!(diagnostics.action_token_id.as_deref(), Some("ACTION_YES"));
        assert_eq!(diagnostics.selected_token_id.as_deref(), Some("ACTION_YES"));
        assert_eq!(
            diagnostics.selected_token_source,
            TOKEN_SOURCE_ACTION_IDENTITY
        );
        assert_eq!(diagnostics.token_matches_action_token, Some(true));
        assert_eq!(diagnostics.opposite_token_id.as_deref(), Some("ACTION_NO"));
        assert_eq!(
            diagnostics.opposite_token_source,
            OPPOSITE_TOKEN_SOURCE_PAIRED_STEP_TOKENS
        );
        assert_eq!(
            diagnostics.order_book_fetch_status,
            Some("runtime_client_missing")
        );
    }

    #[tokio::test]
    async fn selected_depth_missing_action_token_does_not_fallback_to_global_resolver() {
        let mut config =
            PriceToBeatIvMismatchEdgeConfig::crypto_defaults(PriceToBeatSignalFormulaMarketInput {
                best_bid: Some(0.49),
                best_ask: Some(0.51),
            });
        hydrate_action_place_order_iv_mismatch_book_quotes(
            None,
            &json!({
                "flowContext": {
                    "yesTokenId": "GLOBAL_YES",
                    "noTokenId": "GLOBAL_NO"
                }
            }),
            &node(json!({ "sizeMode": "shares", "targetQty": 10 })),
            "btc-updown-5m-1",
            None,
            Some("ACTION_YES"),
            Some("ACTION_NO"),
            "Up",
            None,
            &mut config,
        )
        .await;

        let diagnostics = config.depth_runtime_diagnostics;
        assert_eq!(diagnostics.action_token_id, None);
        assert_eq!(diagnostics.selected_token_id, None);
        assert_eq!(diagnostics.selected_token_source, TOKEN_SOURCE_MISSING);
        assert_eq!(
            diagnostics.order_book_fetch_status,
            Some("selected_token_id_missing")
        );
    }

    #[test]
    fn opposite_token_uses_pair_membership_not_label() {
        let yes_selected = resolve_opposite_token_from_step_pair(
            Some("YES_TOKEN"),
            Some("YES_TOKEN"),
            Some("NO_TOKEN"),
        );
        assert_eq!(yes_selected.selected_pair_side, Some("yes"));
        assert_eq!(yes_selected.opposite_token_id.as_deref(), Some("NO_TOKEN"));

        let no_selected = resolve_opposite_token_from_step_pair(
            Some("NO_TOKEN"),
            Some("YES_TOKEN"),
            Some("NO_TOKEN"),
        );
        assert_eq!(no_selected.selected_pair_side, Some("no"));
        assert_eq!(no_selected.opposite_token_id.as_deref(), Some("YES_TOKEN"));
    }

    #[test]
    fn untrusted_pair_keeps_selected_depth_but_marks_opposite_missing() {
        let resolution = resolve_opposite_token_from_step_pair(
            Some("ACTION_TOKEN"),
            Some("YES_TOKEN"),
            Some("NO_TOKEN"),
        );

        assert_eq!(resolution.selected_pair_side, None);
        assert_eq!(resolution.opposite_token_id, None);
        assert_eq!(
            resolution.opposite_token_source,
            OPPOSITE_TOKEN_SOURCE_UNTRUSTED_PAIR
        );
    }
}
