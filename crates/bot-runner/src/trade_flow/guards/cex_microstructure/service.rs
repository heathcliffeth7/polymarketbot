use super::{
    binance::{binance_stream_url, parse_binance_ws_payload},
    coinbase::{coinbase_subscription_messages, coinbase_ws_url, parse_coinbase_ws_payload},
    types::{
        CexBookSample, CexConsensusSnapshot, CexCurrentPriceSnapshot, CexImpulseSnapshot,
        CexSourceSnapshot, CexTradeSample, CexVenue, TakerSide,
    },
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::LazyLock,
    time::Duration,
};
use tokio_tungstenite::tungstenite::Message;

const MAX_SAMPLE_AGE_MS: i64 = 5 * 60 * 1_000;
const MAX_BOOK_SAMPLES: usize = 4_000;
const MAX_TRADE_SAMPLES: usize = 20_000;
const MAX_SKEW_SAMPLES: usize = 4_000;
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
    books: VecDeque<CexBookSample>,
    trades: VecDeque<CexTradeSample>,
}

#[derive(Debug, Default)]
struct AssetState {
    binance: VenueState,
    coinbase: VenueState,
    skew_samples: VecDeque<(i64, f64)>,
}

struct CexMicrostructureService {
    state: RwLock<HashMap<String, AssetState>>,
    started_assets: RwLock<HashSet<String>>,
}

static SERVICE: LazyLock<CexMicrostructureService> = LazyLock::new(CexMicrostructureService::new);

impl CexMicrostructureService {
    fn new() -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            started_assets: RwLock::new(HashSet::new()),
        }
    }

    fn ensure_started(&self, asset: &str) {
        let asset = asset.trim().to_ascii_lowercase();
        if asset.is_empty()
            || binance_stream_url(&asset).is_none()
            || coinbase_subscription_messages(&asset).is_none()
        {
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
        handle.spawn(binance_loop(asset.clone()));
        handle.spawn(coinbase_loop(asset));
    }

    fn update_book(&self, book: CexBookSample) {
        let asset = book.asset.clone();
        let mut state = self.state.write();
        let entry = state.entry(asset).or_default();
        let venue = venue_state_mut(entry, book.venue);
        let book = merge_book_update(venue.latest_book.as_ref(), book);
        venue.latest_ticker_timestamp_ms = if book.source == "bookTicker" || book.source == "ticker"
        {
            Some(book.timestamp_ms)
        } else {
            venue.latest_ticker_timestamp_ms
        };
        venue.latest_book = Some(book.clone());
        venue.books.push_back(book);
        trim_books(&mut venue.books);
        record_skew_sample(entry);
    }

    fn update_trade(&self, trade: CexTradeSample) {
        let asset = trade.asset.clone();
        let mut state = self.state.write();
        let entry = state.entry(asset).or_default();
        let venue = venue_state_mut(entry, trade.venue);
        venue.trades.push_back(trade);
        trim_trades(&mut venue.trades);
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
}

pub(crate) fn ensure_cex_microstructure_started(asset: &str) {
    SERVICE.ensure_started(asset);
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

fn venue_state(entry: &AssetState, venue: CexVenue) -> &VenueState {
    match venue {
        CexVenue::Binance => &entry.binance,
        CexVenue::Coinbase => &entry.coinbase,
    }
}

fn venue_state_mut(entry: &mut AssetState, venue: CexVenue) -> &mut VenueState {
    match venue {
        CexVenue::Binance => &mut entry.binance,
        CexVenue::Coinbase => &mut entry.coinbase,
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

#[cfg(test)]
pub(crate) fn clear_cex_microstructure_test_state() {
    SERVICE.state.write().clear();
    SERVICE.started_assets.write().clear();
}

#[cfg(test)]
pub(crate) fn seed_cex_book_test_sample(sample: CexBookSample) {
    SERVICE.update_book(sample);
}

#[cfg(test)]
pub(crate) fn seed_cex_trade_test_sample(sample: CexTradeSample) {
    SERVICE.update_trade(sample);
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
