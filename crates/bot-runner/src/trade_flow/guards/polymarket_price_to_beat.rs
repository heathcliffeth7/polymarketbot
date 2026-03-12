use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use parking_lot::Mutex;
use reqwest::header::USER_AGENT;
use serde_json::Value;
use std::{collections::HashMap, sync::LazyLock};

const POLYMARKET_EVENT_BASE_URL: &str = "https://polymarket.com/event";
const POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS: u64 = 10;
const QUERY_NOT_FOUND_RETRY_ATTEMPTS: usize = 5;
const QUERY_NOT_FOUND_RETRY_DELAY_MS: u64 = 4_000;
const QUERY_NOT_FOUND_ERROR_PREFIX: &str = "price to beat query not found in polymarket page for ";

#[derive(Debug, Clone)]
pub(crate) struct PriceToBeatQuerySpec {
    pub(crate) market_slug: String,
    pub(crate) asset: String,
    pub(crate) timeframe: String,
    pub(crate) query_timeframe: &'static str,
    pub(crate) start_at: DateTime<Utc>,
    pub(crate) end_at: DateTime<Utc>,
    pub(crate) event_url: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PolymarketPriceToBeatSnapshot {
    pub(crate) event_url: String,
    pub(crate) asset: String,
    pub(crate) timeframe: String,
    pub(crate) price_to_beat: f64,
}

#[derive(Debug)]
struct PolymarketPriceToBeatService {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, PolymarketPriceToBeatSnapshot>>,
}

static POLYMARKET_PRICE_TO_BEAT_SERVICE: LazyLock<PolymarketPriceToBeatService> =
    LazyLock::new(PolymarketPriceToBeatService::new);

impl PolymarketPriceToBeatService {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .pool_max_idle_per_host(4)
                .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
                .timeout(std::time::Duration::from_secs(
                    POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS,
                ))
                .build()
                .expect("polymarket price to beat http client"),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn fetch_snapshot(&self, market_slug: &str) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.cache.lock().get(market_slug).cloned() {
            return Ok(snapshot);
        }

        let spec = build_price_to_beat_query_spec(market_slug)?;

        for attempt in 0..=QUERY_NOT_FOUND_RETRY_ATTEMPTS {
            let html = self
                .client
                .get(&spec.event_url)
                .header(USER_AGENT, "polymarketbot/price-to-beat-guard")
                .send()
                .await
                .context("requesting polymarket event page")?
                .error_for_status()
                .context("polymarket event page returned error status")?
                .text()
                .await
                .context("reading polymarket event html")?;

            match parse_open_price_from_html(&html, &spec) {
                Ok(price_to_beat) => {
                    if attempt > 0 {
                        tracing::info!(market_slug, attempt, "PRICE_TO_BEAT_QUERY_RETRY_SUCCEEDED");
                    }
                    let snapshot = PolymarketPriceToBeatSnapshot {
                        event_url: spec.event_url,
                        asset: spec.asset.to_ascii_lowercase(),
                        timeframe: spec.timeframe,
                        price_to_beat,
                    };
                    self.cache
                        .lock()
                        .insert(market_slug.to_string(), snapshot.clone());
                    return Ok(snapshot);
                }
                Err(err)
                    if err.to_string().starts_with(QUERY_NOT_FOUND_ERROR_PREFIX)
                        && attempt < QUERY_NOT_FOUND_RETRY_ATTEMPTS =>
                {
                    tracing::warn!(
                        market_slug,
                        attempt,
                        max_attempts = QUERY_NOT_FOUND_RETRY_ATTEMPTS,
                        "PRICE_TO_BEAT_QUERY_NOT_FOUND_RETRYING"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(
                        QUERY_NOT_FOUND_RETRY_DELAY_MS,
                    ))
                    .await;
                }
                Err(err) => return Err(err),
            }
        }

        Err(anyhow!(
            "price to beat query not found after {} retries for {}",
            QUERY_NOT_FOUND_RETRY_ATTEMPTS,
            market_slug
        ))
    }
}

pub(crate) async fn fetch_polymarket_price_to_beat(
    market_slug: &str,
) -> Result<PolymarketPriceToBeatSnapshot> {
    POLYMARKET_PRICE_TO_BEAT_SERVICE
        .fetch_snapshot(market_slug)
        .await
}

pub(crate) fn build_price_to_beat_query_spec(market_slug: &str) -> Result<PriceToBeatQuerySpec> {
    let scope = crate::find_updown_scope_by_slug(market_slug)
        .ok_or_else(|| anyhow!("unsupported updown market slug: {market_slug}"))?;
    let start_at = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .ok_or_else(|| anyhow!("failed to parse cycle start from market slug: {market_slug}"))?;
    let window_secs = match scope.timeframe {
        "5m" => 300,
        "15m" => 900,
        other => {
            return Err(anyhow!(
                "unsupported updown market timeframe for price to beat guard: {other}"
            ))
        }
    };
    let query_timeframe = match scope.timeframe {
        "5m" => "fiveminute",
        "15m" => "fifteen",
        _ => unreachable!(),
    };

    Ok(PriceToBeatQuerySpec {
        market_slug: market_slug.to_string(),
        asset: scope.asset.to_ascii_uppercase(),
        timeframe: scope.timeframe.to_string(),
        query_timeframe,
        start_at,
        end_at: start_at + ChronoDuration::seconds(window_secs),
        event_url: format!("{POLYMARKET_EVENT_BASE_URL}/{market_slug}"),
    })
}

