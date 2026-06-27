use super::{
    binance::binance_asset_symbol,
    bybit::bybit_symbol,
    coinbase::coinbase_product_id,
    gateio::gateio_currency_pair,
    okx::okx_inst_id,
    types::{parse_f64, parse_i64, CexBookSample, CexVenue},
};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::Duration;

const BINANCE_KLINES_URL: &str = "https://api.binance.com/api/v3/klines";
const BYBIT_KLINE_URL: &str = "https://api.bybit.com/v5/market/kline";
const COINBASE_CANDLES_BASE_URL: &str = "https://api.coinbase.com/api/v3/brokerage/market/products";
const OKX_CANDLES_URL: &str = "https://www.okx.com/api/v5/market/candles";
const GATEIO_CANDLES_URL: &str = "https://api.gateio.ws/api/v4/spot/candlesticks";
const CANDLE_MS: i64 = 60_000;

/// Inclusive end of the 1m candle that contains `window_start_ms`.
/// Bybit (and Coinbase) treat `end = window_start + 60_000` as the next candle boundary,
/// so `limit=1` returns the following minute; use last ms of the target minute instead.
fn cex_kline_query_end_ms(window_start_ms: i64) -> i64 {
    window_start_ms + CANDLE_MS - 1
}

fn okx_candles_after_ms(window_start_ms: i64) -> i64 {
    window_start_ms + CANDLE_MS
}

pub(crate) async fn fetch_cex_window_open_book(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
) -> Result<CexBookSample> {
    let open = match venue {
        CexVenue::Binance => fetch_binance_open(asset, window_start_ms).await?,
        CexVenue::Bybit => fetch_bybit_open(asset, window_start_ms).await?,
        CexVenue::Coinbase => fetch_coinbase_open(asset, window_start_ms).await?,
        CexVenue::Okx => fetch_okx_open(asset, window_start_ms).await?,
        CexVenue::Gateio => fetch_gateio_open(asset, window_start_ms).await?,
        CexVenue::Hyperliquid => return Err(anyhow!("hyperliquid REST open backfill unsupported")),
    };
    Ok(open_book_sample(asset, venue, window_start_ms, open))
}

fn http_client() -> reqwest::Client {
    let builder = reqwest::Client::builder();
    let builder = bot_infra::proxy::add_rotating_reqwest_proxy(builder, "cex_open_backfill");
    builder.build().expect("cex open backfill http client")
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
        .header(
            reqwest::header::USER_AGENT,
            "polymarketbot/cex-open-backfill",
        )
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
    let query_end_ms = cex_kline_query_end_ms(window_start_ms);
    for limit in [1_u32, 2] {
        if let Some(open) =
            fetch_bybit_open_with_limit(symbol, window_start_ms, query_end_ms, limit).await?
        {
            return Ok(open);
        }
    }
    Err(anyhow!(
        "bybit open candle missing for window_start_ms={window_start_ms}"
    ))
}

async fn fetch_bybit_open_with_limit(
    symbol: &str,
    window_start_ms: i64,
    query_end_ms: i64,
    limit: u32,
) -> Result<Option<f64>> {
    let response = http_client()
        .get(BYBIT_KLINE_URL)
        .query(&[
            ("category", "spot"),
            ("symbol", symbol),
            ("interval", "1"),
            ("start", &window_start_ms.to_string()),
            ("end", &query_end_ms.to_string()),
            ("limit", &limit.to_string()),
        ])
        .header(
            reqwest::header::USER_AGENT,
            "polymarketbot/cex-open-backfill",
        )
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
    Ok(select_window_candle_open(
        rows.iter(),
        window_start_ms,
        0,
        1,
        1,
    ))
}

async fn fetch_coinbase_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let product_id =
        coinbase_product_id(asset).ok_or_else(|| anyhow!("unsupported coinbase asset={asset}"))?;
    // Coinbase Advanced Trade candles endpoint requires Unix epoch seconds.
    let start = (window_start_ms / 1_000).to_string();
    let end = (cex_kline_query_end_ms(window_start_ms) / 1_000).to_string();
    let url = format!("{COINBASE_CANDLES_BASE_URL}/{product_id}/candles");
    let response = http_client()
        .get(url)
        .query(&[
            ("granularity", "ONE_MINUTE"),
            ("start", start.as_str()),
            ("end", end.as_str()),
        ])
        .header(
            reqwest::header::USER_AGENT,
            "polymarketbot/cex-open-backfill",
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let candles = payload
        .get("candles")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("coinbase candles response missing candles array"))?;
    candles
        .iter()
        .filter_map(|candle| {
            let start_s = candle.get("start").and_then(Value::as_str)?;
            let start_ms = start_s.parse::<i64>().ok()? * 1_000;
            let open = candle
                .get("open")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<f64>().ok())?;
            (open.is_finite() && open > 0.0).then_some((start_ms, open))
        })
        .find(|(start_ms, _)| candle_covers(*start_ms, window_start_ms))
        .map(|(_, open)| open)
        .ok_or_else(|| {
            anyhow!("coinbase open candle missing for window_start_ms={window_start_ms}")
        })
}

async fn fetch_okx_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let inst_id = okx_inst_id(asset).ok_or_else(|| anyhow!("unsupported okx asset={asset}"))?;
    let response = http_client()
        .get(OKX_CANDLES_URL)
        .query(&[
            ("instId", inst_id),
            ("bar", "1m"),
            ("after", &okx_candles_after_ms(window_start_ms).to_string()),
            ("limit", "2"),
        ])
        .header(
            reqwest::header::USER_AGENT,
            "polymarketbot/cex-open-backfill",
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let rows = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("okx candles response missing data array"))?;
    select_window_candle_open(rows.iter(), window_start_ms, 0, 1, 1)
        .ok_or_else(|| anyhow!("okx open candle missing for window_start_ms={window_start_ms}"))
}

