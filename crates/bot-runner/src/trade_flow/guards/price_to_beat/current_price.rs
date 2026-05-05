use super::*;
use crate::trade_flow::guards::cex_microstructure::{
    get_cex_current_price_snapshot, CexMicrostructureSnapshotConfig, CexVenue,
};
use crate::trade_flow::guards::chainlink_price::parse_chainlink_stale_price_details;

pub(super) const CURRENT_PRICE_SOURCE_CHAINLINK: &str = "chainlink_live_data_ws";
pub(super) const CURRENT_PRICE_SOURCE_BINANCE: &str = "binance_cex_ws_mid";
pub(super) const CURRENT_PRICE_SOURCE_COINBASE: &str = "coinbase_cex_ws_mid";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatCurrentPriceSource {
    Chainlink,
    Binance,
    Coinbase,
}

impl PriceToBeatCurrentPriceSource {
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "binance" => Self::Binance,
            "coinbase" => Self::Coinbase,
            _ => Self::Chainlink,
        }
    }

    pub(crate) fn as_config_str(self) -> &'static str {
        match self {
            Self::Chainlink => "chainlink",
            Self::Binance => "binance",
            Self::Coinbase => "coinbase",
        }
    }

    pub(crate) fn current_price_source_label(self) -> &'static str {
        match self {
            Self::Chainlink => CURRENT_PRICE_SOURCE_CHAINLINK,
            Self::Binance => CURRENT_PRICE_SOURCE_BINANCE,
            Self::Coinbase => CURRENT_PRICE_SOURCE_COINBASE,
        }
    }

    fn cex_venue(self) -> Option<CexVenue> {
        match self {
            Self::Chainlink => None,
            Self::Binance => Some(CexVenue::Binance),
            Self::Coinbase => Some(CexVenue::Coinbase),
        }
    }
}

fn is_retryable_chainlink_current_price_error(detail: &str) -> bool {
    detail.starts_with("stale price for ") || detail.starts_with("no cached price for ")
}

