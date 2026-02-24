use crate::market_data::{PriceTick, SnapshotPrice};

#[derive(Debug, Clone)]
pub struct ReconcileResult {
    pub chosen_price: f64,
    pub source: &'static str,
    pub stale_data_ms: u64,
}

pub fn reconcile_tick_and_snapshot(
    tick: Option<&PriceTick>,
    snapshot: &SnapshotPrice,
    now_ms: i64,
) -> ReconcileResult {
    match tick {
        Some(t) => {
            let tick_ms = t.ts.timestamp_millis();
            let stale = now_ms.saturating_sub(tick_ms) as u64;
            if t.ts >= snapshot.ts {
                ReconcileResult {
                    chosen_price: t.price,
                    source: "ws",
                    stale_data_ms: stale,
                }
            } else {
                let snap_stale = now_ms.saturating_sub(snapshot.ts.timestamp_millis()) as u64;
                ReconcileResult {
                    chosen_price: snapshot.price,
                    source: "rest",
                    stale_data_ms: snap_stale,
                }
            }
        }
        None => {
            let snap_stale = now_ms.saturating_sub(snapshot.ts.timestamp_millis()) as u64;
            ReconcileResult {
                chosen_price: snapshot.price,
                source: "rest",
                stale_data_ms: snap_stale,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::{PriceTick, SnapshotPrice};
    use chrono::{Duration, Utc};

    #[test]
    fn prefers_newer_ws_tick() {
        let now = Utc::now();
        let tick = PriceTick {
            market_slug: "m".to_string(),
            side: "UP".to_string(),
            price: 0.61,
            ts: now,
        };
        let snap = SnapshotPrice {
            market_slug: "m".to_string(),
            price: 0.59,
            ts: now - Duration::seconds(1),
        };

        let r = reconcile_tick_and_snapshot(Some(&tick), &snap, now.timestamp_millis());
        assert_eq!(r.source, "ws");
        assert_eq!(r.chosen_price, 0.61);
    }
}
