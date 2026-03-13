use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
    time::Duration,
};
use tokio_tungstenite::tungstenite::Message;

const WS_URL_DEFAULT: &str = "wss://ws-live-data.polymarket.com";
const WS_URL_ENV: &str = "POLYMARKET_LIVE_DATA_WS_URL";
const SUBSCRIPTION_TOPIC: &str = "crypto_prices_chainlink";
const PING_INTERVAL_SECS: u64 = 5;
const RECONNECT_DELAY_SECS: u64 = 2;
const MAX_PRICE_AGE_SECS: i64 = 30;
const MAX_TICK_HISTORY_AGE_SECS: i64 = 4 * 60 * 60;
const MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL: usize = 20_000;
const MAX_CYCLE_OPEN_LATENCY_MS: i64 = 1_000;
const INITIAL_FETCH_RETRIES: usize = 10;
const INITIAL_FETCH_DELAY_MS: u64 = 500;
const CYCLE_OPEN_FETCH_RETRIES: usize = 8;
const CYCLE_OPEN_FETCH_DELAY_MS: u64 = 250;
const NO_CYCLE_OPEN_SNAPSHOT_ERROR_PREFIX: &str = "no cycle-open snapshot for ";

#[derive(Debug, Clone)]
struct CachedPrice {
    value: f64,
    timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkCycleOpenSnapshot {
    pub(crate) price: f64,
    pub(crate) timestamp_ms: i64,
    pub(crate) latency_ms: i64,
}

#[derive(Debug, Default)]
struct SymbolPriceState {
    latest: Option<CachedPrice>,
    ticks: VecDeque<CachedPrice>,
    cycle_open_by_ts: HashMap<i64, ChainlinkCycleOpenSnapshot>,
}

struct ChainlinkPriceService {
    state: RwLock<HashMap<String, SymbolPriceState>>,
    last_error: RwLock<Option<String>>,
    started: AtomicBool,
}

static SERVICE: LazyLock<ChainlinkPriceService> = LazyLock::new(ChainlinkPriceService::new);

#[derive(Debug, Deserialize)]
struct WsMessage {
    topic: Option<String>,
    payload: Option<PricePayload>,
}

#[derive(Debug, Deserialize)]
struct PricePayload {
    symbol: String,
    value: f64,
    timestamp: i64,
}

impl ChainlinkPriceService {
    fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            last_error: RwLock::new(None),
            started: AtomicBool::new(false),
        }
    }

    fn ensure_started(&self) {
        if !self.started.swap(true, Ordering::SeqCst) {
            tokio::spawn(ws_stream_loop());
        }
    }

    fn get_price(&self, asset: &str) -> Result<f64> {
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let state = self.state.read();
        let cached = state
            .get(symbol)
            .and_then(|entry| entry.latest.as_ref())
            .ok_or_else(|| self.no_cached_price_error(symbol))?;
        let age_secs = (Utc::now().timestamp_millis() - cached.timestamp_ms) / 1_000;
        if age_secs > MAX_PRICE_AGE_SECS {
            return Err(anyhow!("stale price for {symbol}: {age_secs}s old"));
        }
        Ok(cached.value)
    }

    fn get_cycle_open_snapshot(
        &self,
        asset: &str,
        cycle_start: DateTime<Utc>,
    ) -> Result<ChainlinkCycleOpenSnapshot> {
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let cycle_start_ms = cycle_start.timestamp_millis();

        {
            let state = self.state.read();
            if let Some(snapshot) = state
                .get(symbol)
                .and_then(|entry| entry.cycle_open_by_ts.get(&cycle_start_ms))
                .cloned()
            {
                return Ok(snapshot);
            }
        }

        let snapshot = {
            let state = self.state.read();
            let Some(entry) = state.get(symbol) else {
                return Err(self.no_cycle_open_snapshot_error(symbol, cycle_start_ms, None));
            };
            resolve_cycle_open_snapshot_from_ticks(symbol, &entry.ticks, cycle_start_ms)?
        };

        let mut state = self.state.write();
        let entry = state.entry(symbol.to_string()).or_default();
        entry
            .cycle_open_by_ts
            .insert(cycle_start_ms, snapshot.clone());
        Ok(snapshot)
    }

    fn no_cached_price_error(&self, symbol: &str) -> anyhow::Error {
        match self.last_error.read().clone() {
            Some(last_error) => {
                anyhow!("no cached price for {symbol}; last ws error: {last_error}")
            }
            None => anyhow!("no cached price for {symbol}"),
        }
    }

    fn no_cycle_open_snapshot_error(
        &self,
        symbol: &str,
        cycle_start_ms: i64,
        detail: Option<String>,
    ) -> anyhow::Error {
        let base = format!("{NO_CYCLE_OPEN_SNAPSHOT_ERROR_PREFIX}{symbol} at {cycle_start_ms}");
        match detail {
            Some(detail) => anyhow!("{base}; {detail}"),
            None => anyhow!("{base}"),
        }
    }

    fn update_price(&self, payload: PricePayload) {
        let tick = CachedPrice {
            value: payload.value,
            timestamp_ms: payload.timestamp,
        };
        let cutoff_ms = payload.timestamp - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
        let mut state = self.state.write();
        let entry = state.entry(payload.symbol).or_default();
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
        entry.cycle_open_by_ts.retain(|cycle_start_ms, snapshot| {
            *cycle_start_ms >= cutoff_ms && snapshot.timestamp_ms >= cutoff_ms
        });
        *self.last_error.write() = None;
    }

    fn record_error(&self, error: &anyhow::Error) {
        *self.last_error.write() = Some(error.to_string());
    }
}