pub(crate) fn parse_open_price_from_html(html: &str, spec: &PriceToBeatQuerySpec) -> Result<f64> {
    let next_data = extract_next_data_json(html)?;
    let parsed: Value = serde_json::from_str(next_data).context("parsing __NEXT_DATA__ json")?;
    let queries = parsed
        .pointer("/props/pageProps/dehydratedState/queries")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("__NEXT_DATA__ queries array missing"))?;

    let expected_start = spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    let expected_end = spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true);

    for query in queries {
        let Some(query_key) = query.get("queryKey").and_then(Value::as_array) else {
            continue;
        };
        if query_key.len() < 6 {
            continue;
        }

        let matches = query_key.first().and_then(Value::as_str) == Some("crypto-prices")
            && query_key.get(1).and_then(Value::as_str) == Some("price")
            && query_key.get(2).and_then(Value::as_str) == Some(spec.asset.as_str())
            && query_key.get(3).and_then(Value::as_str) == Some(expected_start.as_str())
            && query_key.get(4).and_then(Value::as_str) == Some(spec.query_timeframe)
            && query_key.get(5).and_then(Value::as_str) == Some(expected_end.as_str());
        if !matches {
            continue;
        }

        let open_price = query
            .pointer("/state/data/openPrice")
            .and_then(crate::value_as_f64)
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| anyhow!("openPrice missing from matched crypto-prices query"))?;
        return Ok(open_price);
    }

    Err(anyhow!(
        "price to beat query not found in polymarket page for {}",
        spec.market_slug
    ))
}

fn extract_next_data_json(html: &str) -> Result<&str> {
    const NEXT_DATA_TAG: &str = r#"<script id="__NEXT_DATA__""#;
    let tag_start = html
        .find(NEXT_DATA_TAG)
        .ok_or_else(|| anyhow!("__NEXT_DATA__ script tag not found in html"))?;
    let start = html[tag_start..]
        .find('>')
        .ok_or_else(|| anyhow!("__NEXT_DATA__ script tag has no closing >"))?
        + tag_start
        + 1;
    let end = html[start..]
        .find("</script>")
        .ok_or_else(|| anyhow!("__NEXT_DATA__ closing script tag not found"))?
        + start;
    Ok(&html[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_html(next_data_json: &str) -> String {
        format!(
            r#"<html><head></head><body><script id="__NEXT_DATA__" type="application/json" crossorigin="anonymous">{next_data_json}</script></body></html>"#
        )
    }

    #[test]
    fn build_query_spec_for_five_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        assert_eq!(spec.asset, "BTC");
        assert_eq!(spec.timeframe, "5m");
        assert_eq!(spec.query_timeframe, "fiveminute");
        assert_eq!(
            spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:35:00Z"
        );
        assert_eq!(
            spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:40:00Z"
        );
    }

    #[test]
    fn build_query_spec_for_fifteen_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");
        assert_eq!(spec.asset, "BTC");
        assert_eq!(spec.timeframe, "15m");
        assert_eq!(spec.query_timeframe, "fifteen");
        assert_eq!(
            spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:30:00Z"
        );
        assert_eq!(
            spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:45:00Z"
        );
    }

    #[test]
    fn parses_open_price_for_five_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        let html = minimal_html(
            r#"{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["crypto-prices","price","BTC","2026-03-11T12:35:00Z","fiveminute","2026-03-11T12:40:00Z"],"state":{"data":{"openPrice":69279.93484689768,"closePrice":null}}}]}}}}"#,
        );

        let open_price = parse_open_price_from_html(&html, &spec).expect("price");
        assert_eq!(open_price, 69_279.93484689768);
    }

    #[test]
    fn parses_open_price_for_fifteen_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");
        let html = minimal_html(
            r#"{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["crypto-prices","price","BTC","2026-03-11T12:30:00Z","fifteen","2026-03-11T12:45:00Z"],"state":{"data":{"openPrice":69421.75585678649,"closePrice":null}}}]}}}}"#,
        );

        let open_price = parse_open_price_from_html(&html, &spec).expect("price");
        assert_eq!(open_price, 69_421.75585678649);
    }

    #[test]
    fn query_not_found_error_prefix_matches_actual_error() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        let html = minimal_html(r#"{"props":{"pageProps":{"dehydratedState":{"queries":[]}}}}"#);
        let err = parse_open_price_from_html(&html, &spec).unwrap_err();
        assert!(
            err.to_string().starts_with(QUERY_NOT_FOUND_ERROR_PREFIX),
            "error message does not start with expected prefix: {err}",
        );
    }
}
