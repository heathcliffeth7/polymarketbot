#![cfg_attr(test, allow(dead_code))]

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        LazyLock,
    },
    time::Duration,
};
use tokio_tungstenite::tungstenite::Message;

const WS_URL_DEFAULT: &str = "wss://ws-live-data.polymarket.com";
const WS_URL_ENV: &str = "POLYMARKET_LIVE_DATA_WS_URL";
const SUBSCRIPTION_TOPIC: &str = "crypto_prices";
const PING_INTERVAL_SECS: u64 = 5;
const RECONNECT_DELAY_SECS: u64 = 2;
const WS_IDLE_TIMEOUT_SECS: u64 = 15;
const MAX_TICK_HISTORY_AGE_SECS: i64 = 10 * 60;
const MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL: usize = 5_000;
const SUPPORTED_BINANCE_SYMBOLS: &[&str] = &["btcusdt", "ethusdt", "solusdt", "xrpusdt"];

#[derive(Debug, Clone)]
struct CachedPrice {
    value: f64,
    timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BinancePriceSnapshot {
    pub(crate) price: f64,
    pub(crate) timestamp_ms: i64,
    pub(crate) staleness_ms: i64,
}

#[derive(Debug, Default)]
struct SymbolPriceState {
    latest: Option<CachedPrice>,
    ticks: VecDeque<CachedPrice>,
}

struct BinancePriceService {
    state: RwLock<HashMap<String, SymbolPriceState>>,
    last_error: RwLock<Option<String>>,
    warned_unexpected_symbols: RwLock<HashSet<String>>,
    started: AtomicBool,
    session_seq: AtomicU64,
}

static SERVICE: LazyLock<BinancePriceService> = LazyLock::new(BinancePriceService::new);

#[derive(Debug, Deserialize)]
struct WsMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    message_type: Option<String>,
    payload: Option<Value>,
    #[serde(rename = "statusCode")]
    status_code: Option<u16>,
    body: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PricePayload {
    symbol: String,
    value: f64,
    timestamp: i64,
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

impl BinancePriceService {
    fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            last_error: RwLock::new(None),
            warned_unexpected_symbols: RwLock::new(HashSet::new()),
            started: AtomicBool::new(false),
            session_seq: AtomicU64::new(0),
        }
    }

    fn ensure_started(&self) {
        if !self.started.swap(true, Ordering::SeqCst) {
            tokio::spawn(ws_stream_loop());
        }
    }

