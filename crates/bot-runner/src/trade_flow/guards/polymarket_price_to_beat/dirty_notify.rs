use super::{
    PolymarketPriceToBeatService, PolymarketPriceToBeatSnapshot, POLYMARKET_PRICE_TO_BEAT_SERVICE,
};
use std::collections::HashSet;

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
}
