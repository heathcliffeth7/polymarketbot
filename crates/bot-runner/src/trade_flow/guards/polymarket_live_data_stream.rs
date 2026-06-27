use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        LazyLock,
    },
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::Message;

use super::binance_price::{
    binance_live_data_last_successful_tick_at_ms, ingest_binance_live_data_price,
    record_binance_live_data_error, record_binance_live_data_proxy_mode, SUPPORTED_BINANCE_SYMBOLS,
};
use super::chainlink_price::{
    chainlink_live_data_last_successful_tick_at_ms, ingest_chainlink_live_data_price,
    ingest_chainlink_live_data_price_history, record_chainlink_live_data_error,
    record_chainlink_live_data_proxy_mode, take_chainlink_live_data_reconnect_requested,
    ChainlinkLiveDataHistorySample,
};
use super::chainlink_symbols::{
    is_supported_symbol as is_supported_chainlink_symbol, SUPPORTED_RTDS_SYMBOLS,
};
use super::polymarket_live_data_ws::{
    active_live_data_ws_connections, classify_live_data_ws_error, connect_polymarket_live_data_ws,
    LiveDataWsBackoff, PolymarketLiveDataWsConnection,
};

const WS_URL_DEFAULT: &str = "wss://ws-live-data.polymarket.com";
const WS_URL_ENV: &str = "POLYMARKET_LIVE_DATA_WS_URL";
const TOPIC_BINANCE: &str = "crypto_prices";
const TOPIC_CHAINLINK: &str = "crypto_prices_chainlink";
const PING_INTERVAL_SECS: u64 = 5;
const CHAINLINK_REFRESH_INTERVAL_MS: u64 = 500;
const WATCHDOG_INTERVAL_SECS: u64 = 1;
const WS_IDLE_TIMEOUT_SECS: u64 = 15;
const SUBSCRIPTION_NO_TICK_TIMEOUT_SECS: u64 = 10;

static STREAM_STARTED: AtomicBool = AtomicBool::new(false);
static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);
static WATCHED_CHAINLINK_SYMBOLS: LazyLock<RwLock<HashMap<String, Instant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static LAST_WATCHED_CHAINLINK_TICKS: LazyLock<RwLock<HashMap<String, Instant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static LAST_CHAINLINK_REFRESH: LazyLock<RwLock<ChainlinkRefreshState>> =
    LazyLock::new(|| RwLock::new(ChainlinkRefreshState::default()));

