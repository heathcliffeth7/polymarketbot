use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use parking_lot::Mutex;
use reqwest::{
    header::{ACCEPT, CACHE_CONTROL, PRAGMA, USER_AGENT},
    Proxy,
};
use serde_json::Value;
use std::{collections::HashMap, sync::LazyLock};

use super::chainlink_price::{fetch_chainlink_cycle_open, ChainlinkCycleOpenSnapshot};

const POLYMARKET_EVENT_BASE_URL: &str = "https://polymarket.com/event";
const POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS: u64 = 10;
const POLYMARKET_PRICE_TO_BEAT_CONNECT_TIMEOUT_SECS: u64 = 3;
const QUERY_NOT_FOUND_RETRY_ATTEMPTS: usize = 3;
const QUERY_NOT_FOUND_RETRY_DELAY_MS: u64 = 2_000;
const QUERY_NOT_FOUND_ERROR_PREFIX: &str = "price to beat query not found in polymarket page for ";
const PRICE_TO_BEAT_USER_AGENT: &str = "polymarketbot/price-to-beat-guard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatSource {
    Polymarket,
    ChainlinkSnapshot,
}

impl PriceToBeatSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
            Self::ChainlinkSnapshot => "chainlink_snapshot",
        }
    }
}

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
    pub(crate) source: PriceToBeatSource,
    pub(crate) source_latency_ms: Option<i64>,
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
            client: build_price_to_beat_http_client(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    async fn fetch_snapshot(&self, market_slug: &str) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.cache.lock().get(market_slug).cloned() {
            return Ok(snapshot);
        }

        let spec = build_price_to_beat_query_spec(market_slug)?;
        let mut last_error = None;

        for attempt in 0..=QUERY_NOT_FOUND_RETRY_ATTEMPTS {
            // Cache-bust after the first miss because Polymarket sometimes serves a stale
            // dehydrated payload around cycle rollover that is missing the crypto-prices query.
            let request_url = build_event_request_url(
                &spec.event_url,
                (attempt > 0).then(|| Utc::now().timestamp_millis()),
            );
            let html = self
                .client
                .get(&request_url)
                .header(USER_AGENT, PRICE_TO_BEAT_USER_AGENT)
                .header(ACCEPT, "text/html,application/xhtml+xml")
                .header(CACHE_CONTROL, "no-cache")
                .header(PRAGMA, "no-cache")
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
                        source: PriceToBeatSource::Polymarket,
                        source_latency_ms: None,
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
                Err(err) => {
                    last_error = Some(err);
                    break;
                }
            }
        }

        match build_chainlink_fallback_snapshot(&spec).await {
            Ok(snapshot) => {
                tracing::info!(
                    market_slug,
                    asset = %snapshot.asset,
                    latency_ms = snapshot.source_latency_ms,
                    "PRICE_TO_BEAT_CHAINLINK_FALLBACK_USED"
                );
                self.cache
                    .lock()
                    .insert(market_slug.to_string(), snapshot.clone());
                Ok(snapshot)
            }
            Err(fallback_err) => {
                let base_err = last_error.unwrap_or_else(|| {
                    anyhow!(
                        "price to beat query not found after {} retries for {}",
                        QUERY_NOT_FOUND_RETRY_ATTEMPTS,
                        market_slug
                    )
                });
                Err(anyhow!(
                    "{base_err}; chainlink fallback failed: {fallback_err}"
                ))
            }
        }
    }
}

pub(crate) async fn fetch_polymarket_price_to_beat(
    market_slug: &str,
) -> Result<PolymarketPriceToBeatSnapshot> {
    POLYMARKET_PRICE_TO_BEAT_SERVICE
        .fetch_snapshot(market_slug)
        .await
}