async fn fetch_gateio_open(asset: &str, window_start_ms: i64) -> Result<f64> {
    let pair =
        gateio_currency_pair(asset).ok_or_else(|| anyhow!("unsupported gateio asset={asset}"))?;
    let from_s = (window_start_ms / 1_000).to_string();
    let to_s = ((window_start_ms + CANDLE_MS) / 1_000).to_string();
    let response = http_client()
        .get(GATEIO_CANDLES_URL)
        .query(&[
            ("currency_pair", pair),
            ("interval", "1m"),
            ("from", from_s.as_str()),
            ("to", to_s.as_str()),
        ])
        .header(
            reqwest::header::USER_AGENT,
            "polymarketbot/cex-open-backfill",
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let rows = payload
        .as_array()
        .ok_or_else(|| anyhow!("gateio candles response was not an array"))?;
    select_window_candle_open(rows.iter(), window_start_ms, 0, 5, 1_000)
        .ok_or_else(|| anyhow!("gateio open candle missing for window_start_ms={window_start_ms}"))
}

fn select_window_candle_open<'a>(
    rows: impl Iterator<Item = &'a Value>,
    window_start_ms: i64,
    timestamp_index: usize,
    open_index: usize,
    timestamp_multiplier: i64,
) -> Option<f64> {
    rows.filter_map(|row| {
        parse_array_candle_open(row, timestamp_index, open_index, timestamp_multiplier)
    })
    .find(|(start_ms, _)| candle_covers(*start_ms, window_start_ms))
    .map(|(_, open)| open)
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

    #[test]
    fn cex_kline_query_end_ms_is_last_ms_of_target_minute() {
        let window_start_ms = 1_780_291_200_000_i64;
        assert_eq!(
            cex_kline_query_end_ms(window_start_ms),
            window_start_ms + CANDLE_MS - 1
        );
    }

    #[test]
    fn select_window_open_rejects_bybit_next_minute_only_row() {
        let window_start_ms = 1_780_291_200_000_i64;
        let rows = vec![json!([
            "1780291260000",
            "73488.4",
            "73515",
            "73488.4",
            "73515",
            "2.704769",
            "198798.9731248"
        ])];

        assert_eq!(
            select_window_candle_open(rows.iter(), window_start_ms, 0, 1, 1),
            None
        );
    }

    #[test]
    fn select_window_open_picks_window_start_from_bybit_two_row_list() {
        let window_start_ms = 1_780_291_200_000_i64;
        let rows = vec![
            json!([
                "1780291260000",
                "73488.4",
                "73515",
                "73488.4",
                "73515",
                "2.704769",
                "198798.9731248"
            ]),
            json!([
                "1780291200000",
                "73460.4",
                "73488.4",
                "73460.4",
                "73488.4",
                "3.287841",
                "241573.3429012"
            ]),
        ];

        assert_option_f64_close(
            select_window_candle_open(rows.iter(), window_start_ms, 0, 1, 1),
            Some(73_460.4),
        );
    }

    #[test]
    fn select_window_open_picks_window_start_from_coinbase_rows() {
        let window_start_ms = 1_780_291_200_000_i64;
        let rows = vec![
            json!([1_780_291_260_i64, 99.0, 101.0, 73_488.4, 73_515.0]),
            json!([1_780_291_200_i64, 98.0, 100.0, 73_460.4, 73_488.4]),
        ];

        assert_option_f64_close(
            select_window_candle_open(rows.iter(), window_start_ms, 0, 3, 1_000),
            Some(73_460.4),
        );
    }

    #[test]
    fn select_window_open_picks_window_start_from_okx_rows() {
        let window_start_ms = 1_780_291_200_000_i64;
        let rows = vec![
            json!(["1780291260000", "73488.4", "73515", "73488.4", "73515"]),
            json!(["1780291200000", "73460.4", "73488.4", "73460.4", "73488.4"]),
        ];

        assert_option_f64_close(
            select_window_candle_open(rows.iter(), window_start_ms, 0, 1, 1),
            Some(73_460.4),
        );
    }

    #[test]
    fn select_window_open_picks_window_start_from_gateio_rows() {
        let window_start_ms = 1_781_346_660_000_i64;
        let rows = vec![
            json!([
                "1781346720",
                "44561.51714520",
                "63800.9",
                "63801",
                "63785.6",
                "63785.6",
                "0.69856600",
                "false"
            ]),
            json!([
                "1781346660",
                "171671.24562960",
                "63785.5",
                "63785.5",
                "63774.5",
                "63774.5",
                "2.69148200",
                "true"
            ]),
        ];

        assert_option_f64_close(
            select_window_candle_open(rows.iter(), window_start_ms, 0, 5, 1_000),
            Some(63_774.5),
        );
    }

    #[test]
    fn okx_candles_after_uses_next_minute_boundary() {
        let window_start_ms = 1_780_291_200_000_i64;
        assert_eq!(okx_candles_after_ms(window_start_ms), 1_780_291_260_000);
    }

    fn assert_option_f64_close(actual: Option<f64>, expected: Option<f64>) {
        match (actual, expected) {
            (Some(left), Some(right)) => assert!((left - right).abs() < 1e-9, "{left} vs {right}"),
            (None, None) => {}
            (left, right) => panic!("expected {right:?}, got {left:?}"),
        }
    }
}
