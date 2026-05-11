use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketCycleId(pub String);

impl MarketCycleId {
    pub fn from_unix_start(ts: i64) -> Self {
        Self(format!("btc-updown-5m-{ts}"))
    }

    pub fn parse_unix_start(&self) -> Option<i64> {
        self.0.rsplit('-').next()?.parse::<i64>().ok()
    }

    pub fn from_now_rounded_5m(now: DateTime<Utc>) -> Self {
        let ts = now.timestamp();
        let cycle_start = ts - (ts % 300);
        Self::from_unix_start(cycle_start)
    }

    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        let ts = self.parse_unix_start()?;
        Utc.timestamp_opt(ts, 0).single()
    }
}

impl fmt::Display for MarketCycleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
