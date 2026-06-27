#![cfg_attr(test, allow(dead_code))]

use anyhow::{anyhow, Result};
use chrono::Utc;
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        LazyLock,
    },
};

use super::polymarket_live_data_stream::ensure_polymarket_live_data_stream_started;
use super::polymarket_live_data_ws::live_data_ws_error_cache_summary;

const MAX_TICK_HISTORY_AGE_SECS: i64 = 10 * 60;
const MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL: usize = 5_000;
pub(crate) const SUPPORTED_BINANCE_SYMBOLS: &[&str] = &[
    "btcusdt", "ethusdt", "solusdt", "xrpusdt", "dogeusdt", "bnbusdt",
];

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
    last_successful_tick_at_ms: AtomicI64,
    last_proxy_mode: RwLock<Option<String>>,
}

static SERVICE: LazyLock<BinancePriceService> = LazyLock::new(BinancePriceService::new);

impl BinancePriceService {
    fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            last_error: RwLock::new(None),
            warned_unexpected_symbols: RwLock::new(HashSet::new()),
            started: AtomicBool::new(false),
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

    fn update_price(&self, symbol: &str, value: f64, timestamp_ms: i64, session_id: u64) {
        let received_at_ms = Utc::now().timestamp_millis();
        let tick = CachedPrice {
            value,
            timestamp_ms,
        };
        let cutoff_ms = timestamp_ms - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
        let mut state = self.state.write();
        let entry = state.entry(symbol.to_string()).or_default();
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
        let sample_count = entry.ticks.len();
        drop(state);
        tracing::trace!(
            session_id,
            symbol,
            value,
            provider_timestamp_ms = timestamp_ms,
            received_at_ms,
            provider_age_ms = diff_ms(received_at_ms, timestamp_ms),
            sample_count,
            "BINANCE_LIVE_DATA_WS_TICK"
        );
        self.last_successful_tick_at_ms
            .store(received_at_ms, Ordering::SeqCst);
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
        *self.last_error.write() = Some(live_data_ws_error_cache_summary(error));
    }

    fn record_proxy_mode(&self, proxy_mode: &str) {
        *self.last_proxy_mode.write() = Some(proxy_mode.to_string());
    }

    fn last_successful_tick_at_ms(&self) -> i64 {
        self.last_successful_tick_at_ms.load(Ordering::SeqCst)
    }
}

pub(crate) fn ingest_binance_live_data_price(
    symbol: &str,
    value: f64,
    timestamp_ms: i64,
    session_id: u64,
) -> bool {
    if !is_supported_symbol(symbol) {
        if SERVICE.should_warn_unexpected_symbol(symbol) {
            tracing::warn!(session_id, symbol, "BINANCE_LIVE_DATA_WS_UNEXPECTED_SYMBOL");
        }
        return false;
    }
    if !value.is_finite() || value <= 0.0 {
        return false;
    }
    SERVICE.update_price(symbol, value, timestamp_ms, session_id);
    true
}

pub(crate) fn record_binance_live_data_error(error: &anyhow::Error) {
    SERVICE.record_error(error);
}

pub(crate) fn record_binance_live_data_proxy_mode(proxy_mode: &str) {
    SERVICE.record_proxy_mode(proxy_mode);
}

pub(crate) fn binance_live_data_last_successful_tick_at_ms() -> i64 {
    SERVICE.last_successful_tick_at_ms()
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
        "doge" | "dogecoin" => Some("dogeusdt"),
        "bnb" => Some("bnbusdt"),
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
        assert_eq!(asset_to_symbol("doge"), Some("dogeusdt"));
        assert_eq!(asset_to_symbol("dogecoin"), Some("dogeusdt"));
        assert_eq!(asset_to_symbol("bnb"), Some("bnbusdt"));
        assert_eq!(asset_to_symbol("hype"), None);
    }

    #[test]
    fn update_price_makes_snapshot_available() {
        let service = BinancePriceService::new();
        service.update_price("btcusdt", 70_505.34, 1_000, 99);

        let snapshot = service.snapshot("btc", 1_250).expect("snapshot");
        assert_eq!(snapshot.price, 70_505.34);
        assert_eq!(snapshot.timestamp_ms, 1_000);
        assert_eq!(snapshot.staleness_ms, 250);
    }
}