    fn next_session_id(&self) -> u64 {
        self.session_seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn should_warn_unexpected_symbol(&self, symbol: &str) -> bool {
        self.warned_unexpected_symbols
            .write()
            .insert(symbol.to_string())
    }

    fn snapshot(&self, asset: &str, now_ms: i64) -> Result<BinancePriceSnapshot> {
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let state = self.state.read();
        let entry = state
            .get(symbol)
            .ok_or_else(|| self.no_cached_price_error(symbol))?;
        let cached = entry
            .latest
            .as_ref()
            .ok_or_else(|| self.no_cached_price_error(symbol))?;
        Ok(BinancePriceSnapshot {
            price: cached.value,
            timestamp_ms: cached.timestamp_ms,
            staleness_ms: diff_ms(now_ms, cached.timestamp_ms),
        })
    }

    fn update_price(&self, payload: PricePayload, session_id: u64) {
        let received_at_ms = Utc::now().timestamp_millis();
        let tick = CachedPrice {
            value: payload.value,
            timestamp_ms: payload.timestamp,
        };
        let cutoff_ms = payload.timestamp - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
        let mut state = self.state.write();
        let entry = state.entry(payload.symbol.clone()).or_default();
        entry.latest = Some(tick.clone());
        if let Some(last_tick) = entry.ticks.back_mut() {
            if last_tick.timestamp_ms == tick.timestamp_ms {
                *last_tick = tick.clone();
            } else if last_tick.timestamp_ms < tick.timestamp_ms {
                entry.ticks.push_back(tick.clone());
            }
        } else {
            entry.ticks.push_back(tick.clone());
        }
        while entry
            .ticks
            .front()
            .map(|sample| sample.timestamp_ms < cutoff_ms)
            .unwrap_or(false)
        {
            entry.ticks.pop_front();
        }
        while entry.ticks.len() > MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL {
            entry.ticks.pop_front();
        }
        drop(state);
        tracing::debug!(
            session_id,
            symbol = %payload.symbol,
            value = payload.value,
            provider_timestamp_ms = payload.timestamp,
            received_at_ms,
            provider_age_ms = diff_ms(received_at_ms, payload.timestamp),
            "BINANCE_LIVE_DATA_WS_TICK"
        );
        *self.last_error.write() = None;
    }

    fn no_cached_price_error(&self, symbol: &str) -> anyhow::Error {
        match self.last_error.read().clone() {
            Some(last_error) => {
                anyhow!("no cached binance price for {symbol}; last ws error: {last_error}")
            }
            None => anyhow!("no cached binance price for {symbol}"),
        }
    }

    fn record_error(&self, error: &anyhow::Error) {
        *self.last_error.write() = Some(error.to_string());
    }
}

fn build_subscription_message() -> String {
    let subscriptions: Vec<Value> = SUPPORTED_BINANCE_SYMBOLS
        .iter()
        .map(|symbol| {
            json!({
                "topic": SUBSCRIPTION_TOPIC,
                "type": "update",
                "filters": json!({ "symbol": symbol }).to_string(),
            })
        })
        .collect();
    json!({
        "action": "subscribe",
        "subscriptions": subscriptions,
    })
    .to_string()
}

fn ws_idle_deadline(last_inbound_at: tokio::time::Instant) -> tokio::time::Instant {
    last_inbound_at + Duration::from_secs(WS_IDLE_TIMEOUT_SECS)
}

async fn wait_for_ws_idle_timeout(last_inbound_at: tokio::time::Instant) -> anyhow::Error {
    tokio::time::sleep_until(ws_idle_deadline(last_inbound_at)).await;
    anyhow!("binance live data websocket idle timeout")
}

async fn ws_stream_loop() {
    loop {
        let session_id = SERVICE.next_session_id();
        if let Err(err) = ws_stream_once(session_id).await {
            SERVICE.record_error(&err);
            tracing::warn!(session_id, error = %err, "BINANCE_LIVE_DATA_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
    }
}

async fn ws_stream_once(session_id: u64) -> Result<()> {
    let url = std::env::var(WS_URL_ENV).unwrap_or_else(|_| WS_URL_DEFAULT.to_string());
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting to polymarket live data websocket: {url}"))?;
    let (mut sink, mut stream) = ws.split();

    let subscription = build_subscription_message();
    sink.send(Message::Text(subscription.into()))
        .await
        .context("sending binance live data websocket subscription")?;
    tracing::info!(
        session_id,
        url,
        subscribed_symbol_count = SUPPORTED_BINANCE_SYMBOLS.len(),
        "BINANCE_LIVE_DATA_WS_CONNECTED"
    );

    let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_interval.tick().await;
    let mut last_inbound_at = tokio::time::Instant::now();

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                sink.send(Message::Text("PING".into()))
                    .await
                    .context("sending binance live data websocket ping")?;
            }
            err = wait_for_ws_idle_timeout(last_inbound_at) => {
                tracing::warn!(
                    session_id,
                    idle_timeout_secs = WS_IDLE_TIMEOUT_SECS,
                    "BINANCE_LIVE_DATA_WS_IDLE_TIMEOUT"
                );
                return Err(err);
            }
            message = stream.next() => {
                let Some(message) = message else {
                    return Err(anyhow!("binance live data websocket stream ended"));
                };
                let message = message?;
                if matches!(&message, Message::Text(_) | Message::Pong(_)) {
                    last_inbound_at = tokio::time::Instant::now();
                }
                handle_ws_message(message, session_id)?;
            }
        }
    }
}

