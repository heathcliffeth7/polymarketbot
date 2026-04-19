use super::chainlink_price::{get_chainlink_price_start_tick, ChainlinkPriceTimestampSnapshot};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use parking_lot::Mutex;
use reqwest::{
    header::{ACCEPT, CACHE_CONTROL, PRAGMA, USER_AGENT},
    Proxy, Url,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

#[path = "polymarket_price_to_beat/dirty_notify.rs"]
mod dirty_notify;
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
const BG_FETCH_RETRY_ATTEMPTS: usize = 9;
const BG_FETCH_RETRY_DELAY_MS: u64 = 100;
const QUERY_NOT_FOUND_ERROR_PREFIX: &str = "price to beat query not found in polymarket page for ";
const VERIFICATION_PENDING_ERROR_PREFIX: &str = "price to beat awaiting previous window close for ";
const PRICE_TO_BEAT_USER_AGENT: &str = "polymarketbot/price-to-beat-guard";
const PROMOTION_INITIAL_DELAY_MS: u64 = 500;
const PROMOTION_RETRY_DELAY_MS: u64 = 500;
const PROMOTION_MAX_ATTEMPTS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatSource {
    Polymarket,
    ChainlinkRtdsStartTick,
}

impl PriceToBeatSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Polymarket => "polymarket",
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
            PriceToBeatSource::Polymarket | PriceToBeatSource::ChainlinkRtdsStartTick
        )
    }

    pub(crate) fn status(&self) -> &'static str {
        match (self.source, self.verified) {
            (PriceToBeatSource::Polymarket, true) => "polymarket_verified",
            (PriceToBeatSource::Polymarket, false) => "polymarket_provisional",
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
    fetch_inflight: Mutex<HashSet<String>>,
    terminal_failures: Mutex<HashMap<String, String>>,
    promotion_inflight: Mutex<HashSet<String>>,
    dirty_market_slugs: Mutex<HashSet<String>>,
    dirty_update_notify: tokio::sync::Notify,
}

static POLYMARKET_PRICE_TO_BEAT_SERVICE: LazyLock<PolymarketPriceToBeatService> =
    LazyLock::new(PolymarketPriceToBeatService::new);