#[derive(Debug, Clone)]
struct SharedSubscriptionMessage {
    body: String,
    chainlink_filters: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct ChainlinkRefreshState {
    last_requested_at: Option<Instant>,
    latest_sample_age_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChainlinkLiveDataRefreshDiagnostics {
    pub(crate) chainlink_refresh_requested: bool,
    pub(crate) refresh_interval_ms: i64,
    pub(crate) latest_sample_age_ms: Option<i64>,
    pub(crate) last_request_age_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct WsMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    message_type: Option<String>,
    timestamp: Option<i64>,
    payload: Option<Value>,
    #[serde(rename = "statusCode")]
    status_code: Option<u16>,
    body: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PricePayload {
    symbol: String,
    value: f64,
    timestamp: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PriceHistoryPayload {
    symbol: String,
    data: Vec<PriceHistorySample>,
}

#[derive(Debug, Deserialize)]
struct PriceHistorySample {
    value: f64,
    timestamp: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WsMessageHandling {
    any_valid_tick: bool,
    chainlink_valid_tick: bool,
}

impl WsMessageHandling {
    fn none() -> Self {
        Self::default()
    }

    fn binance(valid: bool) -> Self {
        Self {
            any_valid_tick: valid,
            chainlink_valid_tick: false,
        }
    }

    fn chainlink(valid: bool) -> Self {
        Self {
            any_valid_tick: valid,
            chainlink_valid_tick: valid,
        }
    }
}

pub(crate) fn ensure_polymarket_live_data_stream_started() {
    if STREAM_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(ws_stream_loop());
}

pub(crate) fn watch_chainlink_live_data_symbol(symbol: &str) {
    let mut watched = WATCHED_CHAINLINK_SYMBOLS.write();
    if watched.contains_key(symbol) {
        return;
    }
    watched.insert(symbol.to_string(), Instant::now());
    tracing::info!(symbol, "CHAINLINK_WATCHED_SYMBOL_REGISTERED");
}

pub(crate) fn chainlink_live_data_refresh_diagnostics() -> ChainlinkLiveDataRefreshDiagnostics {
    let state = *LAST_CHAINLINK_REFRESH.read();
    ChainlinkLiveDataRefreshDiagnostics {
        chainlink_refresh_requested: state.last_requested_at.is_some(),
        refresh_interval_ms: chainlink_refresh_interval_ms(),
        latest_sample_age_ms: state.latest_sample_age_ms,
        last_request_age_ms: state
            .last_requested_at
            .map(|at| Instant::now().duration_since(at).as_millis() as i64),
    }
}

async fn ws_stream_loop() {
    let mut backoff = LiveDataWsBackoff::new("polymarket_shared");
    loop {
        let session_id = SESSION_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        let chainlink_tick_before = chainlink_live_data_last_successful_tick_at_ms();
        let binance_tick_before = binance_live_data_last_successful_tick_at_ms();
        if let Err(err) = ws_stream_once(session_id).await {
            record_chainlink_live_data_error(&err);
            record_binance_live_data_error(&err);
            let tick_advanced = chainlink_live_data_last_successful_tick_at_ms()
                > chainlink_tick_before
                || binance_live_data_last_successful_tick_at_ms() > binance_tick_before;
            if tick_advanced {
                backoff.reset();
            }
            let delay = backoff.next_delay(session_id);
            let error_info = classify_live_data_ws_error(&err);
            tracing::warn!(
                session_id,
                error = %err,
                error_chain = %format!("{err:#}"),
                error_class = error_info.error_class,
                http_status = ?error_info.http_status,
                proxy_mode = error_info.proxy_mode.unwrap_or("unknown"),
                proxy_configured = bot_infra::proxy::socks5_proxy_configured(),
                live_data_ws_proxy_supported = true,
                backoff_ms = delay.as_millis() as u64,
                active_live_data_ws_connections = active_live_data_ws_connections(),
                "POLYMARKET_LIVE_DATA_WS_ERROR"
            );
            tokio::time::sleep(delay).await;
            continue;
        }
        backoff.reset();
    }
}

async fn ws_stream_once(session_id: u64) -> Result<()> {
    let url = std::env::var(WS_URL_ENV).unwrap_or_else(|_| WS_URL_DEFAULT.to_string());
    let PolymarketLiveDataWsConnection {
        ws,
        info,
        active_guard: _active_connection,
    } = connect_polymarket_live_data_ws("shared", session_id, &url)
        .await
        .with_context(|| format!("connecting to polymarket live data websocket: {url}"))?;
    record_chainlink_live_data_proxy_mode(info.proxy_mode);
    record_binance_live_data_proxy_mode(info.proxy_mode);

    let (mut sink, mut stream) = ws.split();
    let subscription = build_shared_subscription_message();
    sink.send(Message::Text(subscription.body.clone().into()))
        .await
        .context("sending polymarket live data shared subscription")?;
    tracing::info!(
        session_id,
        url,
        subscribed_topics = "crypto_prices,crypto_prices_chainlink",
        binance_symbol_count = SUPPORTED_BINANCE_SYMBOLS.len(),
        chainlink_symbol_count = subscription.chainlink_filters.len(),
        proxy_mode = info.proxy_mode,
        proxy_configured = info.proxy_configured,
        headers_mode = info.headers_mode,
        live_data_ws_proxy_supported = true,
        target_host = %info.target_host,
        target_port = info.target_port,
        active_live_data_ws_connections = info.active_connections,
        "POLYMARKET_LIVE_DATA_WS_CONNECTED"
    );
    tracing::info!(
        session_id,
        topics = "crypto_prices,crypto_prices_chainlink",
        binance_filters = "",
        chainlink_filters = %subscription.chainlink_filters.join(","),
        active_live_data_ws_connections = active_live_data_ws_connections(),
        "POLYMARKET_LIVE_DATA_WS_SUBSCRIBED"
    );

    let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_interval.tick().await;
    let mut chainlink_refresh_interval =
        tokio::time::interval(Duration::from_millis(CHAINLINK_REFRESH_INTERVAL_MS));
    chainlink_refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    chainlink_refresh_interval.tick().await;
    let mut watchdog_interval = tokio::time::interval(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
    watchdog_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    watchdog_interval.tick().await;

    let mut last_inbound_at = tokio::time::Instant::now();
    let subscription_started_at = last_inbound_at;
    let mut first_valid_tick_seen = false;
    let mut first_chainlink_tick_seen = false;
    let chainlink_tick_expected = !subscription.chainlink_filters.is_empty();

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                sink.send(Message::Text("PING".into()))
                    .await
                    .context("sending polymarket live data websocket ping")?;
                if take_chainlink_live_data_reconnect_requested() {
                    return Err(anyhow!("polymarket live data websocket reconnect requested by stale chainlink cache"));
                }
            }
            _ = chainlink_refresh_interval.tick() => {
                if let Some(refresh) = build_chainlink_refresh_subscription_message() {
                    let latest_sample_age_ms = latest_watched_chainlink_tick_age_ms();
                    sink.send(Message::Text(refresh.body.clone().into()))
                        .await
                        .context("sending polymarket live data chainlink refresh subscription")?;
                    record_chainlink_refresh_request(latest_sample_age_ms);
                    tracing::debug!(
                        session_id,
                        chainlink_refresh_requested = true,
                        refresh_interval_ms = chainlink_refresh_interval_ms(),
                        latest_sample_age_ms = ?latest_sample_age_ms,
                        chainlink_filters = %refresh.chainlink_filters.join(","),
                        active_live_data_ws_connections = active_live_data_ws_connections(),
                        "POLYMARKET_LIVE_DATA_WS_CHAINLINK_REFRESH_REQUESTED"
                    );
                }
                if take_chainlink_live_data_reconnect_requested() {
                    return Err(anyhow!("polymarket live data websocket reconnect requested by stale chainlink cache"));
                }
            }
            _ = watchdog_interval.tick() => {
                let now = tokio::time::Instant::now();
                if now.duration_since(last_inbound_at) >= Duration::from_secs(WS_IDLE_TIMEOUT_SECS) {
                    tracing::warn!(
                        session_id,
                        idle_timeout_secs = WS_IDLE_TIMEOUT_SECS,
                        "POLYMARKET_LIVE_DATA_WS_IDLE_TIMEOUT"
                    );
                    return Err(anyhow!("polymarket live data websocket idle timeout"));
                }
                if !first_valid_tick_seen
                    && now.duration_since(subscription_started_at)
                        >= Duration::from_secs(SUBSCRIPTION_NO_TICK_TIMEOUT_SECS)
                {
                    tracing::warn!(
                        session_id,
                        timeout_secs = SUBSCRIPTION_NO_TICK_TIMEOUT_SECS,
                        active_live_data_ws_connections = active_live_data_ws_connections(),
                        "POLYMARKET_LIVE_DATA_WS_SUBSCRIPTION_NO_TICK_TIMEOUT"
                    );
                    return Err(anyhow!(
                        "subscription_no_tick_timeout after {}s without valid live-data tick",
                        SUBSCRIPTION_NO_TICK_TIMEOUT_SECS
                    ));
                }
                if chainlink_tick_expected
                    && !first_chainlink_tick_seen
                    && now.duration_since(subscription_started_at)
                        >= Duration::from_secs(SUBSCRIPTION_NO_TICK_TIMEOUT_SECS)
                {
                    tracing::warn!(
                        session_id,
                        timeout_secs = SUBSCRIPTION_NO_TICK_TIMEOUT_SECS,
                        active_live_data_ws_connections = active_live_data_ws_connections(),
                        "POLYMARKET_LIVE_DATA_WS_SUBSCRIPTION_NO_CHAINLINK_TICK_TIMEOUT"
                    );
                    return Err(anyhow!(
                        "subscription_no_chainlink_tick_timeout after {}s without valid Chainlink live-data tick",
                        SUBSCRIPTION_NO_TICK_TIMEOUT_SECS
                    ));
                }
                if first_chainlink_tick_seen {
                    let timeout = Duration::from_secs(SUBSCRIPTION_NO_TICK_TIMEOUT_SECS);
                    if let Some(symbol) = watched_chainlink_symbol_without_recent_tick(timeout) {
                        tracing::warn!(
                            session_id,
                            symbol,
                            timeout_secs = SUBSCRIPTION_NO_TICK_TIMEOUT_SECS,
                            active_live_data_ws_connections = active_live_data_ws_connections(),
                            "POLYMARKET_LIVE_DATA_WS_WATCHED_CHAINLINK_SYMBOL_NO_TICK_TIMEOUT"
                        );
                        return Err(anyhow!(
                            "watched_chainlink_symbol_no_tick_timeout symbol={} after {}s without watched Chainlink tick",
                            symbol,
                            SUBSCRIPTION_NO_TICK_TIMEOUT_SECS
                        ));
                    }
                }
            }
            message = stream.next() => {
                let Some(message) = message else {
                    return Err(anyhow!("polymarket live data websocket stream ended"));
                };
                let message = message?;
                if matches!(&message, Message::Text(_) | Message::Pong(_)) {
                    last_inbound_at = tokio::time::Instant::now();
                }
                let handling = handle_ws_message(message, session_id)?;
                if handling.any_valid_tick {
                    first_valid_tick_seen = true;
                }
                if handling.chainlink_valid_tick {
                    first_chainlink_tick_seen = true;
                }
                if take_chainlink_live_data_reconnect_requested() {
                    return Err(anyhow!("polymarket live data websocket reconnect requested by stale chainlink cache"));
                }
            }
        }
    }
}

fn build_shared_subscription_message() -> SharedSubscriptionMessage {
    let chainlink_filters = watched_chainlink_subscription_filters();
    let mut subscriptions = vec![json!({
        "topic": TOPIC_BINANCE,
        "type": "update",
    })];
    subscriptions.extend(chainlink_filters.iter().map(|filter| {
        json!({
            "topic": TOPIC_CHAINLINK,
            "type": "*",
            "filters": filter,
        })
    }));
    SharedSubscriptionMessage {
        body: json!({
            "action": "subscribe",
            "subscriptions": subscriptions,
        })
        .to_string(),
        chainlink_filters,
    }
}

fn build_chainlink_refresh_subscription_message() -> Option<SharedSubscriptionMessage> {
    let chainlink_filters = watched_chainlink_subscription_filters();
    if chainlink_filters.is_empty() {
        return None;
    }
    let subscriptions = chainlink_filters
        .iter()
        .map(|filter| {
            json!({
                "topic": TOPIC_CHAINLINK,
                "type": "*",
                "filters": filter,
            })
        })
        .collect::<Vec<_>>();
    Some(SharedSubscriptionMessage {
        body: json!({
            "action": "subscribe",
            "subscriptions": subscriptions,
        })
        .to_string(),
        chainlink_filters,
    })
}

fn watched_chainlink_subscription_filters() -> Vec<String> {
    let watched = WATCHED_CHAINLINK_SYMBOLS.read();
    SUPPORTED_RTDS_SYMBOLS
        .iter()
        .filter(|symbol| watched.contains_key(**symbol))
        .map(|symbol| json!({ "symbol": symbol }).to_string())
        .collect()
}

fn watched_chainlink_symbol_without_recent_tick(timeout: Duration) -> Option<String> {
    let now = Instant::now();
    let watched = WATCHED_CHAINLINK_SYMBOLS.read();
    if watched.is_empty() {
        return None;
    }
    let ticks = LAST_WATCHED_CHAINLINK_TICKS.read();
    watched.iter().find_map(|(symbol, registered_at)| {
        let age = ticks
            .get(symbol)
            .map(|last_tick_at| now.duration_since(*last_tick_at))
            .unwrap_or_else(|| now.duration_since(*registered_at));
        (age >= timeout).then(|| symbol.clone())
    })
}

fn latest_watched_chainlink_tick_age_ms() -> Option<i64> {
    let now = Instant::now();
    LAST_WATCHED_CHAINLINK_TICKS
        .read()
        .values()
        .map(|tick_at| now.duration_since(*tick_at).as_millis() as i64)
        .min()
}

fn record_chainlink_refresh_request(latest_sample_age_ms: Option<i64>) {
    *LAST_CHAINLINK_REFRESH.write() = ChainlinkRefreshState {
        last_requested_at: Some(Instant::now()),
        latest_sample_age_ms,
    };
}

const fn chainlink_refresh_interval_ms() -> i64 {
    CHAINLINK_REFRESH_INTERVAL_MS as i64
}

fn handle_ws_message(message: Message, session_id: u64) -> Result<WsMessageHandling> {
    match message {
        Message::Text(text) => {
            if text.is_empty() || text == "PONG" {
                return Ok(WsMessageHandling::none());
            }
            let parsed = match serde_json::from_str::<WsMessage>(text.as_ref()) {
                Ok(parsed) => parsed,
                Err(err) => {
                    tracing::warn!(
                        session_id,
                        error = %err,
                        raw = %text,
                        "POLYMARKET_LIVE_DATA_WS_MESSAGE_PARSE_FAILED"
                    );
                    return Ok(WsMessageHandling::none());
                }
            };
            reject_status_error(&parsed, session_id)?;
            match parsed.topic.as_deref() {
                Some(TOPIC_CHAINLINK) => {
                    handle_chainlink_message(parsed, session_id).map(WsMessageHandling::chainlink)
                }
                Some(TOPIC_BINANCE) => handle_crypto_prices_message(parsed, session_id),
                Some(topic) => {
                    tracing::debug!(session_id, topic, "POLYMARKET_LIVE_DATA_WS_TOPIC_IGNORED");
                    Ok(WsMessageHandling::none())
                }
                None => Ok(WsMessageHandling::none()),
            }
        }
        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {
            Ok(WsMessageHandling::none())
        }
        Message::Close(frame) => {
            if let Some(frame) = frame {
                Err(anyhow!(
                    "polymarket live data websocket closed: code={} reason={}",
                    frame.code,
                    frame.reason
                ))
            } else {
                Err(anyhow!("polymarket live data websocket closed"))
            }
        }
    }
}

fn reject_status_error(parsed: &WsMessage, session_id: u64) -> Result<()> {
    let Some(status_code) = parsed.status_code else {
        return Ok(());
    };
    if status_code < 400 {
        return Ok(());
    }
    let body_message = parsed
        .body
        .as_ref()
        .and_then(|body| body.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("websocket subscription rejected");
    let body = parsed
        .body
        .as_ref()
        .map(Value::to_string)
        .unwrap_or_default();
    tracing::warn!(
        session_id,
        status_code,
        body = %body,
        error = %body_message,
        "POLYMARKET_LIVE_DATA_WS_SUBSCRIPTION_REJECTED"
    );
    Err(anyhow!(
        "polymarket live data websocket subscription rejected: status_code={} message={}",
        status_code,
        body_message
    ))
}

fn handle_chainlink_message(parsed: WsMessage, session_id: u64) -> Result<bool> {
    let Some(payload_value) = parsed.payload else {
        return Ok(false);
    };
    match parsed.message_type.as_deref() {
        Some("update") | Some("*") => {
            handle_chainlink_update_payload(payload_value, parsed.timestamp, session_id)
        }
        Some("subscribe") => handle_chainlink_history_payload(payload_value, session_id),
        _ => Ok(false),
    }
}

fn handle_chainlink_update_payload(
    payload_value: Value,
    fallback_timestamp_ms: Option<i64>,
    session_id: u64,
) -> Result<bool> {
    let payload: PricePayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_PAYLOAD_SHAPE"
            );
            return Ok(false);
        }
    };
    let Some(timestamp_ms) = payload.timestamp.or(fallback_timestamp_ms) else {
        tracing::debug!(
            session_id,
            symbol = %payload.symbol,
            "CHAINLINK_LIVE_DATA_WS_MISSING_TIMESTAMP"
        );
        return Ok(false);
    };
    let handled =
        ingest_chainlink_live_data_price(&payload.symbol, payload.value, timestamp_ms, session_id);
    if handled
        && WATCHED_CHAINLINK_SYMBOLS
            .read()
            .contains_key(&payload.symbol)
    {
        record_watched_chainlink_tick(&payload.symbol);
    }
    Ok(handled)
}

fn handle_chainlink_history_payload(payload_value: Value, session_id: u64) -> Result<bool> {
    let payload: PriceHistoryPayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_HISTORY_PAYLOAD_SHAPE"
            );
            return Ok(false);
        }
    };
    if !is_supported_chainlink_symbol(&payload.symbol) {
        return Ok(false);
    }
    let mut samples = payload.data;
    samples.sort_by_key(|sample| sample.timestamp);
    let history_samples = samples
        .into_iter()
        .map(|sample| ChainlinkLiveDataHistorySample {
            value: sample.value,
            timestamp_ms: sample.timestamp,
        })
        .collect();
    let summary =
        ingest_chainlink_live_data_price_history(&payload.symbol, history_samples, session_id);
    let handled = summary.sample_count > 0;
    if handled {
        record_watched_chainlink_tick(&payload.symbol);
        tracing::debug!(
            session_id,
            symbol = %payload.symbol,
            sample_count = summary.sample_count,
            latest_timestamp_ms = ?summary.latest_timestamp_ms,
            "CHAINLINK_LIVE_DATA_WS_HISTORY_SNAPSHOT"
        );
    }
    Ok(handled)
}

