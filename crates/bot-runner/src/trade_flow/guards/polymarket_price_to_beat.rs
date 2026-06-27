use super::chainlink_price::{get_chainlink_price_start_tick, ChainlinkPriceTimestampSnapshot};
use anyhow::{anyhow, Context, Result};
use bot_infra::db::PostgresRepository;
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use parking_lot::Mutex;
use reqwest::{
    header::{ACCEPT, CACHE_CONTROL, PRAGMA, RETRY_AFTER, USER_AGENT},
    StatusCode, Url,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicI64, LazyLock},
};

#[path = "polymarket_price_to_beat/dirty_notify.rs"]
mod dirty_notify;
#[path = "polymarket_price_to_beat/boundary.rs"]
mod boundary;
pub(crate) use dirty_notify::{
    clear_price_to_beat_dirty_market_slugs, take_price_to_beat_dirty_market_slugs,
    wait_for_price_to_beat_dirty_market_update,
};

const POLYMARKET_EVENT_BASE_URL: &str = "https://polymarket.com/event";
const POLYMARKET_CRYPTO_PRICE_API_URL: &str = "https://polymarket.com/api/crypto/crypto-price";
const POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS: u64 = 10;
const POLYMARKET_PRICE_TO_BEAT_CONNECT_TIMEOUT_SECS: u64 = 3;
#[allow(dead_code)]
const QUERY_NOT_FOUND_RETRY_ATTEMPTS: usize = 2;
#[allow(dead_code)]
const QUERY_NOT_FOUND_RETRY_DELAY_MS: u64 = 1_000;
const BG_FETCH_RETRY_ATTEMPTS: usize = 2;
const BG_FETCH_RETRY_DELAY_MS: u64 = 2_000;
const BG_FETCH_RETRY_MAX_DELAY_MS: u64 = 30_000;
const PTB_RATE_LIMIT_COOLDOWN_MS: u64 = 30_000;
const PTB_RATE_LIMIT_MAX_RETRY_AFTER_MS: u64 = 120_000;
const PTB_REQUEST_MIN_INTERVAL_MS: i64 = 500;
const QUERY_NOT_FOUND_ERROR_PREFIX: &str = "price to beat query not found in polymarket page for ";
const VERIFICATION_PENDING_ERROR_PREFIX: &str = "price to beat awaiting previous window close for ";
const RATE_LIMITED_ERROR_PREFIX: &str = "price to beat rate limited for ";
const PRICE_TO_BEAT_USER_AGENT: &str = "polymarketbot/price-to-beat-guard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatSource {
    Polymarket,
    ChainlinkRtdsPreviousClose,
    ChainlinkRtdsStartTick,
}

impl PriceToBeatSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
            Self::ChainlinkRtdsPreviousClose => "chainlink_rtds_previous_close",
            Self::ChainlinkRtdsStartTick => "chainlink_rtds_start_tick",
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
    pub(crate) verified: bool,
    pub(crate) source_latency_ms: Option<i64>,
    #[allow(dead_code)]
    pub(crate) fetched_at: DateTime<Utc>,
}

impl PolymarketPriceToBeatSnapshot {
    fn is_verified_polymarket(&self) -> bool {
        self.source == PriceToBeatSource::Polymarket && self.verified
    }

    fn is_lookup_ready(&self) -> bool {
        matches!(
            self.source,
            PriceToBeatSource::Polymarket
                | PriceToBeatSource::ChainlinkRtdsPreviousClose
                | PriceToBeatSource::ChainlinkRtdsStartTick
        )
    }

    fn should_verify_with_http(&self) -> bool {
        self.source == PriceToBeatSource::Polymarket
            && !self.verified
            && price_to_beat_http_fallback_enabled()
    }

