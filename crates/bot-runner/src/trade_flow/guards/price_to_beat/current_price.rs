use super::*;
use crate::trade_flow::guards::chainlink_price::parse_chainlink_stale_price_details;

pub(super) const CURRENT_PRICE_SOURCE_CHAINLINK: &str = "chainlink_live_data_ws";
pub(super) const CURRENT_PRICE_SOURCE_COINBASE_FALLBACK: &str = "coinbase_spot_fallback";
pub(super) const CURRENT_PRICE_FALLBACK_TIMEOUT_MS: u64 = 1_000;

fn is_retryable_chainlink_current_price_error(detail: &str) -> bool {
    detail.starts_with("stale price for ") || detail.starts_with("no cached price for ")
}

fn format_chainlink_carryover_pending_detail(
    market_slug: &str,
    asset: &str,
    gap_ms: Option<i64>,
    chainlink_error: &str,
    fallback_error: Option<&str>,
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
    let fallback_error = fallback_error.unwrap_or("none");
    format!(
        "asset={asset}; snapshot_source=chainlink_carryover; market_slug={market_slug}; primary_source={CURRENT_PRICE_SOURCE_CHAINLINK}; fallback_source=coinbase_spot; gap_ms={gap_ms}; provider_age_ms={provider_age_ms}; receive_age_ms={receive_age_ms}; awaiting_authoritative_polymarket_snapshot=true; chainlink_error={chainlink_error}; fallback_error={fallback_error}"
    )
}

pub(super) fn map_current_price_error(
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    chainlink_error: &str,
    fallback_error: Option<&str>,
) -> (&'static str, String) {
    if snapshot_source == PriceToBeatSource::ChainlinkCarryover
        && is_retryable_chainlink_current_price_error(chainlink_error)
    {
        return (
            "price_to_beat_pending",
            format_chainlink_carryover_pending_detail(
                market_slug,
                asset,
                snapshot_gap_ms,
                chainlink_error,
                fallback_error,
            ),
        );
    }

    (
        "current_price_unavailable",
        format!(
            "asset={asset}; market_slug={market_slug}; primary_source={CURRENT_PRICE_SOURCE_CHAINLINK}; fallback_source=coinbase_spot; chainlink_error={chainlink_error}; fallback_error={}",
            fallback_error.unwrap_or("none")
        ),
    )
}

fn should_fallback_to_coinbase(chainlink_error: &str) -> bool {
    is_retryable_chainlink_current_price_error(chainlink_error)
}

pub(super) fn resolve_current_price_result(
    snapshot_source: PriceToBeatSource,
    market_slug: &str,
    asset: &str,
    snapshot_gap_ms: Option<i64>,
    chainlink_result: std::result::Result<f64, &str>,
    coinbase_result: Option<std::result::Result<f64, &str>>,
) -> std::result::Result<(f64, &'static str), (&'static str, String)> {
    match chainlink_result {
        Ok(price) => Ok((price, CURRENT_PRICE_SOURCE_CHAINLINK)),
        Err(chainlink_error) if should_fallback_to_coinbase(chainlink_error) => {
            match coinbase_result {
                Some(Ok(price)) => Ok((price, CURRENT_PRICE_SOURCE_COINBASE_FALLBACK)),
                Some(Err(fallback_error)) => Err(map_current_price_error(
                    snapshot_source,
                    market_slug,
                    asset,
                    snapshot_gap_ms,
                    chainlink_error,
                    Some(fallback_error),
                )),
                None => Err(map_current_price_error(
                    snapshot_source,
                    market_slug,
                    asset,
                    snapshot_gap_ms,
                    chainlink_error,
                    Some("not_attempted"),
                )),
            }
        }
        Err(chainlink_error) => Err(map_current_price_error(
            snapshot_source,
            market_slug,
            asset,
            snapshot_gap_ms,
            chainlink_error,
            coinbase_result.and_then(Result::err),
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
    let owned_asset = asset.to_string();
    let coinbase_task = tokio::spawn(async move {
        tokio::time::timeout(
            std::time::Duration::from_millis(CURRENT_PRICE_FALLBACK_TIMEOUT_MS),
            crate::fetch_underlying_reference_current_price(&owned_asset),
        )
        .await
    });

    if let Ok(price) = chainlink_result.as_ref() {
        coinbase_task.abort();
        return Ok((*price, CURRENT_PRICE_SOURCE_CHAINLINK));
    }

    let coinbase_result: Option<std::result::Result<f64, String>> = match coinbase_task.await {
        Ok(Ok(Ok(price))) => Some(Ok(price)),
        Ok(Ok(Err(err))) => Some(Err(err.to_string())),
        Ok(Err(_elapsed)) => Some(Err("coinbase timeout".to_string())),
        Err(join_err) => Some(Err(format!("coinbase task join error: {join_err}"))),
    };

    resolve_current_price_result(
        snapshot_source,
        market_slug,
        asset,
        snapshot_gap_ms,
        chainlink_result
            .as_ref()
            .map(|price| *price)
            .map_err(|err| err.as_str()),
        coinbase_result.as_ref().map(|result| {
            result
                .as_ref()
                .map(|price| *price)
                .map_err(|err| err.as_str())
        }),
    )
}

pub(super) fn format_current_price_label(source: &str) -> String {
    match source {
        CURRENT_PRICE_SOURCE_CHAINLINK => "Current (Chainlink)".to_string(),
        CURRENT_PRICE_SOURCE_COINBASE_FALLBACK => "Current (Coinbase fallback)".to_string(),
        other => format!("Current ({other})"),
    }
}
