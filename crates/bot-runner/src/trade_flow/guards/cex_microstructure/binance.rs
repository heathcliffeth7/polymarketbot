use super::types::{parse_f64, parse_i64, CexBookSample, CexTradeSample, CexVenue, TakerSide};
use serde_json::{json, Value};

pub(crate) const BINANCE_DEFAULT_WS_URL: &str = "wss://stream.binance.com:9443/stream";
pub(crate) const BINANCE_WS_URL_ENV: &str = "EARLY_STALE_BINANCE_WS_URL";

pub(crate) fn binance_asset_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("btcusdt"),
        "eth" => Some("ethusdt"),
        "sol" => Some("solusdt"),
        "xrp" => Some("xrpusdt"),
        "doge" | "dogecoin" => Some("dogeusdt"),
        "bnb" => Some("bnbusdt"),
        _ => None,
    }
}

pub(crate) fn binance_stream_url(asset: &str) -> Option<String> {
    let symbol = binance_asset_symbol(asset)?;
    let base = std::env::var(BINANCE_WS_URL_ENV)
        .unwrap_or_else(|_| BINANCE_DEFAULT_WS_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    Some(format!(
        "{base}?streams={symbol}@aggTrade/{symbol}@bookTicker/{symbol}@depth5@100ms"
    ))
}

pub(crate) fn parse_binance_ws_payload(payload: &Value, asset: &str, now_ms: i64) -> ParsedBinance {
    let data = payload.get("data").unwrap_or(payload);
    match resolve_binance_payload_kind(payload, data) {
        Some(BinancePayloadKind::AggTrade) => ParsedBinance {
            trade: parse_binance_agg_trade(data, asset),
            book: None,
        },
        Some(BinancePayloadKind::BookTicker) => ParsedBinance {
            trade: None,
            book: parse_binance_book_ticker(data, asset, now_ms),
        },
        Some(BinancePayloadKind::Depth) => ParsedBinance {
            trade: None,
            book: parse_binance_depth(data, asset, now_ms),
        },
        _ => ParsedBinance::default(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinancePayloadKind {
    AggTrade,
    BookTicker,
    Depth,
}

fn resolve_binance_payload_kind(payload: &Value, data: &Value) -> Option<BinancePayloadKind> {
    if let Some(event) = data
        .get("e")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|event| !event.is_empty())
    {
        return match event {
            "aggTrade" => Some(BinancePayloadKind::AggTrade),
            "bookTicker" => Some(BinancePayloadKind::BookTicker),
            "depthUpdate" => Some(BinancePayloadKind::Depth),
            _ => None,
        };
    }

    let stream = payload.get("stream").and_then(Value::as_str)?.trim();
    let topic = stream
        .split_once('@')
        .map(|(_, topic)| topic)
        .unwrap_or(stream);
    if topic.eq_ignore_ascii_case("bookTicker") {
        return Some(BinancePayloadKind::BookTicker);
    }

    let topic = topic.to_ascii_lowercase();
    (topic == "depth5" || topic.starts_with("depth5@")).then_some(BinancePayloadKind::Depth)
}

#[derive(Debug, Default)]
pub(crate) struct ParsedBinance {
    pub(crate) trade: Option<CexTradeSample>,
    pub(crate) book: Option<CexBookSample>,
}

pub(crate) fn parse_binance_agg_trade(payload: &Value, asset: &str) -> Option<CexTradeSample> {
    let price = parse_f64(payload.get("p"))?;
    let size = parse_f64(payload.get("q"))?;
    let timestamp_ms = parse_i64(payload.get("T").or_else(|| payload.get("E")))?;
    let buyer_is_maker = payload.get("m")?.as_bool()?;
    Some(CexTradeSample {
        venue: CexVenue::Binance,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms,
        price,
        size,
        taker_side: if buyer_is_maker {
            TakerSide::Sell
        } else {
            TakerSide::Buy
        },
    })
}

pub(crate) fn parse_binance_book_ticker(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let bid = parse_f64(payload.get("b"))?;
    let ask = parse_f64(payload.get("a"))?;
    valid_bid_ask(bid, ask)?;
    Some(CexBookSample {
        venue: CexVenue::Binance,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(payload.get("E")).unwrap_or(now_ms),
        bid,
        ask,
        bid_size: parse_f64(payload.get("B")),
        ask_size: parse_f64(payload.get("A")),
        source: "bookTicker",
    })
}

pub(crate) fn parse_binance_depth(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let best_bid = payload
        .get("b")
        .or_else(|| payload.get("bids"))?
        .as_array()?
        .iter()
        .filter_map(parse_depth_level)
        .max_by(|left, right| left.0.total_cmp(&right.0))?;
    let best_ask = payload
        .get("a")
        .or_else(|| payload.get("asks"))?
        .as_array()?
        .iter()
        .filter_map(parse_depth_level)
        .min_by(|left, right| left.0.total_cmp(&right.0))?;
    valid_bid_ask(best_bid.0, best_ask.0)?;
    Some(CexBookSample {
        venue: CexVenue::Binance,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(payload.get("E")).unwrap_or(now_ms),
        bid: best_bid.0,
        ask: best_ask.0,
        bid_size: Some(best_bid.1),
        ask_size: Some(best_ask.1),
        source: "depth5",
    })
}

fn parse_depth_level(value: &Value) -> Option<(f64, f64)> {
    let row = value.as_array()?;
    let price = parse_f64(row.first())?;
    let size = parse_f64(row.get(1))?;
    (price.is_finite() && price > 0.0 && size.is_finite() && size >= 0.0).then_some((price, size))
}

fn valid_bid_ask(bid: f64, ask: f64) -> Option<()> {
    (bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask).then_some(())
}

#[allow(dead_code)]
pub(crate) fn binance_subscription_hint(asset: &str) -> Option<Value> {
    let symbol = binance_asset_symbol(asset)?;
    Some(json!({
        "streams": [
            format!("{symbol}@aggTrade"),
            format!("{symbol}@bookTicker"),
            format!("{symbol}@depth5@100ms")
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binance_symbol_supports_bnb_and_doge_but_not_hype() {
        assert_eq!(binance_asset_symbol("doge"), Some("dogeusdt"));
        assert_eq!(binance_asset_symbol("dogecoin"), Some("dogeusdt"));
        assert_eq!(binance_asset_symbol("bnb"), Some("bnbusdt"));
        assert_eq!(binance_asset_symbol("hype"), None);
    }

    #[test]
    fn binance_aggtrade_m_true_counts_as_taker_sell() {
        let trade = parse_binance_agg_trade(
            &json!({"e":"aggTrade","p":"67500.0","q":"0.10","T":123,"m":true}),
            "btc",
        )
        .expect("trade");

        assert_eq!(trade.taker_side, TakerSide::Sell);
    }

    #[test]
    fn binance_aggtrade_m_false_counts_as_taker_buy() {
        let trade = parse_binance_agg_trade(
            &json!({"e":"aggTrade","p":"67500.0","q":"0.10","T":123,"m":false}),
            "btc",
        )
        .expect("trade");

        assert_eq!(trade.taker_side, TakerSide::Buy);
    }

    #[test]
    fn binance_depth_extracts_top_of_book() {
        let book = parse_binance_depth(
            &json!({
                "e":"depthUpdate",
                "E":500,
                "b":[["67499.0","1.0"],["67500.0","0.5"]],
                "a":[["67502.0","0.4"],["67501.0","0.7"]]
            }),
            "btc",
            999,
        )
        .expect("book");

        assert_eq!(book.bid, 67500.0);
        assert_eq!(book.ask, 67501.0);
        assert_eq!(book.source, "depth5");
    }

    #[test]
    fn combined_stream_book_ticker_extracts_top_of_book() {
        let parsed = parse_binance_ws_payload(
            &json!({
                "stream": "btcusdt@bookTicker",
                "data": {
                    "u": 93054243644i64,
                    "s": "BTCUSDT",
                    "b": "78747.50000000",
                    "B": "0.98092000",
                    "a": "78747.51000000",
                    "A": "1.39515000"
                }
            }),
            "btc",
            999,
        );
        let book = parsed.book.expect("book");

        assert!(parsed.trade.is_none());
        assert_eq!(book.bid, 78747.5);
        assert_eq!(book.ask, 78747.51);
        assert_eq!(book.bid_size, Some(0.98092));
        assert_eq!(book.ask_size, Some(1.39515));
        assert_eq!(book.timestamp_ms, 999);
        assert_eq!(book.source, "bookTicker");
    }

    #[test]
    fn combined_stream_depth_extracts_top_of_book() {
        let parsed = parse_binance_ws_payload(
            &json!({
                "stream": "btcusdt@depth5@100ms",
                "data": {
                    "lastUpdateId": 93054243644i64,
                    "bids": [["78747.50", "0.98"], ["78747.49", "0.70"]],
                    "asks": [["78747.51", "1.39"], ["78748.68", "0.07"]]
                }
            }),
            "btc",
            999,
        );
        let book = parsed.book.expect("book");

        assert!(parsed.trade.is_none());
        assert_eq!(book.bid, 78747.5);
        assert_eq!(book.ask, 78747.51);
        assert_eq!(book.bid_size, Some(0.98));
        assert_eq!(book.ask_size, Some(1.39));
        assert_eq!(book.timestamp_ms, 999);
        assert_eq!(book.source, "depth5");
    }
}
