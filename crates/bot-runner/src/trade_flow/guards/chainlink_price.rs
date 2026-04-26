use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::{Mutex, RwLock};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
        LazyLock,
    },
    time::Duration,
};
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

const WS_URL_DEFAULT: &str = "wss://ws-live-data.polymarket.com";
const WS_URL_ENV: &str = "POLYMARKET_LIVE_DATA_WS_URL";
const SUBSCRIPTION_TOPIC: &str = "crypto_prices_chainlink";
const PING_INTERVAL_SECS: u64 = 5;
const RECONNECT_DELAY_SECS: u64 = 2;
const WS_IDLE_TIMEOUT_SECS: u64 = 15;
const MAX_PRICE_AGE_SECS: i64 = 30;
const STALE_RECONNECT_REQUEST_COOLDOWN_SECS: i64 = 10;
const MAX_TICK_HISTORY_AGE_SECS: i64 = 4 * 60 * 60;
const MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL: usize = 20_000;
#[cfg(test)]
const MAX_NEAR_TIMESTAMP_PAST_TOLERANCE_MS: i64 = 60_000;
#[cfg(test)]
const MAX_NEAR_TIMESTAMP_FUTURE_TOLERANCE_MS: i64 = 2_000;
const PTB_START_TICK_MAX_PAST_TOLERANCE_MS: i64 = 2_000;
const PTB_START_TICK_MAX_FUTURE_TOLERANCE_MS: i64 = 2_000;
const STALE_PROVIDER_WARN_INTERVAL_MS: i64 = MAX_PRICE_AGE_SECS * 1_000;
const SUPPORTED_RTDS_SYMBOLS: &[&str] = &["btc/usd", "eth/usd", "sol/usd", "xrp/usd"];