fn asset_to_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("btc/usd"),
        "eth" => Some("eth/usd"),
        "sol" => Some("sol/usd"),
        "xrp" => Some("xrp/usd"),
        _ => None,
    }
}

fn resolve_cycle_open_snapshot_from_ticks(
    symbol: &str,
    ticks: &VecDeque<CachedPrice>,
    cycle_start_ms: i64,
) -> Result<ChainlinkCycleOpenSnapshot> {
    let Some(tick) = ticks
        .iter()
        .find(|sample| sample.timestamp_ms >= cycle_start_ms)
    else {
        let latest_tick_detail = ticks
            .back()
            .map(|sample| format!("latest cached tick ts={}", sample.timestamp_ms))
            .unwrap_or_else(|| "no cached ticks".to_string());
        return Err(anyhow!(
            "{NO_CYCLE_OPEN_SNAPSHOT_ERROR_PREFIX}{symbol} at {cycle_start_ms}; {latest_tick_detail}"
        ));
    };
    let latency_ms = tick.timestamp_ms - cycle_start_ms;
    if latency_ms > MAX_CYCLE_OPEN_LATENCY_MS {
        return Err(anyhow!(
            "{NO_CYCLE_OPEN_SNAPSHOT_ERROR_PREFIX}{symbol} at {cycle_start_ms}; first tick latency {}ms exceeds {}ms",
            latency_ms,
            MAX_CYCLE_OPEN_LATENCY_MS
        ));
    }
    Ok(ChainlinkCycleOpenSnapshot {
        price: tick.value,
        timestamp_ms: tick.timestamp_ms,
        latency_ms,
    })
}