    pub(crate) fn status(&self) -> &'static str {
        match (self.source, self.verified) {
            (PriceToBeatSource::Polymarket, true) => "polymarket_verified",
            (PriceToBeatSource::Polymarket, false) => "polymarket_provisional",
            (PriceToBeatSource::ChainlinkRtdsPreviousClose, _) => "rtds_previous_close",
            (PriceToBeatSource::ChainlinkRtdsStartTick, _) => "rtds_live",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PriceToBeatLookup {
    Ready(PolymarketPriceToBeatSnapshot),
    Pending,
    Unavailable(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CryptoPriceApiResponse {
    open_price: Option<f64>,
    close_price: Option<f64>,
    #[serde(default)]
    completed: bool,
}

impl CryptoPriceApiResponse {
    fn sanitized_open_price(&self) -> Option<f64> {
        self.open_price
            .filter(|value| value.is_finite() && *value > 0.0)
    }

    fn sanitized_close_price(&self) -> Option<f64> {
        self.close_price
            .filter(|value| value.is_finite() && *value > 0.0)
    }

    fn verified_close_price(&self) -> Option<f64> {
        self.completed.then_some(())?;
        self.sanitized_close_price()
    }
}

#[derive(Debug)]
struct PolymarketPriceToBeatService {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, PolymarketPriceToBeatSnapshot>>,
    previous_close_cache: Mutex<HashMap<String, f64>>,
    fetch_inflight: Mutex<HashSet<String>>,
    window_fetch_inflight: Mutex<HashSet<String>>,
    terminal_failures: Mutex<HashMap<String, String>>,
    rate_limit_until_ms: AtomicI64,
    next_request_at_ms: AtomicI64,
    dirty_market_slugs: Mutex<HashSet<String>>,
    dirty_update_notify: tokio::sync::Notify,
}

#[derive(Debug)]
struct WindowFetchGuard<'a> {
    service: &'a PolymarketPriceToBeatService,
    request_url: String,
}

impl<'a> WindowFetchGuard<'a> {
    fn acquire(service: &'a PolymarketPriceToBeatService, request_url: String) -> Option<Self> {
        if service
            .window_fetch_inflight
            .lock()
            .insert(request_url.clone())
        {
            Some(Self {
                service,
                request_url,
            })
        } else {
            None
        }
    }
}

impl Drop for WindowFetchGuard<'_> {
    fn drop(&mut self) {
        self.service
            .window_fetch_inflight
            .lock()
            .remove(&self.request_url);
    }
}

static POLYMARKET_PRICE_TO_BEAT_SERVICE: LazyLock<PolymarketPriceToBeatService> =
    LazyLock::new(PolymarketPriceToBeatService::new);

impl PolymarketPriceToBeatService {
    fn new() -> Self {
        Self {
            client: build_price_to_beat_http_client(),
            cache: Mutex::new(HashMap::new()),
            previous_close_cache: Mutex::new(HashMap::new()),
            fetch_inflight: Mutex::new(HashSet::new()),
            window_fetch_inflight: Mutex::new(HashSet::new()),
            terminal_failures: Mutex::new(HashMap::new()),
            rate_limit_until_ms: AtomicI64::new(0),
            next_request_at_ms: AtomicI64::new(0),
            dirty_market_slugs: Mutex::new(HashSet::new()),
            dirty_update_notify: tokio::sync::Notify::new(),
        }
    }

    fn has_cached_snapshot(&self, market_slug: &str) -> bool {
        self.cache.lock().contains_key(market_slug)
    }

    fn current_snapshot(&self, market_slug: &str) -> Option<PolymarketPriceToBeatSnapshot> {
        self.cache.lock().get(market_slug).cloned()
    }

    fn take_terminal_failure(&self, market_slug: &str) -> Option<String> {
        self.terminal_failures.lock().remove(market_slug)
    }

    fn record_terminal_failure(&self, market_slug: &str, detail: String) {
        self.terminal_failures
            .lock()
            .insert(market_slug.to_string(), detail);
    }

    fn begin_background_fetch(&self, market_slug: &str) -> bool {
        self.fetch_inflight.lock().insert(market_slug.to_string())
    }

    fn finish_background_fetch(&self, market_slug: &str) {
        self.fetch_inflight.lock().remove(market_slug);
    }

    fn background_retry_delay_ms(&self, attempt: usize) -> u64 {
        let backoff_ms = backoff_delay_ms(attempt);
        self.rate_limit_cooldown_remaining_ms()
            .map(|cooldown_ms| cooldown_ms.max(backoff_ms))
            .unwrap_or(backoff_ms)
    }

    fn has_authoritative_snapshot(&self, market_slug: &str) -> bool {
        self.current_snapshot(market_slug)
            .map(|snapshot| snapshot.verified && snapshot.is_lookup_ready())
            .unwrap_or(false)
    }

    fn seed_snapshot(
        &self,
        market_slug: &str,
        asset: &str,
        timeframe: &str,
        price: f64,
        source_latency_ms: Option<i64>,
    ) -> bool {
        self.seed_snapshot_with_source(
            market_slug,
            asset,
            timeframe,
            price,
            PriceToBeatSource::ChainlinkRtdsStartTick,
            source_latency_ms,
        )
    }

    fn seed_snapshot_with_source(
        &self,
        market_slug: &str,
        asset: &str,
        timeframe: &str,
        price: f64,
        source: PriceToBeatSource,
        source_latency_ms: Option<i64>,
    ) -> bool {
        let mut cache = self.cache.lock();
        if let Some(snapshot) = cache.get(market_slug) {
            if snapshot.is_lookup_ready() {
                return false;
            }
        }
        cache.insert(
            market_slug.to_string(),
            PolymarketPriceToBeatSnapshot {
                event_url: format!("{POLYMARKET_EVENT_BASE_URL}/{market_slug}"),
                asset: asset.to_ascii_lowercase(),
                timeframe: timeframe.to_string(),
                price_to_beat: price,
                source,
                verified: true,
                source_latency_ms,
                fetched_at: Utc::now(),
            },
        );
        drop(cache);
        self.mark_dirty_market_slug(market_slug);
        true
    }

    fn try_seed_snapshot_from_chainlink_with<F>(
        &self,
        market_slug: &str,
        resolve_chainlink_seed: F,
    ) -> Result<Option<PolymarketPriceToBeatSnapshot>>
    where
        F: FnOnce(&PriceToBeatQuerySpec) -> Result<ChainlinkPriceTimestampSnapshot>,
    {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_lookup_ready() {
                return Ok(Some(snapshot));
            }
        }

        let spec = build_price_to_beat_query_spec(market_slug)?;
        let chainlink_snapshot = resolve_chainlink_seed(&spec)?;
        let source_latency_ms =
            Some((chainlink_snapshot.timestamp_ms - spec.start_at.timestamp_millis()).abs());
        let seeded = self.seed_snapshot(
            market_slug,
            &spec.asset,
            &spec.timeframe,
            chainlink_snapshot.price,
            source_latency_ms,
        );
        if seeded {
            tracing::info!(
                market_slug = %spec.market_slug,
                asset = %spec.asset,
                timeframe = %spec.timeframe,
                price_to_beat = chainlink_snapshot.price,
                chainlink_tick_ts = chainlink_snapshot.timestamp_ms,
                source_latency_ms,
                "PRICE_TO_BEAT_SEEDED_FROM_CHAINLINK_ON_DEMAND"
            );
        }
        Ok(self
            .current_snapshot(market_slug)
            .filter(PolymarketPriceToBeatSnapshot::is_lookup_ready))
    }

    #[allow(dead_code)]
    async fn fetch_snapshot(&self, market_slug: &str) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_lookup_ready() {
                return Ok(snapshot);
            }
        }
        self.fetch_snapshot_from_network(market_slug).await
    }

    async fn fetch_snapshot_once(
        &self,
        market_slug: &str,
        cache_buster_ms: Option<i64>,
        verify_only: bool,
    ) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.verified && snapshot.is_lookup_ready() {
                return Ok(snapshot);
            }
        }
        let spec = build_price_to_beat_query_spec(market_slug)?;
        let previous_spec = build_previous_price_to_beat_query_spec(&spec)?;

        if let Some(price_to_beat) = self.cached_previous_close(&previous_spec.market_slug) {
            let snapshot = build_polymarket_snapshot(&spec, price_to_beat, true);
            let _ = self.store_verified_polymarket_snapshot(market_slug, snapshot.clone());
            tracing::info!(
                market_slug = %spec.market_slug,
                previous_market_slug = %previous_spec.market_slug,
                price_to_beat = snapshot.price_to_beat,
                "PRICE_TO_BEAT_VERIFIED_FROM_PREVIOUS_CLOSE"
            );
            return Ok(snapshot);
        }

        if !price_to_beat_http_fallback_enabled() {
            tracing::debug!(market_slug, "PRICE_TO_BEAT_HTTP_FALLBACK_DISABLED");
            return Err(anyhow!(
                "{VERIFICATION_PENDING_ERROR_PREFIX}{market_slug} http fallback disabled"
            ));
        }

        if let Some(cooldown_ms) = self.rate_limit_cooldown_remaining_ms() {
            return Err(anyhow!(
                "{RATE_LIMITED_ERROR_PREFIX}{cooldown_ms}ms on {}",
                spec.market_slug
            ));
        }

        let previous_api = match self
            .fetch_crypto_price_window_from_api(&previous_spec)
            .await
        {
            Ok(response) => Some(response),
            Err(err) => {
                if is_price_to_beat_rate_limited_error(&err) {
                    return Err(err);
                }
                if !is_price_to_beat_query_pending(&err) {
                    tracing::warn!(
                        market_slug = %spec.market_slug,
                        previous_market_slug = %previous_spec.market_slug,
                        error = %err,
                        "PRICE_TO_BEAT_PREVIOUS_CLOSE_FETCH_FAILED"
                    );
                }
                None
            }
        };

        if let Some(price_to_beat) = previous_api
            .as_ref()
            .and_then(CryptoPriceApiResponse::verified_close_price)
        {
            self.store_previous_close(&previous_spec.market_slug, price_to_beat);
            let snapshot = build_polymarket_snapshot(&spec, price_to_beat, true);
            if let Some(previous_snapshot) =
                self.store_verified_polymarket_snapshot(market_slug, snapshot.clone())
            {
                let mismatch = (previous_snapshot.price_to_beat - snapshot.price_to_beat).abs();
                if mismatch > 0.0 {
                    tracing::warn!(
                        market_slug = %spec.market_slug,
                        previous_price_to_beat = previous_snapshot.price_to_beat,
                        previous_source = previous_snapshot.source.as_str(),
                        previous_verified = previous_snapshot.verified,
                        verified_price_to_beat = snapshot.price_to_beat,
                        mismatch,
                        "PRICE_TO_BEAT_VERIFIED_CACHE_OVERWRITE"
                    );
                }
            }
            tracing::info!(
                market_slug = %spec.market_slug,
                previous_market_slug = %previous_spec.market_slug,
                price_to_beat = snapshot.price_to_beat,
                "PRICE_TO_BEAT_VERIFIED_FROM_PREVIOUS_CLOSE"
            );
            return Ok(snapshot);
        }

        if verify_only {
            return Err(anyhow!(
                "{VERIFICATION_PENDING_ERROR_PREFIX}{}",
                spec.market_slug
            ));
        }

        let current_api = match self.fetch_crypto_price_window_from_api(&spec).await {
            Ok(response) => Some(response),
            Err(err) => {
                if is_price_to_beat_rate_limited_error(&err) {
                    return Err(err);
                }
                tracing::warn!(
                    market_slug = %spec.market_slug,
                    error = %err,
                    "PRICE_TO_BEAT_API_FETCH_FAILED"
                );
                None
            }
        };

        if let Some(price_to_beat) = current_api
            .as_ref()
            .and_then(CryptoPriceApiResponse::sanitized_open_price)
        {
            let snapshot = build_polymarket_snapshot(&spec, price_to_beat, false);
            if let Some(previous_snapshot) =
                self.store_provisional_polymarket_snapshot(market_slug, snapshot.clone())
            {
                let mismatch = (previous_snapshot.price_to_beat - snapshot.price_to_beat).abs();
                if mismatch > 0.0 || previous_snapshot.source != snapshot.source {
                    tracing::warn!(
                        market_slug = %spec.market_slug,
                        previous_price_to_beat = previous_snapshot.price_to_beat,
                        previous_source = previous_snapshot.source.as_str(),
                        previous_status = previous_snapshot.status(),
                        provisional_price_to_beat = snapshot.price_to_beat,
                        provisional_status = snapshot.status(),
                        mismatch,
                        "PRICE_TO_BEAT_PROVISIONAL_CACHE_OVERWRITE"
                    );
                }
            }
            tracing::info!(
                market_slug = %spec.market_slug,
                previous_market_slug = %previous_spec.market_slug,
                price_to_beat = snapshot.price_to_beat,
                "PRICE_TO_BEAT_API_PROVISIONAL_CAPTURED"
            );
            return Ok(snapshot);
        }

        let request_url = build_event_request_url(&spec.event_url, cache_buster_ms);
        tracing::debug!(
            market_slug = %spec.market_slug,
            cache_buster_ms,
            "PRICE_TO_BEAT_HTML_FALLBACK_STARTED"
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
        let price_to_beat = parse_open_price_from_html(&html, &spec)?;
        let snapshot = build_polymarket_snapshot(&spec, price_to_beat, false);
        tracing::info!(
            market_slug = %spec.market_slug,
            price_to_beat = snapshot.price_to_beat,
            "PRICE_TO_BEAT_HTML_FALLBACK_PROVISIONAL_CAPTURED"
        );
        let _ = self.store_provisional_polymarket_snapshot(market_slug, snapshot.clone());
        Ok(snapshot)
    }

    async fn fetch_crypto_price_window_from_api(
        &self,
        spec: &PriceToBeatQuerySpec,
    ) -> Result<CryptoPriceApiResponse> {
        let request_url = build_crypto_price_api_url(spec)?;
        let _window_guard =
            WindowFetchGuard::acquire(self, request_url.clone()).ok_or_else(|| {
                anyhow!(
                    "{VERIFICATION_PENDING_ERROR_PREFIX}{} window in-flight",
                    spec.market_slug
                )
            })?;
        self.pace_crypto_price_request().await;
        let response = self
            .client
            .get(request_url)
            .header(USER_AGENT, PRICE_TO_BEAT_USER_AGENT)
            .header(ACCEPT, "application/json")
            .header(CACHE_CONTROL, "no-cache")
            .header(PRAGMA, "no-cache")
            .send()
            .await
            .context("requesting polymarket crypto-price api")?;
        let status = response.status();
        let retry_after_ms = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after_ms)
            .unwrap_or(PTB_RATE_LIMIT_COOLDOWN_MS)
            .min(PTB_RATE_LIMIT_MAX_RETRY_AFTER_MS);
        let response_body = response
            .text()
            .await
            .context("reading polymarket crypto-price api body")?;
        if status == StatusCode::TOO_MANY_REQUESTS {
            self.arm_rate_limit_cooldown(retry_after_ms);
            tracing::warn!(
                market_slug = %spec.market_slug,
                status = %status,
                retry_after_ms,
                body = %response_body,
                "PRICE_TO_BEAT_API_RATE_LIMITED"
            );
            return Err(anyhow!(
                "{RATE_LIMITED_ERROR_PREFIX}{retry_after_ms}ms on {}",
                spec.market_slug
            ));
        }
        if !status.is_success() {
            tracing::warn!(market_slug = %spec.market_slug, status = %status, body = %response_body, "PRICE_TO_BEAT_API_HTTP_ERROR");
            return Err(anyhow!(
                "polymarket crypto-price api returned status {status}: {response_body}"
            ));
        }
        parse_crypto_price_api_response(&response_body, spec)
    }

    #[allow(dead_code)]
    async fn fetch_snapshot_from_network(
        &self,
        market_slug: &str,
    ) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_verified_polymarket() {
                return Ok(snapshot);
            }
        }
        let mut last_error = None;

        for attempt in 0..=QUERY_NOT_FOUND_RETRY_ATTEMPTS {
            // Cache-bust after the first miss because Polymarket sometimes serves a stale
            // dehydrated payload around cycle rollover that is missing the crypto-prices query.
            match self
                .fetch_snapshot_once(
                    market_slug,
                    (attempt > 0).then(|| Utc::now().timestamp_millis()),
                    false,
                )
                .await
            {
                Ok(snapshot) => {
                    if snapshot.is_verified_polymarket() {
                        if attempt > 0 {
                            tracing::info!(
                                market_slug,
                                attempt,
                                "PRICE_TO_BEAT_QUERY_RETRY_SUCCEEDED"
                            );
                        }
                        return Ok(snapshot);
                    }
                    if attempt < QUERY_NOT_FOUND_RETRY_ATTEMPTS {
                        tracing::warn!(
                            market_slug,
                            attempt,
                            max_attempts = QUERY_NOT_FOUND_RETRY_ATTEMPTS,
                            price_to_beat = snapshot.price_to_beat,
                            "PRICE_TO_BEAT_VERIFICATION_RETRYING"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            QUERY_NOT_FOUND_RETRY_DELAY_MS,
                        ))
                        .await;
                        continue;
                    }
                    last_error = Some(anyhow!("{VERIFICATION_PENDING_ERROR_PREFIX}{market_slug}"));
                    break;
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

        Err(last_error.unwrap_or_else(|| {
            anyhow!(
                "price to beat query not found after {} retries for {}",
                QUERY_NOT_FOUND_RETRY_ATTEMPTS,
                market_slug
            )
        }))
    }

    fn ensure_background_fetch(&'static self, market_slug: &str) {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.verified && snapshot.is_lookup_ready() {
                return;
            }
        }
        if !price_to_beat_http_fallback_enabled() {
            tracing::debug!(market_slug, "PRICE_TO_BEAT_HTTP_FALLBACK_DISABLED");
            return;
        }
        if !self.begin_background_fetch(market_slug) {
            return;
        }

        let market_slug = market_slug.to_string();
        tokio::spawn(async move {
            let result = async {
                tracing::info!(market_slug = %market_slug, "PRICE_TO_BEAT_BACKGROUND_FETCH_STARTED");
                for attempt in 0..=BG_FETCH_RETRY_ATTEMPTS {
                    match self
                        .fetch_snapshot_once(
                            &market_slug,
                            (attempt > 0).then(|| Utc::now().timestamp_millis()),
                            true,
                        )
                        .await
                    {
                        Ok(snapshot) => {
                            if snapshot.is_verified_polymarket() {
                                tracing::info!(
                                    market_slug = %market_slug,
                                    attempt,
                                    source = snapshot.source.as_str(),
                                    status = snapshot.status(),
                                    verified = snapshot.verified,
                                    "PRICE_TO_BEAT_BACKGROUND_FETCH_SUCCEEDED"
                                );
                                crate::FLOW_PROCESS_NOTIFY.notify_one();
                                return None;
                            }
                            if attempt < BG_FETCH_RETRY_ATTEMPTS {
                                tracing::debug!(
                                    market_slug = %market_slug,
                                    attempt,
                                    max_attempts = BG_FETCH_RETRY_ATTEMPTS,
                                    price_to_beat = snapshot.price_to_beat,
                                    "PRICE_TO_BEAT_VERIFICATION_RETRYING"
                                );
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    self.background_retry_delay_ms(attempt),
                                ))
                                .await;
                                continue;
                            }
                            return None;
                        }
                        Err(err)
                            if is_price_to_beat_query_pending(&err)
                                && attempt < BG_FETCH_RETRY_ATTEMPTS =>
                        {
                            tracing::debug!(
                                market_slug = %market_slug,
                                attempt,
                                max_attempts = BG_FETCH_RETRY_ATTEMPTS,
                                "PRICE_TO_BEAT_BACKGROUND_FETCH_RETRYING"
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(
                                self.background_retry_delay_ms(attempt),
                            ))
                            .await;
                        }
                        Err(err) => {
                            if is_price_to_beat_query_pending(&err) {
                                return None;
                            }
                            let detail = err.to_string();
                            tracing::warn!(
                                market_slug = %market_slug,
                                attempt,
                                error = %detail,
                                "PRICE_TO_BEAT_BACKGROUND_FETCH_FAILED"
                            );
                            return Some(detail);
                        }
                    }
                }
                None
            }
            .await;

            if let Some(detail) = result {
                self.record_terminal_failure(&market_slug, detail);
                crate::FLOW_PROCESS_NOTIFY.notify_one();
            }
            self.finish_background_fetch(&market_slug);
        });
    }

    #[cfg(test)]
    fn try_cached_or_spawn(&'static self, market_slug: &str) -> PriceToBeatLookup {
        self.try_cached_or_spawn_with(market_slug, |_| {
            Err(anyhow!("chainlink seed unavailable in local test service"))
        })
    }

    fn try_cached_or_spawn_with<F>(
        &'static self,
        market_slug: &str,
        resolve_chainlink_seed: F,
    ) -> PriceToBeatLookup
    where
        F: FnOnce(&PriceToBeatQuerySpec) -> Result<ChainlinkPriceTimestampSnapshot>,
    {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_lookup_ready() {
                if snapshot.should_verify_with_http() {
                    self.ensure_background_fetch(market_slug);
                }
                return PriceToBeatLookup::Ready(snapshot);
            }
        }
        match self.try_seed_snapshot_from_chainlink_with(market_slug, resolve_chainlink_seed) {
            Ok(Some(snapshot)) => {
                if snapshot.should_verify_with_http() {
                    self.ensure_background_fetch(market_slug);
                }
                return PriceToBeatLookup::Ready(snapshot);
            }
            Ok(None) => {}
            Err(err) => {
                tracing::debug!(
                    market_slug,
                    error = %err,
                    "PRICE_TO_BEAT_CHAINLINK_ON_DEMAND_SEED_MISS"
                );
            }
        }
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_lookup_ready() {
                if snapshot.should_verify_with_http() {
                    self.ensure_background_fetch(market_slug);
                }
                return PriceToBeatLookup::Ready(snapshot);
            }
            self.ensure_background_fetch(market_slug);
            return PriceToBeatLookup::Pending;
        }
        if let Some(detail) = self.take_terminal_failure(market_slug) {
            return PriceToBeatLookup::Unavailable(detail);
        }
        self.ensure_background_fetch(market_slug);
        PriceToBeatLookup::Pending
    }
}

