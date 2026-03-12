use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use std::{
    collections::HashMap,
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
const INITIAL_FETCH_RETRIES: usize = 10;
const INITIAL_FETCH_DELAY_MS: u64 = 500;

#[derive(Debug, Clone)]
struct CachedPrice {
    value: f64,
    timestamp_ms: i64,
}

struct ChainlinkPriceService {
    cache: RwLock<HashMap<String, CachedPrice>>,
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
            cache: RwLock::new(HashMap::new()),
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
        let cache = self.cache.read();
        let cached = cache
            .get(symbol)
            .ok_or_else(|| self.no_cached_price_error(symbol))?;
        let age_secs = (Utc::now().timestamp_millis() - cached.timestamp_ms) / 1_000;
        if age_secs > MAX_PRICE_AGE_SECS {
            return Err(anyhow!("stale price for {symbol}: {age_secs}s old"));
        }
        Ok(cached.value)
    }

    fn no_cached_price_error(&self, symbol: &str) -> anyhow::Error {
        match self.last_error.read().clone() {
            Some(last_error) => {
                anyhow!("no cached price for {symbol}; last ws error: {last_error}")
            }
            None => anyhow!("no cached price for {symbol}"),
        }
    }

    fn update_price(&self, payload: PricePayload) {
        self.cache.write().insert(
            payload.symbol,
            CachedPrice {
                value: payload.value,
                timestamp_ms: payload.timestamp,
            },
        );
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
        service.cache.write().insert(
            "btc/usd".to_string(),
            CachedPrice {
                value: 70_505.34,
                timestamp_ms: Utc::now().timestamp_millis() - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
            },
        );

        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("stale price for btc/usd"));
    }

    #[test]
    fn get_price_returns_cached_value_when_price_is_fresh() {
        let service = ChainlinkPriceService::new();
        service.cache.write().insert(
            "eth/usd".to_string(),
            CachedPrice {
                value: 2_069.351149877574,
                timestamp_ms: Utc::now().timestamp_millis(),
            },
        );

        let price = service.get_price("eth").unwrap();
        assert_eq!(price, 2_069.351149877574);
    }
}