fn handle_ws_message(message: Message, session_id: u64) -> Result<()> {
    match message {
        Message::Text(text) => {
            if text == "PONG" {
                return Ok(());
            }
            let parsed = match serde_json::from_str::<WsMessage>(text.as_ref()) {
                Ok(parsed) => parsed,
                Err(err) => {
                    if text.contains(SUBSCRIPTION_TOPIC) {
                        tracing::warn!(
                            session_id,
                            error = %err,
                            raw = %text,
                            "BINANCE_LIVE_DATA_WS_MESSAGE_PARSE_FAILED"
                        );
                    }
                    return Ok(());
                }
            };
            if let Some(status_code) = parsed.status_code {
                if status_code >= 400 {
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
                        "BINANCE_LIVE_DATA_WS_SUBSCRIPTION_REJECTED"
                    );
                    return Err(anyhow!(
                        "binance live data websocket subscription rejected: status_code={} message={}",
                        status_code,
                        body_message
                    ));
                }
            }
            if parsed.topic.as_deref() != Some(SUBSCRIPTION_TOPIC) {
                return Ok(());
            }
            let Some(message_type) = parsed.message_type.as_deref() else {
                return Ok(());
            };
            let Some(payload_value) = parsed.payload else {
                return Ok(());
            };
            match message_type {
                "update" => handle_price_update_payload(payload_value, session_id),
                "subscribe" => handle_price_history_payload(payload_value, session_id),
                _ => Ok(()),
            }
        }
        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => Ok(()),
        Message::Close(frame) => {
            if let Some(frame) = frame {
                Err(anyhow!(
                    "binance live data websocket closed: code={} reason={}",
                    frame.code,
                    frame.reason
                ))
            } else {
                Err(anyhow!("binance live data websocket closed"))
            }
        }
    }
}

fn handle_price_update_payload(payload_value: Value, session_id: u64) -> Result<()> {
    let payload: PricePayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "BINANCE_LIVE_DATA_WS_UNEXPECTED_PAYLOAD_SHAPE"
            );
            return Ok(());
        }
    };
    update_supported_price(payload, session_id);
    Ok(())
}

fn handle_price_history_payload(payload_value: Value, session_id: u64) -> Result<()> {
    let payload: PriceHistoryPayload = match serde_json::from_value(payload_value.clone()) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::warn!(
                session_id,
                error = %err,
                payload = %payload_value,
                "BINANCE_LIVE_DATA_WS_UNEXPECTED_HISTORY_PAYLOAD_SHAPE"
            );
            return Ok(());
        }
    };
    let Some(last_sample) = payload.data.last() else {
        return Ok(());
    };
    update_supported_price(
        PricePayload {
            symbol: payload.symbol,
            value: last_sample.value,
            timestamp: last_sample.timestamp,
        },
        session_id,
    );
    Ok(())
}

fn update_supported_price(payload: PricePayload, session_id: u64) {
    if !is_supported_symbol(&payload.symbol) {
        if SERVICE.should_warn_unexpected_symbol(&payload.symbol) {
            tracing::warn!(
                session_id,
                symbol = %payload.symbol,
                "BINANCE_LIVE_DATA_WS_UNEXPECTED_SYMBOL"
            );
        }
        return;
    }
    if !payload.value.is_finite() || payload.value <= 0.0 {
        return;
    }
    SERVICE.update_price(payload, session_id);
}

pub(crate) fn get_binance_price_snapshot(asset: &str, now_ms: i64) -> Result<BinancePriceSnapshot> {
    #[cfg(not(test))]
    SERVICE.ensure_started();
    SERVICE.snapshot(asset, now_ms)
}

pub(crate) fn ensure_binance_price_stream_started() {
    #[cfg(not(test))]
    SERVICE.ensure_started();
}

fn diff_ms(later_ms: i64, earlier_ms: i64) -> i64 {
    (later_ms - earlier_ms).max(0)
}

fn is_supported_symbol(symbol: &str) -> bool {
    SUPPORTED_BINANCE_SYMBOLS
        .iter()
        .any(|candidate| symbol.eq_ignore_ascii_case(candidate))
}