#[allow(dead_code)]
pub(crate) async fn fetch_polymarket_price_to_beat(
    market_slug: &str,
) -> Result<PolymarketPriceToBeatSnapshot> {
    POLYMARKET_PRICE_TO_BEAT_SERVICE
        .fetch_snapshot(market_slug)
        .await
}

pub(crate) fn get_price_to_beat_cached(market_slug: &str) -> Option<PolymarketPriceToBeatSnapshot> {
    POLYMARKET_PRICE_TO_BEAT_SERVICE
        .current_snapshot(market_slug)
        .filter(PolymarketPriceToBeatSnapshot::is_lookup_ready)
}

pub(crate) fn try_price_to_beat_cached_or_spawn(market_slug: &str) -> PriceToBeatLookup {
    POLYMARKET_PRICE_TO_BEAT_SERVICE.try_cached_or_spawn_with(market_slug, |spec| {
        get_chainlink_price_start_tick(&spec.asset, spec.start_at.timestamp_millis())
    })
}

pub(crate) fn seed_price_to_beat_from_chainlink(
    market_slug: &str,
    asset: &str,
    timeframe: &str,
    price: f64,
    source_latency_ms: Option<i64>,
) -> bool {
    POLYMARKET_PRICE_TO_BEAT_SERVICE.seed_snapshot(
        market_slug,
        asset,
        timeframe,
        price,
        source_latency_ms,
    )
}

