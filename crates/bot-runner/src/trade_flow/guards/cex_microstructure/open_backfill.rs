use super::{
    binance::binance_asset_symbol,
    bybit::bybit_symbol,
    coinbase::coinbase_product_id,
    types::{parse_f64, parse_i64, CexBookSample, CexVenue},
};
use anyhow::{anyhow, Context, Result};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::time::Duration;

const BINANCE_KLINES_URL: &str = "https://api.binance.com/api/v3/klines";
const BYBIT_KLINE_URL: &str = "https://api.bybit.com/v5/market/kline";
const COINBASE_CANDLES_BASE_URL: &str = "https://api.exchange.coinbase.com/products";
const CANDLE_MS: i64 = 60_000;

pub(crate) async fn fetch_cex_window_open_book(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
) -> Result<CexBookSample> {
    let open = match venue {
        CexVenue::Binance => fetch_binance_open(asset, window_start_ms).await?,
        CexVenue::Bybit => fetch_bybit_open(asset, window_start_ms).await?,
        CexVenue::Coinbase => fetch_coinbase_open(asset, window_start_ms).await?,
        CexVenue::Hyperliquid => return Err(anyhow!("hyperliquid REST open backfill unsupported")),
    };
    Ok(open_book_sample(asset, venue, window_start_ms, open))
}

fn http_client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn fetch_binance_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let symbol = binance_asset_symbol(asset)
        .ok_or_else(|| anyhow!("unsupported binance asset={asset}"))?
        .to_ascii_uppercase();
    let response = http_client()
        .get(BINANCE_KLINES_URL)
        .query(&[
            ("symbol", symbol.as_str()),
            ("interval", "1m"),
            ("startTime", &window_start_ms.to_string()),
            ("endTime", &(window_start_ms + CANDLE_MS).to_string()),
            ("limit", "1"),
        ])
        .header(reqwest::header::USER_AGENT, "polymarketbot/cex-open-backfill")
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let rows = payload
        .as_array()
        .ok_or_else(|| anyhow!("binance klines response was not an array"))?;
    rows.iter()
        .filter_map(|row| parse_array_candle_open(row, 0, 1, 1))
        .find(|(start_ms, _)| candle_covers(*start_ms, window_start_ms))
        .map(|(_, open)| open)
        .ok_or_else(|| anyhow!("binance open candle missing for window_start_ms={window_start_ms}"))
}

async fn fetch_bybit_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let symbol = bybit_symbol(asset).ok_or_else(|| anyhow!("unsupported bybit asset={asset}"))?;
    let response = http_client()
        .get(BYBIT_KLINE_URL)
        .query(&[
            ("category", "spot"),
            ("symbol", symbol),
            ("interval", "1"),
            ("start", &window_start_ms.to_string()),
            ("end", &(window_start_ms + CANDLE_MS).to_string()),
            ("limit", "1"),
        ])
        .header(reqwest::header::USER_AGENT, "polymarketbot/cex-open-backfill")
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let rows = payload
        .get("result")
        .and_then(|value| value.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("bybit kline response missing result.list"))?;
    rows.iter()
        .filter_map(|row| parse_array_candle_open(row, 0, 1, 1))
        .find(|(start_ms, _)| candle_covers(*start_ms, window_start_ms))
        .map(|(_, open)| open)
        .ok_or_else(|| anyhow!("bybit open candle missing for window_start_ms={window_start_ms}"))
}

async fn fetch_coinbase_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let product_id =
        coinbase_product_id(asset).ok_or_else(|| anyhow!("unsupported coinbase asset={asset}"))?;
    let start = Utc
        .timestamp_millis_opt(window_start_ms)
        .single()
        .context("invalid coinbase candle start timestamp")?
        .to_rfc3339();
    let end = Utc
        .timestamp_millis_opt(window_start_ms + CANDLE_MS)
        .single()
        .context("invalid coinbase candle end timestamp")?
        .to_rfc3339();
    let url = format!("{COINBASE_CANDLES_BASE_URL}/{product_id}/candles");
    let response = http_client()
        .get(url)
        .query(&[
            ("granularity", "60"),
            ("start", start.as_str()),
            ("end", end.as_str()),
        ])
        .header(reqwest::header::USER_AGENT, "polymarketbot/cex-open-backfill")
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let rows = payload
        .as_array()
        .ok_or_else(|| anyhow!("coinbase candles response was not an array"))?;
    rows.iter()
        .filter_map(|row| parse_array_candle_open(row, 0, 3, 1_000))
        .find(|(start_ms, _)| candle_covers(*start_ms, window_start_ms))
        .map(|(_, open)| open)
        .ok_or_else(|| anyhow!("coinbase open candle missing for window_start_ms={window_start_ms}"))
}

fn parse_array_candle_open(
    row: &Value,
    timestamp_index: usize,
    open_index: usize,
    timestamp_multiplier: i64,
) -> Option<(i64, f64)> {
    let row = row.as_array()?;
    let timestamp = parse_i64(row.get(timestamp_index))?.checked_mul(timestamp_multiplier)?;
    let open = parse_f64(row.get(open_index))?;
    (open.is_finite() && open > 0.0).then_some((timestamp, open))
}

fn candle_covers(candle_start_ms: i64, window_start_ms: i64) -> bool {
    candle_start_ms <= window_start_ms && window_start_ms < candle_start_ms + CANDLE_MS
}

fn open_book_sample(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
    open_mid: f64,
) -> CexBookSample {
    CexBookSample {
        venue,
        asset: asset.trim().to_ascii_lowercase(),
        timestamp_ms: window_start_ms,
        bid: open_mid,
        ask: open_mid,
        bid_size: None,
        ask_size: None,
        source: "rest_open",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_binance_style_open() {
        let row = json!([1774013100000i64, "100.5"]);

        assert_eq!(
            parse_array_candle_open(&row, 0, 1, 1),
            Some((1774013100000, 100.5))
        );
    }

    #[test]
    fn parses_coinbase_style_open() {
        let row = json!([1774013100i64, 99.0, 101.0, 100.5, 100.8]);

        assert_eq!(
            parse_array_candle_open(&row, 0, 3, 1_000),
            Some((1774013100000, 100.5))
        );
    }
}
