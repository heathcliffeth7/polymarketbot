use super::{
    binance::{binance_stream_url, parse_binance_ws_payload},
    bybit::{bybit_subscription_message, bybit_ws_url, parse_bybit_ws_payload},
    coinbase::{coinbase_subscription_messages, coinbase_ws_url, parse_coinbase_ws_payload},
    hyperliquid::{
        hyperliquid_subscription_message, hyperliquid_ws_url, parse_hyperliquid_ws_payload,
    },
    open_backfill::fetch_cex_window_open_book,
    types::{
        CexBookSample, CexConsensusSnapshot, CexCurrentPriceSnapshot, CexImpulseSnapshot,
        CexSourceSnapshot, CexTradeSample, CexVenue, CexVenueDeltaSnapshot, TakerSide,
    },
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::LazyLock,
    time::Duration,
};
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

const MAX_SAMPLE_AGE_MS: i64 = 5 * 60 * 1_000;
const MAX_BOOK_SAMPLES: usize = 4_000;
const MAX_TRADE_SAMPLES: usize = 20_000;
const MAX_SKEW_SAMPLES: usize = 4_000;
const MAX_WINDOW_OPEN_BOOK_AGE_MS: i64 = 10_000;
const OPEN_BACKFILL_FAIL_COOLDOWN_MS: i64 = 30_000;
const RECONNECT_DELAY_MS: u64 = 500;
const WS_IDLE_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone)]
pub(crate) struct CexMicrostructureSnapshotConfig {
    pub(crate) impulse_window_ms: i64,
    pub(crate) min_move_usd: f64,
    pub(crate) min_velocity_usd_per_sec: f64,
    pub(crate) min_taker_imbalance: f64,
    pub(crate) source_skew_baseline_window_ms: i64,
    pub(crate) max_book_stale_ms: i64,
    pub(crate) max_trade_stale_ms: i64,
    pub(crate) max_ticker_stale_ms: i64,
}

impl Default for CexMicrostructureSnapshotConfig {
    fn default() -> Self {
        Self {
            impulse_window_ms: 15_000,
            min_move_usd: 12.0,
            min_velocity_usd_per_sec: 0.50,
            min_taker_imbalance: 0.58,
            source_skew_baseline_window_ms: 60_000,
            max_book_stale_ms: 750,
            max_trade_stale_ms: 1_000,
            max_ticker_stale_ms: 750,
        }
    }
}

#[derive(Debug, Default)]
struct VenueState {
    latest_book: Option<CexBookSample>,
    latest_ticker_timestamp_ms: Option<i64>,
    pinned_window_opens: HashMap<i64, CexBookSample>,
    books: VecDeque<CexBookSample>,
    trades: VecDeque<CexTradeSample>,
}

#[derive(Debug, Default)]
struct AssetState {
    binance: VenueState,
    coinbase: VenueState,
    hyperliquid: VenueState,
    bybit: VenueState,
    skew_samples: VecDeque<(i64, f64)>,
}

struct CexMicrostructureService {
    state: RwLock<HashMap<String, AssetState>>,
    started_assets: RwLock<HashSet<String>>,
    open_backfill_inflight: Mutex<HashSet<String>>,
    open_backfill_fail_cooldown_until_ms: Mutex<HashMap<String, i64>>,
    dirty_assets: Mutex<HashSet<String>>,
    dirty_update_notify: Notify,
}

static SERVICE: LazyLock<CexMicrostructureService> = LazyLock::new(CexMicrostructureService::new);