pub(crate) async fn seed_price_to_beat_from_rtds_previous_close(
    repo: &PostgresRepository,
    market_slug: &str,
) -> Result<Option<PolymarketPriceToBeatSnapshot>> {
    boundary::seed_from_rtds_previous_close(&POLYMARKET_PRICE_TO_BEAT_SERVICE, repo, market_slug)
        .await
}

pub(crate) async fn record_price_to_beat_open_boundary_from_chainlink(
    repo: &PostgresRepository,
    market_slug: &str,
    price: f64,
    timestamp_ms: i64,
) -> Result<()> {
    boundary::record_open_boundary_from_chainlink(repo, market_slug, price, timestamp_ms).await
}

#[cfg(test)]
pub(crate) fn clear_price_to_beat_test_state() {
    let service = &POLYMARKET_PRICE_TO_BEAT_SERVICE;
    service.cache.lock().clear();
    service.fetch_inflight.lock().clear();
    service.window_fetch_inflight.lock().clear();
    service.terminal_failures.lock().clear();
    service.previous_close_cache.lock().clear();
    service
        .rate_limit_until_ms
        .store(0, std::sync::atomic::Ordering::Relaxed);
    service
        .next_request_at_ms
        .store(0, std::sync::atomic::Ordering::Relaxed);
    service.dirty_market_slugs.lock().clear();
}