fn record_watched_chainlink_tick(symbol: &str) {
    if WATCHED_CHAINLINK_SYMBOLS.read().contains_key(symbol) {
        LAST_WATCHED_CHAINLINK_TICKS
            .write()
            .insert(symbol.to_string(), Instant::now());
    }
}

fn handle_crypto_prices_message(parsed: WsMessage, session_id: u64) -> Result<WsMessageHandling> {
    let is_chainlink_payload = payload_symbol(parsed.payload.as_ref())
        .map(is_supported_chainlink_symbol)
        .unwrap_or(false);
    if is_chainlink_payload {
        handle_chainlink_message(parsed, session_id).map(WsMessageHandling::chainlink)
    } else {
        handle_binance_message(parsed, session_id).map(WsMessageHandling::binance)
    }
}

fn payload_symbol(payload: Option<&Value>) -> Option<&str> {
    payload?
        .get("symbol")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|symbol| !symbol.is_empty())
}

fn handle_binance_message(parsed: WsMessage, session_id: u64) -> Result<bool> {
    let Some(message_type) = parsed.message_type.as_deref() else {
        return Ok(false);
    };
    let Some(payload_value) = parsed.payload else {
        return Ok(false);
    };
    match message_type {
        "update" => handle_binance_update_payload(payload_value, parsed.timestamp, session_id),
        "subscribe" => handle_binance_history_payload(payload_value, session_id),
        _ => Ok(false),
    }
}

