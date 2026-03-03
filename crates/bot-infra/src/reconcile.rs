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
            // PRCE-02: When WS tick and REST snapshot have equal timestamps,
            // prefer WS data (>=). Rationale: WS ticks arrive in real-time via
            // the CLOB WebSocket feed and represent the most recent market
            // activity. REST snapshots are polled and may lag behind the actual
            // last trade. At equal timestamps, the WS price is at least as
            // fresh and typically more accurate for trigger evaluation.
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

    #[test]
    fn ws_wins_at_equal_timestamp() {
        let now = Utc::now();
        let tick = PriceTick {
            market_slug: "m".to_string(),
            side: "UP".to_string(),
            price: 0.65,
            ts: now,
        };
        let snap = SnapshotPrice {
            market_slug: "m".to_string(),
            price: 0.63,
            ts: now, // Same timestamp as tick
        };

        let r = reconcile_tick_and_snapshot(Some(&tick), &snap, now.timestamp_millis());
        assert_eq!(r.source, "ws", "WS must win when timestamps are equal (>= tie-break)");
        assert_eq!(r.chosen_price, 0.65);
    }

    #[test]
    fn rest_wins_when_strictly_newer() {
        let now = Utc::now();
        let tick = PriceTick {
            market_slug: "m".to_string(),
            side: "UP".to_string(),
            price: 0.61,
            ts: now - Duration::seconds(2),
        };
        let snap = SnapshotPrice {
            market_slug: "m".to_string(),
            price: 0.64,
            ts: now, // Strictly newer than tick
        };

        let r = reconcile_tick_and_snapshot(Some(&tick), &snap, now.timestamp_millis());
        assert_eq!(r.source, "rest", "REST must win when snapshot is strictly newer");
        assert_eq!(r.chosen_price, 0.64);
    }

    #[test]
    fn rest_fallback_when_no_tick() {
        let now = Utc::now();
        let snap = SnapshotPrice {
            market_slug: "m".to_string(),
            price: 0.60,
            ts: now,
        };

        let r = reconcile_tick_and_snapshot(None, &snap, now.timestamp_millis());
        assert_eq!(r.source, "rest", "REST used when no WS tick available");
        assert_eq!(r.chosen_price, 0.60);
    }
}