#[derive(Debug, Clone)]
struct CachedPrice {
    value: f64,
    timestamp_ms: i64,
    received_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkPriceTimestampSnapshot {
    pub(crate) price: f64,
    pub(crate) timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkPriceWindowStats {
    pub(crate) open_price: f64,
    pub(crate) high_price: f64,
    pub(crate) low_price: f64,
    pub(crate) close_price: f64,
    pub(crate) sample_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkPriceSample {
    pub(crate) price: f64,
    pub(crate) timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainlinkStalePriceDetails {
    pub(crate) provider_age_ms: i64,
    pub(crate) receive_age_ms: i64,
    pub(crate) provider_timestamp_ms: i64,
    pub(crate) received_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainlinkNearTimestampRejectionDetails {
    pub(crate) gap_ms: i64,
    pub(crate) provider_age_ms: i64,
    pub(crate) candidate_timestamp_ms: i64,
    pub(crate) candidate_received_at_ms: i64,
}

#[derive(Debug, Default)]
struct SymbolPriceState {
    latest: Option<CachedPrice>,
    ticks: VecDeque<CachedPrice>,
    last_received_at_ms: Option<i64>,
    last_stale_reason: Option<String>,
    last_stale_warning_at_ms: Option<i64>,
}

struct ChainlinkPriceService {
    state: RwLock<HashMap<String, SymbolPriceState>>,
    last_error: RwLock<Option<String>>,
    warned_unexpected_symbols: RwLock<HashSet<String>>,
    dirty_assets: Mutex<HashSet<String>>,
    dirty_update_notify: Notify,
    started: AtomicBool,
    reconnect_requested: AtomicBool,
    last_reconnect_request_at_ms: AtomicI64,
    session_seq: AtomicU64,
}

static SERVICE: LazyLock<ChainlinkPriceService> = LazyLock::new(ChainlinkPriceService::new);

#[derive(Debug, Deserialize)]
struct WsMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    message_type: Option<String>,
    timestamp: Option<i64>,
    payload: Option<Value>,
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
            warned_unexpected_symbols: RwLock::new(HashSet::new()),
            dirty_assets: Mutex::new(HashSet::new()),
            dirty_update_notify: Notify::new(),
            started: AtomicBool::new(false),
            reconnect_requested: AtomicBool::new(false),
            last_reconnect_request_at_ms: AtomicI64::new(0),
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

    fn take_reconnect_requested(&self) -> bool {
        self.reconnect_requested.swap(false, Ordering::SeqCst)
    }

    fn request_reconnect_if_cooldown_elapsed(
        &self,
        symbol: &str,
        provider_age_ms: i64,
        receive_age_ms: i64,
        now_ms: i64,
    ) -> bool {
        let cooldown_ms = STALE_RECONNECT_REQUEST_COOLDOWN_SECS * 1_000;
        loop {
            let last_requested_at_ms = self.last_reconnect_request_at_ms.load(Ordering::SeqCst);
            if last_requested_at_ms > 0 && diff_ms(now_ms, last_requested_at_ms) < cooldown_ms {
                return false;
            }
            if self
                .last_reconnect_request_at_ms
                .compare_exchange(
                    last_requested_at_ms,
                    now_ms,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                self.reconnect_requested.store(true, Ordering::SeqCst);
                tracing::warn!(
                    symbol,
                    provider_age_ms,
                    receive_age_ms,
                    reconnect_cooldown_secs = STALE_RECONNECT_REQUEST_COOLDOWN_SECS,
                    "CHAINLINK_LIVE_DATA_WS_RECONNECT_REQUESTED"
                );
                return true;
            }
        }
    }

    fn get_price(&self, asset: &str) -> Result<f64> {
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let (cached, last_received_at_ms) = {
            let state = self.state.read();
            let entry = state
                .get(symbol)
                .ok_or_else(|| self.no_cached_price_error(symbol))?;
            let cached = entry
                .latest
                .as_ref()
                .cloned()
                .ok_or_else(|| self.no_cached_price_error(symbol))?;
            (cached, entry.last_received_at_ms)
        };
        let now_ms = Utc::now().timestamp_millis();
        let age_secs = diff_ms(now_ms, cached.timestamp_ms) / 1_000;
        if age_secs > MAX_PRICE_AGE_SECS {
            let received_at_ms = last_received_at_ms.unwrap_or(cached.received_at_ms);
            let provider_age_ms = diff_ms(now_ms, cached.timestamp_ms);
            let receive_age_ms = diff_ms(now_ms, received_at_ms);
            self.request_reconnect_if_cooldown_elapsed(
                symbol,
                provider_age_ms,
                receive_age_ms,
                now_ms,
            );
            let error = self.build_stale_price_error(symbol, &cached, Some(received_at_ms), now_ms);
            self.record_stale_reason(symbol, error.to_string());
            return Err(error);
        }
        Ok(cached.value)
    }

    #[cfg(test)]
    fn get_price_near_timestamp(
        &self,
        asset: &str,
        target_ms: i64,
    ) -> Result<ChainlinkPriceTimestampSnapshot> {
        self.get_price_near_timestamp_with_tolerance(
            asset,
            target_ms,
            MAX_NEAR_TIMESTAMP_PAST_TOLERANCE_MS,
            MAX_NEAR_TIMESTAMP_FUTURE_TOLERANCE_MS,
        )
    }

    fn get_price_near_timestamp_with_tolerance(
        &self,
        asset: &str,
        target_ms: i64,
        max_past_tolerance_ms: i64,
        max_future_tolerance_ms: i64,
    ) -> Result<ChainlinkPriceTimestampSnapshot> {
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let state = self.state.read();
        let entry = state
            .get(symbol)
            .ok_or_else(|| self.no_cached_price_error(symbol))?;

        let mut best_before: Option<&CachedPrice> = None;
        let mut best_future: Option<&CachedPrice> = None;
        for sample in entry.ticks.iter() {
            if sample.timestamp_ms <= target_ms {
                if best_before
                    .map(|current| sample.timestamp_ms > current.timestamp_ms)
                    .unwrap_or(true)
                {
                    best_before = Some(sample);
                }
                continue;
            }

            if best_future
                .map(|current| sample.timestamp_ms < current.timestamp_ms)
                .unwrap_or(true)
            {
                best_future = Some(sample);
            }
        }
        if let Some(latest) = entry.latest.as_ref() {
            let latest_is_already_tracked = entry
                .ticks
                .back()
                .map(|tick| tick.timestamp_ms == latest.timestamp_ms)
                .unwrap_or(false);
            if !latest_is_already_tracked {
                if latest.timestamp_ms <= target_ms {
                    if best_before
                        .map(|current| latest.timestamp_ms > current.timestamp_ms)
                        .unwrap_or(true)
                    {
                        best_before = Some(latest);
                    }
                } else if best_future
                    .map(|current| latest.timestamp_ms < current.timestamp_ms)
                    .unwrap_or(true)
                {
                    best_future = Some(latest);
                }
            }
        }

        if let Some(sample) = best_before {
            let gap_ms = target_ms - sample.timestamp_ms;
            if gap_ms > max_past_tolerance_ms {
                return Err(self.build_near_timestamp_past_too_old_error(symbol, sample, gap_ms));
            }
            return Ok(ChainlinkPriceTimestampSnapshot {
                price: sample.value,
                timestamp_ms: sample.timestamp_ms,
            });
        }

        let Some(sample) = best_future else {
            return Err(anyhow!(
                "no cached price near timestamp for {symbol}: target_ms={target_ms}"
            ));
        };
        let diff_ms = sample.timestamp_ms - target_ms;
        if diff_ms > max_future_tolerance_ms {
            return Err(anyhow!(
                "no cached price near timestamp for {symbol}: first future tick is {diff_ms}ms away"
            ));
        }

        Ok(ChainlinkPriceTimestampSnapshot {
            price: sample.value,
            timestamp_ms: sample.timestamp_ms,
        })
    }

    fn get_window_stats(
        &self,
        asset: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<ChainlinkPriceWindowStats> {
        anyhow::ensure!(
            end_ms > start_ms,
            "invalid chainlink window stats range: start_ms={start_ms}, end_ms={end_ms}"
        );
        let open = self
            .get_price_near_timestamp_with_tolerance(
                asset,
                start_ms,
                PTB_START_TICK_MAX_PAST_TOLERANCE_MS,
                PTB_START_TICK_MAX_FUTURE_TOLERANCE_MS,
            )
            .context("resolving chainlink window open tick")?;
        let close = self
            .get_price_near_timestamp_with_tolerance(asset, end_ms, MAX_PRICE_AGE_SECS * 1_000, 0)
            .context("resolving chainlink window close tick")?;
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let state = self.state.read();
        let entry = state
            .get(symbol)
            .ok_or_else(|| self.no_cached_price_error(symbol))?;

        let mut samples = Vec::new();
        let mut seen_timestamps = HashSet::new();
        let mut push_sample = |timestamp_ms: i64, value: f64| {
            if !value.is_finite() || !seen_timestamps.insert(timestamp_ms) {
                return;
            }
            samples.push((timestamp_ms, value));
        };

        push_sample(open.timestamp_ms, open.price);
        for sample in entry.ticks.iter() {
            if sample.timestamp_ms < open.timestamp_ms || sample.timestamp_ms > end_ms {
                continue;
            }
            push_sample(sample.timestamp_ms, sample.value);
        }
        if let Some(latest) = entry.latest.as_ref() {
            if latest.timestamp_ms >= open.timestamp_ms && latest.timestamp_ms <= end_ms {
                push_sample(latest.timestamp_ms, latest.value);
            }
        }
        push_sample(close.timestamp_ms, close.price);
        samples.sort_by_key(|(timestamp_ms, _)| *timestamp_ms);

        let (high_price, low_price) = samples.iter().fold(
            (f64::NEG_INFINITY, f64::INFINITY),
            |(high, low), (_, value)| (high.max(*value), low.min(*value)),
        );

        Ok(ChainlinkPriceWindowStats {
            open_price: open.price,
            high_price,
            low_price,
            close_price: close.price,
            sample_count: samples.len(),
        })
    }

    fn get_price_samples(
        &self,
        asset: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<ChainlinkPriceSample>> {
        anyhow::ensure!(
            end_ms > start_ms,
            "invalid chainlink sample range: start_ms={start_ms}, end_ms={end_ms}"
        );
        let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
        let state = self.state.read();
        let entry = state
            .get(symbol)
            .ok_or_else(|| self.no_cached_price_error(symbol))?;
        let mut samples = Vec::new();
        let mut seen_timestamps = HashSet::new();
        let mut push_sample = |sample: &CachedPrice| {
            if sample.timestamp_ms < start_ms
                || sample.timestamp_ms > end_ms
                || !sample.value.is_finite()
                || !seen_timestamps.insert(sample.timestamp_ms)
            {
                return;
            }
            samples.push(ChainlinkPriceSample {
                price: sample.value,
                timestamp_ms: sample.timestamp_ms,
            });
        };
        for sample in entry.ticks.iter() {
            push_sample(sample);
        }
        if let Some(latest) = entry.latest.as_ref() {
            push_sample(latest);
        }
        samples.sort_by_key(|sample| sample.timestamp_ms);
        anyhow::ensure!(
            !samples.is_empty(),
            "no cached chainlink samples for {symbol}"
        );
        Ok(samples)
    }

    fn no_cached_price_error(&self, symbol: &str) -> anyhow::Error {
        match self.last_error.read().clone() {
            Some(last_error) => {
                anyhow!("no cached price for {symbol}; last ws error: {last_error}")
            }
            None => anyhow!("no cached price for {symbol}"),
        }
    }

    fn build_near_timestamp_past_too_old_error(
        &self,
        symbol: &str,
        cached: &CachedPrice,
        gap_ms: i64,
    ) -> anyhow::Error {
        let provider_age_ms = diff_ms(Utc::now().timestamp_millis(), cached.timestamp_ms);
        anyhow!(
            "no cached price near timestamp for {symbol}: closest past tick is {gap_ms}ms away (gap_ms={gap_ms}, provider_age_ms={provider_age_ms}, candidate_timestamp_ms={}, candidate_received_at_ms={})",
            cached.timestamp_ms,
            cached.received_at_ms,
        )
    }

    fn build_stale_price_error(
        &self,
        symbol: &str,
        cached: &CachedPrice,
        last_received_at_ms: Option<i64>,
        now_ms: i64,
    ) -> anyhow::Error {
        let provider_age_ms = diff_ms(now_ms, cached.timestamp_ms);
        let received_at_ms = last_received_at_ms.unwrap_or(cached.received_at_ms);
        let receive_age_ms = diff_ms(now_ms, received_at_ms);
        let base = format!(
            "stale price for {symbol}: {}s old (provider_age_ms={provider_age_ms}, receive_age_ms={receive_age_ms}, provider_timestamp_ms={}, received_at_ms={received_at_ms})",
            provider_age_ms / 1_000,
            cached.timestamp_ms,
        );
        match self.last_error.read().clone() {
            Some(last_error) => anyhow!("{base}; last ws error: {last_error}"),
            None => anyhow!("{base}"),
        }
    }

    fn record_stale_reason(&self, symbol: &str, reason: String) {
        if let Some(entry) = self.state.write().get_mut(symbol) {
            entry.last_stale_reason = Some(reason);
        }
    }

    fn mark_dirty_asset(&self, symbol: &str) {
        let Some(asset) = symbol_to_asset(symbol) else {
            return;
        };
        self.dirty_assets.lock().insert(asset.to_string());
        self.dirty_update_notify.notify_one();
    }

    fn take_dirty_assets(&self) -> Vec<String> {
        self.dirty_assets.lock().iter().cloned().collect()
    }

    fn clear_dirty_assets(&self, assets: &[String]) {
        if assets.is_empty() {
            return;
        }
        let asset_set: HashSet<&str> = assets.iter().map(String::as_str).collect();
        self.dirty_assets
            .lock()
            .retain(|asset| !asset_set.contains(asset.as_str()));
    }

    fn update_price(&self, payload: PricePayload, session_id: u64) {
        let received_at_ms = Utc::now().timestamp_millis();
        let provider_age_ms = diff_ms(received_at_ms, payload.timestamp);
        let stale_on_arrival_reason = (provider_age_ms > (MAX_PRICE_AGE_SECS * 1_000)).then(|| {
            format!(
                "provider timestamp stale on arrival for {} (provider_age_ms={provider_age_ms}, receive_age_ms=0, provider_timestamp_ms={}, received_at_ms={received_at_ms})",
                payload.symbol,
                payload.timestamp,
            )
        });
        let tick = CachedPrice {
            value: payload.value,
            timestamp_ms: payload.timestamp,
            received_at_ms,
        };
        let cutoff_ms = payload.timestamp - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
        let mut should_warn_stale_on_arrival = false;
        let mut state = self.state.write();
        let entry = state.entry(payload.symbol.clone()).or_default();
        entry.last_received_at_ms = Some(received_at_ms);
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
        if let Some(reason) = stale_on_arrival_reason.clone() {
            entry.last_stale_reason = Some(reason);
            should_warn_stale_on_arrival = entry
                .last_stale_warning_at_ms
                .map(|last_warn_at_ms| {
                    diff_ms(received_at_ms, last_warn_at_ms) >= STALE_PROVIDER_WARN_INTERVAL_MS
                })
                .unwrap_or(true);
            if should_warn_stale_on_arrival {
                entry.last_stale_warning_at_ms = Some(received_at_ms);
            }
        } else {
            entry.last_stale_reason = None;
            entry.last_stale_warning_at_ms = None;
        }
        drop(state);
        tracing::debug!(
            session_id,
            symbol = %payload.symbol,
            value = payload.value,
            provider_timestamp_ms = payload.timestamp,
            received_at_ms,
            provider_age_ms,
            "CHAINLINK_LIVE_DATA_WS_TICK"
        );
        self.mark_dirty_asset(&payload.symbol);
        if should_warn_stale_on_arrival {
            tracing::warn!(
                session_id,
                symbol = %payload.symbol,
                value = payload.value,
                provider_timestamp_ms = payload.timestamp,
                received_at_ms,
                provider_age_ms,
                "CHAINLINK_LIVE_DATA_WS_STALE_PROVIDER_TIMESTAMP"
            );
        }
        *self.last_error.write() = None;
    }

    fn record_error(&self, error: &anyhow::Error) {
        *self.last_error.write() = Some(error.to_string());
    }
}

fn build_subscription_message() -> String {
    json!({
        "action": "subscribe",
        "subscriptions": [{
            "topic": SUBSCRIPTION_TOPIC,
            "type": "*",
            "filters": "",
        }],
    })
    .to_string()
}

fn diff_ms(later_ms: i64, earlier_ms: i64) -> i64 {
    (later_ms - earlier_ms).max(0)
}

fn parse_named_i64_field(text: &str, field: &str) -> Option<i64> {
    let needle = format!("{field}=");
    let start = text.find(&needle)? + needle.len();
    let end = text[start..]
        .find(|ch: char| !matches!(ch, '-' | '0'..='9'))
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    text[start..end].parse().ok()
}

pub(crate) fn parse_chainlink_stale_price_details(
    error_text: &str,
) -> Option<ChainlinkStalePriceDetails> {
    if !error_text.starts_with("stale price for ") {
        return None;
    }
    Some(ChainlinkStalePriceDetails {
        provider_age_ms: parse_named_i64_field(error_text, "provider_age_ms")?,
        receive_age_ms: parse_named_i64_field(error_text, "receive_age_ms")?,
        provider_timestamp_ms: parse_named_i64_field(error_text, "provider_timestamp_ms")?,
        received_at_ms: parse_named_i64_field(error_text, "received_at_ms")?,
    })
}

pub(crate) fn parse_chainlink_near_timestamp_rejection_details(
    error_text: &str,
) -> Option<ChainlinkNearTimestampRejectionDetails> {
    if !error_text.contains("closest past tick is") {
        return None;
    }
    Some(ChainlinkNearTimestampRejectionDetails {
        gap_ms: parse_named_i64_field(error_text, "gap_ms")?,
        provider_age_ms: parse_named_i64_field(error_text, "provider_age_ms")?,
        candidate_timestamp_ms: parse_named_i64_field(error_text, "candidate_timestamp_ms")?,
        candidate_received_at_ms: parse_named_i64_field(error_text, "candidate_received_at_ms")?,
    })
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

fn symbol_to_asset(symbol: &str) -> Option<&'static str> {
    match symbol.trim().to_ascii_lowercase().as_str() {
        "btc/usd" => Some("btc"),
        "eth/usd" => Some("eth"),
        "sol/usd" => Some("sol"),
        "xrp/usd" => Some("xrp"),
        _ => None,
    }
}

fn is_supported_symbol(symbol: &str) -> bool {
    SUPPORTED_RTDS_SYMBOLS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(symbol))
}

fn build_ws_idle_timeout_error() -> anyhow::Error {
    anyhow!(
        "live data websocket idle timeout after {WS_IDLE_TIMEOUT_SECS}s without inbound text/pong"
    )
}

fn build_reconnect_requested_error() -> anyhow::Error {
    anyhow!("live data websocket reconnect requested after stale current price read")
}

fn ws_idle_deadline(last_inbound_at: tokio::time::Instant) -> tokio::time::Instant {
    last_inbound_at + Duration::from_secs(WS_IDLE_TIMEOUT_SECS)
}

async fn wait_for_ws_idle_timeout(last_inbound_at: tokio::time::Instant) -> anyhow::Error {
    tokio::time::sleep_until(ws_idle_deadline(last_inbound_at)).await;
    build_ws_idle_timeout_error()
}

async fn ws_stream_loop() {
    loop {
        let session_id = SERVICE.next_session_id();
        if let Err(err) = ws_stream_once(session_id).await {
            SERVICE.record_error(&err);
            tracing::warn!(session_id, error = %err, "CHAINLINK_LIVE_DATA_WS_ERROR");
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
        .context("sending live data websocket subscription")?;
    tracing::info!(
        session_id,
        url,
        subscribed_symbol_count = SUPPORTED_RTDS_SYMBOLS.len(),
        "CHAINLINK_LIVE_DATA_WS_CONNECTED"
    );
    SERVICE.take_reconnect_requested();

    let mut ping_interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_interval.tick().await;
    let mut last_inbound_at = tokio::time::Instant::now();

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                sink.send(Message::Text("PING".into()))
                    .await
                    .context("sending live data websocket ping")?;
                if SERVICE.take_reconnect_requested() {
                    return Err(build_reconnect_requested_error());
                }
            }
            err = wait_for_ws_idle_timeout(last_inbound_at) => {
                tracing::warn!(
                    session_id,
                    idle_timeout_secs = WS_IDLE_TIMEOUT_SECS,
                    "CHAINLINK_LIVE_DATA_WS_IDLE_TIMEOUT"
                );
                return Err(err);
            }
            message = stream.next() => {
                let Some(message) = message else {
                    return Err(anyhow!("live data websocket stream ended"));
                };
                let message = message?;
                if matches!(&message, Message::Text(_) | Message::Pong(_)) {
                    last_inbound_at = tokio::time::Instant::now();
                }
                handle_ws_message(message, session_id)?;
                if SERVICE.take_reconnect_requested() {
                    return Err(build_reconnect_requested_error());
                }
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
                            "CHAINLINK_LIVE_DATA_WS_MESSAGE_PARSE_FAILED"
                        );
                    }
                    return Ok(());
                }
            };
            if parsed.topic.as_deref() != Some(SUBSCRIPTION_TOPIC) {
                return Ok(());
            }
            if let Some(message_type) = parsed.message_type.as_deref() {
                if message_type != "update" {
                    return Ok(());
                }
            }
            let Some(payload_value) = parsed.payload else {
                return Ok(());
            };
            let payload: PricePayload = match serde_json::from_value(payload_value.clone()) {
                Ok(payload) => payload,
                Err(err) => {
                    tracing::warn!(
                        session_id,
                        error = %err,
                        outer_timestamp = parsed.timestamp,
                        payload = %payload_value,
                        "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_PAYLOAD_SHAPE"
                    );
                    return Ok(());
                }
            };
            if !is_supported_symbol(&payload.symbol) {
                if SERVICE.should_warn_unexpected_symbol(&payload.symbol) {
                    tracing::warn!(
                        session_id,
                        symbol = %payload.symbol,
                        "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_SYMBOL"
                    );
                }
                return Ok(());
            }
            if !payload.value.is_finite() || payload.value <= 0.0 {
                return Ok(());
            }
            SERVICE.update_price(payload, session_id);
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

pub(crate) fn get_chainlink_price_cached(asset: &str) -> Result<f64> {
    SERVICE.ensure_started();
    SERVICE.get_price(asset)
}

pub(crate) fn ensure_chainlink_price_stream_started() {
    SERVICE.ensure_started();
}

pub(crate) async fn wait_for_chainlink_dirty_asset_update() {
    SERVICE.ensure_started();
    SERVICE.dirty_update_notify.notified().await;
}

pub(crate) fn take_chainlink_dirty_assets() -> Vec<String> {
    SERVICE.take_dirty_assets()
}

pub(crate) fn clear_chainlink_dirty_assets(assets: &[String]) {
    SERVICE.clear_dirty_assets(assets);
}

pub(crate) fn get_chainlink_price_start_tick(
    asset: &str,
    target_ms: i64,
) -> Result<ChainlinkPriceTimestampSnapshot> {
    SERVICE.ensure_started();
    SERVICE.get_price_near_timestamp_with_tolerance(
        asset,
        target_ms,
        PTB_START_TICK_MAX_PAST_TOLERANCE_MS,
        PTB_START_TICK_MAX_FUTURE_TOLERANCE_MS,
    )
}

pub(crate) fn get_chainlink_price_near_timestamp(
    asset: &str,
    target_ms: i64,
) -> Result<ChainlinkPriceTimestampSnapshot> {
    SERVICE.ensure_started();
    SERVICE.get_price_near_timestamp_with_tolerance(asset, target_ms, 60_000, 2_000)
}

pub(crate) fn get_chainlink_price_window_stats(
    asset: &str,
    start_ms: i64,
    end_ms: i64,
) -> Result<ChainlinkPriceWindowStats> {
    SERVICE.ensure_started();
    SERVICE.get_window_stats(asset, start_ms, end_ms)
}

pub(crate) fn get_chainlink_price_samples(
    asset: &str,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ChainlinkPriceSample>> {
    SERVICE.ensure_started();
    SERVICE.get_price_samples(asset, start_ms, end_ms)
}

#[cfg(test)]
pub(crate) fn seed_chainlink_price_test_ticks(asset: &str, samples: &[(i64, f64)]) -> Result<()> {
    let symbol = asset_to_symbol(asset).ok_or_else(|| anyhow!("unsupported asset: {asset}"))?;
    let ticks = samples
        .iter()
        .map(|(timestamp_ms, value)| CachedPrice {
            value: *value,
            timestamp_ms: *timestamp_ms,
            received_at_ms: *timestamp_ms,
        })
        .collect::<VecDeque<_>>();
    let latest = ticks.back().cloned();
    SERVICE.state.write().insert(
        symbol.to_string(),
        SymbolPriceState {
            latest,
            ticks,
            last_received_at_ms: samples.last().map(|(timestamp_ms, _)| *timestamp_ms),
            last_stale_reason: None,
            last_stale_warning_at_ms: None,
        },
    );
    *SERVICE.last_error.write() = None;
    Ok(())
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
    fn update_price_marks_dirty_asset_and_clear_removes_it() {
        let service = ChainlinkPriceService::new();

        service.update_price(
            PricePayload {
                symbol: "btc/usd".to_string(),
                value: 70_505.34,
                timestamp: Utc::now().timestamp_millis(),
            },
            99,
        );

        assert_eq!(service.take_dirty_assets(), vec!["btc".to_string()]);
        service.clear_dirty_assets(&["btc".to_string()]);
        assert!(service.take_dirty_assets().is_empty());
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
        let now_ms = Utc::now().timestamp_millis();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 70_505.34,
                    timestamp_ms: now_ms - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
                    received_at_ms: now_ms - 250,
                }),
                last_received_at_ms: Some(now_ms - 250),
                ..SymbolPriceState::default()
            },
        );

        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("stale price for btc/usd"));
        assert!(err.contains("receive_age_ms="));
    }

