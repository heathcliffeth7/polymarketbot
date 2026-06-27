use super::{
    PolymarketPriceToBeatService, PolymarketPriceToBeatSnapshot, POLYMARKET_PRICE_TO_BEAT_SERVICE,
    PTB_REQUEST_MIN_INTERVAL_MS,
};
use chrono::Utc;
use std::{collections::HashSet, sync::atomic::Ordering};

impl PolymarketPriceToBeatService {
    pub(super) fn mark_dirty_market_slug(&self, market_slug: &str) {
        self.dirty_market_slugs
            .lock()
            .insert(market_slug.trim().to_ascii_lowercase());
        self.dirty_update_notify.notify_one();
    }

    fn take_dirty_market_slugs(&self) -> Vec<String> {
        self.dirty_market_slugs.lock().iter().cloned().collect()
    }

    fn clear_dirty_market_slugs(&self, market_slugs: &[String]) {
        if market_slugs.is_empty() {
            return;
        }
        let market_set: HashSet<&str> = market_slugs.iter().map(String::as_str).collect();
        self.dirty_market_slugs
            .lock()
            .retain(|market_slug| !market_set.contains(market_slug.as_str()));
    }

    pub(super) fn store_verified_polymarket_snapshot(
        &self,
        market_slug: &str,
        snapshot: PolymarketPriceToBeatSnapshot,
    ) -> Option<PolymarketPriceToBeatSnapshot> {
        let previous = self.cache.lock().insert(market_slug.to_string(), snapshot);
        self.mark_dirty_market_slug(market_slug);
        previous
    }

    pub(super) fn store_provisional_polymarket_snapshot(
        &self,
        market_slug: &str,
        snapshot: PolymarketPriceToBeatSnapshot,
    ) -> Option<PolymarketPriceToBeatSnapshot> {
        let mut cache = self.cache.lock();
        if cache
            .get(market_slug)
            .map(PolymarketPriceToBeatSnapshot::is_verified_polymarket)
            .unwrap_or(false)
        {
            return None;
        }
        let previous = cache.insert(market_slug.to_string(), snapshot);
        drop(cache);
        self.mark_dirty_market_slug(market_slug);
        previous
    }

    pub(super) fn cached_previous_close(&self, previous_market_slug: &str) -> Option<f64> {
        self.previous_close_cache
            .lock()
            .get(previous_market_slug)
            .copied()
    }

    pub(super) fn store_previous_close(&self, previous_market_slug: &str, close_price: f64) {
        self.previous_close_cache
            .lock()
            .entry(previous_market_slug.to_string())
            .or_insert(close_price);
    }

    pub(super) fn arm_rate_limit_cooldown(&self, cooldown_ms: u64) {
        let until_ms = Utc::now()
            .timestamp_millis()
            .saturating_add(cooldown_ms as i64);
        let mut current = self.rate_limit_until_ms.load(Ordering::Relaxed);
        while until_ms > current {
            match self.rate_limit_until_ms.compare_exchange(
                current,
                until_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(next) => current = next,
            }
        }
    }

    pub(super) fn rate_limit_cooldown_remaining_ms(&self) -> Option<u64> {
        let until_ms = self.rate_limit_until_ms.load(Ordering::Relaxed);
        let now_ms = Utc::now().timestamp_millis();
        (until_ms > now_ms).then(|| (until_ms - now_ms) as u64)
    }

    pub(super) async fn pace_crypto_price_request(&self) {
        let now_ms = Utc::now().timestamp_millis();
        let slot_ms;
        loop {
            let previous_ms = self.next_request_at_ms.load(Ordering::Relaxed);
            let candidate_ms = previous_ms.max(now_ms);
            let next_ms = candidate_ms.saturating_add(PTB_REQUEST_MIN_INTERVAL_MS);
            match self.next_request_at_ms.compare_exchange(
                previous_ms,
                next_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    slot_ms = candidate_ms;
                    break;
                }
                Err(_) => continue,
            }
        }
        let wait_ms = slot_ms.saturating_sub(now_ms);
        if wait_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
        }
    }
}

pub(crate) async fn wait_for_price_to_beat_dirty_market_update() {
    POLYMARKET_PRICE_TO_BEAT_SERVICE
        .dirty_update_notify
        .notified()
        .await;
}