fn handle_binance_update_payload(
    payload_value: Value,
    fallback_timestamp_ms: Option<i64>,
    session_id: u64,
) -> Result<bool> {
    let payload: PricePayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "BINANCE_LIVE_DATA_WS_UNEXPECTED_PAYLOAD_SHAPE"
            );
            return Ok(false);
        }
    };
    let Some(timestamp_ms) = payload.timestamp.or(fallback_timestamp_ms) else {
        tracing::debug!(
            session_id,
            symbol = %payload.symbol,
            "BINANCE_LIVE_DATA_WS_MISSING_TIMESTAMP"
        );
        return Ok(false);
    };
    Ok(ingest_binance_live_data_price(
        &payload.symbol,
        payload.value,
        timestamp_ms,
        session_id,
    ))
}

fn handle_binance_history_payload(payload_value: Value, session_id: u64) -> Result<bool> {
    let payload: PriceHistoryPayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "BINANCE_LIVE_DATA_WS_UNEXPECTED_HISTORY_PAYLOAD_SHAPE"
            );
            return Ok(false);
        }
    };
    let Some(last_sample) = payload.data.last() else {
        return Ok(false);
    };
    Ok(ingest_binance_live_data_price(
        &payload.symbol,
        last_sample.value,
        last_sample.timestamp,
        session_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::binance_price::{
        clear_binance_price_test_state, get_binance_price_snapshot,
    };
    use crate::trade_flow::guards::chainlink_price::{
        clear_chainlink_price_test_state, get_chainlink_price_cached,
        get_chainlink_price_test_snapshot,
    };
    use chrono::Utc;

    static TEST_LOCK: LazyLock<parking_lot::Mutex<()>> =
        LazyLock::new(|| parking_lot::Mutex::new(()));

    fn clear_watched_chainlink_test_state() {
        WATCHED_CHAINLINK_SYMBOLS.write().clear();
        LAST_WATCHED_CHAINLINK_TICKS.write().clear();
        *LAST_CHAINLINK_REFRESH.write() = ChainlinkRefreshState::default();
    }

    #[test]
    fn shared_subscription_without_watched_chainlink_symbols_uses_binance_only() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();

        let message = build_shared_subscription_message();
        let value: Value = serde_json::from_str(&message.body).expect("subscription json");
        let subscriptions = value["subscriptions"].as_array().expect("subscriptions");

        assert_eq!(subscriptions.len(), 1);
        assert_eq!(
            subscriptions[0].get("topic").and_then(Value::as_str),
            Some(TOPIC_BINANCE)
        );
        assert_eq!(
            subscriptions[0].get("filters").and_then(Value::as_str),
            None
        );
        assert!(message.chainlink_filters.is_empty());
    }

    #[test]
    fn chainlink_refresh_subscription_is_none_without_watched_symbols() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();

        assert!(build_chainlink_refresh_subscription_message().is_none());
    }

    #[test]
    fn chainlink_refresh_subscription_uses_watched_symbol_filters_only() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();
        watch_chainlink_live_data_symbol("sol/usd");

        let message = build_chainlink_refresh_subscription_message().expect("refresh subscription");
        let value: Value = serde_json::from_str(&message.body).expect("subscription json");
        let subscriptions = value["subscriptions"].as_array().expect("subscriptions");

        assert_eq!(subscriptions.len(), 1);
        assert_eq!(
            subscriptions[0].get("topic").and_then(Value::as_str),
            Some(TOPIC_CHAINLINK)
        );
        assert_eq!(
            subscriptions[0].get("type").and_then(Value::as_str),
            Some("*")
        );
        assert_eq!(
            subscriptions[0].get("filters").and_then(Value::as_str),
            Some("{\"symbol\":\"sol/usd\"}")
        );
        assert_eq!(message.chainlink_filters, vec!["{\"symbol\":\"sol/usd\"}"]);
    }

    #[tokio::test]
    async fn chainlink_current_cache_miss_registers_watched_symbol_for_refresh() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();
        clear_chainlink_price_test_state();

        let error = get_chainlink_price_cached("sol").expect_err("empty sol cache");
        assert!(error.to_string().contains("no cached price for sol/usd"));

        let message = build_chainlink_refresh_subscription_message().expect("refresh subscription");
        let value: Value = serde_json::from_str(&message.body).expect("subscription json");
        let subscriptions = value["subscriptions"].as_array().expect("subscriptions");

        assert_eq!(subscriptions.len(), 1);
        assert_eq!(
            subscriptions[0].get("filters").and_then(Value::as_str),
            Some("{\"symbol\":\"sol/usd\"}")
        );
    }

    #[test]
    fn chainlink_refresh_diagnostics_records_request_and_latest_sample_age() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();

        record_chainlink_refresh_request(Some(1_234));
        let diagnostics = chainlink_live_data_refresh_diagnostics();

        assert!(diagnostics.chainlink_refresh_requested);
        assert_eq!(
            diagnostics.refresh_interval_ms,
            chainlink_refresh_interval_ms()
        );
        assert_eq!(diagnostics.refresh_interval_ms, 500);
        assert_eq!(diagnostics.latest_sample_age_ms, Some(1_234));
        assert!(diagnostics.last_request_age_ms.unwrap_or_default() >= 0);
    }

    #[test]
    fn shared_subscription_uses_expected_topics_and_filters() {
        let _guard = TEST_LOCK.lock();
        clear_watched_chainlink_test_state();
        watch_chainlink_live_data_symbol("btc/usd");
        watch_chainlink_live_data_symbol("sol/usd");

        let message = build_shared_subscription_message();
        let value: Value = serde_json::from_str(&message.body).expect("subscription json");
        assert_eq!(
            value.get("action").and_then(Value::as_str),
            Some("subscribe")
        );
        let subscriptions = value["subscriptions"].as_array().expect("subscriptions");
        assert_eq!(subscriptions.len(), 3);
        assert_eq!(
            subscriptions[0].get("topic").and_then(Value::as_str),
            Some(TOPIC_BINANCE)
        );
        assert_eq!(
            subscriptions[0].get("type").and_then(Value::as_str),
            Some("update")
        );
        assert_eq!(
            subscriptions[0].get("filters").and_then(Value::as_str),
            None
        );
        assert_eq!(
            subscriptions[1].get("topic").and_then(Value::as_str),
            Some(TOPIC_CHAINLINK)
        );
        assert_eq!(
            subscriptions[1].get("type").and_then(Value::as_str),
            Some("*")
        );
        assert_eq!(
            subscriptions[1].get("filters").and_then(Value::as_str),
            Some("{\"symbol\":\"btc/usd\"}")
        );
        assert_eq!(
            subscriptions[2].get("topic").and_then(Value::as_str),
            Some(TOPIC_CHAINLINK)
        );
        assert_eq!(
            subscriptions[2].get("type").and_then(Value::as_str),
            Some("*")
        );
        assert_eq!(
            subscriptions[2].get("filters").and_then(Value::as_str),
            Some("{\"symbol\":\"sol/usd\"}")
        );
        assert_eq!(
            message.chainlink_filters,
            vec![
                "{\"symbol\":\"btc/usd\"}".to_string(),
                "{\"symbol\":\"sol/usd\"}".to_string()
            ]
        );
    }

    #[test]
    fn routes_chainlink_topic_to_chainlink_cache() {
        let _guard = TEST_LOCK.lock();
        clear_chainlink_price_test_state();
        clear_binance_price_test_state();
        let handled = handle_ws_message(
            Message::Text(
                json!({
                    "topic": TOPIC_CHAINLINK,
                    "type": "update",
                    "payload": {
                        "symbol": "eth/usd",
                        "value": 1625.25,
                        "timestamp": Utc::now().timestamp_millis()
                    }
                })
                .to_string()
                .into(),
            ),
            42,
        )
        .expect("chainlink message");

        assert!(handled.any_valid_tick);
        assert!(handled.chainlink_valid_tick);
        assert_eq!(
            get_chainlink_price_test_snapshot("eth").expect("chainlink snapshot"),
            1625.25
        );
        assert!(get_binance_price_snapshot("eth", Utc::now().timestamp_millis()).is_err());
    }

    #[test]
    fn routes_binance_topic_to_binance_cache() {
        let _guard = TEST_LOCK.lock();
        clear_chainlink_price_test_state();
        clear_binance_price_test_state();
        let handled = handle_ws_message(
            Message::Text(
                json!({
                    "topic": TOPIC_BINANCE,
                    "type": "update",
                    "payload": {
                        "symbol": "ethusdt",
                        "value": 1625.50,
                        "timestamp": 2_000
                    }
                })
                .to_string()
                .into(),
            ),
            43,
        )
        .expect("binance message");

        let snapshot = get_binance_price_snapshot("eth", 2_250).expect("binance snapshot");
        assert!(handled.any_valid_tick);
        assert!(!handled.chainlink_valid_tick);
        assert_eq!(snapshot.price, 1625.50);
        assert_eq!(snapshot.timestamp_ms, 2_000);
        assert_eq!(snapshot.staleness_ms, 250);
        assert!(get_chainlink_price_test_snapshot("eth").is_err());
    }

    #[test]
    fn wrong_topic_symbol_format_does_not_fill_cache() {
        let _guard = TEST_LOCK.lock();
        clear_chainlink_price_test_state();
        clear_binance_price_test_state();
        let handled = handle_ws_message(
            Message::Text(
                json!({
                    "topic": TOPIC_CHAINLINK,
                    "type": "update",
                    "payload": {
                        "symbol": "ethusdt",
                        "value": 1625.50,
                        "timestamp": Utc::now().timestamp_millis()
                    }
                })
                .to_string()
                .into(),
            ),
            44,
        )
        .expect("wrong symbol message");

        assert!(!handled.any_valid_tick);
        assert!(!handled.chainlink_valid_tick);
        assert!(get_chainlink_price_test_snapshot("eth").is_err());
        assert!(get_binance_price_snapshot("eth", Utc::now().timestamp_millis()).is_err());
    }

    #[test]
    fn subscription_rejection_is_terminal_error() {
        let err = handle_ws_message(
            Message::Text(
                json!({
                    "statusCode": 400,
                    "body": { "message": "invalid Subscription.Filters" }
                })
                .to_string()
                .into(),
            ),
            45,
        )
        .expect_err("subscription rejection should be an error");

        assert!(err.to_string().contains("subscription rejected"));
        assert!(err.to_string().contains("invalid Subscription.Filters"));
    }

    #[test]
    fn binance_history_payload_seeds_snapshot() {
        let _guard = TEST_LOCK.lock();
        clear_binance_price_test_state();
        clear_chainlink_price_test_state();
        let handled = handle_ws_message(
            Message::Text(
                json!({
                    "topic": TOPIC_BINANCE,
                    "type": "subscribe",
                    "payload": {
                        "symbol": "btcusdt",
                        "data": [
                            { "timestamp": 1_000, "value": 70_500.0 },
                            { "timestamp": 2_000, "value": 70_525.5 }
                        ]
                    }
                })
                .to_string()
                .into(),
            ),
            46,
        )
        .expect("history payload");

        let snapshot = get_binance_price_snapshot("btc", 2_250).expect("snapshot");
        assert!(handled.any_valid_tick);
        assert!(!handled.chainlink_valid_tick);
        assert_eq!(snapshot.price, 70_525.5);
        assert_eq!(snapshot.timestamp_ms, 2_000);
        assert_eq!(snapshot.staleness_ms, 250);
        assert!(get_chainlink_price_test_snapshot("btc").is_err());
    }

    #[test]
    fn crypto_prices_slash_symbol_history_payload_seeds_chainlink_cache() {
        let _guard = TEST_LOCK.lock();
        clear_chainlink_price_test_state();
        clear_binance_price_test_state();
        let now_ms = Utc::now().timestamp_millis();
        let handled = handle_ws_message(
            Message::Text(
                json!({
                    "topic": TOPIC_BINANCE,
                    "type": "subscribe",
                    "payload": {
                        "symbol": "btc/usd",
                        "data": [
                            { "timestamp": now_ms - 1_000, "value": 70_500.0 },
                            { "timestamp": now_ms, "value": 70_525.5 }
                        ]
                    }
                })
                .to_string()
                .into(),
            ),
            47,
        )
        .expect("chainlink history payload");

        assert!(handled.any_valid_tick);
        assert!(handled.chainlink_valid_tick);
        assert_eq!(
            get_chainlink_price_test_snapshot("btc").expect("chainlink snapshot"),
            70_525.5
        );
        assert!(get_binance_price_snapshot("btc", 2_250).is_err());
    }
}
