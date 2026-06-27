use parking_lot::Mutex;
use std::{collections::HashMap, sync::LazyLock};

const CHAINLINK_PROVIDER_GAP_INFO_MS: i64 = 3_000;
const CHAINLINK_PROVIDER_GAP_WARN_MS: i64 = 5_000;
const CHAINLINK_PROVIDER_GAP_HIGH_WARN_MS: i64 = 9_000;
const CHAINLINK_PROVIDER_GAP_CRITICAL_MS: i64 = 20_000;

static PROVIDER_GAP_TRACKER: LazyLock<Mutex<ChainlinkProviderGapTracker>> =
    LazyLock::new(|| Mutex::new(ChainlinkProviderGapTracker::default()));

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChainlinkProviderGap {
    previous_provider_timestamp_ms: i64,
    current_provider_timestamp_ms: i64,
    provider_round_gap_ms: i64,
    received_at_ms: i64,
    missed_provider_seconds: i64,
}

impl ChainlinkProviderGap {
    fn receipt_age_ms(&self) -> i64 {
        self.received_at_ms
            .saturating_sub(self.current_provider_timestamp_ms)
    }

    fn severity(&self) -> &'static str {
        if self.provider_round_gap_ms > CHAINLINK_PROVIDER_GAP_CRITICAL_MS {
            "critical/provider_gap"
        } else if self.provider_round_gap_ms > CHAINLINK_PROVIDER_GAP_HIGH_WARN_MS {
            "high_warn"
        } else if self.provider_round_gap_ms > CHAINLINK_PROVIDER_GAP_WARN_MS {
            "warn"
        } else {
            "info"
        }
    }
}

#[derive(Debug, Default)]
struct ChainlinkProviderGapTracker {
    last_provider_timestamp_by_symbol: HashMap<String, i64>,
}

impl ChainlinkProviderGapTracker {
    fn observe(
        &mut self,
        symbol: &str,
        current_provider_timestamp_ms: i64,
        received_at_ms: i64,
    ) -> Option<ChainlinkProviderGap> {
        let Some(previous_provider_timestamp_ms) =
            self.last_provider_timestamp_by_symbol.get(symbol).copied()
        else {
            self.last_provider_timestamp_by_symbol
                .insert(symbol.to_string(), current_provider_timestamp_ms);
            return None;
        };

        if current_provider_timestamp_ms <= previous_provider_timestamp_ms {
            return None;
        }

        self.last_provider_timestamp_by_symbol
            .insert(symbol.to_string(), current_provider_timestamp_ms);

        let provider_round_gap_ms = current_provider_timestamp_ms - previous_provider_timestamp_ms;
        if provider_round_gap_ms <= CHAINLINK_PROVIDER_GAP_INFO_MS {
            return None;
        }

        Some(ChainlinkProviderGap {
            previous_provider_timestamp_ms,
            current_provider_timestamp_ms,
            provider_round_gap_ms,
            received_at_ms,
            missed_provider_seconds: (provider_round_gap_ms / 1_000).saturating_sub(1),
        })
    }
}