    #[test]
    fn stale_get_price_requests_reconnect() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 70_505.34,
                    timestamp_ms: now_ms - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
                    received_at_ms: now_ms - 250,
                }),
                last_received_at_ms: Some(now_ms - 250),
                ..SymbolPriceState::default()
            },
        );

        assert!(!service.take_reconnect_requested());
        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("stale price for btc/usd"));
        assert!(service.take_reconnect_requested());
    }

    #[test]
    fn stale_reconnect_request_is_rate_limited() {
        let service = ChainlinkPriceService::new();

        assert!(service.request_reconnect_if_cooldown_elapsed("btc/usd", 31_000, 31_000, 1_000_000));
        assert!(service.take_reconnect_requested());

        assert!(!service.request_reconnect_if_cooldown_elapsed(
            "btc/usd",
            35_000,
            35_000,
            1_000_000 + ((STALE_RECONNECT_REQUEST_COOLDOWN_SECS - 1) * 1_000)
        ));
        assert!(!service.take_reconnect_requested());

        assert!(service.request_reconnect_if_cooldown_elapsed(
            "btc/usd",
            41_000,
            41_000,
            1_000_000 + (STALE_RECONNECT_REQUEST_COOLDOWN_SECS * 1_000)
        ));
        assert!(service.take_reconnect_requested());
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
                    received_at_ms: Utc::now().timestamp_millis(),
                }),
                ..SymbolPriceState::default()
            },
        );

        let price = service.get_price("eth").unwrap();
        assert_eq!(price, 2_069.351149877574);
    }

    #[test]
    fn get_price_recovers_after_fresh_tick_replaces_stale_cache() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 2_069.0,
                    timestamp_ms: now_ms - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
                    received_at_ms: now_ms - 250,
                }),
                last_received_at_ms: Some(now_ms - 250),
                ..SymbolPriceState::default()
            },
        );

        let err = service.get_price("eth").unwrap_err().to_string();
        assert!(err.contains("stale price for eth/usd"));
        assert!(service.take_reconnect_requested());

        service.update_price(
            PricePayload {
                symbol: "eth/usd".to_string(),
                value: 2_071.25,
                timestamp: Utc::now().timestamp_millis(),
            },
            7,
        );

        let price = service
            .get_price("eth")
            .expect("fresh price after new tick");
        assert_eq!(price, 2_071.25);
    }

    #[test]
    fn get_price_near_timestamp_prefers_latest_tick_before_target() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![
                    CachedPrice {
                        value: 70_010.0,
                        timestamp_ms: 1_770_000_000_000,
                        received_at_ms: 1_770_000_000_000,
                    },
                    CachedPrice {
                        value: 70_020.0,
                        timestamp_ms: 1_770_000_001_500,
                        received_at_ms: 1_770_000_001_500,
                    },
                    CachedPrice {
                        value: 70_030.0,
                        timestamp_ms: 1_770_000_003_000,
                        received_at_ms: 1_770_000_003_000,
                    },
                ]),
                ..SymbolPriceState::default()
            },
        );

        let snapshot = service
            .get_price_near_timestamp("btc", 1_770_000_002_000)
            .expect("snapshot");
        assert_eq!(
            snapshot,
            ChainlinkPriceTimestampSnapshot {
                price: 70_020.0,
                timestamp_ms: 1_770_000_001_500,
            }
        );
    }

    #[test]
    fn get_price_near_timestamp_uses_near_future_tick_when_no_prior_tick_exists() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 2_215.5,
                    timestamp_ms: 1_770_000_001_250,
                    received_at_ms: 1_770_000_001_250,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let snapshot = service
            .get_price_near_timestamp("eth", 1_770_000_000_000)
            .expect("snapshot");
        assert_eq!(
            snapshot,
            ChainlinkPriceTimestampSnapshot {
                price: 2_215.5,
                timestamp_ms: 1_770_000_001_250,
            }
        );
    }

    #[test]
    fn get_price_near_timestamp_rejects_past_tick_outside_tolerance() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 70_010.0,
                    timestamp_ms: 1_770_000_000_000,
                    received_at_ms: 1_770_000_000_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let err = service
            .get_price_near_timestamp(
                "btc",
                1_770_000_000_000 + MAX_NEAR_TIMESTAMP_PAST_TOLERANCE_MS + 1,
            )
            .unwrap_err()
            .to_string();
        assert!(err.contains("closest past tick is"));
        assert!(err.contains("gap_ms="));
        assert!(err.contains("provider_age_ms="));
        assert!(err.contains("candidate_timestamp_ms="));
        assert!(err.contains("candidate_received_at_ms="));
    }

    #[test]
    fn get_price_near_timestamp_rejects_future_tick_outside_tolerance() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "sol/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 145.0,
                    timestamp_ms: 1_770_000_005_000,
                    received_at_ms: 1_770_000_005_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let err = service
            .get_price_near_timestamp("sol", 1_770_000_000_000)
            .unwrap_err()
            .to_string();
        assert!(err.contains("first future tick is"));
    }

    #[test]
    fn get_price_near_timestamp_with_tolerance_uses_short_past_window_for_ptb() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 70_010.0,
                    timestamp_ms: 1_770_000_000_000,
                    received_at_ms: 1_770_000_000_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let snapshot = service
            .get_price_near_timestamp_with_tolerance(
                "btc",
                1_770_000_001_500,
                PTB_START_TICK_MAX_PAST_TOLERANCE_MS,
                PTB_START_TICK_MAX_FUTURE_TOLERANCE_MS,
            )
            .expect("snapshot");
        assert_eq!(
            snapshot,
            ChainlinkPriceTimestampSnapshot {
                price: 70_010.0,
                timestamp_ms: 1_770_000_000_000,
            }
        );
    }

    #[test]
    fn get_window_stats_uses_open_high_low_close_inside_range() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 70_120.0,
                    timestamp_ms: 1_600,
                    received_at_ms: 1_600,
                }),
                ticks: VecDeque::from(vec![
                    CachedPrice {
                        value: 70_000.0,
                        timestamp_ms: 1_000,
                        received_at_ms: 1_000,
                    },
                    CachedPrice {
                        value: 70_050.0,
                        timestamp_ms: 1_100,
                        received_at_ms: 1_100,
                    },
                    CachedPrice {
                        value: 69_980.0,
                        timestamp_ms: 1_250,
                        received_at_ms: 1_250,
                    },
                    CachedPrice {
                        value: 70_120.0,
                        timestamp_ms: 1_450,
                        received_at_ms: 1_450,
                    },
                    CachedPrice {
                        value: 70_090.0,
                        timestamp_ms: 1_500,
                        received_at_ms: 1_500,
                    },
                ]),
                ..SymbolPriceState::default()
            },
        );

        let stats = service
            .get_window_stats("btc", 1_000, 1_500)
            .expect("window stats");
        assert_eq!(
            stats,
            ChainlinkPriceWindowStats {
                open_price: 70_000.0,
                high_price: 70_120.0,
                low_price: 69_980.0,
                close_price: 70_090.0,
                sample_count: 5,
            }
        );
    }

    #[test]
    fn get_window_stats_requires_close_tick_not_too_old() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "eth/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![
                    CachedPrice {
                        value: 2_000.0,
                        timestamp_ms: 100_000,
                        received_at_ms: 100_000,
                    },
                    CachedPrice {
                        value: 2_030.0,
                        timestamp_ms: 120_000,
                        received_at_ms: 120_000,
                    },
                ]),
                latest: Some(CachedPrice {
                    value: 2_030.0,
                    timestamp_ms: 120_000,
                    received_at_ms: 120_000,
                }),
                ..SymbolPriceState::default()
            },
        );

        let err = service
            .get_window_stats("eth", 100_000, 200_000)
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("resolving chainlink window close tick"));
        assert!(err
            .chain()
            .any(|source| source.to_string().contains("closest past tick is")));
    }

    #[test]
    fn get_price_near_timestamp_with_tolerance_rejects_old_tick_for_ptb() {
        let service = ChainlinkPriceService::new();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 70_010.0,
                    timestamp_ms: 1_770_000_000_000,
                    received_at_ms: 1_770_000_000_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        let err = service
            .get_price_near_timestamp_with_tolerance(
                "btc",
                1_770_000_000_000 + PTB_START_TICK_MAX_PAST_TOLERANCE_MS + 1,
                PTB_START_TICK_MAX_PAST_TOLERANCE_MS,
                PTB_START_TICK_MAX_FUTURE_TOLERANCE_MS,
            )
            .unwrap_err()
            .to_string();
        assert!(err.contains("closest past tick is"));
        assert!(err.contains("gap_ms="));
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
                    received_at_ms: 1_770_000_005_000,
                }),
                ticks: VecDeque::from(vec![CachedPrice {
                    value: 2_150.0,
                    timestamp_ms: 1_770_000_005_000,
                    received_at_ms: 1_770_000_005_000,
                }]),
                ..SymbolPriceState::default()
            },
        );

        service.update_price(
            PricePayload {
                symbol: "eth/usd".to_string(),
                value: 2_141.25,
                timestamp: 1_770_000_003_000,
            },
            42,
        );

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

    #[test]
    fn stale_price_error_includes_provider_and_receive_age_details() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();
        service.state.write().insert(
            "btc/usd".to_string(),
            SymbolPriceState {
                latest: Some(CachedPrice {
                    value: 70_505.34,
                    timestamp_ms: now_ms - ((MAX_PRICE_AGE_SECS + 1) * 1_000),
                    received_at_ms: now_ms - 250,
                }),
                last_received_at_ms: Some(now_ms - 250),
                ..SymbolPriceState::default()
            },
        );

        let err = service.get_price("btc").unwrap_err().to_string();
        assert!(err.contains("stale price for btc/usd"));
        assert!(err.contains("provider_age_ms="));
        assert!(err.contains("receive_age_ms="));
        assert!(err.contains("provider_timestamp_ms="));
        assert!(err.contains("received_at_ms="));
    }

    #[test]
    fn parse_chainlink_stale_price_details_extracts_structured_fields() {
        let details = parse_chainlink_stale_price_details(
            "stale price for btc/usd: 123s old (provider_age_ms=123000, receive_age_ms=250, provider_timestamp_ms=1774000000000, received_at_ms=1774000122750)",
        )
        .expect("structured stale detail");
        assert_eq!(details.provider_age_ms, 123000);
        assert_eq!(details.receive_age_ms, 250);
        assert_eq!(details.provider_timestamp_ms, 1774000000000);
        assert_eq!(details.received_at_ms, 1774000122750);
    }

    #[test]
    fn parse_chainlink_near_timestamp_rejection_details_extracts_structured_fields() {
        let details = parse_chainlink_near_timestamp_rejection_details(
            "no cached price near timestamp for btc/usd: closest past tick is 217000ms away (gap_ms=217000, provider_age_ms=216937, candidate_timestamp_ms=1774012890000, candidate_received_at_ms=1774013106847)",
        )
        .expect("structured near-timestamp detail");
        assert_eq!(details.gap_ms, 217000);
        assert_eq!(details.provider_age_ms, 216937);
        assert_eq!(details.candidate_timestamp_ms, 1774012890000);
        assert_eq!(details.candidate_received_at_ms, 1774013106847);
    }

    #[test]
    fn build_subscription_message_uses_single_empty_filter_subscription() {
        let value: Value = serde_json::from_str(&build_subscription_message()).expect("json");
        let subs = value
            .get("subscriptions")
            .and_then(Value::as_array)
            .expect("subscriptions");
        assert_eq!(
            value.get("action").and_then(Value::as_str),
            Some("subscribe")
        );
        assert_eq!(subs.len(), 1);
        assert_eq!(
            subs[0].get("topic").and_then(Value::as_str),
            Some(SUBSCRIPTION_TOPIC)
        );
        assert_eq!(subs[0].get("type").and_then(Value::as_str), Some("*"));
        assert_eq!(subs[0].get("filters").and_then(Value::as_str), Some(""));
    }

    #[test]
    fn is_supported_symbol_matches_rtds_symbol_list_case_insensitively() {
        assert!(is_supported_symbol("eth/usd"));
        assert!(is_supported_symbol("BTC/USD"));
        assert!(!is_supported_symbol("ethUsd"));
        assert!(!is_supported_symbol("doge/usd"));
    }

    #[test]
    fn ws_idle_deadline_uses_configured_window() {
        let last_inbound_at = tokio::time::Instant::now();
        let deadline = ws_idle_deadline(last_inbound_at);
        assert_eq!(
            deadline.duration_since(last_inbound_at).as_secs(),
            WS_IDLE_TIMEOUT_SECS
        );
    }

    #[test]
    fn ws_idle_timeout_error_mentions_text_pong_window() {
        let err = build_ws_idle_timeout_error();
        assert!(err
            .to_string()
            .contains("live data websocket idle timeout after"));
        assert!(err.to_string().contains("text/pong"));
    }
}