async fn ws_stream_loop() {
    loop {
        if let Err(err) = ws_stream_once().await {
            SERVICE.record_error(&err);
            tracing::warn!(error = %err, "CHAINLINK_LIVE_DATA_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
    }
}

async fn ws_stream_once() -> Result<()> {
    let url = std::env::var(WS_URL_ENV).unwrap_or_else(|_| WS_URL_DEFAULT.to_string());
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting to polymarket live data websocket: {url}"))?;
    let (mut sink, mut stream) = ws.split();

    let subscription = serde_json::json!({
        "action": "subscribe",
        "subscriptions": [{
            "topic": SUBSCRIPTION_TOPIC,
            "type": "*",
            "filters": ""
        }]
    });
    sink.send(Message::Text(subscription.to_string().into()))
        .await
        .context("sending live data websocket subscription")?;
    tracing::info!("CHAINLINK_LIVE_DATA_WS_CONNECTED");

    let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_interval.tick().await;

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                sink.send(Message::Text("PING".into()))
                    .await
                    .context("sending live data websocket ping")?;
            }
            message = stream.next() => {
                let Some(message) = message else {
                    return Err(anyhow!("live data websocket stream ended"));
                };
                handle_ws_message(message?)?;
            }
        }
    }
}

fn handle_ws_message(message: Message) -> Result<()> {
    match message {
        Message::Text(text) => {
            if text == "PONG" {
                return Ok(());
            }
            let parsed = match serde_json::from_str::<WsMessage>(text.as_ref()) {
                Ok(parsed) => parsed,
                Err(_) => return Ok(()),
            };
            if parsed.topic.as_deref() != Some(SUBSCRIPTION_TOPIC) {
                return Ok(());
            }
            let Some(payload) = parsed.payload else {
                return Ok(());
            };
            if !payload.value.is_finite() || payload.value <= 0.0 {
                return Ok(());
            }
            SERVICE.update_price(payload);
            Ok(())
        }
        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => Ok(()),
        Message::Close(frame) => {
            if let Some(frame) = frame {
                Err(anyhow!(
                    "live data websocket closed: code={} reason={}",
                    frame.code,
                    frame.reason
                ))
            } else {
                Err(anyhow!("live data websocket closed"))
            }
        }
    }
}

pub(crate) async fn fetch_chainlink_price(asset: &str) -> Result<f64> {
    SERVICE.ensure_started();

    for attempt in 0..=INITIAL_FETCH_RETRIES {
        match SERVICE.get_price(asset) {
            Ok(price) => return Ok(price),
            Err(err)
                if err.to_string().starts_with("no cached price for ")
                    && attempt < INITIAL_FETCH_RETRIES =>
            {
                tokio::time::sleep(Duration::from_millis(INITIAL_FETCH_DELAY_MS)).await;
            }
            Err(err) => return Err(err),
        }
    }

    SERVICE.get_price(asset)
}

