use super::types::{parse_f64, parse_i64, CexBookSample, CexVenue};
use serde_json::{json, Value};

pub(crate) const GATEIO_DEFAULT_WS_URL: &str = "wss://api.gateio.ws/ws/v4/";
pub(crate) const GATEIO_WS_URL_ENV: &str = "EARLY_STALE_GATEIO_WS_URL";

pub(crate) fn gateio_ws_url() -> String {
    std::env::var(GATEIO_WS_URL_ENV).unwrap_or_else(|_| GATEIO_DEFAULT_WS_URL.to_string())
}

pub(crate) fn gateio_currency_pair(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC_USDT"),
        "eth" => Some("ETH_USDT"),
        "sol" => Some("SOL_USDT"),
        "xrp" => Some("XRP_USDT"),
        "doge" | "dogecoin" => Some("DOGE_USDT"),
        "bnb" => Some("BNB_USDT"),
        _ => None,
    }
}

pub(crate) fn gateio_subscription_message(asset: &str) -> Option<Value> {
    let pair = gateio_currency_pair(asset)?;
    Some(json!({
        "time": chrono::Utc::now().timestamp(),
        "channel": "spot.book_ticker",
        "event": "subscribe",
        "payload": [pair],
    }))
}

pub(crate) fn parse_gateio_ws_payload(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    if payload.get("channel").and_then(Value::as_str) != Some("spot.book_ticker") {
        return None;
    }
    if payload.get("event").and_then(Value::as_str) != Some("update") {
        return None;
    }
    let result = payload.get("result")?;
    let bid = parse_f64(result.get("b"))?;
    let ask = parse_f64(result.get("a"))?;
    valid_bid_ask(bid, ask)?;
    Some(CexBookSample {
        venue: CexVenue::Gateio,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(result.get("t"))
            .or_else(|| parse_i64(payload.get("time_ms")))
            .unwrap_or(now_ms),
        bid,
        ask,
        bid_size: parse_f64(result.get("B")),
        ask_size: parse_f64(result.get("A")),
        source: "book_ticker",
    })
}

fn valid_bid_ask(bid: f64, ask: f64) -> Option<()> {
    (bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gateio_symbol_supports_target_assets() {
        assert_eq!(gateio_currency_pair("btc"), Some("BTC_USDT"));
        assert_eq!(gateio_currency_pair("eth"), Some("ETH_USDT"));
        assert_eq!(gateio_currency_pair("sol"), Some("SOL_USDT"));
        assert_eq!(gateio_currency_pair("xrp"), Some("XRP_USDT"));
        assert_eq!(gateio_currency_pair("doge"), Some("DOGE_USDT"));
        assert_eq!(gateio_currency_pair("dogecoin"), Some("DOGE_USDT"));
        assert_eq!(gateio_currency_pair("bnb"), Some("BNB_USDT"));
        assert_eq!(gateio_currency_pair("hype"), None);
    }

    #[test]
    fn gateio_subscription_uses_book_ticker() {
        let message = gateio_subscription_message("btc").expect("subscription");

        assert_eq!(message["channel"], "spot.book_ticker");
        assert_eq!(message["event"], "subscribe");
        assert_eq!(message["payload"][0], "BTC_USDT");
    }

    #[test]
    fn gateio_book_ticker_extracts_top_of_book() {
        let book = parse_gateio_ws_payload(
            &json!({
                "time": 1606293275,
                "time_ms": 1606293275723i64,
                "channel": "spot.book_ticker",
                "event": "update",
                "result": {
                    "t": 1606293275123i64,
                    "u": 48733182,
                    "s": "BTC_USDT",
                    "b": "19177.79",
                    "B": "0.0003341504",
                    "a": "19179.38",
                    "A": "0.09"
                }
            }),
            "btc",
            999,
        )
        .expect("book");

        assert_eq!(book.venue, CexVenue::Gateio);
        assert_eq!(book.bid, 19177.79);
        assert_eq!(book.ask, 19179.38);
        assert_eq!(book.bid_size, Some(0.0003341504));
        assert_eq!(book.ask_size, Some(0.09));
        assert_eq!(book.timestamp_ms, 1606293275123);
        assert_eq!(book.source, "book_ticker");
    }

    #[test]
    fn gateio_book_ticker_rejects_crossed_book() {
        let book = parse_gateio_ws_payload(
            &json!({
                "channel": "spot.book_ticker",
                "event": "update",
                "result": {
                    "t": 1606293275123i64,
                    "s": "BTC_USDT",
                    "b": "101.0",
                    "B": "1.0",
                    "a": "100.0",
                    "A": "1.0"
                }
            }),
            "btc",
            999,
        );

        assert!(book.is_none());
    }
}
