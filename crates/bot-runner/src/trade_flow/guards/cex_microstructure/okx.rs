use super::types::{parse_f64, parse_i64, CexBookSample, CexVenue};
use serde_json::{json, Value};

pub(crate) const OKX_DEFAULT_WS_URL: &str = "wss://ws.okx.com:8443/ws/v5/public";
pub(crate) const OKX_WS_URL_ENV: &str = "EARLY_STALE_OKX_WS_URL";

pub(crate) fn okx_ws_url() -> String {
    std::env::var(OKX_WS_URL_ENV).unwrap_or_else(|_| OKX_DEFAULT_WS_URL.to_string())
}

pub(crate) fn okx_inst_id(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC-USDT"),
        "eth" => Some("ETH-USDT"),
        "sol" => Some("SOL-USDT"),
        "xrp" => Some("XRP-USDT"),
        "doge" | "dogecoin" => Some("DOGE-USDT"),
        "bnb" => Some("BNB-USDT"),
        _ => None,
    }
}

pub(crate) fn okx_subscription_message(asset: &str) -> Option<Value> {
    let inst_id = okx_inst_id(asset)?;
    Some(json!({
        "op": "subscribe",
        "args": [{
            "channel": "books5",
            "instId": inst_id,
        }],
    }))
}

pub(crate) fn parse_okx_ws_payload(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Vec<CexBookSample> {
    if payload.get("event").is_some() {
        return Vec::new();
    }
    let channel = payload
        .get("arg")
        .and_then(|arg| arg.get("channel"))
        .and_then(Value::as_str);
    if channel != Some("books5") {
        return Vec::new();
    }
    payload
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|data| parse_okx_books5(data, asset, now_ms))
        .collect()
}

fn parse_okx_books5(data: &Value, asset: &str, now_ms: i64) -> Option<CexBookSample> {
    let best_bid = data
        .get("bids")?
        .as_array()?
        .iter()
        .filter_map(parse_level)
        .max_by(|left, right| left.0.total_cmp(&right.0))?;
    let best_ask = data
        .get("asks")?
        .as_array()?
        .iter()
        .filter_map(parse_level)
        .min_by(|left, right| left.0.total_cmp(&right.0))?;
    valid_bid_ask(best_bid.0, best_ask.0)?;
    Some(CexBookSample {
        venue: CexVenue::Okx,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(data.get("ts")).unwrap_or(now_ms),
        bid: best_bid.0,
        ask: best_ask.0,
        bid_size: Some(best_bid.1),
        ask_size: Some(best_ask.1),
        source: "books5",
    })
}

fn parse_level(value: &Value) -> Option<(f64, f64)> {
    let row = value.as_array()?;
    let price = parse_f64(row.first())?;
    let size = parse_f64(row.get(1))?;
    (price.is_finite() && price > 0.0 && size.is_finite() && size >= 0.0).then_some((price, size))
}

fn valid_bid_ask(bid: f64, ask: f64) -> Option<()> {
    (bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn okx_symbol_supports_target_assets() {
        assert_eq!(okx_inst_id("btc"), Some("BTC-USDT"));
        assert_eq!(okx_inst_id("eth"), Some("ETH-USDT"));
        assert_eq!(okx_inst_id("sol"), Some("SOL-USDT"));
        assert_eq!(okx_inst_id("xrp"), Some("XRP-USDT"));
        assert_eq!(okx_inst_id("doge"), Some("DOGE-USDT"));
        assert_eq!(okx_inst_id("dogecoin"), Some("DOGE-USDT"));
        assert_eq!(okx_inst_id("bnb"), Some("BNB-USDT"));
        assert_eq!(okx_inst_id("hype"), None);
    }

    #[test]
    fn okx_subscription_uses_books5() {
        let message = okx_subscription_message("btc").expect("subscription");

        assert_eq!(message["op"], "subscribe");
        assert_eq!(message["args"][0]["channel"], "books5");
        assert_eq!(message["args"][0]["instId"], "BTC-USDT");
    }

    #[test]
    fn okx_books5_extracts_top_of_book() {
        let books = parse_okx_ws_payload(
            &json!({
                "arg": { "channel": "books5", "instId": "BTC-USDT" },
                "data": [{
                    "asks": [["67502.0", "0.4", "0", "1"], ["67501.0", "0.7", "0", "1"]],
                    "bids": [["67499.0", "1.0", "0", "1"], ["67500.0", "0.5", "0", "1"]],
                    "ts": "1774013100123"
                }]
            }),
            "btc",
            999,
        );

        assert_eq!(books.len(), 1);
        let book = &books[0];
        assert_eq!(book.venue, CexVenue::Okx);
        assert_eq!(book.bid, 67500.0);
        assert_eq!(book.ask, 67501.0);
        assert_eq!(book.bid_size, Some(0.5));
        assert_eq!(book.ask_size, Some(0.7));
        assert_eq!(book.timestamp_ms, 1774013100123);
        assert_eq!(book.source, "books5");
    }

    #[test]
    fn okx_books5_rejects_crossed_book() {
        let books = parse_okx_ws_payload(
            &json!({
                "arg": { "channel": "books5", "instId": "BTC-USDT" },
                "data": [{
                    "asks": [["100.0", "1", "0", "1"]],
                    "bids": [["101.0", "1", "0", "1"]],
                    "ts": "1774013100123"
                }]
            }),
            "btc",
            999,
        );

        assert!(books.is_empty());
    }
}