impl PolymarketPriceToBeatService {
    fn new() -> Self {
        Self {
            client: build_price_to_beat_http_client(),
            cache: Mutex::new(HashMap::new()),
            fetch_inflight: Mutex::new(HashSet::new()),
            terminal_failures: Mutex::new(HashMap::new()),
            promotion_inflight: Mutex::new(HashSet::new()),
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

    fn has_authoritative_snapshot(&self, market_slug: &str) -> bool {
        self.current_snapshot(market_slug)
            .map(|snapshot| snapshot.is_verified_polymarket())
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
                source: PriceToBeatSource::ChainlinkRtdsStartTick,
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

    fn begin_promotion(&self, market_slug: &str) -> bool {
        self.promotion_inflight
            .lock()
            .insert(market_slug.to_string())
    }

    fn finish_promotion(&self, market_slug: &str) {
        self.promotion_inflight.lock().remove(market_slug);
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
    ) -> Result<PolymarketPriceToBeatSnapshot> {
        if let Some(snapshot) = self.current_snapshot(market_slug) {
            if snapshot.is_verified_polymarket() {
                return Ok(snapshot);
            }
        }
        let spec = build_price_to_beat_query_spec(market_slug)?;
        let previous_spec = build_previous_price_to_beat_query_spec(&spec)?;
        let (current_api_result, previous_api_result) = tokio::join!(
            self.fetch_crypto_price_window_from_api(&spec),
            self.fetch_crypto_price_window_from_api(&previous_spec)
        );

        let current_api = match current_api_result {
            Ok(response) => Some(response),
            Err(err) => {
                tracing::warn!(
                    market_slug = %spec.market_slug,
                    error = %err,
                    "PRICE_TO_BEAT_API_FETCH_FAILED"
                );
                None
            }
        };
        let previous_api = match previous_api_result {
            Ok(response) => Some(response),
            Err(err) => {
                tracing::warn!(
                    market_slug = %spec.market_slug,
                    previous_market_slug = %previous_spec.market_slug,
                    error = %err,
                    "PRICE_TO_BEAT_PREVIOUS_CLOSE_FETCH_FAILED"
                );
                None
            }
        };

        if let Some(price_to_beat) = previous_api
            .as_ref()
            .and_then(CryptoPriceApiResponse::verified_close_price)
        {
            let snapshot = build_polymarket_snapshot(&spec, price_to_beat, true);
            if let Some(current_open_price) = current_api
                .as_ref()
                .and_then(CryptoPriceApiResponse::sanitized_open_price)
            {
                let mismatch = (current_open_price - price_to_beat).abs();
                if mismatch > 0.0 {
                    tracing::warn!(
                        market_slug = %spec.market_slug,
                        previous_market_slug = %previous_spec.market_slug,
                        provisional_open_price = current_open_price,
                        verified_close_price = price_to_beat,
                        mismatch,
                        "PRICE_TO_BEAT_PROVISIONAL_MISMATCH_DETECTED"
                    );
                }
            }
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
        let response_body = self
            .client
            .get(request_url)
            .header(USER_AGENT, PRICE_TO_BEAT_USER_AGENT)
            .header(ACCEPT, "application/json")
            .header(CACHE_CONTROL, "no-cache")
            .header(PRAGMA, "no-cache")
            .send()
            .await
            .context("requesting polymarket crypto-price api")?
            .error_for_status()
            .context("polymarket crypto-price api returned error status")?
            .text()
            .await
            .context("reading polymarket crypto-price api body")?;
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
            if snapshot.is_verified_polymarket() {
                return;
            }
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
                                    BG_FETCH_RETRY_DELAY_MS,
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
                                BG_FETCH_RETRY_DELAY_MS,
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
                if !snapshot.is_verified_polymarket() {
                    self.ensure_background_fetch(market_slug);
                }
                return PriceToBeatLookup::Ready(snapshot);
            }
        }
        match self.try_seed_snapshot_from_chainlink_with(market_slug, resolve_chainlink_seed) {
            Ok(Some(snapshot)) => {
                if !snapshot.is_verified_polymarket() {
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
                if !snapshot.is_verified_polymarket() {
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
    let lookup = POLYMARKET_PRICE_TO_BEAT_SERVICE.try_cached_or_spawn_with(market_slug, |spec| {
        get_chainlink_price_start_tick(&spec.asset, spec.start_at.timestamp_millis())
    });
    if matches!(&lookup, PriceToBeatLookup::Ready(snapshot) if !snapshot.is_verified_polymarket()) {
        schedule_price_to_beat_promotion(market_slug);
    }
    lookup
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

pub(crate) fn schedule_price_to_beat_promotion(market_slug: &str) {
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_authoritative_snapshot(market_slug) {
        return;
    }
    if !POLYMARKET_PRICE_TO_BEAT_SERVICE.begin_promotion(market_slug) {
        return;
    }

    let market_slug = market_slug.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(PROMOTION_INITIAL_DELAY_MS)).await;

        for attempt in 0..PROMOTION_MAX_ATTEMPTS {
            if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_authoritative_snapshot(&market_slug) {
                break;
            }

            match POLYMARKET_PRICE_TO_BEAT_SERVICE
                .fetch_snapshot_once(
                    &market_slug,
                    (attempt > 0).then(|| Utc::now().timestamp_millis()),
                )
                .await
            {
                Ok(snapshot) => {
                    if snapshot.is_verified_polymarket() {
                        tracing::info!(
                            market_slug,
                            price_to_beat = snapshot.price_to_beat,
                            source = snapshot.source.as_str(),
                            status = snapshot.status(),
                            verified = snapshot.verified,
                            "PRICE_TO_BEAT_UPGRADED_TO_POLYMARKET"
                        );
                        crate::FLOW_PROCESS_NOTIFY.notify_one();
                        break;
                    }
                    if attempt + 1 < PROMOTION_MAX_ATTEMPTS {
                        tracing::warn!(
                            market_slug,
                            attempt,
                            max_attempts = PROMOTION_MAX_ATTEMPTS,
                            price_to_beat = snapshot.price_to_beat,
                            status = snapshot.status(),
                            "PRICE_TO_BEAT_VERIFICATION_RETRYING"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            PROMOTION_RETRY_DELAY_MS,
                        ))
                        .await;
                    }
                }
                Err(err)
                    if is_price_to_beat_query_pending(&err)
                        && attempt + 1 < PROMOTION_MAX_ATTEMPTS =>
                {
                    tracing::warn!(
                        market_slug,
                        attempt,
                        max_attempts = PROMOTION_MAX_ATTEMPTS,
                        "PRICE_TO_BEAT_PROMOTION_RETRYING"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(PROMOTION_RETRY_DELAY_MS))
                        .await;
                }
                Err(err) => {
                    tracing::warn!(
                        market_slug,
                        error = %err,
                        "PRICE_TO_BEAT_UPGRADE_FAILED"
                    );
                    break;
                }
            }
        }

        POLYMARKET_PRICE_TO_BEAT_SERVICE.finish_promotion(&market_slug);
    });
}

pub(crate) fn warm_price_to_beat_cache_bg(market_slug: &str) {
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_authoritative_snapshot(market_slug) {
        return;
    }
    if POLYMARKET_PRICE_TO_BEAT_SERVICE.has_cached_snapshot(market_slug) {
        schedule_price_to_beat_promotion(market_slug);
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
        schedule_price_to_beat_promotion(market_slug);
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

    fn test_snapshot(source: PriceToBeatSource) -> PolymarketPriceToBeatSnapshot {
        PolymarketPriceToBeatSnapshot {
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            asset: "btc".to_string(),
            timeframe: "5m".to_string(),
            price_to_beat: 69_279.93484689768,
            source,
            verified: source == PriceToBeatSource::Polymarket,
            source_latency_ms: Some(125),
            fetched_at: Utc::now(),
        }
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
    fn build_crypto_price_api_url_for_five_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");

        let url = build_crypto_price_api_url(&spec).expect("url");

        assert_eq!(
            url,
            "https://polymarket.com/api/crypto/crypto-price?symbol=BTC&eventStartTime=2026-03-11T12%3A35%3A00Z&variant=fiveminute&endDate=2026-03-11T12%3A40%3A00Z"
        );
    }

    #[test]
    fn build_crypto_price_api_url_for_fifteen_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");

        let url = build_crypto_price_api_url(&spec).expect("url");

        assert_eq!(
            url,
            "https://polymarket.com/api/crypto/crypto-price?symbol=BTC&eventStartTime=2026-03-11T12%3A30%3A00Z&variant=fifteen&endDate=2026-03-11T12%3A45%3A00Z"
        );
    }

    #[test]
    fn builds_previous_query_spec_for_five_minute_market() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");

        let previous = build_previous_price_to_beat_query_spec(&spec).expect("previous spec");

        assert_eq!(previous.market_slug, "btc-updown-5m-1773232200");
        assert_eq!(
            previous.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:30:00Z"
        );
        assert_eq!(
            previous.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-03-11T12:35:00Z"
        );
    }

    #[test]
    fn parses_open_price_from_crypto_price_api_response() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        let response_body = r#"{"openPrice":69279.93484689768,"closePrice":null,"completed":false,"incomplete":true,"cached":true}"#;

        let response = parse_crypto_price_api_response(response_body, &spec).expect("response");

        assert_eq!(response.sanitized_open_price(), Some(69_279.93484689768));
        assert_eq!(response.verified_close_price(), None);
    }

    #[test]
    fn parses_null_open_price_from_crypto_price_api_response_as_pending() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        let response_body = r#"{"openPrice":null,"closePrice":null,"completed":false,"incomplete":true,"cached":false}"#;

        let response = parse_crypto_price_api_response(response_body, &spec).expect("pending");

        assert_eq!(response.sanitized_open_price(), None);
        assert_eq!(response.verified_close_price(), None);
    }

    #[test]
    fn parses_verified_close_price_from_crypto_price_api_response() {
        let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
        let response_body = r#"{"openPrice":69279.93484689768,"closePrice":69282.125,"completed":true,"incomplete":false,"cached":true}"#;

        let response = parse_crypto_price_api_response(response_body, &spec).expect("response");

        assert_eq!(response.sanitized_open_price(), Some(69_279.93484689768));
        assert_eq!(response.verified_close_price(), Some(69_282.125));
    }

    #[test]
    fn detects_query_pending_error_prefixes() {
        let err = anyhow!(
            "price to beat query not found in polymarket page for btc-updown-5m-1773232500"
        );
        assert!(is_price_to_beat_query_pending(&err));

        let retry_err = anyhow!(
            "price to beat query not found after {} retries for {}",
            QUERY_NOT_FOUND_RETRY_ATTEMPTS,
            "btc-updown-5m-1773232500"
        );
        assert!(is_price_to_beat_query_pending(&retry_err));

        let verification_err =
            anyhow!("price to beat awaiting previous window close for btc-updown-5m-1773232500");
        assert!(is_price_to_beat_query_pending(&verification_err));

        let other_err = anyhow!("__NEXT_DATA__ script tag not found in html");
        assert!(!is_price_to_beat_query_pending(&other_err));
    }

    #[test]
    fn seed_snapshot_inserts_chainlink_rtds_start_tick_when_cache_is_empty() {
        let service = PolymarketPriceToBeatService::new();

        let seeded =
            service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_200.0, Some(450));

        assert!(seeded);
        let snapshot = service
            .current_snapshot("btc-updown-5m-1773232500")
            .expect("snapshot");
        assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
        assert!(snapshot.verified);
        assert_eq!(snapshot.price_to_beat, 69_200.0);
        assert_eq!(snapshot.source_latency_ms, Some(450));
    }