fn build_price_to_beat_http_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .pool_max_idle_per_host(4)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .connect_timeout(std::time::Duration::from_secs(
            POLYMARKET_PRICE_TO_BEAT_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS,
        ));
    if let Ok(proxy_url) = std::env::var("SOCKS5_PROXY_URL") {
        match Proxy::all(&proxy_url) {
            Ok(proxy) => {
                builder = builder.proxy(proxy);
            }
            Err(err) => {
                tracing::warn!(error = %err, "PRICE_TO_BEAT_PROXY_CONFIG_INVALID");
            }
        }
    }
    builder
        .build()
        .expect("polymarket price to beat http client")
}

fn build_event_request_url(event_url: &str, cache_buster_ms: Option<i64>) -> String {
    match cache_buster_ms {
        Some(cache_buster_ms) => {
            let separator = if event_url.contains('?') { '&' } else { '?' };
            format!("{event_url}{separator}ptb_ts={cache_buster_ms}")
        }
        None => event_url.to_string(),
    }
}

async fn build_chainlink_fallback_snapshot(
    spec: &PriceToBeatQuerySpec,
) -> Result<PolymarketPriceToBeatSnapshot> {
    let cycle_open = fetch_chainlink_cycle_open(&spec.asset, spec.start_at)
        .await
        .context("fetching chainlink cycle-open fallback")?;
    Ok(chainlink_snapshot_to_price_to_beat_snapshot(
        spec,
        &cycle_open,
    ))
}

fn chainlink_snapshot_to_price_to_beat_snapshot(
    spec: &PriceToBeatQuerySpec,
    cycle_open: &ChainlinkCycleOpenSnapshot,
) -> PolymarketPriceToBeatSnapshot {
    PolymarketPriceToBeatSnapshot {
        event_url: spec.event_url.clone(),
        asset: spec.asset.to_ascii_lowercase(),
        timeframe: spec.timeframe.clone(),
        price_to_beat: cycle_open.price,
        source: PriceToBeatSource::ChainlinkSnapshot,
        source_latency_ms: Some(cycle_open.latency_ms),
    }
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

    let available_keys: Vec<&Value> = queries.iter().filter_map(|q| q.get("queryKey")).collect();
    tracing::warn!(
        market_slug = %spec.market_slug,
        expected_asset = %spec.asset,
        expected_start = %expected_start,
        expected_end = %expected_end,
        expected_timeframe = spec.query_timeframe,
        available_query_count = queries.len(),
        available_keys = %serde_json::to_string(&available_keys).unwrap_or_default(),
        "PRICE_TO_BEAT_QUERY_NOT_FOUND_DEBUG"
    );

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

    #[test]
    fn build_event_request_url_leaves_base_url_unchanged_without_cache_buster() {
        assert_eq!(
            build_event_request_url("https://polymarket.com/event/foo", None),
            "https://polymarket.com/event/foo"
        );
    }

    #[test]
    fn build_event_request_url_appends_cache_buster_query() {
        assert_eq!(
            build_event_request_url("https://polymarket.com/event/foo", Some(123)),
            "https://polymarket.com/event/foo?ptb_ts=123"
        );
    }

    #[test]
    fn build_event_request_url_respects_existing_query_string() {
        assert_eq!(
            build_event_request_url("https://polymarket.com/event/foo?lang=en", Some(123)),
            "https://polymarket.com/event/foo?lang=en&ptb_ts=123"
        );
    }

    #[test]
    fn chainlink_snapshot_conversion_marks_source_and_latency() {
        let spec = build_price_to_beat_query_spec("sol-updown-5m-1773324600").expect("spec");
        let snapshot = ChainlinkCycleOpenSnapshot {
            price: 87.13950000781921,
            timestamp_ms: spec.start_at.timestamp_millis() + 125,
            latency_ms: 125,
        };

        let converted = chainlink_snapshot_to_price_to_beat_snapshot(&spec, &snapshot);
        assert_eq!(converted.asset, "sol");
        assert_eq!(converted.timeframe, "5m");
        assert_eq!(converted.price_to_beat, 87.13950000781921);
        assert_eq!(converted.source, PriceToBeatSource::ChainlinkSnapshot);
        assert_eq!(converted.source_latency_ms, Some(125));
    }
}
