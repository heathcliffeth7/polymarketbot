use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        LazyLock,
    },
};
use tokio::sync::Notify;

use super::chainlink_symbols::{asset_to_symbol, is_supported_symbol, symbol_to_asset};
use super::polymarket_live_data_stream::{
    ensure_polymarket_live_data_stream_started, watch_chainlink_live_data_symbol,
};
use super::polymarket_live_data_ws::{
    classify_live_data_ws_error_text, live_data_ws_error_cache_summary,
};

mod history;
mod provider_gap;
pub(crate) use history::{
    ingest_chainlink_live_data_price_history, ChainlinkLiveDataHistorySample,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainlinkPriceSampleReadiness {
    pub(crate) asset: String,
    pub(crate) symbol: Option<String>,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
    pub(crate) sample_count: Option<usize>,
    pub(crate) delta_count: Option<usize>,
    pub(crate) last_symbol_tick_age_ms: Option<i64>,
    pub(crate) last_symbol_received_age_ms: Option<i64>,
    pub(crate) last_ws_error_class: Option<String>,
    pub(crate) last_ws_http_status: Option<u16>,
    pub(crate) proxy_mode: Option<String>,
    pub(crate) error: Option<String>,
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
    last_successful_tick_at_ms: AtomicI64,
    last_proxy_mode: RwLock<Option<String>>,
}

static SERVICE: LazyLock<ChainlinkPriceService> = LazyLock::new(ChainlinkPriceService::new);

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
            last_successful_tick_at_ms: AtomicI64::new(0),
            last_proxy_mode: RwLock::new(None),
        }
    }

    fn ensure_started(&self) {
        if !self.started.swap(true, Ordering::SeqCst) {
            ensure_polymarket_live_data_stream_started();
        }
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
        let Some(entry) = state.get(symbol) else {
            drop(state);
            let error = self.no_cached_price_error(symbol);
            self.request_sample_cache_miss_reconnect(
                symbol,
                &error.to_string(),
                start_ms,
                end_ms,
                Utc::now().timestamp_millis(),
            );
            return Err(error);
        };
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
        drop(state);
        if samples.is_empty() {
            let error = anyhow!("no cached chainlink samples for {symbol}");
            self.request_sample_cache_miss_reconnect(
                symbol,
                &error.to_string(),
                start_ms,
                end_ms,
                Utc::now().timestamp_millis(),
            );
            return Err(error);
        }
        Ok(samples)
    }

    fn sample_readiness(
        &self,
        asset: &str,
        start_ms: i64,
        end_ms: i64,
        now_ms: i64,
    ) -> ChainlinkPriceSampleReadiness {
        let symbol = asset_to_symbol(asset).map(ToString::to_string);
        let ws_diagnostics = chainlink_live_data_ws_diagnostics();
        let mut readiness = ChainlinkPriceSampleReadiness {
            asset: asset.to_string(),
            symbol: symbol.clone(),
            start_ms,
            end_ms,
            sample_count: None,
            delta_count: None,
            last_symbol_tick_age_ms: None,
            last_symbol_received_age_ms: None,
            last_ws_error_class: ws_diagnostics.last_ws_error_class,
            last_ws_http_status: ws_diagnostics.last_ws_http_status,
            proxy_mode: ws_diagnostics.proxy_mode,
            error: None,
        };
        let Some(symbol) = symbol else {
            readiness.error = Some(format!("unsupported asset: {asset}"));
            return readiness;
        };
        let state = self.state.read();
        let Some(entry) = state.get(symbol.as_str()) else {
            readiness.error = Some(self.no_cached_price_error(symbol.as_str()).to_string());
            return readiness;
        };
        let mut seen_timestamps = HashSet::new();
        let mut sample_count = 0usize;
        for sample in entry.ticks.iter().chain(entry.latest.iter()) {
            if sample.timestamp_ms >= start_ms
                && sample.timestamp_ms <= end_ms
                && sample.value.is_finite()
                && seen_timestamps.insert(sample.timestamp_ms)
            {
                sample_count += 1;
            }
        }
        readiness.sample_count = Some(sample_count);
        readiness.delta_count = Some(sample_count.saturating_sub(1));
        readiness.last_symbol_tick_age_ms = entry
            .latest
            .as_ref()
            .map(|latest| diff_ms(now_ms, latest.timestamp_ms));
        readiness.last_symbol_received_age_ms = entry
            .last_received_at_ms
            .map(|received_at_ms| diff_ms(now_ms, received_at_ms));
        if sample_count == 0 {
            readiness.error = Some(format!("no cached chainlink samples for {symbol}"));
        }
        readiness
    }

    fn request_sample_cache_miss_reconnect(
        &self,
        symbol: &str,
        error: &str,
        start_ms: i64,
        end_ms: i64,
        now_ms: i64,
    ) {
        if self.request_reconnect_if_cooldown_elapsed(symbol, 0, 0, now_ms) {
            tracing::warn!(
                symbol,
                sample_window_start_ms = start_ms,
                sample_window_end_ms = end_ms,
                error,
                "CHAINLINK_SAMPLE_CACHE_MISS_RECONNECT_REQUESTED"
            );
        }
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

    fn update_price(&self, symbol: &str, value: f64, timestamp_ms: i64, session_id: u64) {
        let received_at_ms = Utc::now().timestamp_millis();
        let provider_age_ms = diff_ms(received_at_ms, timestamp_ms);
        let stale_on_arrival_reason = (provider_age_ms > (MAX_PRICE_AGE_SECS * 1_000)).then(|| {
            format!(
                "provider timestamp stale on arrival for {} (provider_age_ms={provider_age_ms}, receive_age_ms=0, provider_timestamp_ms={}, received_at_ms={received_at_ms})",
                symbol,
                timestamp_ms,
            )
        });
        let tick = CachedPrice {
            value,
            timestamp_ms,
            received_at_ms,
        };
        let cutoff_ms = timestamp_ms - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
        let mut should_warn_stale_on_arrival = false;
        let mut state = self.state.write();
        let entry = state.entry(symbol.to_string()).or_default();
        entry.last_received_at_ms = Some(received_at_ms);
        if entry
            .latest
            .as_ref()
            .map(|latest| tick.timestamp_ms >= latest.timestamp_ms)
            .unwrap_or(true)
        {
            entry.latest = Some(tick.clone());
        }
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
        let sample_count = entry.ticks.len();
        drop(state);
        tracing::trace!(
            session_id,
            symbol,
            value,
            provider_timestamp_ms = timestamp_ms,
            received_at_ms,
            provider_age_ms,
            sample_count,
            "CHAINLINK_LIVE_DATA_WS_TICK"
        );
        provider_gap::record_chainlink_provider_tick_gap(
            symbol,
            timestamp_ms,
            received_at_ms,
            session_id,
        );
        self.last_successful_tick_at_ms
            .store(received_at_ms, Ordering::SeqCst);
        self.mark_dirty_asset(symbol);
        if should_warn_stale_on_arrival {
            tracing::warn!(
                session_id,
                symbol,
                value,
                provider_timestamp_ms = timestamp_ms,
                received_at_ms,
                provider_age_ms,
                "CHAINLINK_LIVE_DATA_WS_STALE_PROVIDER_TIMESTAMP"
            );
        }
        *self.last_error.write() = None;
    }

    fn record_error(&self, error: &anyhow::Error) {
        *self.last_error.write() = Some(live_data_ws_error_cache_summary(error));
    }

    fn record_proxy_mode(&self, proxy_mode: &str) {
        *self.last_proxy_mode.write() = Some(proxy_mode.to_string());
    }

    fn last_successful_tick_at_ms(&self) -> i64 {
        self.last_successful_tick_at_ms.load(Ordering::SeqCst)
    }
}

