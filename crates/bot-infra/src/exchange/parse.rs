use super::*;

const SUPPORTED_UPDOWN_SLUG_PREFIXES: [&str; 11] = [
    "btc-updown-5m-",
    "btc-updown-15m-",
    "eth-updown-5m-",
    "eth-updown-15m-",
    "sol-updown-5m-",
    "sol-updown-15m-",
    "xrp-updown-5m-",
    "xrp-updown-15m-",
    "doge-updown-5m-",
    "bnb-updown-5m-",
    "hype-updown-5m-",
];

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DataApiInventoryPosition {
    pub(crate) asset: Option<String>,
    #[serde(rename = "tokenId")]
    pub(crate) token_id: Option<String>,
    #[serde(rename = "clobTokenId")]
    pub(crate) clob_token_id: Option<String>,
    pub(crate) size: Option<Value>,
    pub(crate) balance: Option<Value>,
}

fn parse_string_array(v: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|x| x.as_str().map(ToString::to_string))
            .collect();
    }

    if let Some(raw) = v.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(raw) {
            return parsed;
        }
    }

    Vec::new()
}

pub(crate) fn parse_json_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(v)) => v.as_f64(),
        Some(Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}

pub(crate) fn parse_gamma_market(item: &serde_json::Value) -> Option<GammaMarket> {
    parse_gamma_market_impl(item, true)
}

pub(crate) fn parse_gamma_market_any(item: &serde_json::Value) -> Option<GammaMarket> {
    parse_gamma_market_impl(item, false)
}

fn parse_gamma_market_impl(
    item: &serde_json::Value,
    require_supported_updown_slug: bool,
) -> Option<GammaMarket> {
    let slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if slug.is_empty() {
        return None;
    }
    if require_supported_updown_slug
        && !SUPPORTED_UPDOWN_SLUG_PREFIXES
            .iter()
            .any(|prefix| slug.starts_with(prefix))
    {
        return None;
    }

    let (yes_token_id, no_token_id) = parse_yes_no_token_ids(item);
    let maker_base_fee = item
        .get("makerBaseFee")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let neg_risk = item
        .get("negRisk")
        .or_else(|| item.get("neg_risk"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let order_price_min_tick_size = parse_json_f64(
        item.get("orderPriceMinTickSize")
            .or_else(|| item.get("order_price_min_tick_size"))
            .or_else(|| item.get("minimum_tick_size"))
            .or_else(|| item.get("tick_size")),
    );
    let order_min_size = parse_json_f64(
        item.get("orderMinSize")
            .or_else(|| item.get("order_min_size"))
            .or_else(|| item.get("min_order_size")),
    );
    Some(GammaMarket {
        slug,
        condition_id: item
            .get("conditionId")
            .or_else(|| item.get("condition_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        end_date_iso: item
            .get("endDate")
            .or_else(|| item.get("end_date_iso"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        active: item
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        closed: item
            .get("closed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        yes_token_id,
        no_token_id,
        maker_base_fee,
        neg_risk,
        order_price_min_tick_size,
        order_min_size,
    })
}

fn parse_outcome_side(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" => Some("yes"),
        "no" | "down" => Some("no"),
        _ => None,
    }
}

pub(crate) fn parse_yes_no_token_ids(item: &serde_json::Value) -> (Option<String>, Option<String>) {
    let direct_yes = item
        .get("yesTokenId")
        .or_else(|| item.get("yes_token_id"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let direct_no = item
        .get("noTokenId")
        .or_else(|| item.get("no_token_id"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    if direct_yes.is_some() || direct_no.is_some() {
        return (direct_yes, direct_no);
    }

    let mut outcomes = Vec::new();
    if let Some(v) = item.get("outcomes") {
        outcomes = parse_string_array(v)
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect();
    }

    let mut clob_token_ids = Vec::new();
    if let Some(v) = item
        .get("clobTokenIds")
        .or_else(|| item.get("clob_token_ids"))
    {
        clob_token_ids = parse_string_array(v);
    }

    if outcomes.len() >= 2 && clob_token_ids.len() >= 2 {
        let mut yes = None;
        let mut no = None;
        for (idx, outcome) in outcomes.iter().enumerate() {
            match parse_outcome_side(outcome) {
                Some("yes") => yes = clob_token_ids.get(idx).cloned(),
                Some("no") => no = clob_token_ids.get(idx).cloned(),
                _ => {}
            }
        }
        if yes.is_some() || no.is_some() {
            return (yes, no);
        }
    }

    if let Some(tokens) = item.get("tokens").and_then(|v| v.as_array()) {
        let mut yes = None;
        let mut no = None;
        for token in tokens {
            let outcome = token
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            let token_id = token
                .get("token_id")
                .or_else(|| token.get("tokenId"))
                .or_else(|| token.get("clobTokenId"))
                .or_else(|| token.get("id"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            match parse_outcome_side(&outcome) {
                Some("yes") if yes.is_none() => yes = token_id.clone(),
                Some("no") if no.is_none() => no = token_id.clone(),
                _ => {}
            }
        }
        if yes.is_some() || no.is_some() {
            return (yes, no);
        }
    }

    if clob_token_ids.len() >= 2 {
        return (
            clob_token_ids.first().cloned(),
            clob_token_ids.get(1).cloned(),
        );
    }

    (None, None)
}

pub(crate) fn parse_f64_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(v) => v.as_f64(),
        Value::String(v) => v.parse::<f64>().ok(),
        _ => None,
    }
}