pub(crate) fn take_price_to_beat_dirty_market_slugs() -> Vec<String> {
    POLYMARKET_PRICE_TO_BEAT_SERVICE.take_dirty_market_slugs()
}

pub(crate) fn clear_price_to_beat_dirty_market_slugs(market_slugs: &[String]) {
    POLYMARKET_PRICE_TO_BEAT_SERVICE.clear_dirty_market_slugs(market_slugs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::polymarket_price_to_beat::PriceToBeatSource;
    use chrono::Utc;
    use std::sync::atomic::Ordering;

    fn test_snapshot(
        market_slug: &str,
        price_to_beat: f64,
        source: PriceToBeatSource,
        verified: bool,
    ) -> PolymarketPriceToBeatSnapshot {
        PolymarketPriceToBeatSnapshot {
            event_url: format!("https://polymarket.com/event/{market_slug}"),
            asset: "btc".to_string(),
            timeframe: "5m".to_string(),
            price_to_beat,
            source,
            verified,
            source_latency_ms: Some(0),
            fetched_at: Utc::now(),
        }
    }

    #[test]
    fn seed_snapshot_marks_dirty_market_slug() {
        let market_slug = "btc-updown-5m-2774013200";
        let service = PolymarketPriceToBeatService::new();
        assert!(service.seed_snapshot(market_slug, "btc", "5m", 70_000.0, Some(0)));

        let dirty = service.take_dirty_market_slugs();
        assert!(dirty.contains(&market_slug.to_string()));
        service.clear_dirty_market_slugs(&dirty);
    }

    #[test]
    fn provisional_snapshot_store_marks_dirty_market_slug() {
        let market_slug = "btc-updown-5m-2774013300";
        let service = PolymarketPriceToBeatService::new();
        service.store_provisional_polymarket_snapshot(
            market_slug,
            test_snapshot(market_slug, 70_000.0, PriceToBeatSource::Polymarket, false),
        );

        let dirty = service.take_dirty_market_slugs();
        assert!(dirty.contains(&market_slug.to_string()));
        service.clear_dirty_market_slugs(&dirty);
    }

    #[test]
    fn verified_snapshot_store_marks_dirty_market_slug() {
        let market_slug = "btc-updown-5m-2774013400";
        let service = PolymarketPriceToBeatService::new();
        service.store_verified_polymarket_snapshot(
            market_slug,
            test_snapshot(market_slug, 70_010.0, PriceToBeatSource::Polymarket, true),
        );

        let dirty = service.take_dirty_market_slugs();
        assert!(dirty.contains(&market_slug.to_string()));
        service.clear_dirty_market_slugs(&dirty);
    }

    #[test]
    fn previous_close_cache_is_write_once() {
        let service = PolymarketPriceToBeatService::new();
        service.store_previous_close("btc-updown-5m-2774013100", 70_010.0);
        service.store_previous_close("btc-updown-5m-2774013100", 70_020.0);

        assert_eq!(
            service.cached_previous_close("btc-updown-5m-2774013100"),
            Some(70_010.0)
        );
    }

    #[test]
    fn rate_limit_cooldown_reports_remaining_time() {
        let service = PolymarketPriceToBeatService::new();

        service.arm_rate_limit_cooldown(30_000);

        let remaining = service
            .rate_limit_cooldown_remaining_ms()
            .expect("cooldown");
        assert!(remaining > 0);
        assert!(remaining <= 30_000);
    }

    #[test]
    fn rate_limit_cooldown_keeps_monotonic_max() {
        let service = PolymarketPriceToBeatService::new();
        service.arm_rate_limit_cooldown(60_000);
        let first_until = service.rate_limit_until_ms.load(Ordering::Relaxed);

        service.arm_rate_limit_cooldown(1_000);

        assert_eq!(
            service.rate_limit_until_ms.load(Ordering::Relaxed),
            first_until
        );
    }

    #[tokio::test]
    async fn crypto_price_pacer_reserves_monotonic_slots() {
        let service = PolymarketPriceToBeatService::new();

        service.pace_crypto_price_request().await;
        let first_next = service.next_request_at_ms.load(Ordering::Relaxed);
        service.pace_crypto_price_request().await;
        let second_next = service.next_request_at_ms.load(Ordering::Relaxed);

        assert!(second_next - first_next >= PTB_REQUEST_MIN_INTERVAL_MS);
    }
}
