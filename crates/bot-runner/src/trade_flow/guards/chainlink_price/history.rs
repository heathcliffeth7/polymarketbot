use chrono::Utc;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::atomic::Ordering,
};

use super::{
    diff_ms, is_supported_symbol, CachedPrice, ChainlinkPriceService, SymbolPriceState,
    MAX_PRICE_AGE_SECS, MAX_TICK_HISTORY_AGE_SECS, MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL, SERVICE,
    STALE_PROVIDER_WARN_INTERVAL_MS,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChainlinkLiveDataHistorySample {
    pub(crate) value: f64,
    pub(crate) timestamp_ms: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ChainlinkLiveDataHistoryIngestSummary {
    pub(crate) sample_count: usize,
    pub(crate) latest_timestamp_ms: Option<i64>,
}

pub(crate) fn ingest_chainlink_live_data_price_history(
    symbol: &str,
    samples: Vec<ChainlinkLiveDataHistorySample>,
    session_id: u64,
) -> ChainlinkLiveDataHistoryIngestSummary {
    if !is_supported_symbol(symbol) {
        if SERVICE.should_warn_unexpected_symbol(symbol) {
            tracing::warn!(
                session_id,
                symbol,
                "CHAINLINK_LIVE_DATA_WS_UNEXPECTED_SYMBOL"
            );
        }
        return ChainlinkLiveDataHistoryIngestSummary::default();
    }

    let received_at_ms = Utc::now().timestamp_millis();
    let mut ticks = samples
        .into_iter()
        .filter(|sample| sample.value.is_finite() && sample.value > 0.0)
        .map(|sample| CachedPrice {
            value: sample.value,
            timestamp_ms: sample.timestamp_ms,
            received_at_ms,
        })
        .collect::<Vec<_>>();
    if ticks.is_empty() {
        return ChainlinkLiveDataHistoryIngestSummary::default();
    }

    ticks.sort_by_key(|tick| tick.timestamp_ms);
    SERVICE.update_price_history(symbol, ticks, received_at_ms, session_id)
}

impl ChainlinkPriceService {
    fn update_price_history(
        &self,
        symbol: &str,
        ticks: Vec<CachedPrice>,
        received_at_ms: i64,
        session_id: u64,
    ) -> ChainlinkLiveDataHistoryIngestSummary {
        let sample_count = ticks.len();
        let batch_latest = ticks.last().cloned();
        let latest_timestamp_ms = batch_latest.as_ref().map(|tick| tick.timestamp_ms);
        let mut stale_warning_tick = None;

        {
            let mut state = self.state.write();
            let entry = state.entry(symbol.to_string()).or_default();
            merge_history_ticks(entry, &ticks);
            entry.last_received_at_ms = Some(received_at_ms);

            if should_replace_latest(entry.latest.as_ref(), batch_latest.as_ref()) {
                if let Some(latest) = batch_latest.clone() {
                    update_effective_latest(
                        entry,
                        symbol,
                        latest,
                        received_at_ms,
                        &mut stale_warning_tick,
                    );
                }
            }
        }

        self.last_successful_tick_at_ms
            .store(received_at_ms, Ordering::SeqCst);
        self.mark_dirty_asset(symbol);

        if let Some(tick) = stale_warning_tick {
            tracing::warn!(
                session_id,
                symbol,
                value = tick.value,
                provider_timestamp_ms = tick.timestamp_ms,
                received_at_ms,
                provider_age_ms = diff_ms(received_at_ms, tick.timestamp_ms),
                "CHAINLINK_LIVE_DATA_WS_STALE_PROVIDER_TIMESTAMP"
            );
        }
        *self.last_error.write() = None;

        ChainlinkLiveDataHistoryIngestSummary {
            sample_count,
            latest_timestamp_ms,
        }
    }
}

fn should_replace_latest(existing: Option<&CachedPrice>, candidate: Option<&CachedPrice>) -> bool {
    match (existing, candidate) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(existing), Some(candidate)) => candidate.timestamp_ms >= existing.timestamp_ms,
    }
}

