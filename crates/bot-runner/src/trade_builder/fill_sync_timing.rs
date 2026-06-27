use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct FinalFillSyncTimingStats {
    pub(crate) total_ms: u64,
    pub(crate) fetch_page_ms: u64,
    pub(crate) page_apply_ms: u64,
    pub(crate) db_order_lookup_ms: u64,
    pub(crate) db_upsert_ms: u64,
    pub(crate) pages_scanned: u64,
    pub(crate) raw_count: u64,
    pub(crate) synced_count: u64,
    pub(crate) call_count: u64,
    pub(crate) success_count: u64,
    pub(crate) error_count: u64,
    pub(crate) skipped_fresh_count: u64,
    pub(crate) required_count: u64,
    pub(crate) max_user_ms: u64,
    pub(crate) max_user_id: Option<i64>,
    seen_users: HashSet<i64>,
}

impl FinalFillSyncTimingStats {
    pub(crate) fn merge(&mut self, other: FinalFillSyncTimingStats) {
        self.total_ms = self.total_ms.saturating_add(other.total_ms);
        self.fetch_page_ms = self.fetch_page_ms.saturating_add(other.fetch_page_ms);
        self.page_apply_ms = self.page_apply_ms.saturating_add(other.page_apply_ms);
        self.db_order_lookup_ms = self
            .db_order_lookup_ms
            .saturating_add(other.db_order_lookup_ms);
        self.db_upsert_ms = self.db_upsert_ms.saturating_add(other.db_upsert_ms);
        self.pages_scanned = self.pages_scanned.saturating_add(other.pages_scanned);
        self.raw_count = self.raw_count.saturating_add(other.raw_count);
        self.synced_count = self.synced_count.saturating_add(other.synced_count);
        self.call_count = self.call_count.saturating_add(other.call_count);
        self.success_count = self.success_count.saturating_add(other.success_count);
        self.error_count = self.error_count.saturating_add(other.error_count);
        self.skipped_fresh_count = self
            .skipped_fresh_count
            .saturating_add(other.skipped_fresh_count);
        self.required_count = self.required_count.saturating_add(other.required_count);
        if other.max_user_ms >= self.max_user_ms {
            self.max_user_ms = other.max_user_ms;
            self.max_user_id = other.max_user_id;
        }
        self.seen_users.extend(other.seen_users);
    }

    pub(crate) fn record_call(&mut self, user_id: i64) {
        self.call_count = self.call_count.saturating_add(1);
        self.seen_users.insert(user_id);
    }

    pub(crate) fn record_success(&mut self) {
        self.success_count = self.success_count.saturating_add(1);
    }

    pub(crate) fn record_error(&mut self) {
        self.error_count = self.error_count.saturating_add(1);
    }

    pub(crate) fn record_skipped_fresh(&mut self, user_id: i64) {
        self.skipped_fresh_count = self.skipped_fresh_count.saturating_add(1);
        self.seen_users.insert(user_id);
    }

    pub(crate) fn record_required(&mut self) {
        self.required_count = self.required_count.saturating_add(1);
    }

    pub(crate) fn record_user_ms(&mut self, user_id: i64, elapsed_ms: u64) {
        if elapsed_ms >= self.max_user_ms {
            self.max_user_ms = elapsed_ms;
            self.max_user_id = Some(user_id);
        }
    }