pub(crate) fn warm_price_to_beat_cache_bg(market_slug: &str) {
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_authoritative_snapshot(market_slug) {
        return;
    }
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_cached_snapshot(market_slug) {
        POLYMARKET_PRICE_TO_BEAT_SERVICE.ensure_background_fetch(market_slug);
        return;
    }
    POLYMARKET_PRICE_TO_BEAT_SERVICE.ensure_background_fetch(market_slug);
}

#[allow(dead_code)]
pub(crate) async fn warm_price_to_beat_cache(market_slug: &str) {
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_authoritative_snapshot(market_slug) {
        return;
    }
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_cached_snapshot(market_slug) {
        POLYMARKET_PRICE_TO_BEAT_SERVICE.ensure_background_fetch(market_slug);
        return;
    }
    match POLYMARKET_PRICE_TO_BEAT_SERVICE
        .fetch_snapshot_from_network(market_slug)
        .await
    {
        Ok(snapshot) => {
            tracing::info!(
                market_slug,
                source = snapshot.source.as_str(),
                age_ms = (Utc::now() - snapshot.fetched_at).num_milliseconds(),
                "PRICE_TO_BEAT_CACHE_WARMED"
            );
        }
        Err(err) => {
            tracing::warn!(
                market_slug,
                error = %err,
                "PRICE_TO_BEAT_CACHE_WARM_FAILED"
            );
        }
    }
}

