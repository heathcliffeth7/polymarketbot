use super::*;
use crate::trade_flow::guards::chainlink_price::parse_chainlink_stale_price_details;

pub(super) const CURRENT_PRICE_SOURCE_CHAINLINK: &str = "chainlink_live_data_ws";

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
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    chainlink_error: &str,
) -> (&'static str, String) {
    if snapshot_source == PriceToBeatSource::ChainlinkRtdsStartTick
        && is_retryable_chainlink_current_price_error(chainlink_error)
    {
        return (
            "price_to_beat_pending",
            format_chainlink_rtds_pending_detail(
                market_slug,
                asset,
                snapshot_gap_ms,
                chainlink_error,
            ),
        );
    }

    (
        "current_price_unavailable",
        format!(
            "asset={asset}; market_slug={market_slug}; primary_source={CURRENT_PRICE_SOURCE_CHAINLINK}; chainlink_error={chainlink_error}"
        ),
    )
}

pub(super) fn resolve_current_price_result(
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    chainlink_result: std::result::Result<f64, &str>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    match chainlink_result {
        Ok(price) => Ok((price, CURRENT_PRICE_SOURCE_CHAINLINK)),
        Err(chainlink_error) => Err(map_current_price_error(
            snapshot_source,
            market_slug,
            asset,
            snapshot_gap_ms,
            chainlink_error,
        )),
    }
}

pub(super) async fn resolve_price_to_beat_current_price(
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    let chainlink_result = get_chainlink_price_cached(asset).map_err(|err| err.to_string());
    resolve_current_price_result(
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

pub(super) fn format_current_price_label(source: &str) -> String {
    match source {
        CURRENT_PRICE_SOURCE_CHAINLINK => "Current (Chainlink)".to_string(),
        other => format!("Current ({other})"),
    }
}