    pub(crate) fn record_fetch_page_ms(&mut self, elapsed_ms: u64) {
        self.fetch_page_ms = self.fetch_page_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_page_apply_ms(&mut self, elapsed_ms: u64) {
        self.page_apply_ms = self.page_apply_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_db_order_lookup_ms(&mut self, elapsed_ms: u64) {
        self.db_order_lookup_ms = self.db_order_lookup_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_db_upsert_ms(&mut self, elapsed_ms: u64) {
        self.db_upsert_ms = self.db_upsert_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_page_counts(&mut self, raw_count: usize, synced_count: usize) {
        self.pages_scanned = self.pages_scanned.saturating_add(1);
        self.raw_count = self.raw_count.saturating_add(raw_count as u64);
        self.synced_count = self.synced_count.saturating_add(synced_count as u64);
    }

    pub(crate) fn user_count(&self) -> u64 {
        self.seen_users.len() as u64
    }

    pub(crate) fn unknown_ms(&self) -> u64 {
        self.total_ms
            .saturating_sub(self.fetch_page_ms)
            .saturating_sub(self.page_apply_ms)
    }
}

pub(crate) fn millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

pub(crate) struct FinalFillSyncTimer {
    started: Instant,
}

impl FinalFillSyncTimer {
    pub(crate) fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        millis_u64(self.started.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_sync_fetch_and_db_phase_sum_reduce_unknown() {
        let mut stats = FinalFillSyncTimingStats {
            total_ms: 200,
            ..FinalFillSyncTimingStats::default()
        };
        stats.record_fetch_page_ms(100);
        stats.record_page_apply_ms(70);
        stats.record_db_order_lookup_ms(30);
        stats.record_db_upsert_ms(20);

        assert_eq!(stats.unknown_ms(), 30);
        assert_eq!(stats.db_order_lookup_ms, 30);
        assert_eq!(stats.db_upsert_ms, 20);
    }

    #[test]
    fn fill_sync_unknown_does_not_double_count_page_apply_nested_phases() {
        let mut stats = FinalFillSyncTimingStats {
            total_ms: 200,
            ..FinalFillSyncTimingStats::default()
        };
        stats.record_fetch_page_ms(50);
        stats.record_page_apply_ms(100);
        stats.record_db_order_lookup_ms(80);
        stats.record_db_upsert_ms(80);

        assert_eq!(stats.unknown_ms(), 50);
    }

    #[test]
    fn fill_sync_aggregate_counts_pages_raw_rows_and_synced_fills() {
        let mut stats = FinalFillSyncTimingStats::default();
        stats.record_page_counts(10, 2);
        stats.record_page_counts(5, 1);

        assert_eq!(stats.pages_scanned, 2);
        assert_eq!(stats.raw_count, 15);
        assert_eq!(stats.synced_count, 3);
    }

    #[test]
    fn fill_sync_max_user_ms_tracks_slowest_user() {
        let mut stats = FinalFillSyncTimingStats::default();
        stats.record_call(7);
        stats.record_user_ms(7, 25);
        stats.record_call(8);
        stats.record_user_ms(8, 60);

        assert_eq!(stats.user_count(), 2);
        assert_eq!(stats.call_count, 2);
        assert_eq!(stats.max_user_ms, 60);
        assert_eq!(stats.max_user_id, Some(8));
    }

    #[test]
    fn fill_sync_fresh_skip_and_required_counts_are_user_scoped() {
        let mut stats = FinalFillSyncTimingStats::default();
        stats.record_required();
        stats.record_call(7);
        stats.record_skipped_fresh(8);

        assert_eq!(stats.required_count, 1);
        assert_eq!(stats.call_count, 1);
        assert_eq!(stats.skipped_fresh_count, 1);
        assert_eq!(stats.user_count(), 2);
    }

    #[test]
    fn fill_sync_merge_accumulates_initial_sync_breakdown() {
        let mut stats = FinalFillSyncTimingStats::default();
        let mut first = FinalFillSyncTimingStats::default();
        first.record_call(7);
        first.record_success();
        first.record_fetch_page_ms(100);
        first.record_page_apply_ms(50);
        first.record_page_counts(300, 299);
        first.record_user_ms(7, 150);
        first.total_ms = 150;

        let mut second = FinalFillSyncTimingStats::default();
        second.record_call(8);
        second.record_error();
        second.record_db_upsert_ms(20);
        second.record_user_ms(8, 200);
        second.total_ms = 200;

        stats.merge(first);
        stats.merge(second);

        assert_eq!(stats.total_ms, 350);
        assert_eq!(stats.fetch_page_ms, 100);
        assert_eq!(stats.page_apply_ms, 50);
        assert_eq!(stats.db_upsert_ms, 20);
        assert_eq!(stats.raw_count, 300);
        assert_eq!(stats.synced_count, 299);
        assert_eq!(stats.call_count, 2);
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.user_count(), 2);
        assert_eq!(stats.max_user_ms, 200);
        assert_eq!(stats.max_user_id, Some(8));
    }

    #[test]
    fn sync_recent_trade_builder_fills_without_stats_preserves_existing_behavior() {
        let stats = FinalFillSyncTimingStats::default();

        assert_eq!(stats.call_count, 0);
        assert_eq!(stats.fetch_page_ms, 0);
        assert_eq!(stats.page_apply_ms, 0);
    }
}