impl CexMicrostructureService {
    fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            started_assets: RwLock::new(HashSet::new()),
            open_backfill_inflight: Mutex::new(HashSet::new()),
            open_backfill_fail_cooldown_until_ms: Mutex::new(HashMap::new()),
            dirty_assets: Mutex::new(HashSet::new()),
            dirty_update_notify: Notify::new(),
        }
    }

    fn ensure_started(&self, asset: &str) {
        let asset = asset.trim().to_ascii_lowercase();
        if asset.is_empty() {
            return;
        }
        let start_binance = binance_stream_url(&asset).is_some();
        let start_coinbase = coinbase_subscription_messages(&asset).is_some();
        let start_hyperliquid = hyperliquid_subscription_message(&asset).is_some();
        let start_bybit = bybit_subscription_message(&asset).is_some();
        if !start_binance && !start_coinbase && !start_hyperliquid && !start_bybit {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        {
            let mut started = self.started_assets.write();
            if !started.insert(asset.clone()) {
                return;
            }
        }
        if start_binance {
            handle.spawn(binance_loop(asset.clone()));
        }
        if start_coinbase {
            handle.spawn(coinbase_loop(asset.clone()));
        }
        if start_hyperliquid {
            handle.spawn(hyperliquid_loop(asset.clone()));
        }
        if start_bybit {
            handle.spawn(bybit_loop(asset));
        }
    }

    fn update_book(&self, book: CexBookSample) {
        let asset = book.asset.clone();
        let mut state = self.state.write();
        let entry = state.entry(asset.clone()).or_default();
        let venue = venue_state_mut(entry, book.venue);
        let book = merge_book_update(venue.latest_book.as_ref(), book);
        venue.latest_ticker_timestamp_ms = if matches!(
            book.source,
            "bookTicker" | "ticker" | "l2Book" | "orderbook.1"
        ) {
            Some(book.timestamp_ms)
        } else {
            venue.latest_ticker_timestamp_ms
        };
        venue.latest_book = Some(book.clone());
        venue.books.push_back(book);
        trim_books(&mut venue.books);
        record_skew_sample(entry);
        self.mark_dirty_asset(&asset);
    }

    fn update_trade(&self, trade: CexTradeSample) {
        let asset = trade.asset.clone();
        let mut state = self.state.write();
        let entry = state.entry(asset).or_default();
        let venue = venue_state_mut(entry, trade.venue);
        venue.trades.push_back(trade);
        trim_trades(&mut venue.trades);
    }

    fn mark_dirty_asset(&self, asset: &str) {
        self.dirty_assets.lock().insert(asset.to_string());
        self.dirty_update_notify.notify_one();
    }

    async fn wait_for_dirty_asset_update(&self) {
        self.dirty_update_notify.notified().await;
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

    fn snapshot(
        &self,
        asset: &str,
        now_ms: i64,
        config: &CexMicrostructureSnapshotConfig,
    ) -> Result<CexConsensusSnapshot> {
        let normalized_asset = asset.trim().to_ascii_lowercase();
        let state = self.state.read();
        let entry = state
            .get(&normalized_asset)
            .ok_or_else(|| anyhow!("cex source missing for asset={normalized_asset}"))?;
        let binance =
            source_snapshot(CexVenue::Binance, &entry.binance, now_ms, config, "binance")?;
        let coinbase = source_snapshot(
            CexVenue::Coinbase,
            &entry.coinbase,
            now_ms,
            config,
            "coinbase",
        )?;
        let consensus_side =
            if binance.impulse.side.is_some() && binance.impulse.side == coinbase.impulse.side {
                binance.impulse.side
            } else {
                None
            };
        let source_skew_usd = binance.mid - coinbase.mid;
        let baseline_source_skew_usd = median_recent_skew(
            &entry.skew_samples,
            now_ms,
            config.source_skew_baseline_window_ms,
        );
        let normalized_source_skew_usd =
            source_skew_usd - baseline_source_skew_usd.unwrap_or(source_skew_usd);

        Ok(CexConsensusSnapshot {
            asset: normalized_asset,
            spot_mid: (binance.mid + coinbase.mid) / 2.0,
            binance,
            coinbase,
            consensus_side,
            source_skew_usd,
            baseline_source_skew_usd,
            normalized_source_skew_usd,
        })
    }

    fn current_price_snapshot(
        &self,
        asset: &str,
        venue: CexVenue,
        now_ms: i64,
        config: &CexMicrostructureSnapshotConfig,
    ) -> Result<CexCurrentPriceSnapshot> {
        let normalized_asset = asset.trim().to_ascii_lowercase();
        let state = self.state.read();
        let entry = state
            .get(&normalized_asset)
            .ok_or_else(|| anyhow!("cex source missing for asset={normalized_asset}"))?;
        current_price_snapshot(
            venue,
            venue_state(entry, venue),
            now_ms,
            config,
            venue.as_str(),
        )
    }

    fn venue_delta_snapshot(
        &self,
        asset: &str,
        venue: CexVenue,
        window_start_ms: i64,
        now_ms: i64,
        min_move_usd: f64,
        max_book_stale_ms: i64,
    ) -> Result<CexVenueDeltaSnapshot> {
        let normalized_asset = asset.trim().to_ascii_lowercase();
        let state = self.state.read();
        let entry = state
            .get(&normalized_asset)
            .ok_or_else(|| anyhow!("cex source missing for asset={normalized_asset}"))?;
        let result = venue_delta_snapshot(
            venue,
            venue_state(entry, venue),
            window_start_ms,
            now_ms,
            min_move_usd,
            max_book_stale_ms,
            venue.as_str(),
        );
        let needs_open_backfill = result
            .as_ref()
            .err()
            .is_some_and(|err| err.to_string().contains("window open book missing"));
        drop(state);
        if needs_open_backfill {
            self.schedule_window_open_backfill(normalized_asset, venue, window_start_ms);
        }
        result
    }

    fn has_pinned_rest_window_open(
        &self,
        asset: &str,
        venue: CexVenue,
        window_start_ms: i64,
    ) -> bool {
        let state = self.state.read();
        let Some(entry) = state.get(asset) else {
            return false;
        };
        window_open_book(venue, venue_state(entry, venue), window_start_ms).is_some()
    }

    fn schedule_window_open_backfill(&self, asset: String, venue: CexVenue, window_start_ms: i64) {
        if matches!(venue, CexVenue::Hyperliquid) {
            return;
        }
        let key = format!("{}:{}:{}", asset, venue.as_str(), window_start_ms);
        if self.has_pinned_rest_window_open(&asset, venue, window_start_ms) {
            return;
        }
        let now_ms = Utc::now().timestamp_millis();
        if self
            .open_backfill_fail_cooldown_until_ms
            .lock()
            .get(&key)
            .is_some_and(|until_ms| now_ms < *until_ms)
        {
            return;
        }
        {
            let mut inflight = self.open_backfill_inflight.lock();
            if !inflight.insert(key.clone()) {
                return;
            }
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.open_backfill_inflight.lock().remove(&key);
            return;
        };
        handle.spawn(async move {
            let result = fetch_cex_window_open_book(&asset, venue, window_start_ms).await;
            SERVICE.finish_window_open_backfill(asset, venue, window_start_ms, key, result);
        });
    }

    fn finish_window_open_backfill(
        &self,
        asset: String,
        venue: CexVenue,
        window_start_ms: i64,
        key: String,
        result: Result<CexBookSample>,
    ) {
        self.open_backfill_inflight.lock().remove(&key);
        self.open_backfill_fail_cooldown_until_ms.lock().remove(&key);
        match result {
            Ok(book) => {
                let open_mid = book.mid();
                let open_source = cex_delta_open_source(&book);
                let mut state = self.state.write();
                let entry = state.entry(asset.clone()).or_default();
                venue_state_mut(entry, venue)
                    .pinned_window_opens
                    .insert(window_start_ms, book);
                drop(state);
                tracing::debug!(
                    asset = %asset,
                    venue = venue.as_str(),
                    window_start_ms,
                    open_mid,
                    open_source,
                    "CEX_WINDOW_OPEN_BACKFILL_SUCCEEDED"
                );
                self.mark_dirty_asset(&asset);
            }
            Err(err) => {
                if self.has_pinned_rest_window_open(&asset, venue, window_start_ms) {
                    return;
                }
                let cooldown_until_ms =
                    Utc::now().timestamp_millis() + OPEN_BACKFILL_FAIL_COOLDOWN_MS;
                self.open_backfill_fail_cooldown_until_ms
                    .lock()
                    .insert(key, cooldown_until_ms);
                tracing::debug!(
                    asset = %asset,
                    venue = venue.as_str(),
                    window_start_ms,
                    error = %err,
                    "CEX_WINDOW_OPEN_BACKFILL_FAILED"
                );
            }
        }
    }
}

pub(crate) fn ensure_cex_microstructure_started(asset: &str) {
    SERVICE.ensure_started(asset);
}

pub(crate) fn prefetch_cex_window_opens(asset: &str, window_start_ms: i64) {
    let normalized_asset = asset.trim().to_ascii_lowercase();
    if normalized_asset.is_empty() {
        return;
    }
    SERVICE.ensure_started(&normalized_asset);
    for venue in [CexVenue::Bybit, CexVenue::Binance, CexVenue::Coinbase] {
        SERVICE.schedule_window_open_backfill(normalized_asset.clone(), venue, window_start_ms);
    }
}

pub(crate) fn get_cex_microstructure_snapshot(
    asset: &str,
    config: &CexMicrostructureSnapshotConfig,
) -> Result<CexConsensusSnapshot> {
    SERVICE.snapshot(asset, Utc::now().timestamp_millis(), config)
}

pub(crate) fn get_cex_current_price_snapshot(
    asset: &str,
    venue: CexVenue,
    config: &CexMicrostructureSnapshotConfig,
) -> Result<CexCurrentPriceSnapshot> {
    SERVICE.ensure_started(asset);
    SERVICE.current_price_snapshot(asset, venue, Utc::now().timestamp_millis(), config)
}

pub(crate) fn get_cex_venue_delta_snapshot(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
    min_move_usd: f64,
    max_book_stale_ms: i64,
) -> Result<CexVenueDeltaSnapshot> {
    SERVICE.ensure_started(asset);
    SERVICE.venue_delta_snapshot(
        asset,
        venue,
        window_start_ms,
        Utc::now().timestamp_millis(),
        min_move_usd,
        max_book_stale_ms,
    )
}

pub(crate) async fn wait_for_cex_microstructure_dirty_asset_update() {
    SERVICE.wait_for_dirty_asset_update().await;
}

pub(crate) fn take_cex_microstructure_dirty_assets() -> Vec<String> {
    SERVICE.take_dirty_assets()
}

pub(crate) fn clear_cex_microstructure_dirty_assets(assets: &[String]) {
    SERVICE.clear_dirty_assets(assets);
}

fn venue_state(entry: &AssetState, venue: CexVenue) -> &VenueState {
    match venue {
        CexVenue::Binance => &entry.binance,
        CexVenue::Coinbase => &entry.coinbase,
        CexVenue::Hyperliquid => &entry.hyperliquid,
        CexVenue::Bybit => &entry.bybit,
    }
}

fn venue_state_mut(entry: &mut AssetState, venue: CexVenue) -> &mut VenueState {
    match venue {
        CexVenue::Binance => &mut entry.binance,
        CexVenue::Coinbase => &mut entry.coinbase,
        CexVenue::Hyperliquid => &mut entry.hyperliquid,
        CexVenue::Bybit => &mut entry.bybit,
    }
}

fn merge_book_update(previous: Option<&CexBookSample>, incoming: CexBookSample) -> CexBookSample {
    if incoming.source != "level2" {
        return incoming;
    }
    let Some(previous) = previous else {
        return incoming;
    };
    if incoming.bid_size.is_some() && incoming.ask_size.is_none() {
        return CexBookSample {
            bid: incoming.bid,
            ask: previous.ask,
            ask_size: previous.ask_size,
            ..incoming
        };
    }
    if incoming.ask_size.is_some() && incoming.bid_size.is_none() {
        return CexBookSample {
            bid: previous.bid,
            ask: incoming.ask,
            bid_size: previous.bid_size,
            ..incoming
        };
    }
    incoming
}

fn source_snapshot(
    venue: CexVenue,
    state: &VenueState,
    now_ms: i64,
    config: &CexMicrostructureSnapshotConfig,
    label: &str,
) -> Result<CexSourceSnapshot> {
    let book = state
        .latest_book
        .as_ref()
        .ok_or_else(|| anyhow!("{label} book missing"))?;
    let latest_trade = state
        .trades
        .back()
        .ok_or_else(|| anyhow!("{label} trade missing"))?;
    let ticker_timestamp = state
        .latest_ticker_timestamp_ms
        .ok_or_else(|| anyhow!("{label} ticker missing"))?;
    let book_staleness_ms = now_ms.saturating_sub(book.timestamp_ms);
    let trade_staleness_ms = now_ms.saturating_sub(latest_trade.timestamp_ms);
    let ticker_staleness_ms = now_ms.saturating_sub(ticker_timestamp);
    ensure_stale(label, "book", book_staleness_ms, config.max_book_stale_ms)?;
    ensure_stale(
        label,
        "trade",
        trade_staleness_ms,
        config.max_trade_stale_ms,
    )?;
    ensure_stale(
        label,
        "ticker",
        ticker_staleness_ms,
        config.max_ticker_stale_ms,
    )?;
    Ok(CexSourceSnapshot {
        venue,
        mid: book.mid(),
        bid: book.bid,
        ask: book.ask,
        book_staleness_ms,
        trade_staleness_ms,
        ticker_staleness_ms,
        impulse: impulse_snapshot(&state.trades, now_ms, config),
    })
}

fn current_price_snapshot(
    venue: CexVenue,
    state: &VenueState,
    now_ms: i64,
    config: &CexMicrostructureSnapshotConfig,
    label: &str,
) -> Result<CexCurrentPriceSnapshot> {
    let book = state
        .latest_book
        .as_ref()
        .ok_or_else(|| anyhow!("{label} book missing"))?;
    let ticker_timestamp = state
        .latest_ticker_timestamp_ms
        .ok_or_else(|| anyhow!("{label} ticker missing"))?;
    let book_staleness_ms = now_ms.saturating_sub(book.timestamp_ms);
    let ticker_staleness_ms = now_ms.saturating_sub(ticker_timestamp);
    ensure_stale(label, "book", book_staleness_ms, config.max_book_stale_ms)?;
    ensure_stale(
        label,
        "ticker",
        ticker_staleness_ms,
        config.max_ticker_stale_ms,
    )?;
    Ok(CexCurrentPriceSnapshot {
        venue,
        mid: book.mid(),
        bid: book.bid,
        ask: book.ask,
        book_staleness_ms,
        ticker_staleness_ms,
    })
}

fn venue_delta_snapshot(
    venue: CexVenue,
    state: &VenueState,
    window_start_ms: i64,
    now_ms: i64,
    min_move_usd: f64,
    max_book_stale_ms: i64,
    label: &str,
) -> Result<CexVenueDeltaSnapshot> {
    let current = state
        .latest_book
        .as_ref()
        .ok_or_else(|| anyhow!("{label} book missing"))?;
    let open = window_open_book(venue, state, window_start_ms)
        .ok_or_else(|| anyhow!("{label} window open book missing"))?;
    let book_staleness_ms = now_ms.saturating_sub(current.timestamp_ms);
    ensure_stale(label, "book", book_staleness_ms, max_book_stale_ms)?;
    let delta_usd = current.mid() - open.mid();
    let threshold = min_move_usd.abs();
    let side = if delta_usd >= threshold {
        Some("up")
    } else if -delta_usd >= threshold {
        Some("down")
    } else {
        None
    };
    Ok(CexVenueDeltaSnapshot {
        venue,
        open_mid: open.mid(),
        current_mid: current.mid(),
        delta_usd,
        side,
        role: None,
        directional_gap: None,
        threshold_hit: None,
        open_source: cex_delta_open_source(open),
        open_timestamp_ms: open.timestamp_ms,
        current_timestamp_ms: current.timestamp_ms,
        open_lag_ms: open.timestamp_ms - window_start_ms,
        book_staleness_ms,
    })
}

fn window_open_book(
    venue: CexVenue,
    state: &VenueState,
    window_start_ms: i64,
) -> Option<&CexBookSample> {
    let pinned = state.pinned_window_opens.get(&window_start_ms);
    if venue_requires_rest_open(venue) {
        return pinned.filter(|sample| sample.source == "rest_open");
    }
    pinned.or_else(|| {
        state
            .books
            .iter()
            .rev()
            .find(|sample| window_open_sample_is_close_enough(sample, window_start_ms))
    })
}

fn venue_requires_rest_open(venue: CexVenue) -> bool {
    matches!(
        venue,
        CexVenue::Binance | CexVenue::Bybit | CexVenue::Coinbase
    )
}

fn window_open_sample_is_close_enough(sample: &CexBookSample, window_start_ms: i64) -> bool {
    sample.timestamp_ms <= window_start_ms
        && window_start_ms - sample.timestamp_ms <= MAX_WINDOW_OPEN_BOOK_AGE_MS
}

fn cex_delta_open_source(open: &CexBookSample) -> &'static str {
    if open.source == "rest_open" {
        "rest_kline_open"
    } else {
        "ws_window_open"
    }
}

