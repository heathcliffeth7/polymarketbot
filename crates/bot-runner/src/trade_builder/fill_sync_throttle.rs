use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard},
    time::{Duration, Instant},
};

const INVENTORY_INITIAL_FILL_SYNC_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct UserFillSyncThrottle {
    interval: Duration,
    last_allowed_at: HashMap<i64, Instant>,
}

impl UserFillSyncThrottle {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_allowed_at: HashMap::new(),
        }
    }

    fn allow(&mut self, user_id: i64, now: Instant) -> bool {
        let should_allow = self
            .last_allowed_at
            .get(&user_id)
            .map(|last| now.duration_since(*last) >= self.interval)
            .unwrap_or(true);
        if should_allow {
            self.last_allowed_at.insert(user_id, now);
        }
        should_allow
    }
}

static INVENTORY_INITIAL_FILL_SYNC_THROTTLE: LazyLock<Mutex<UserFillSyncThrottle>> =
    LazyLock::new(|| {
        Mutex::new(UserFillSyncThrottle::new(
            INVENTORY_INITIAL_FILL_SYNC_INTERVAL,
        ))
    });

fn lock_inventory_initial_fill_sync_throttle() -> MutexGuard<'static, UserFillSyncThrottle> {
    INVENTORY_INITIAL_FILL_SYNC_THROTTLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn inventory_initial_fill_sync_due(user_id: i64, now: Instant) -> bool {
    lock_inventory_initial_fill_sync_throttle().allow(user_id, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_allows_same_user_once_per_interval() {
        let mut throttle = UserFillSyncThrottle::new(Duration::from_secs(30));
        let now = Instant::now();

        assert!(throttle.allow(7, now));
        assert!(!throttle.allow(7, now + Duration::from_secs(29)));
        assert!(throttle.allow(7, now + Duration::from_secs(30)));
    }

    #[test]
    fn throttle_is_user_scoped() {
        let mut throttle = UserFillSyncThrottle::new(Duration::from_secs(30));
        let now = Instant::now();

        assert!(throttle.allow(7, now));
        assert!(throttle.allow(8, now + Duration::from_secs(1)));
    }
}