/// Polymarket sayfasi yuklendi ama cycle-open `crypto-prices` query'si henuz hazir degil.
pub(crate) fn is_price_to_beat_query_pending(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.starts_with(QUERY_NOT_FOUND_ERROR_PREFIX)
        || msg.starts_with("price to beat query not found after")
        || msg.starts_with(VERIFICATION_PENDING_ERROR_PREFIX)
        || msg.starts_with(RATE_LIMITED_ERROR_PREFIX)
}

fn is_price_to_beat_rate_limited_error(err: &anyhow::Error) -> bool {
    err.to_string().starts_with(RATE_LIMITED_ERROR_PREFIX)
}

fn price_to_beat_http_fallback_enabled() -> bool {
    std::env::var("PTB_ENABLE_POLYMARKET_CRYPTO_PRICE_FALLBACK")
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn parse_retry_after_ms(value: &str) -> Option<u64> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000))
        .filter(|milliseconds| *milliseconds > 0)
}

fn backoff_delay_ms(attempt: usize) -> u64 {
    let multiplier = 1_u64.checked_shl(attempt.min(10) as u32).unwrap_or(1);
    BG_FETCH_RETRY_DELAY_MS
        .saturating_mul(multiplier)
        .min(BG_FETCH_RETRY_MAX_DELAY_MS)
}