fn ensure_stale(label: &str, kind: &str, age_ms: i64, max_ms: i64) -> Result<()> {
    if age_ms <= max_ms {
        return Ok(());
    }
    Err(anyhow!(
        "{label} {kind} stale: age_ms={age_ms} max_ms={max_ms}"
    ))
}

fn impulse_snapshot(
    trades: &VecDeque<CexTradeSample>,
    now_ms: i64,
    config: &CexMicrostructureSnapshotConfig,
) -> CexImpulseSnapshot {
    let cutoff = now_ms.saturating_sub(config.impulse_window_ms.max(1));
    let window = trades
        .iter()
        .filter(|trade| trade.timestamp_ms >= cutoff && trade.timestamp_ms <= now_ms)
        .collect::<Vec<_>>();
    if window.len() < 2 {
        return empty_impulse(window.len());
    }
    let first = window.first().expect("window first");
    let last = window.last().expect("window last");
    let move_usd = last.price - first.price;
    let elapsed_sec = ((last.timestamp_ms - first.timestamp_ms).max(1) as f64 / 1_000.0).max(0.001);
    let velocity_usd_per_sec = move_usd / elapsed_sec;
    let buy_notional = side_notional(&window, TakerSide::Buy);
    let sell_notional = side_notional(&window, TakerSide::Sell);
    let total_notional = buy_notional + sell_notional;
    let buy_imbalance = if total_notional > 0.0 {
        buy_notional / total_notional
    } else {
        0.0
    };
    let sell_imbalance = if total_notional > 0.0 {
        sell_notional / total_notional
    } else {
        0.0
    };

    let side = if move_usd >= config.min_move_usd
        && velocity_usd_per_sec >= config.min_velocity_usd_per_sec
        && buy_imbalance >= config.min_taker_imbalance
    {
        Some("up")
    } else if -move_usd >= config.min_move_usd
        && -velocity_usd_per_sec >= config.min_velocity_usd_per_sec
        && sell_imbalance >= config.min_taker_imbalance
    {
        Some("down")
    } else {
        None
    };

    CexImpulseSnapshot {
        side,
        move_usd,
        velocity_usd_per_sec,
        taker_imbalance: match side {
            Some("up") => buy_imbalance,
            Some("down") => sell_imbalance,
            _ => buy_imbalance.max(sell_imbalance),
        },
        trade_count: window.len(),
    }
}

