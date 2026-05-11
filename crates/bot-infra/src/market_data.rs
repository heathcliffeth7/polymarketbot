use anyhow::Result;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct PriceTick {
    pub market_slug: String,
    pub side: String,
    pub price: f64,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SnapshotPrice {
    pub market_slug: String,
    pub price: f64,
    pub ts: DateTime<Utc>,
}

pub trait MarketDataProvider {
    fn next_tick(&mut self, market_slug: &str) -> Result<Option<PriceTick>>;
    fn snapshot(&self, market_slug: &str) -> Result<SnapshotPrice>;
}

/// Mock provider that simulates a short ws stream, then a stale gap, then fresh data.
#[derive(Debug, Default)]
pub struct MockMarketDataProvider {
    cursor: usize,
}

impl MockMarketDataProvider {
    pub fn new() -> Self {
        Self { cursor: 0 }
    }
}

impl MarketDataProvider for MockMarketDataProvider {
    fn next_tick(&mut self, market_slug: &str) -> Result<Option<PriceTick>> {
        self.cursor += 1;
        let now = Utc::now();

        // Simulate sparse stream every third iteration to exercise fallback path.
        if self.cursor % 3 == 0 {
            return Ok(None);
        }

        let price = match self.cursor % 5 {
            0 => 0.58,
            1 => 0.60,
            2 => 0.62,
            3 => 0.59,
            _ => 0.61,
        };

        Ok(Some(PriceTick {
            market_slug: market_slug.to_string(),
            side: "UP".to_string(),
            price,
            ts: now,
        }))
    }

    fn snapshot(&self, market_slug: &str) -> Result<SnapshotPrice> {
        Ok(SnapshotPrice {
            market_slug: market_slug.to_string(),
            price: 0.60,
            ts: Utc::now(),
        })
    }
}