fn format_chainlink_rtds_pending_detail(
    market_slug: &str,
    asset: &str,
    gap_ms: Option<i64>,
    chainlink_error: &str,
) -> String {
    let structured = parse_chainlink_stale_price_details(chainlink_error);
    let provider_age_ms = structured
        .as_ref()
        .map(|details| details.provider_age_ms.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let receive_age_ms = structured
        .as_ref()
        .map(|details| details.receive_age_ms.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let gap_ms = gap_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "asset={asset}; snapshot_source=chainlink_rtds_start_tick; market_slug={market_slug}; primary_source={CURRENT_PRICE_SOURCE_CHAINLINK}; gap_ms={gap_ms}; provider_age_ms={provider_age_ms}; receive_age_ms={receive_age_ms}; awaiting_current_price_tick=true; chainlink_error={chainlink_error}"
    )
}

pub(super) fn map_current_price_error(
    current_source: PriceToBeatCurrentPriceSource,
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    current_price_error: &str,
) -> (&'static str, String) {
    if current_source == PriceToBeatCurrentPriceSource::Chainlink
        && snapshot_source == PriceToBeatSource::ChainlinkRtdsStartTick
        && is_retryable_chainlink_current_price_error(current_price_error)
    {
        return (
            "price_to_beat_pending",
            format_chainlink_rtds_pending_detail(
                market_slug,
                asset,
                snapshot_gap_ms,
                current_price_error,
            ),
        );
    }

    (
        "current_price_unavailable",
        format!(
            "asset={asset}; market_slug={market_slug}; primary_source={}; selected_current_price_source={}; current_price_error={current_price_error}",
            current_source.current_price_source_label(),
            current_source.as_config_str(),
        ),
    )
}

fn resolve_current_price_result_for_source(
    current_source: PriceToBeatCurrentPriceSource,
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    current_result: std::result::Result<f64, &str>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    match current_result {
        Ok(price) => Ok((price, current_source.current_price_source_label())),
        Err(current_price_error) => Err(map_current_price_error(
            current_source,
            snapshot_source,
            market_slug,
            asset,
            snapshot_gap_ms,
            current_price_error,
        )),
    }
}

#[cfg(test)]
pub(super) fn resolve_current_price_result(
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    chainlink_result: std::result::Result<f64, &str>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    resolve_current_price_result_for_source(
        PriceToBeatCurrentPriceSource::Chainlink,
        snapshot_source,
        market_slug,
        asset,
        snapshot_gap_ms,
        chainlink_result,
    )
}

pub(crate) fn resolve_price_to_beat_current_price_snapshot(
    current_source: PriceToBeatCurrentPriceSource,
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    if let Some(venue) = current_source.cex_venue() {
        let config = CexMicrostructureSnapshotConfig::default();
        let cex_result = get_cex_current_price_snapshot(asset, venue, &config)
            .map(|snapshot| snapshot.mid)
            .map_err(|err| err.to_string());
        return resolve_current_price_result_for_source(
            current_source,
            snapshot_source,
            market_slug,
            asset,
            snapshot_gap_ms,
            cex_result
                .as_ref()
                .map(|price| *price)
                .map_err(|err| err.as_str()),
        );
    }

    let chainlink_result = get_chainlink_price_cached(asset).map_err(|err| err.to_string());
    resolve_current_price_result_for_source(
        current_source,
        snapshot_source,
        market_slug,
        asset,
        snapshot_gap_ms,
        chainlink_result
            .as_ref()
            .map(|price| *price)
            .map_err(|err| err.as_str()),
    )
}

pub(super) async fn resolve_price_to_beat_current_price(
    current_source: PriceToBeatCurrentPriceSource,
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    resolve_price_to_beat_current_price_snapshot(
        current_source,
        snapshot_source,
        market_slug,
        asset,
        snapshot_gap_ms,
    )
}

pub(super) fn format_current_price_label(source: &str) -> String {
    match source {
        CURRENT_PRICE_SOURCE_CHAINLINK => "Current (Chainlink)".to_string(),
        CURRENT_PRICE_SOURCE_BINANCE => "Current (Binance)".to_string(),
        CURRENT_PRICE_SOURCE_COINBASE => "Current (Coinbase)".to_string(),
        other => format!("Current ({other})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, seed_cex_book_test_sample, CexBookSample,
    };
    use chrono::Utc;

    fn seed_current_book(venue: CexVenue, mid: f64) {
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_book_test_sample(CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: now_ms,
            bid: mid - 1.0,
            ask: mid + 1.0,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        });
    }

    #[test]
    fn current_source_parse_defaults_to_chainlink() {
        assert_eq!(
            PriceToBeatCurrentPriceSource::parse(None),
            PriceToBeatCurrentPriceSource::Chainlink
        );
        assert_eq!(
            PriceToBeatCurrentPriceSource::parse(Some("binance")),
            PriceToBeatCurrentPriceSource::Binance
        );
        assert_eq!(
            PriceToBeatCurrentPriceSource::parse(Some("coinbase")),
            PriceToBeatCurrentPriceSource::Coinbase
        );
    }

    #[test]
    fn resolves_binance_current_price_from_fresh_book_ticker() {
        clear_cex_microstructure_test_state();
        seed_current_book(CexVenue::Binance, 67_500.0);

        let resolved = resolve_price_to_beat_current_price_snapshot(
            PriceToBeatCurrentPriceSource::Binance,
            PriceToBeatSource::Polymarket,
            "btc-updown-5m-1774013100",
            "btc",
            None,
        )
        .expect("binance current price");

        assert_eq!(resolved, (67_500.0, CURRENT_PRICE_SOURCE_BINANCE));
    }

    #[test]
    fn resolves_coinbase_current_price_from_fresh_book_ticker() {
        clear_cex_microstructure_test_state();
        seed_current_book(CexVenue::Coinbase, 67_480.0);

        let resolved = resolve_price_to_beat_current_price_snapshot(
            PriceToBeatCurrentPriceSource::Coinbase,
            PriceToBeatSource::Polymarket,
            "btc-updown-5m-1774013100",
            "btc",
            None,
        )
        .expect("coinbase current price");

        assert_eq!(resolved, (67_480.0, CURRENT_PRICE_SOURCE_COINBASE));
    }

    #[test]
    fn selected_cex_missing_blocks_without_chainlink_fallback() {
        clear_cex_microstructure_test_state();

        let error = resolve_price_to_beat_current_price_snapshot(
            PriceToBeatCurrentPriceSource::Binance,
            PriceToBeatSource::Polymarket,
            "btc-updown-5m-1774013100",
            "btc",
            None,
        )
        .expect_err("missing binance should block");

        assert_eq!(error.0, "current_price_unavailable");
        assert!(error.1.contains("selected_current_price_source=binance"));
        assert!(error.1.contains("primary_source=binance_cex_ws_mid"));
    }
}