pub(crate) fn ingest_chainlink_live_data_price(
    symbol: &str,
    value: f64,
    timestamp_ms: i64,
    session_id: u64,
) -> bool {
    if !is_supported_symbol(symbol) {
        if SERVICE.should_warn_unexpected_symbol(symbol) {
            tracing::warn!(
                session_id,
                symbol,
                "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_SYMBOL"
            );
        }
        return false;
    }
    if !value.is_finite() || value <= 0.0 {
        return false;
    }
    SERVICE.update_price(symbol, value, timestamp_ms, session_id);
    true
}

pub(crate) fn record_chainlink_live_data_error(error: &anyhow::Error) {
    SERVICE.record_error(error);
}

pub(crate) fn record_chainlink_live_data_proxy_mode(proxy_mode: &str) {
    SERVICE.record_proxy_mode(proxy_mode);
}

pub(crate) fn chainlink_live_data_last_successful_tick_at_ms() -> i64 {
    SERVICE.last_successful_tick_at_ms()
}

pub(crate) fn take_chainlink_live_data_reconnect_requested() -> bool {
    SERVICE.take_reconnect_requested()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainlinkLiveDataWsDiagnostics {
    pub(crate) last_ws_error_class: Option<String>,
    pub(crate) last_ws_http_status: Option<u16>,
    pub(crate) last_successful_tick_age_ms: Option<i64>,
    pub(crate) proxy_mode: Option<String>,
}

pub(crate) fn chainlink_live_data_ws_diagnostics() -> ChainlinkLiveDataWsDiagnostics {
    let last_error = SERVICE.last_error.read().clone();
    let error_info = last_error.as_deref().map(classify_live_data_ws_error_text);
    let last_successful_tick_at_ms = SERVICE.last_successful_tick_at_ms();
    let last_successful_tick_age_ms = (last_successful_tick_at_ms > 0)
        .then(|| diff_ms(Utc::now().timestamp_millis(), last_successful_tick_at_ms));
    ChainlinkLiveDataWsDiagnostics {
        last_ws_error_class: error_info.as_ref().map(|info| info.error_class.to_string()),
        last_ws_http_status: error_info.as_ref().and_then(|info| info.http_status),
        last_successful_tick_age_ms,
        proxy_mode: error_info
            .and_then(|info| info.proxy_mode.map(ToString::to_string))
            .or_else(|| SERVICE.last_proxy_mode.read().clone()),
    }
}

pub(crate) fn chainlink_symbol_for_asset(asset: &str) -> Option<&'static str> {
    asset_to_symbol(asset)
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

pub(crate) fn get_chainlink_price_cached(asset: &str) -> Result<f64> {
    if let Some(symbol) = asset_to_symbol(asset) {
        watch_chainlink_live_data_symbol(symbol);
    }
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
    if let Some(symbol) = asset_to_symbol(asset) {
        watch_chainlink_live_data_symbol(symbol);
    }
    SERVICE.ensure_started();
    SERVICE.get_price_samples(asset, start_ms, end_ms)
}

pub(crate) fn chainlink_price_sample_readiness(
    asset: &str,
    start_ms: i64,
    end_ms: i64,
) -> ChainlinkPriceSampleReadiness {
    if let Some(symbol) = asset_to_symbol(asset) {
        watch_chainlink_live_data_symbol(symbol);
    }
    SERVICE.ensure_started();
    SERVICE.sample_readiness(asset, start_ms, end_ms, Utc::now().timestamp_millis())
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
pub(crate) fn clear_chainlink_price_test_state() {
    SERVICE.state.write().clear();
    *SERVICE.last_error.write() = None;
    *SERVICE.last_proxy_mode.write() = None;
    SERVICE
        .last_successful_tick_at_ms
        .store(0, Ordering::SeqCst);
    SERVICE.reconnect_requested.store(false, Ordering::SeqCst);
    SERVICE
        .last_reconnect_request_at_ms
        .store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn get_chainlink_price_test_snapshot(asset: &str) -> Result<f64> {
    SERVICE.get_price(asset)
}

#[cfg(test)]
mod tests {
    use super::super::chainlink_symbols::is_supported_symbol;
    use super::*;

    #[test]
    fn update_price_marks_dirty_asset_and_clear_removes_it() {
        let service = ChainlinkPriceService::new();

        service.update_price("btc/usd", 70_505.34, Utc::now().timestamp_millis(), 99);

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
    fn get_price_samples_cache_miss_requests_one_reconnect_within_cooldown() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();

        let err = service
            .get_price_samples("sol", now_ms - 45_000, now_ms)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no cached price for sol/usd"));
        assert!(service.take_reconnect_requested());

        let second_err = service
            .get_price_samples("sol", now_ms - 45_000, now_ms)
            .unwrap_err()
            .to_string();
        assert!(second_err.contains("no cached price for sol/usd"));
        assert!(!service.take_reconnect_requested());
    }

    #[test]
    fn sample_readiness_reports_symbol_counts_and_exact_cache_miss() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();

        let empty = service.sample_readiness("sol", now_ms - 45_000, now_ms, now_ms);
        assert_eq!(empty.asset, "sol");
        assert_eq!(empty.symbol.as_deref(), Some("sol/usd"));
        assert_eq!(empty.sample_count, None);
        assert_eq!(empty.delta_count, None);
        assert_eq!(empty.error.as_deref(), Some("no cached price for sol/usd"));

        service.update_price("sol/usd", 65.25, now_ms - 500, 1);
        let ready = service.sample_readiness("sol", now_ms - 45_000, now_ms, now_ms);
        assert_eq!(ready.sample_count, Some(1));
        assert_eq!(ready.delta_count, Some(0));
        assert_eq!(ready.last_symbol_tick_age_ms, Some(500));
        assert_eq!(ready.last_symbol_received_age_ms, Some(0));
        assert_eq!(ready.error, None);
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

        service.update_price("eth/usd", 2_071.25, Utc::now().timestamp_millis(), 7);

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
    fn update_price_does_not_move_latest_backward() {
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

        service.update_price("eth/usd", 2_141.25, 1_770_000_003_000, 42);

        let state = service.state.read();
        let entry = state.get("eth/usd").expect("eth state");
        let latest = entry.latest.as_ref().expect("latest");
        assert_eq!(latest.value, 2_150.0);
        assert_eq!(latest.timestamp_ms, 1_770_000_005_000);
        assert!(entry.last_received_at_ms.is_some());
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
    fn is_supported_symbol_matches_rtds_symbol_list_case_insensitively() {
        assert!(is_supported_symbol("eth/usd"));
        assert!(is_supported_symbol("BTC/USD"));
        assert!(is_supported_symbol("doge/usd"));
        assert!(!is_supported_symbol("ethUsd"));
        assert!(!is_supported_symbol("dogeusd"));
    }
}