    #[test]
    fn seed_snapshot_does_not_overwrite_polymarket_value() {
        let service = PolymarketPriceToBeatService::new();
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            test_snapshot(PriceToBeatSource::Polymarket),
        );

        let seeded =
            service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_100.0, Some(900));

        assert!(!seeded);
        let snapshot = service
            .current_snapshot("btc-updown-5m-1773232500")
            .expect("snapshot");
        assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
        assert!(snapshot.verified);
        assert_eq!(snapshot.price_to_beat, 69_279.93484689768);
    }

    #[test]
    fn seed_snapshot_does_not_overwrite_provisional_polymarket_value() {
        let service = PolymarketPriceToBeatService::new();
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            PolymarketPriceToBeatSnapshot {
                event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
                asset: "btc".to_string(),
                timeframe: "5m".to_string(),
                price_to_beat: 69_100.0,
                source: PriceToBeatSource::Polymarket,
                verified: false,
                source_latency_ms: None,
                fetched_at: Utc::now(),
            },
        );

        let seeded =
            service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_200.0, Some(450));

        assert!(!seeded);
        let snapshot = service
            .current_snapshot("btc-updown-5m-1773232500")
            .expect("snapshot");
        assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
        assert!(!snapshot.verified);
        assert_eq!(snapshot.price_to_beat, 69_100.0);
        assert_eq!(snapshot.source_latency_ms, None);
    }

    #[tokio::test]
    async fn fetch_snapshot_returns_seeded_cache_without_network_lookup() {
        let service = PolymarketPriceToBeatService::new();
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            test_snapshot(PriceToBeatSource::ChainlinkRtdsStartTick),
        );

        let snapshot = service
            .fetch_snapshot("btc-updown-5m-1773232500")
            .await
            .expect("seeded snapshot");

        assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
        assert_eq!(snapshot.price_to_beat, 69_279.93484689768);
    }

    #[tokio::test]
    async fn try_cached_or_spawn_returns_ready_for_seeded_snapshot() {
        let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            test_snapshot(PriceToBeatSource::ChainlinkRtdsStartTick),
        );

        let lookup = service.try_cached_or_spawn("btc-updown-5m-1773232500");

        match lookup {
            PriceToBeatLookup::Ready(snapshot) => {
                assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
            }
            other => panic!("expected ready lookup, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn try_cached_or_spawn_returns_ready_for_provisional_snapshot() {
        let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            PolymarketPriceToBeatSnapshot {
                event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
                asset: "btc".to_string(),
                timeframe: "5m".to_string(),
                price_to_beat: 69_300.0,
                source: PriceToBeatSource::Polymarket,
                verified: false,
                source_latency_ms: None,
                fetched_at: Utc::now(),
            },
        );

        let lookup = service.try_cached_or_spawn("btc-updown-5m-1773232500");

        assert!(matches!(lookup, PriceToBeatLookup::Ready(_)));
    }

    #[tokio::test]
    async fn try_cached_or_spawn_with_returns_ready_when_chainlink_seed_is_available() {
        let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));

        let lookup = service.try_cached_or_spawn_with("btc-updown-5m-1773232500", |_| {
            Ok(ChainlinkPriceTimestampSnapshot {
                price: 69_200.0,
                timestamp_ms: 1_773_232_500_000,
            })
        });

        match lookup {
            PriceToBeatLookup::Ready(snapshot) => {
                assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
                assert_eq!(snapshot.price_to_beat, 69_200.0);
            }
            other => panic!("expected ready lookup, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn try_cached_or_spawn_with_promotes_provisional_snapshot_from_chainlink_seed() {
        let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
        service.cache.lock().insert(
            "btc-updown-5m-1773232500".to_string(),
            PolymarketPriceToBeatSnapshot {
                event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
                asset: "btc".to_string(),
                timeframe: "5m".to_string(),
                price_to_beat: 69_100.0,
                source: PriceToBeatSource::Polymarket,
                verified: false,
                source_latency_ms: None,
                fetched_at: Utc::now(),
            },
        );

        let lookup = service.try_cached_or_spawn_with("btc-updown-5m-1773232500", |_| {
            Ok(ChainlinkPriceTimestampSnapshot {
                price: 69_200.0,
                timestamp_ms: 1_773_232_500_000,
            })
        });

        match lookup {
            PriceToBeatLookup::Ready(snapshot) => {
                assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
                assert_eq!(snapshot.price_to_beat, 69_100.0);
            }
            other => panic!("expected ready lookup, got {other:?}"),
        }
    }

    #[test]
    fn try_cached_or_spawn_returns_terminal_failure_once() {
        let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
        service.record_terminal_failure(
            "btc-updown-5m-1773232500",
            "__NEXT_DATA__ script tag not found in html".to_string(),
        );

        let first = service.try_cached_or_spawn("btc-updown-5m-1773232500");
        let second = service.take_terminal_failure("btc-updown-5m-1773232500");

        match first {
            PriceToBeatLookup::Unavailable(detail) => {
                assert_eq!(detail, "__NEXT_DATA__ script tag not found in html");
            }
            other => panic!("expected unavailable lookup, got {other:?}"),
        }
        assert!(second.is_none());
    }
}
