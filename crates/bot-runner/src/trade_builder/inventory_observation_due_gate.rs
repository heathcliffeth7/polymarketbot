use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use bot_infra::db::PendingTradeBuilderFirstVisibleInventoryObservation;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub(crate) struct InventoryObservationDueKey {
    pub(crate) parent_builder_order_id: i64,
    pub(crate) user_id: i64,
    pub(crate) token_id: String,
}

impl InventoryObservationDueKey {
    pub(crate) fn from_observation(
        observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    ) -> Self {
        Self {
            parent_builder_order_id: observation.parent_builder_order_id,
            user_id: observation.user_id,
            token_id: observation.token_id.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum InventoryObservationDueResult {
    Visible,
    NotVisible,
    ReadError,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum InventoryObservationDueLastResult {
    Visible,
    NotVisible,
    ReadError,
}

impl From<InventoryObservationDueResult> for InventoryObservationDueLastResult {
    fn from(value: InventoryObservationDueResult) -> Self {
        match value {
            InventoryObservationDueResult::Visible => Self::Visible,
            InventoryObservationDueResult::NotVisible => Self::NotVisible,
            InventoryObservationDueResult::ReadError => Self::ReadError,
        }
    }
}

#[derive(Debug, Clone)]
struct InventoryObservationDueState {
    next_due_at: Instant,
    not_visible_streak: u32,
    #[allow(dead_code)]
    last_checked_at: Instant,
    #[allow(dead_code)]
    last_result: InventoryObservationDueLastResult,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub(crate) struct InventoryObservationDueGateSnapshot {
    pub(crate) due_count: u64,
    pub(crate) skipped_not_due_count: u64,
    pub(crate) backoff_active_count: u64,
    pub(crate) backoff_reset_count: u64,
    pub(crate) not_visible_streak_max: u64,
    pub(crate) next_due_min_ms: Option<u64>,
    pub(crate) next_due_max_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub(crate) struct InventoryObservationDueGateUpdate {
    pub(crate) backoff_active_count: u64,
    pub(crate) backoff_reset_count: u64,
    pub(crate) not_visible_streak_max: u64,
    pub(crate) next_due_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct InventoryObservationDueGate {
    states: HashMap<InventoryObservationDueKey, InventoryObservationDueState>,
}

static INVENTORY_OBSERVATION_DUE_GATE: LazyLock<Mutex<InventoryObservationDueGate>> =
    LazyLock::new(|| Mutex::new(InventoryObservationDueGate::default()));

fn lock_due_gate() -> MutexGuard<'static, InventoryObservationDueGate> {
    INVENTORY_OBSERVATION_DUE_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn filter_due_inventory_observations(
    observations: Vec<PendingTradeBuilderFirstVisibleInventoryObservation>,
    now: Instant,
) -> (
    Vec<PendingTradeBuilderFirstVisibleInventoryObservation>,
    InventoryObservationDueGateSnapshot,
) {
    lock_due_gate().filter_due(observations, now)
}

pub(crate) fn record_inventory_observation_due_result(
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    result: InventoryObservationDueResult,
    now: Instant,
) -> InventoryObservationDueGateUpdate {
    let key = InventoryObservationDueKey::from_observation(observation);
    lock_due_gate().record_result(key, result, now)
}

impl InventoryObservationDueGate {
    pub(crate) fn filter_due(
        &mut self,
        observations: Vec<PendingTradeBuilderFirstVisibleInventoryObservation>,
        now: Instant,
    ) -> (
        Vec<PendingTradeBuilderFirstVisibleInventoryObservation>,
        InventoryObservationDueGateSnapshot,
    ) {
        let pending_keys = observations
            .iter()
            .map(InventoryObservationDueKey::from_observation)
            .collect::<HashSet<_>>();
        let before_retain_count = self.states.len();
        self.states.retain(|key, _| pending_keys.contains(key));

        let mut snapshot = InventoryObservationDueGateSnapshot {
            backoff_reset_count: before_retain_count
                .saturating_sub(self.states.len())
                .min(u64::MAX as usize) as u64,
            ..InventoryObservationDueGateSnapshot::default()
        };
        for state in self.states.values() {
            snapshot.not_visible_streak_max = snapshot
                .not_visible_streak_max
                .max(state.not_visible_streak as u64);
        }

        let mut due_observations = Vec::with_capacity(observations.len());
        for observation in observations {
            let key = InventoryObservationDueKey::from_observation(&observation);
            match self.states.get(&key) {
                Some(state) if now < state.next_due_at => {
                    snapshot.skipped_not_due_count =
                        snapshot.skipped_not_due_count.saturating_add(1);
                    snapshot.backoff_active_count = snapshot.backoff_active_count.saturating_add(1);
                    snapshot.record_next_due_ms(duration_ms(
                        state.next_due_at.saturating_duration_since(now),
                    ));
                }
                _ => {
                    snapshot.due_count = snapshot.due_count.saturating_add(1);
                    due_observations.push(observation);
                }
            }
        }

        (due_observations, snapshot)
    }

    pub(crate) fn record_result(
        &mut self,
        key: InventoryObservationDueKey,
        result: InventoryObservationDueResult,
        now: Instant,
    ) -> InventoryObservationDueGateUpdate {
        match result {
            InventoryObservationDueResult::Visible => {
                let was_backed_off = self.states.remove(&key).is_some();
                InventoryObservationDueGateUpdate {
                    backoff_reset_count: u64::from(was_backed_off),
                    ..InventoryObservationDueGateUpdate::default()
                }
            }
            InventoryObservationDueResult::NotVisible => {
                let not_visible_streak = self
                    .states
                    .get(&key)
                    .map(|state| state.not_visible_streak.saturating_add(1))
                    .unwrap_or(1);
                let delay = not_visible_backoff(not_visible_streak);
                self.states.insert(
                    key,
                    InventoryObservationDueState {
                        next_due_at: now + delay,
                        not_visible_streak,
                        last_checked_at: now,
                        last_result: result.into(),
                    },
                );
                InventoryObservationDueGateUpdate {
                    backoff_active_count: 1,
                    not_visible_streak_max: not_visible_streak as u64,
                    next_due_ms: Some(duration_ms(delay)),
                    ..InventoryObservationDueGateUpdate::default()
                }
            }
            InventoryObservationDueResult::ReadError => {
                let not_visible_streak = self
                    .states
                    .get(&key)
                    .map(|state| state.not_visible_streak)
                    .unwrap_or(0);
                let delay = read_error_backoff();
                self.states.insert(
                    key,
                    InventoryObservationDueState {
                        next_due_at: now + delay,
                        not_visible_streak,
                        last_checked_at: now,
                        last_result: result.into(),
                    },
                );
                InventoryObservationDueGateUpdate {
                    backoff_active_count: 1,
                    not_visible_streak_max: not_visible_streak as u64,
                    next_due_ms: Some(duration_ms(delay)),
                    ..InventoryObservationDueGateUpdate::default()
                }
            }
        }
    }
}

impl InventoryObservationDueGateSnapshot {
    fn record_next_due_ms(&mut self, ms: u64) {
        self.next_due_min_ms = Some(self.next_due_min_ms.map_or(ms, |current| current.min(ms)));
        self.next_due_max_ms = Some(self.next_due_max_ms.map_or(ms, |current| current.max(ms)));
    }
}

fn not_visible_backoff(not_visible_streak: u32) -> Duration {
    match not_visible_streak {
        0 | 1 => Duration::from_millis(500),
        2 => Duration::from_secs(1),
        3 => Duration::from_secs(2),
        4..=9 => Duration::from_secs(5),
        10..=29 => Duration::from_secs(15),
        _ => Duration::from_secs(30),
    }
}

fn read_error_backoff() -> Duration {
    Duration::from_secs(1)
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn observation(
        parent_builder_order_id: i64,
        user_id: i64,
        token_id: &str,
    ) -> PendingTradeBuilderFirstVisibleInventoryObservation {
        PendingTradeBuilderFirstVisibleInventoryObservation {
            parent_builder_order_id,
            observer_builder_order_id: None,
            user_id,
            market_slug: "btc-up-down".to_string(),
            token_id: token_id.to_string(),
            outcome_label: "Up".to_string(),
            exchange_order_id: None,
            baseline_visible_qty: None,
            submitted_dynamic_qty: None,
            resolved_fill_qty: None,
            submit_reference_price: None,
            fill_reference_price: None,
            fill_qty_source: None,
            fee_rate_bps: 0,
            fill_observed_at: Utc::now(),
            parent_order_status: "completed".to_string(),
            parent_order_filled_qty: 0.0,
        }
    }

    #[test]
    fn not_visible_record_gets_backoff_and_is_skipped_until_due() {
        let mut gate = InventoryObservationDueGate::default();
        let now = Instant::now();
        let first = observation(1, 7, " token-a ");

        let (due, snapshot) = gate.filter_due(vec![first.clone()], now);
        assert_eq!(due.len(), 1);
        assert_eq!(snapshot.due_count, 1);

        let update = gate.record_result(
            InventoryObservationDueKey::from_observation(&first),
            InventoryObservationDueResult::NotVisible,
            now,
        );
        assert_eq!(update.next_due_ms, Some(500));
        assert_eq!(update.not_visible_streak_max, 1);

        let (due, snapshot) =
            gate.filter_due(vec![first.clone()], now + Duration::from_millis(499));
        assert!(due.is_empty());
        assert_eq!(snapshot.skipped_not_due_count, 1);
        assert_eq!(snapshot.backoff_active_count, 1);
        assert_eq!(snapshot.not_visible_streak_max, 1);

        let (due, snapshot) =
            gate.filter_due(vec![first.clone()], now + Duration::from_millis(500));
        assert_eq!(due.len(), 1);
        assert_eq!(snapshot.due_count, 1);
    }

    #[test]
    fn visible_result_clears_backoff_state() {
        let mut gate = InventoryObservationDueGate::default();
        let now = Instant::now();
        let first = observation(1, 7, "token-a");
        let key = InventoryObservationDueKey::from_observation(&first);
        gate.record_result(key.clone(), InventoryObservationDueResult::NotVisible, now);

        let update = gate.record_result(
            key,
            InventoryObservationDueResult::Visible,
            now + Duration::from_secs(1),
        );
        assert_eq!(update.backoff_reset_count, 1);

        let (due, snapshot) = gate.filter_due(vec![first.clone()], now + Duration::from_secs(1));
        assert_eq!(due.len(), 1);
        assert_eq!(snapshot.due_count, 1);
        assert_eq!(snapshot.skipped_not_due_count, 0);
    }

    #[test]
    fn read_error_uses_short_backoff() {
        let mut gate = InventoryObservationDueGate::default();
        let now = Instant::now();
        let first = observation(1, 7, "token-a");
        let key = InventoryObservationDueKey::from_observation(&first);

        let update = gate.record_result(key, InventoryObservationDueResult::ReadError, now);
        assert_eq!(update.next_due_ms, Some(1_000));

        let (due, snapshot) =
            gate.filter_due(vec![first.clone()], now + Duration::from_millis(999));
        assert!(due.is_empty());
        assert_eq!(snapshot.skipped_not_due_count, 1);

        let (due, _) = gate.filter_due(vec![first], now + Duration::from_secs(1));
        assert_eq!(due.len(), 1);
    }

    #[test]
    fn backoff_is_per_parent_user_token_not_cross_parent() {
        let mut gate = InventoryObservationDueGate::default();
        let now = Instant::now();
        let first = observation(1, 7, "token-a");
        let second = observation(2, 7, "token-a");
        gate.record_result(
            InventoryObservationDueKey::from_observation(&first),
            InventoryObservationDueResult::NotVisible,
            now,
        );

        let (due, snapshot) = gate.filter_due(
            vec![first, second.clone()],
            now + Duration::from_millis(100),
        );
        assert_eq!(due.len(), 1);
        assert_eq!(
            due[0].parent_builder_order_id,
            second.parent_builder_order_id
        );
        assert_eq!(snapshot.skipped_not_due_count, 1);
        assert_eq!(snapshot.due_count, 1);
    }

    #[test]
    fn no_longer_pending_state_is_reset() {
        let mut gate = InventoryObservationDueGate::default();
        let now = Instant::now();
        let first = observation(1, 7, "token-a");
        gate.record_result(
            InventoryObservationDueKey::from_observation(&first),
            InventoryObservationDueResult::NotVisible,
            now,
        );

        let (_due, snapshot) = gate.filter_due(Vec::new(), now + Duration::from_millis(100));
        assert_eq!(snapshot.backoff_reset_count, 1);

        let (due, snapshot) =
            gate.filter_due(vec![first.clone()], now + Duration::from_millis(100));
        assert_eq!(due.len(), 1);
        assert_eq!(snapshot.due_count, 1);
    }
}