fn empty_impulse(trade_count: usize) -> CexImpulseSnapshot {
    CexImpulseSnapshot {
        side: None,
        move_usd: 0.0,
        velocity_usd_per_sec: 0.0,
        taker_imbalance: 0.0,
        trade_count,
    }
}

fn side_notional(window: &[&CexTradeSample], side: TakerSide) -> f64 {
    window
        .iter()
        .filter(|trade| trade.taker_side == side)
        .map(|trade| trade.notional())
        .sum()
}

fn record_skew_sample(entry: &mut AssetState) {
    let (Some(binance), Some(coinbase)) = (&entry.binance.latest_book, &entry.coinbase.latest_book)
    else {
        return;
    };
    let timestamp_ms = binance.timestamp_ms.max(coinbase.timestamp_ms);
    entry
        .skew_samples
        .push_back((timestamp_ms, binance.mid() - coinbase.mid()));
    trim_skew(&mut entry.skew_samples);
}

fn median_recent_skew(
    samples: &VecDeque<(i64, f64)>,
    now_ms: i64,
    baseline_window_ms: i64,
) -> Option<f64> {
    let cutoff = now_ms.saturating_sub(baseline_window_ms.max(1));
    let mut values = samples
        .iter()
        .filter(|(timestamp_ms, value)| *timestamp_ms >= cutoff && value.is_finite())
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}