pub(crate) async fn fetch_chainlink_cycle_open(
    asset: &str,
    cycle_start: DateTime<Utc>,
) -> Result<ChainlinkCycleOpenSnapshot> {
    SERVICE.ensure_started();

    for attempt in 0..=CYCLE_OPEN_FETCH_RETRIES {
        match SERVICE.get_cycle_open_snapshot(asset, cycle_start) {
            Ok(snapshot) => return Ok(snapshot),
            Err(err)
                if err
                    .to_string()
                    .starts_with(NO_CYCLE_OPEN_SNAPSHOT_ERROR_PREFIX)
                    && attempt < CYCLE_OPEN_FETCH_RETRIES =>
            {
                tokio::time::sleep(Duration::from_millis(CYCLE_OPEN_FETCH_DELAY_MS)).await;
            }
            Err(err) => return Err(err),
        }
    }

    SERVICE.get_cycle_open_snapshot(asset, cycle_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_to_symbol_maps_supported_assets() {
        assert_eq!(asset_to_symbol("btc"), Some("btc/usd"));
        assert_eq!(asset_to_symbol("ETH"), Some("eth/usd"));
        assert_eq!(asset_to_symbol(" sol "), Some("sol/usd"));
        assert_eq!(asset_to_symbol("xrp"), Some("xrp/usd"));
        assert_eq!(asset_to_symbol("doge"), None);
    }

    #[test]
    fn get_price_errors_when_cache_is_empty() {
        let service = ChainlinkPriceService::new();
        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("no cached price for btc/usd"));
    }

    #[test]
    fn get_price_errors_when_price_is_stale() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 70_505.34,
                    timestamp_ms: Utc::now().timestamp_millis()
                        - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
                }),
                ..SymbolPriceState::default()
            },
        );

        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("stale price for btc/usd"));
    }

    #[test]
    fn get_price_returns_cached_value_when_price_is_fresh() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 2_069.351149877574,
                    timestamp_ms: Utc::now().timestamp_millis(),
                }),
                ..SymbolPriceState::default()
            },
        );

        let price = service.get_price("eth").unwrap();
        assert_eq!(price, 2_069.351149877574);
    }

    #[test]
    fn get_cycle_open_snapshot_returns_first_tick_after_cycle_start() {
        let service = ChainlinkPriceService::new();
        let cycle_start = DateTime::<Utc>::from_timestamp_millis(1_770_000_000_000).unwrap();
        service.state.write().insert(
            "sol/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![
                    CachedPrice {
                        value: 100.0,
                        timestamp_ms: cycle_start.timestamp_millis() - 250,
                    },
                    CachedPrice {
                        value: 101.5,
                        timestamp_ms: cycle_start.timestamp_millis() + 250,
                    },
                    CachedPrice {
                        value: 103.0,
                        timestamp_ms: cycle_start.timestamp_millis() + 750,
                    },
                ]),
                ..SymbolPriceState::default()
            },
        );

        let snapshot = service
            .get_cycle_open_snapshot("sol", cycle_start)
            .expect("cycle-open snapshot");
        assert_eq!(snapshot.price, 101.5);
        assert_eq!(snapshot.latency_ms, 250);
    }

    #[test]
    fn get_cycle_open_snapshot_rejects_high_latency_tick() {
        let service = ChainlinkPriceService::new();
        let cycle_start = DateTime::<Utc>::from_timestamp_millis(1_770_000_000_000).unwrap();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 70_100.0,
                    timestamp_ms: cycle_start.timestamp_millis() + MAX_CYCLE_OPEN_LATENCY_MS + 1,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let err = service
            .get_cycle_open_snapshot("btc", cycle_start)
            .unwrap_err()
            .to_string();
        assert!(err.contains("first tick latency"));
    }

    #[test]
    fn get_cycle_open_snapshot_uses_cached_entry() {
        let service = ChainlinkPriceService::new();
        let cycle_start = DateTime::<Utc>::from_timestamp_millis(1_770_000_000_000).unwrap();
        let cached = ChainlinkCycleOpenSnapshot {
            price: 2_105.2,
            timestamp_ms: cycle_start.timestamp_millis() + 125,
            latency_ms: 125,
        };
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                cycle_open_by_ts: HashMap::from([(cycle_start.timestamp_millis(), cached.clone())]),
                ..SymbolPriceState::default()
            },
        );

        let snapshot = service
            .get_cycle_open_snapshot("eth", cycle_start)
            .expect("cached cycle-open snapshot");
        assert_eq!(snapshot, cached);
    }

    #[test]
    fn update_price_refreshes_latest_even_when_timestamp_moves_backward() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 2_150.0,
                    timestamp_ms: 1_770_000_005_000,
                }),
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 2_150.0,
                    timestamp_ms: 1_770_000_005_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        service.update_price(PricePayload {
            symbol: "eth/usd".to_string(),
            value: 2_141.25,
            timestamp: 1_770_000_003_000,
        });

        let state = service.state.read();
        let entry = state.get("eth/usd").expect("eth state");
        let latest = entry.latest.as_ref().expect("latest");
        assert_eq!(latest.value, 2_141.25);
        assert_eq!(latest.timestamp_ms, 1_770_000_003_000);
        assert_eq!(
            entry.ticks.len(),
            1,
            "out-of-order tick should not extend history"
        );
    }
}