pub(crate) fn record_chainlink_provider_tick_gap(
    symbol: &str,
    current_provider_timestamp_ms: i64,
    received_at_ms: i64,
    session_id: u64,
) {
    let gap =
        PROVIDER_GAP_TRACKER
            .lock()
            .observe(symbol, current_provider_timestamp_ms, received_at_ms);
    let Some(gap) = gap else {
        return;
    };

    let severity = gap.severity();
    let receipt_age_ms = gap.receipt_age_ms();
    match severity {
        "critical/provider_gap" => tracing::error!(
            session_id,
            asset = symbol,
            previous_provider_ts = gap.previous_provider_timestamp_ms,
            current_provider_ts = gap.current_provider_timestamp_ms,
            provider_round_gap_ms = gap.provider_round_gap_ms,
            receipt_age_ms,
            received_at_ms = gap.received_at_ms,
            missed_provider_seconds = gap.missed_provider_seconds,
            severity,
            "CHAINLINK_LIVE_DATA_PROVIDER_ROUND_GAP"
        ),
        "warn" | "high_warn" => tracing::warn!(
            session_id,
            asset = symbol,
            previous_provider_ts = gap.previous_provider_timestamp_ms,
            current_provider_ts = gap.current_provider_timestamp_ms,
            provider_round_gap_ms = gap.provider_round_gap_ms,
            receipt_age_ms,
            received_at_ms = gap.received_at_ms,
            missed_provider_seconds = gap.missed_provider_seconds,
            severity,
            "CHAINLINK_LIVE_DATA_PROVIDER_ROUND_GAP"
        ),
        _ => tracing::info!(
            session_id,
            asset = symbol,
            previous_provider_ts = gap.previous_provider_timestamp_ms,
            current_provider_ts = gap.current_provider_timestamp_ms,
            provider_round_gap_ms = gap.provider_round_gap_ms,
            receipt_age_ms,
            received_at_ms = gap.received_at_ms,
            missed_provider_seconds = gap.missed_provider_seconds,
            severity,
            "CHAINLINK_LIVE_DATA_PROVIDER_ROUND_GAP"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chainlink_provider_gap_ignores_one_second_steps() {
        let mut tracker = ChainlinkProviderGapTracker::default();

        assert_eq!(tracker.observe("btc/usd", 1_000, 1_100), None);
        assert_eq!(tracker.observe("btc/usd", 2_000, 2_100), None);
        assert_eq!(tracker.observe("btc/usd", 3_000, 3_100), None);
    }

    #[test]
    fn chainlink_provider_gap_reports_large_provider_gap() {
        let mut tracker = ChainlinkProviderGapTracker::default();

        assert_eq!(tracker.observe("btc/usd", 1_000, 1_100), None);
        let gap = tracker
            .observe("btc/usd", 9_000, 9_200)
            .expect("provider round gap");
        assert_eq!(
            gap,
            ChainlinkProviderGap {
                previous_provider_timestamp_ms: 1_000,
                current_provider_timestamp_ms: 9_000,
                provider_round_gap_ms: 8_000,
                received_at_ms: 9_200,
                missed_provider_seconds: 7,
            }
        );
        assert_eq!(gap.receipt_age_ms(), 200);
        assert_eq!(gap.severity(), "warn");
    }

    #[test]
    fn chainlink_provider_gap_classifies_severity_buckets() {
        for (gap_ms, expected_severity) in [
            (4_000, "info"),
            (8_000, "warn"),
            (12_000, "high_warn"),
            (25_000, "critical/provider_gap"),
        ] {
            let gap = ChainlinkProviderGap {
                previous_provider_timestamp_ms: 1_000,
                current_provider_timestamp_ms: 1_000 + gap_ms,
                provider_round_gap_ms: gap_ms,
                received_at_ms: 1_000 + gap_ms + 150,
                missed_provider_seconds: (gap_ms / 1_000).saturating_sub(1),
            };
            assert_eq!(gap.receipt_age_ms(), 150);
            assert_eq!(gap.severity(), expected_severity);
        }
    }

    #[test]
    fn chainlink_provider_gap_ignores_duplicate_and_out_of_order_ticks() {
        let mut tracker = ChainlinkProviderGapTracker::default();

        assert_eq!(tracker.observe("btc/usd", 5_000, 5_100), None);
        assert_eq!(tracker.observe("btc/usd", 5_000, 5_200), None);
        assert_eq!(tracker.observe("btc/usd", 4_000, 5_300), None);
        assert_eq!(
            tracker.observe("btc/usd", 9_000, 9_400),
            Some(ChainlinkProviderGap {
                previous_provider_timestamp_ms: 5_000,
                current_provider_timestamp_ms: 9_000,
                provider_round_gap_ms: 4_000,
                received_at_ms: 9_400,
                missed_provider_seconds: 3,
            })
        );
    }

    #[test]
    fn chainlink_provider_gap_tracks_symbols_independently() {
        let mut tracker = ChainlinkProviderGapTracker::default();

        assert_eq!(tracker.observe("btc/usd", 1_000, 1_100), None);
        assert_eq!(tracker.observe("eth/usd", 8_000, 8_100), None);
        assert_eq!(tracker.observe("eth/usd", 9_000, 9_100), None);
        assert_eq!(
            tracker.observe("btc/usd", 9_000, 9_200),
            Some(ChainlinkProviderGap {
                previous_provider_timestamp_ms: 1_000,
                current_provider_timestamp_ms: 9_000,
                provider_round_gap_ms: 8_000,
                received_at_ms: 9_200,
                missed_provider_seconds: 7,
            })
        );
    }
}