fn trim_books(samples: &mut VecDeque<CexBookSample>) {
    trim_by_timestamp(samples, |sample| sample.timestamp_ms, MAX_BOOK_SAMPLES);
}

fn trim_trades(samples: &mut VecDeque<CexTradeSample>) {
    trim_by_timestamp(samples, |sample| sample.timestamp_ms, MAX_TRADE_SAMPLES);
}

fn trim_skew(samples: &mut VecDeque<(i64, f64)>) {
    trim_by_timestamp(samples, |sample| sample.0, MAX_SKEW_SAMPLES);
}

fn trim_by_timestamp<T>(
    samples: &mut VecDeque<T>,
    timestamp: impl Fn(&T) -> i64,
    max_samples: usize,
) {
    let latest = samples.back().map(&timestamp).unwrap_or(0);
    let cutoff = latest.saturating_sub(MAX_SAMPLE_AGE_MS);
    while samples
        .front()
        .map(|sample| timestamp(sample) < cutoff)
        .unwrap_or(false)
    {
        samples.pop_front();
    }
    while samples.len() > max_samples {
        samples.pop_front();
    }
}

async fn binance_loop(asset: String) {
    loop {
        if let Err(err) = binance_once(&asset).await {
            tracing::warn!(asset = %asset, error = %err, "EARLY_STALE_BINANCE_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

async fn binance_once(asset: &str) -> Result<()> {
    let url = binance_stream_url(asset).ok_or_else(|| anyhow!("unsupported binance asset"))?;
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting binance microstructure websocket: {url}"))?;
    let (_, mut stream) = ws.split();
    loop {
        match tokio::time::timeout(Duration::from_secs(WS_IDLE_TIMEOUT_SECS), stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(payload) = serde_json::from_str::<Value>(&text) {
                    let parsed =
                        parse_binance_ws_payload(&payload, asset, Utc::now().timestamp_millis());
                    if let Some(book) = parsed.book {
                        SERVICE.update_book(book);
                    }
                    if let Some(trade) = parsed.trade {
                        SERVICE.update_trade(trade);
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => return Err(err.into()),
            Err(_) => return Err(anyhow!("binance microstructure websocket idle timeout")),
        }
    }
}

async fn coinbase_loop(asset: String) {
    loop {
        if let Err(err) = coinbase_once(&asset).await {
            tracing::warn!(asset = %asset, error = %err, "EARLY_STALE_COINBASE_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

async fn coinbase_once(asset: &str) -> Result<()> {
    let url = coinbase_ws_url();
    let messages = coinbase_subscription_messages(asset)
        .ok_or_else(|| anyhow!("unsupported coinbase asset"))?;
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting coinbase microstructure websocket: {url}"))?;
    let (mut sink, mut stream) = ws.split();
    for message in messages {
        sink.send(Message::Text(message.to_string().into()))
            .await
            .context("sending coinbase microstructure subscription")?;
    }
    loop {
        match tokio::time::timeout(Duration::from_secs(WS_IDLE_TIMEOUT_SECS), stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(payload) = serde_json::from_str::<Value>(&text) {
                    let parsed =
                        parse_coinbase_ws_payload(&payload, asset, Utc::now().timestamp_millis());
                    for book in parsed.books {
                        SERVICE.update_book(book);
                    }
                    for trade in parsed.trades {
                        SERVICE.update_trade(trade);
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(payload)))) => {
                let _ = sink.send(Message::Pong(payload)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => return Err(err.into()),
            Err(_) => return Err(anyhow!("coinbase microstructure websocket idle timeout")),
        }
    }
}

async fn hyperliquid_loop(asset: String) {
    loop {
        if let Err(err) = hyperliquid_once(&asset).await {
            tracing::warn!(asset = %asset, error = %err, "EARLY_STALE_HYPERLIQUID_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

async fn hyperliquid_once(asset: &str) -> Result<()> {
    let url = hyperliquid_ws_url();
    let message = hyperliquid_subscription_message(asset)
        .ok_or_else(|| anyhow!("unsupported hyperliquid asset"))?;
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting hyperliquid microstructure websocket: {url}"))?;
    let (mut sink, mut stream) = ws.split();
    sink.send(Message::Text(message.to_string().into()))
        .await
        .context("sending hyperliquid microstructure subscription")?;
    loop {
        match tokio::time::timeout(Duration::from_secs(WS_IDLE_TIMEOUT_SECS), stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(payload) = serde_json::from_str::<Value>(&text) {
                    if let Some(book) =
                        parse_hyperliquid_ws_payload(&payload, asset, Utc::now().timestamp_millis())
                    {
                        SERVICE.update_book(book);
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(payload)))) => {
                let _ = sink.send(Message::Pong(payload)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => return Err(err.into()),
            Err(_) => return Err(anyhow!("hyperliquid microstructure websocket idle timeout")),
        }
    }
}

async fn bybit_loop(asset: String) {
    loop {
        if let Err(err) = bybit_once(&asset).await {
            tracing::warn!(asset = %asset, error = %err, "EARLY_STALE_BYBIT_WS_ERROR");
        }
        tokio::time::sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

async fn bybit_once(asset: &str) -> Result<()> {
    let url = bybit_ws_url();
    let message =
        bybit_subscription_message(asset).ok_or_else(|| anyhow!("unsupported bybit asset"))?;
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("connecting bybit microstructure websocket: {url}"))?;
    let (mut sink, mut stream) = ws.split();
    sink.send(Message::Text(message.to_string().into()))
        .await
        .context("sending bybit microstructure subscription")?;
    loop {
        match tokio::time::timeout(Duration::from_secs(WS_IDLE_TIMEOUT_SECS), stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(payload) = serde_json::from_str::<Value>(&text) {
                    if let Some(book) =
                        parse_bybit_ws_payload(&payload, asset, Utc::now().timestamp_millis())
                    {
                        SERVICE.update_book(book);
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(payload)))) => {
                let _ = sink.send(Message::Pong(payload)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(()),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => return Err(err.into()),
            Err(_) => return Err(anyhow!("bybit microstructure websocket idle timeout")),
        }
    }
}

#[cfg(test)]
pub(crate) fn clear_cex_microstructure_test_state() {
    SERVICE.state.write().clear();
    SERVICE.started_assets.write().clear();
    SERVICE.open_backfill_inflight.lock().clear();
    SERVICE.open_backfill_fail_cooldown_until_ms.lock().clear();
    SERVICE.dirty_assets.lock().clear();
}

#[cfg(test)]
pub(crate) fn seed_cex_book_test_sample(sample: CexBookSample) {
    if sample.source == "rest_open" {
        seed_cex_open_test_sample(sample);
        return;
    }
    SERVICE.update_book(sample);
}

#[cfg(test)]
pub(crate) fn seed_cex_open_test_sample(sample: CexBookSample) {
    let asset = sample.asset.clone();
    let mut state = SERVICE.state.write();
    let entry = state.entry(asset).or_default();
    let venue = venue_state_mut(entry, sample.venue);
    venue
        .pinned_window_opens
        .insert(sample.timestamp_ms, sample);
}

#[cfg(test)]
pub(crate) fn seed_cex_trade_test_sample(sample: CexTradeSample) {
    SERVICE.update_trade(sample);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static CEX_MICROSTRUCTURE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_cex_microstructure_test_state() -> MutexGuard<'static, ()> {
        CEX_MICROSTRUCTURE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn book(venue: CexVenue, ts: i64, bid: f64, ask: f64) -> CexBookSample {
        CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: ts,
            bid,
            ask,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        }
    }

    fn trade(venue: CexVenue, ts: i64, price: f64, side: TakerSide) -> CexTradeSample {
        CexTradeSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: ts,
            price,
            size: 1.0,
            taker_side: side,
        }
    }

    #[test]
    fn cex_snapshot_requires_matching_consensus_side() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Binance, 20_000, 67_519.0, 67_521.0));
        seed_cex_book_test_sample(book(CexVenue::Coinbase, 20_000, 67_518.0, 67_522.0));
        for venue in [CexVenue::Binance, CexVenue::Coinbase] {
            seed_cex_trade_test_sample(trade(venue, 6_000, 67_500.0, TakerSide::Buy));
            seed_cex_trade_test_sample(trade(venue, 20_000, 67_520.0, TakerSide::Buy));
        }

        let snapshot = SERVICE
            .snapshot("btc", 20_100, &CexMicrostructureSnapshotConfig::default())
            .expect("snapshot");

        assert_eq!(snapshot.consensus_side, Some("up"));
        assert!(snapshot.normalized_source_skew_usd.abs() <= 0.001);
    }

    #[tokio::test]
    async fn schedule_window_open_backfill_skips_when_rest_open_already_pinned() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let window_start_ms = 1780291200000_i64;
        seed_cex_open_test_sample(CexBookSample {
            venue: CexVenue::Bybit,
            asset: "btc".to_string(),
            timestamp_ms: window_start_ms,
            bid: 73_460.4,
            ask: 73_460.4,
            bid_size: None,
            ask_size: None,
            source: "rest_open",
        });
        seed_cex_book_test_sample(book(CexVenue::Bybit, window_start_ms + 30_000, 73_500.0, 73_502.0));

        SERVICE.schedule_window_open_backfill("btc".to_string(), CexVenue::Bybit, window_start_ms);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let key = format!("btc:bybit:{window_start_ms}");
        assert!(!SERVICE.open_backfill_inflight.lock().contains(&key));
    }

    #[test]
    fn venue_delta_snapshot_rejects_bybit_pre_window_ws_book_without_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Bybit, 10_000, 99.0, 101.0));
        seed_cex_book_test_sample(book(CexVenue::Bybit, 20_000, 109.0, 111.0));

        let error = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Bybit, 10_000, 20_100, 1.0, 500)
            .expect_err("bybit ws book must not become window open");

        assert!(error.to_string().contains("window open book missing"));
    }

    #[test]
    fn venue_delta_snapshot_rejects_late_first_book_as_window_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Bybit, 190_000, 99.0, 101.0));
        seed_cex_book_test_sample(book(CexVenue::Bybit, 200_000, 89.0, 91.0));

        let error = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Bybit, 0, 200_100, 1.0, 500)
            .expect_err("late service-start book must not become window open");

        assert!(error.to_string().contains("window open book missing"));
    }

    #[test]
    fn venue_delta_snapshot_rejects_binance_late_book_as_window_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Binance, 41_201, 73_838.97, 73_839.02));

        let error = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Binance, 0, 41_301, 1.0, 500)
            .expect_err("binance late book must not become window open");

        assert!(error.to_string().contains("window open book missing"));
    }

    #[test]
    fn venue_delta_snapshot_rejects_binance_pre_window_ws_book_without_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Binance, 9_800, 73_821.98, 73_822.00));
        seed_cex_book_test_sample(book(CexVenue::Binance, 20_000, 73_838.97, 73_839.02));

        let error = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Binance, 10_000, 20_100, 1.0, 500)
            .expect_err("binance ws book must not become window open");

        assert!(error.to_string().contains("window open book missing"));
    }

    #[test]
    fn venue_delta_snapshot_prefers_pinned_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_open_test_sample(CexBookSample {
            venue: CexVenue::Bybit,
            asset: "btc".to_string(),
            timestamp_ms: 0,
            bid: 100.0,
            ask: 100.0,
            bid_size: None,
            ask_size: None,
            source: "rest_open",
        });
        seed_cex_book_test_sample(book(CexVenue::Bybit, 190_000, 89.0, 91.0));

        let snapshot = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Bybit, 0, 190_100, 1.0, 500)
            .expect("delta snapshot");

        assert_eq!(snapshot.open_mid, 100.0);
        assert_eq!(snapshot.current_mid, 90.0);
        assert_eq!(snapshot.delta_usd, -10.0);
        assert_eq!(snapshot.open_source, "rest_kline_open");
    }

    #[test]
    fn venue_delta_snapshot_uses_coinbase_pinned_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_open_test_sample(CexBookSample {
            venue: CexVenue::Coinbase,
            asset: "btc".to_string(),
            timestamp_ms: 0,
            bid: 100.0,
            ask: 100.0,
            bid_size: None,
            ask_size: None,
            source: "rest_open",
        });
        seed_cex_book_test_sample(book(CexVenue::Coinbase, 190_000, 110.0, 112.0));

        let snapshot = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Coinbase, 0, 190_100, 1.0, 500)
            .expect("delta snapshot");

        assert_eq!(snapshot.open_mid, 100.0);
        assert_eq!(snapshot.current_mid, 111.0);
        assert_eq!(snapshot.delta_usd, 11.0);
        assert_eq!(snapshot.open_lag_ms, 0);
        assert_eq!(snapshot.open_source, "rest_kline_open");
    }

    #[test]
    fn venue_delta_snapshot_rejects_coinbase_pre_window_ws_book_without_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_book_test_sample(book(CexVenue::Coinbase, 9_800, 99.0, 101.0));
        seed_cex_book_test_sample(book(CexVenue::Coinbase, 20_000, 109.0, 111.0));

        let error = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Coinbase, 10_000, 20_100, 1.0, 500)
            .expect_err("coinbase ws book must not become window open");

        assert!(error.to_string().contains("window open book missing"));
    }

    #[test]
    fn venue_delta_snapshot_uses_binance_pinned_rest_open() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_cex_open_test_sample(CexBookSample {
            venue: CexVenue::Binance,
            asset: "btc".to_string(),
            timestamp_ms: 0,
            bid: 73_821.99,
            ask: 73_821.99,
            bid_size: None,
            ask_size: None,
            source: "rest_open",
        });
        seed_cex_book_test_sample(book(CexVenue::Binance, 41_201, 73_838.97, 73_839.02));

        let snapshot = SERVICE
            .venue_delta_snapshot("btc", CexVenue::Binance, 0, 41_301, 1.0, 500)
            .expect("delta snapshot");

        assert_eq!(snapshot.open_mid, 73_821.99);
        assert_eq!(snapshot.current_mid, 73_838.995);
        assert!((snapshot.delta_usd - 17.005).abs() < 0.000_000_1);
        assert_eq!(snapshot.open_lag_ms, 0);
        assert_eq!(snapshot.open_source, "rest_kline_open");
    }
}