fn asset_to_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" | "bitcoin" => Some("btcusdt"),
        "eth" | "ethereum" => Some("ethusdt"),
        "sol" | "solana" => Some("solusdt"),
        "xrp" => Some("xrpusdt"),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn seed_binance_price_test_ticks(asset: &str, samples: &[(i64, f64)]) -> Result<()> {
    let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
    let ticks = samples
        .iter()
        .map(|(timestamp_ms, value)| CachedPrice {
            value: *value,
            timestamp_ms: *timestamp_ms,
        })
        .collect::<VecDeque<_>>();
    let latest = ticks.back().cloned();
    SERVICE
        .state
        .write()
        .insert(symbol.to_string(), SymbolPriceState { latest, ticks });
    *SERVICE.last_error.write() = None;
    Ok(())
}

#[cfg(test)]
pub(crate) fn clear_binance_price_test_state() {
    SERVICE.state.write().clear();
    *SERVICE.last_error.write() = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_to_symbol_maps_supported_assets() {
        assert_eq!(asset_to_symbol("btc"), Some("btcusdt"));
        assert_eq!(asset_to_symbol("ETH"), Some("ethusdt"));
        assert_eq!(asset_to_symbol(" sol "), Some("solusdt"));
        assert_eq!(asset_to_symbol("xrp"), Some("xrpusdt"));
        assert_eq!(asset_to_symbol("doge"), None);
    }

    #[test]
    fn update_price_makes_snapshot_available() {
        let service = BinancePriceService::new();
        service.update_price(
            PricePayload {
                symbol: "btcusdt".to_string(),
                value: 70_505.34,
                timestamp: 1_000,
            },
            99,
        );

        let snapshot = service.snapshot("btc", 1_250).expect("snapshot");
        assert_eq!(snapshot.price, 70_505.34);
        assert_eq!(snapshot.timestamp_ms, 1_000);
        assert_eq!(snapshot.staleness_ms, 250);
    }

    #[test]
    fn subscription_message_uses_binance_topic_and_filters() {
        let value: Value =
            serde_json::from_str(&build_subscription_message()).expect("subscription json");
        assert_eq!(
            value.get("action").and_then(Value::as_str),
            Some("subscribe")
        );
        let subscriptions = value["subscriptions"].as_array().expect("subscriptions");
        assert_eq!(subscriptions.len(), SUPPORTED_BINANCE_SYMBOLS.len());
        for (subscription, symbol) in subscriptions.iter().zip(SUPPORTED_BINANCE_SYMBOLS) {
            assert_eq!(
                subscription.get("topic").and_then(Value::as_str),
                Some(SUBSCRIPTION_TOPIC)
            );
            assert_eq!(
                subscription.get("type").and_then(Value::as_str),
                Some("update")
            );
            let filter = subscription
                .get("filters")
                .and_then(Value::as_str)
                .expect("filter");
            let filter: Value = serde_json::from_str(filter).expect("filter json");
            assert_eq!(filter.get("symbol").and_then(Value::as_str), Some(*symbol));
        }
    }

    #[test]
    fn handle_ws_message_returns_error_for_subscription_rejection() {
        let err = handle_ws_message(
            Message::Text(
                json!({
                    "statusCode": 400,
                    "body": { "message": "invalid Subscription.Filters" }
                })
                .to_string()
                .into(),
            ),
            7,
        )
        .expect_err("subscription rejection should be an error");

        assert!(err.to_string().contains("subscription rejected"));
        assert!(err.to_string().contains("invalid Subscription.Filters"));
    }

    #[test]
    fn handle_ws_message_seeds_snapshot_from_history_payload() {
        clear_binance_price_test_state();
        handle_ws_message(
            Message::Text(
                json!({
                    "topic": SUBSCRIPTION_TOPIC,
                    "type": "subscribe",
                    "timestamp": 2_010,
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
            8,
        )
        .expect("history payload");

        let snapshot = SERVICE.snapshot("btc", 2_250).expect("snapshot");
        assert_eq!(snapshot.price, 70_525.5);
        assert_eq!(snapshot.timestamp_ms, 2_000);
        assert_eq!(snapshot.staleness_ms, 250);
    }
}