fn update_effective_latest(
    entry: &mut SymbolPriceState,
    symbol: &str,
    latest: CachedPrice,
    received_at_ms: i64,
    stale_warning_tick: &mut Option<CachedPrice>,
) {
    let provider_age_ms = diff_ms(received_at_ms, latest.timestamp_ms);
    let stale_on_arrival_reason = (provider_age_ms > (MAX_PRICE_AGE_SECS * 1_000)).then(|| {
        format!(
            "provider timestamp stale on arrival for {} (provider_age_ms={provider_age_ms}, receive_age_ms=0, provider_timestamp_ms={}, received_at_ms={received_at_ms})",
            symbol,
            latest.timestamp_ms,
        )
    });

    if let Some(reason) = stale_on_arrival_reason {
        entry.last_stale_reason = Some(reason);
        let should_warn = entry
            .last_stale_warning_at_ms
            .map(|last_warn_at_ms| {
                diff_ms(received_at_ms, last_warn_at_ms) >= STALE_PROVIDER_WARN_INTERVAL_MS
            })
            .unwrap_or(true);
        if should_warn {
            entry.last_stale_warning_at_ms = Some(received_at_ms);
            *stale_warning_tick = Some(latest.clone());
        }
    } else {
        entry.last_stale_reason = None;
        entry.last_stale_warning_at_ms = None;
    }
    entry.latest = Some(latest);
}

fn merge_history_ticks(entry: &mut SymbolPriceState, ticks: &[CachedPrice]) {
    let cutoff_source_ms = entry
        .latest
        .as_ref()
        .map(|tick| tick.timestamp_ms)
        .into_iter()
        .chain(ticks.last().map(|tick| tick.timestamp_ms))
        .max()
        .unwrap_or_default();
    let cutoff_ms = cutoff_source_ms - (MAX_TICK_HISTORY_AGE_SECS * 1_000);
    let mut by_timestamp = BTreeMap::new();

    for tick in entry
        .ticks
        .iter()
        .chain(ticks.iter())
        .filter(|tick| tick.timestamp_ms >= cutoff_ms)
    {
        by_timestamp.insert(tick.timestamp_ms, tick.clone());
    }

    let mut merged = by_timestamp.into_values().collect::<Vec<_>>();
    if merged.len() > MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL {
        let drain_until = merged.len() - MAX_TICK_HISTORY_SAMPLES_PER_SYMBOL;
        merged.drain(0..drain_until);
    }
    entry.ticks = VecDeque::from(merged);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_does_not_move_fresh_latest_backward() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();
        service.update_price("sol/usd", 65.25, now_ms, 1);

        let summary = service.update_price_history(
            "sol/usd",
            vec![
                CachedPrice {
                    value: 65.10,
                    timestamp_ms: now_ms - 60_000,
                    received_at_ms: now_ms,
                },
                CachedPrice {
                    value: 65.12,
                    timestamp_ms: now_ms - 50_000,
                    received_at_ms: now_ms,
                },
            ],
            now_ms + 1,
            2,
        );

        assert_eq!(summary.sample_count, 2);
        assert_eq!(summary.latest_timestamp_ms, Some(now_ms - 50_000));
        assert_eq!(service.get_price("sol").expect("fresh latest"), 65.25);
        assert!(!service.take_reconnect_requested());
        assert_eq!(service.take_dirty_assets(), vec!["sol".to_string()]);
    }

    #[test]
    fn empty_cache_history_uses_newest_batch_sample_as_latest() {
        let service = ChainlinkPriceService::new();
        let now_ms = Utc::now().timestamp_millis();

        let summary = service.update_price_history(
            "btc/usd",
            vec![
                CachedPrice {
                    value: 70_500.0,
                    timestamp_ms: now_ms - 1_000,
                    received_at_ms: now_ms,
                },
                CachedPrice {
                    value: 70_525.5,
                    timestamp_ms: now_ms,
                    received_at_ms: now_ms,
                },
            ],
            now_ms,
            3,
        );

        assert_eq!(
            summary,
            ChainlinkLiveDataHistoryIngestSummary {
                sample_count: 2,
                latest_timestamp_ms: Some(now_ms),
            }
        );
        assert_eq!(service.get_price("btc").expect("batch latest"), 70_525.5);
        assert_eq!(service.take_dirty_assets(), vec!["btc".to_string()]);
    }
}