fn build_price_to_beat_http_client() -> reqwest::Client {
    let builder = reqwest::Client::builder()
        .pool_max_idle_per_host(4)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .connect_timeout(std::time::Duration::from_secs(
            POLYMARKET_PRICE_TO_BEAT_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            POLYMARKET_PRICE_TO_BEAT_TIMEOUT_SECS,
        ));
    let builder = bot_infra::proxy::add_rotating_reqwest_proxy(builder, "price_to_beat");
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

fn build_crypto_price_api_url(spec: &PriceToBeatQuerySpec) -> Result<String> {
    build_crypto_price_api_url_for_window(
        spec.asset.as_str(),
        spec.query_timeframe,
        spec.start_at,
        spec.end_at,
    )
}

fn build_crypto_price_api_url_for_window(
    asset: &str,
    query_timeframe: &str,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
) -> Result<String> {
    let expected_start = start_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    let expected_end = end_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    Url::parse_with_params(
        POLYMARKET_CRYPTO_PRICE_API_URL,
        &[
            ("symbol", asset),
            ("eventStartTime", expected_start.as_str()),
            ("variant", query_timeframe),
            ("endDate", expected_end.as_str()),
        ],
    )
    .map(|url| url.to_string())
    .context("building polymarket crypto-price api url")
}

fn parse_crypto_price_api_response(
    response_body: &str,
    spec: &PriceToBeatQuerySpec,
) -> Result<CryptoPriceApiResponse> {
    let response: CryptoPriceApiResponse =
        serde_json::from_str(response_body).context("parsing polymarket crypto-price json")?;
    if response.sanitized_open_price().is_none() && response.verified_close_price().is_none() {
        tracing::debug!(
            market_slug = %spec.market_slug,
            expected_asset = %spec.asset,
            expected_start = %spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            expected_end = %spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            expected_timeframe = spec.query_timeframe,
            response_body = %response_body,
            "PRICE_TO_BEAT_API_OPEN_PRICE_PENDING"
        );
    }
    Ok(response)
}

fn build_polymarket_snapshot(
    spec: &PriceToBeatQuerySpec,
    price_to_beat: f64,
    verified: bool,
) -> PolymarketPriceToBeatSnapshot {
    PolymarketPriceToBeatSnapshot {
        event_url: spec.event_url.clone(),
        asset: spec.asset.to_ascii_lowercase(),
        timeframe: spec.timeframe.clone(),
        price_to_beat,
        source: PriceToBeatSource::Polymarket,
        verified,
        source_latency_ms: None,
        fetched_at: Utc::now(),
    }
}

fn build_previous_price_to_beat_query_spec(
    spec: &PriceToBeatQuerySpec,
) -> Result<PriceToBeatQuerySpec> {
    let scope =
        crate::find_updown_scope_by_asset_timeframe(spec.asset.as_str(), spec.timeframe.as_str())
            .ok_or_else(|| {
            anyhow!(
                "unsupported asset/timeframe for previous price to beat query: {}/{}",
                spec.asset,
                spec.timeframe
            )
        })?;
    let window = spec.end_at - spec.start_at;
    let start_at = spec.start_at - window;
    let market_slug = format!("{}{}", scope.slug_prefix, start_at.timestamp());
    Ok(PriceToBeatQuerySpec {
        market_slug: market_slug.clone(),
        asset: spec.asset.clone(),
        timeframe: spec.timeframe.clone(),
        query_timeframe: spec.query_timeframe,
        start_at,
        end_at: spec.start_at,
        event_url: format!("{POLYMARKET_EVENT_BASE_URL}/{market_slug}"),
    })
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
            ));
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
mod tests;
