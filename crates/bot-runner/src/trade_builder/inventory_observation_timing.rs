use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use bot_infra::exchange::TokenInventorySnapshot;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum InventoryObservationReadResult {
    Visible { qty: f64 },
    NotVisible,
    ReadError,
}

#[derive(Debug, Clone)]
pub(crate) enum InventoryPositionsSnapshotCacheEntry {
    Snapshot(TokenInventorySnapshot),
    Unsupported,
    ReadError,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub(crate) struct InventoryObservationCacheKey {
    pub(crate) executor_user_id: i64,
    pub(crate) token_id: String,
}

impl InventoryObservationCacheKey {
    pub(crate) fn new(executor_user_id: i64, token_id: &str) -> Option<Self> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            return None;
        }
        Some(Self {
            executor_user_id,
            token_id: token_id.to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum InventoryObservationPhase {
    ExternalLookup,
    InitialFillSync,
    ApplyTotal,
    ExecutorLookup,
    ConfigLookup,
    SnapshotCache,
    TokenResultCache,
    RecordFinalize,
    DbObservationInsert,
    ParentRebase,
    UnknownOrUnattributed,
}

impl InventoryObservationPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExternalLookup => "builder_inventory_observation_external_lookup",
            Self::InitialFillSync => "builder_inventory_observation_initial_fill_sync",
            Self::ApplyTotal => "builder_inventory_observation_apply_total",
            Self::ExecutorLookup => "builder_inventory_observation_executor_lookup",
            Self::ConfigLookup => "builder_inventory_observation_config_lookup",
            Self::SnapshotCache => "builder_inventory_observation_snapshot_cache",
            Self::TokenResultCache => "builder_inventory_observation_token_result_cache",
            Self::RecordFinalize => "builder_inventory_observation_record_finalize",
            Self::DbObservationInsert => "builder_inventory_observation_db_observation_insert",
            Self::ParentRebase => "builder_inventory_observation_parent_rebase",
            Self::UnknownOrUnattributed => "builder_inventory_observation_unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct InventoryObservationPhaseDuration {
    pub(crate) phase: &'static str,
    pub(crate) ms: u64,
}

fn select_slowest_phase(
    candidates: &[InventoryObservationPhaseDuration],
) -> InventoryObservationPhaseDuration {
    candidates
        .iter()
        .copied()
        .fold(
            None,
            |best: Option<InventoryObservationPhaseDuration>, candidate| match best {
                Some(best) if best.ms >= candidate.ms => Some(best),
                _ => Some(candidate),
            },
        )
        .unwrap_or(InventoryObservationPhaseDuration {
            phase: InventoryObservationPhase::UnknownOrUnattributed.as_str(),
            ms: 0,
        })
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InventoryObservationTimingStats {
    pub(crate) total_ms: u64,
    pub(crate) attempted_count: u64,
    pub(crate) success_count: u64,
    pub(crate) not_visible_count: u64,
    pub(crate) error_count: u64,
    pub(crate) external_error_count: u64,
    pub(crate) cached_error_count: u64,
    pub(crate) db_insert_error_count: u64,
    pub(crate) parent_rebase_error_count: u64,
    pub(crate) due_count: u64,
    pub(crate) skipped_not_due_count: u64,
    pub(crate) backoff_active_count: u64,
    pub(crate) backoff_reset_count: u64,
    pub(crate) not_visible_streak_max: u64,
    pub(crate) next_due_min_ms: u64,
    pub(crate) next_due_max_ms: u64,
    pub(crate) initial_fill_sync_skipped_no_due_count: u64,
    pub(crate) cache_hit_count: u64,
    pub(crate) cache_miss_count: u64,
    pub(crate) uncacheable_count: u64,
    pub(crate) positions_snapshot_record_hit_count: u64,
    pub(crate) positions_snapshot_record_miss_count: u64,
    pub(crate) positions_snapshot_fetch_count: u64,
    pub(crate) positions_snapshot_fetch_ms: u64,
    pub(crate) positions_snapshot_error_count: u64,
    pub(crate) positions_snapshot_cached_error_count: u64,
    pub(crate) positions_snapshot_unsupported_count: u64,
    pub(crate) positions_snapshot_row_count: u64,
    pub(crate) positions_snapshot_alias_count: u64,
    pub(crate) token_lookup_count: u64,
    pub(crate) token_lookup_ms: u64,
    pub(crate) token_visible_count: u64,
    pub(crate) token_not_visible_count: u64,
    pub(crate) fallback_available_token_qty_ms: u64,
    pub(crate) external_lookup_ms: u64,
    pub(crate) config_lookup_ms: u64,
    pub(crate) config_lookup_count: u64,
    pub(crate) executor_lookup_ms: u64,
    pub(crate) executor_lookup_count: u64,
    pub(crate) initial_fill_sync_ms: u64,
    pub(crate) initial_fill_sync_call_count: u64,
    pub(crate) initial_fill_sync_user_count: u64,
    pub(crate) initial_fill_sync_detail:
        crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats,
    pub(crate) snapshot_cache_ms: u64,
    pub(crate) token_result_cache_ms: u64,
    pub(crate) apply_total_ms: u64,
    pub(crate) apply_prepare_ms: u64,
    pub(crate) record_finalize_ms: u64,
    pub(crate) db_observation_insert_ms: u64,
    pub(crate) parent_rebase_ms: u64,
    pub(crate) max_ms: u64,
    pub(crate) max_phase: Option<&'static str>,
    pub(crate) max_record_id: Option<i64>,
    pub(crate) max_market_slug: Option<String>,
    pub(crate) max_token_id: Option<String>,
    pub(crate) max_user_id: Option<i64>,
    pub(crate) over_100ms_count: u64,
    pub(crate) over_250ms_count: u64,
    pub(crate) over_1000ms_count: u64,
    seen_users: HashSet<i64>,
    seen_tokens: HashSet<String>,
    seen_keys: HashSet<InventoryObservationCacheKey>,
}

impl InventoryObservationTimingStats {
    pub(crate) fn record_attempt(&mut self, executor_user_id: i64, token_id: &str) {
        self.attempted_count = self.attempted_count.saturating_add(1);
        self.seen_users.insert(executor_user_id);
        let token_id = token_id.trim();
        if !token_id.is_empty() {
            self.seen_tokens.insert(token_id.to_string());
        }
    }

    pub(crate) fn record_cache_miss(&mut self, key: &InventoryObservationCacheKey) {
        self.cache_miss_count = self.cache_miss_count.saturating_add(1);
        self.seen_keys.insert(key.clone());
    }

    pub(crate) fn record_cache_hit(&mut self, key: &InventoryObservationCacheKey) {
        self.cache_hit_count = self.cache_hit_count.saturating_add(1);
        self.seen_keys.insert(key.clone());
    }

    pub(crate) fn record_uncacheable(&mut self) {
        self.uncacheable_count = self.uncacheable_count.saturating_add(1);
    }

    pub(crate) fn record_due_gate_snapshot(
        &mut self,
        snapshot: crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueGateSnapshot,
    ) {
        self.due_count = self.due_count.saturating_add(snapshot.due_count);
        self.skipped_not_due_count = self
            .skipped_not_due_count
            .saturating_add(snapshot.skipped_not_due_count);
        self.backoff_active_count = self
            .backoff_active_count
            .saturating_add(snapshot.backoff_active_count);
        self.backoff_reset_count = self
            .backoff_reset_count
            .saturating_add(snapshot.backoff_reset_count);
        self.not_visible_streak_max = self
            .not_visible_streak_max
            .max(snapshot.not_visible_streak_max);
        self.record_next_due_ms(snapshot.next_due_min_ms);
        self.record_next_due_ms(snapshot.next_due_max_ms);
    }

    pub(crate) fn record_due_gate_update(
        &mut self,
        update: crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueGateUpdate,
    ) {
        self.backoff_active_count = self
            .backoff_active_count
            .saturating_add(update.backoff_active_count);
        self.backoff_reset_count = self
            .backoff_reset_count
            .saturating_add(update.backoff_reset_count);
        self.not_visible_streak_max = self
            .not_visible_streak_max
            .max(update.not_visible_streak_max);
        self.record_next_due_ms(update.next_due_ms);
    }

    pub(crate) fn record_initial_fill_sync_skipped_no_due(&mut self) {
        self.initial_fill_sync_skipped_no_due_count = self
            .initial_fill_sync_skipped_no_due_count
            .saturating_add(1);
    }

    fn record_next_due_ms(&mut self, next_due_ms: Option<u64>) {
        let Some(next_due_ms) = next_due_ms else {
            return;
        };
        self.next_due_min_ms = if self.next_due_min_ms == 0 {
            next_due_ms
        } else {
            self.next_due_min_ms.min(next_due_ms)
        };
        self.next_due_max_ms = self.next_due_max_ms.max(next_due_ms);
    }

    pub(crate) fn record_positions_snapshot_record_miss(&mut self) {
        self.positions_snapshot_record_miss_count =
            self.positions_snapshot_record_miss_count.saturating_add(1);
    }

    pub(crate) fn record_positions_snapshot_record_hit(&mut self) {
        self.positions_snapshot_record_hit_count =
            self.positions_snapshot_record_hit_count.saturating_add(1);
    }

    pub(crate) fn record_positions_snapshot_fetch(
        &mut self,
        elapsed_ms: u64,
        row_count: usize,
        alias_count: usize,
    ) {
        self.positions_snapshot_fetch_count = self.positions_snapshot_fetch_count.saturating_add(1);
        self.positions_snapshot_fetch_ms =
            self.positions_snapshot_fetch_ms.saturating_add(elapsed_ms);
        self.positions_snapshot_row_count = self
            .positions_snapshot_row_count
            .saturating_add(row_count as u64);
        self.positions_snapshot_alias_count = self
            .positions_snapshot_alias_count
            .saturating_add(alias_count as u64);
        self.external_lookup_ms = self.external_lookup_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_positions_snapshot_error(&mut self) {
        self.positions_snapshot_error_count = self.positions_snapshot_error_count.saturating_add(1);
    }

    pub(crate) fn record_positions_snapshot_cached_error(&mut self) {
        self.positions_snapshot_cached_error_count =
            self.positions_snapshot_cached_error_count.saturating_add(1);
    }

    pub(crate) fn record_positions_snapshot_unsupported(&mut self) {
        self.positions_snapshot_unsupported_count =
            self.positions_snapshot_unsupported_count.saturating_add(1);
    }

    pub(crate) fn record_token_lookup(
        &mut self,
        elapsed_ms: u64,
        result: InventoryObservationReadResult,
    ) {
        self.token_lookup_count = self.token_lookup_count.saturating_add(1);
        self.token_lookup_ms = self.token_lookup_ms.saturating_add(elapsed_ms);
        self.external_lookup_ms = self.external_lookup_ms.saturating_add(elapsed_ms);
        match result {
            InventoryObservationReadResult::Visible { .. } => {
                self.token_visible_count = self.token_visible_count.saturating_add(1);
            }
            InventoryObservationReadResult::NotVisible => {
                self.token_not_visible_count = self.token_not_visible_count.saturating_add(1);
            }
            InventoryObservationReadResult::ReadError => {}
        }
    }

    pub(crate) fn record_fallback_available_token_qty_ms(&mut self, elapsed_ms: u64) {
        self.fallback_available_token_qty_ms = self
            .fallback_available_token_qty_ms
            .saturating_add(elapsed_ms);
        self.external_lookup_ms = self.external_lookup_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_config_lookup(&mut self, elapsed_ms: u64) {
        self.config_lookup_count = self.config_lookup_count.saturating_add(1);
        self.config_lookup_ms = self.config_lookup_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_executor_lookup(&mut self, elapsed_ms: u64) {
        self.executor_lookup_count = self.executor_lookup_count.saturating_add(1);
        self.executor_lookup_ms = self.executor_lookup_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_initial_fill_sync(&mut self, elapsed_ms: u64) {
        self.initial_fill_sync_call_count = self.initial_fill_sync_call_count.saturating_add(1);
        self.initial_fill_sync_user_count = self.initial_fill_sync_user_count.saturating_add(1);
        self.initial_fill_sync_ms = self.initial_fill_sync_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_snapshot_cache_ms(&mut self, elapsed_ms: u64) {
        self.snapshot_cache_ms = self.snapshot_cache_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_token_result_cache_ms(&mut self, elapsed_ms: u64) {
        self.token_result_cache_ms = self.token_result_cache_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_apply_total_ms(&mut self, elapsed_ms: u64) {
        self.apply_total_ms = self.apply_total_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_apply_prepare_ms(&mut self, elapsed_ms: u64) {
        self.apply_prepare_ms = self.apply_prepare_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_record_finalize_ms(&mut self, elapsed_ms: u64) {
        self.record_finalize_ms = self.record_finalize_ms.saturating_add(elapsed_ms);
    }

    pub(crate) fn record_success(&mut self) {
        self.success_count = self.success_count.saturating_add(1);
    }

    pub(crate) fn record_not_visible(&mut self) {
        self.not_visible_count = self.not_visible_count.saturating_add(1);
    }

    pub(crate) fn record_external_error(&mut self) {
        self.external_error_count = self.external_error_count.saturating_add(1);
        self.error_count = self.error_count.saturating_add(1);
    }

    pub(crate) fn record_cached_error(&mut self) {
        self.cached_error_count = self.cached_error_count.saturating_add(1);
        self.error_count = self.error_count.saturating_add(1);
    }

    pub(crate) fn record_db_insert_error(&mut self) {
        self.db_insert_error_count = self.db_insert_error_count.saturating_add(1);
        self.error_count = self.error_count.saturating_add(1);
    }

    pub(crate) fn record_parent_rebase_error(&mut self) {
        self.parent_rebase_error_count = self.parent_rebase_error_count.saturating_add(1);
        self.error_count = self.error_count.saturating_add(1);
    }

    pub(crate) fn add_phase_ms(&mut self, phase: InventoryObservationPhase, ms: u64) {
        match phase {
            InventoryObservationPhase::ExternalLookup => {
                self.external_lookup_ms = self.external_lookup_ms.saturating_add(ms);
            }
            InventoryObservationPhase::InitialFillSync => self.record_initial_fill_sync(ms),
            InventoryObservationPhase::ApplyTotal => self.record_apply_total_ms(ms),
            InventoryObservationPhase::ExecutorLookup => self.record_executor_lookup(ms),
            InventoryObservationPhase::ConfigLookup => self.record_config_lookup(ms),
            InventoryObservationPhase::SnapshotCache => self.record_snapshot_cache_ms(ms),
            InventoryObservationPhase::TokenResultCache => self.record_token_result_cache_ms(ms),
            InventoryObservationPhase::RecordFinalize => self.record_record_finalize_ms(ms),
            InventoryObservationPhase::DbObservationInsert => {
                self.db_observation_insert_ms = self.db_observation_insert_ms.saturating_add(ms);
            }
            InventoryObservationPhase::ParentRebase => {
                self.parent_rebase_ms = self.parent_rebase_ms.saturating_add(ms);
            }
            InventoryObservationPhase::UnknownOrUnattributed => {}
        }
    }

    pub(crate) fn record_latency(
        &mut self,
        elapsed_ms: u64,
        record_id: i64,
        market_slug: &str,
        token_id: &str,
        user_id: i64,
        max_phase: &'static str,
    ) {
        if elapsed_ms > 100 {
            self.over_100ms_count = self.over_100ms_count.saturating_add(1);
        }
        if elapsed_ms > 250 {
            self.over_250ms_count = self.over_250ms_count.saturating_add(1);
        }
        if elapsed_ms > 1_000 {
            self.over_1000ms_count = self.over_1000ms_count.saturating_add(1);
        }
        if elapsed_ms >= self.max_ms {
            self.max_ms = elapsed_ms;
            self.max_phase = Some(max_phase);
            self.max_record_id = Some(record_id);
            self.max_market_slug = Some(market_slug.to_string());
            self.max_token_id = Some(token_id.to_string());
            self.max_user_id = Some(user_id);
        }
    }

    pub(crate) fn unique_user_count(&self) -> u64 {
        self.seen_users.len() as u64
    }

    pub(crate) fn unique_token_count(&self) -> u64 {
        self.seen_tokens.len() as u64
    }

    pub(crate) fn unique_key_count(&self) -> u64 {
        self.seen_keys.len() as u64
    }

    pub(crate) fn duplicate_key_count(&self) -> u64 {
        self.attempted_count
            .saturating_sub(self.unique_key_count())
            .saturating_sub(self.uncacheable_count)
    }

    pub(crate) fn phase_sum_ms(&self) -> u64 {
        self.config_lookup_ms
            .saturating_add(self.executor_lookup_ms)
            .saturating_add(self.initial_fill_sync_ms)
            .saturating_add(self.snapshot_cache_ms)
            .saturating_add(self.token_result_cache_ms)
            .saturating_add(self.external_lookup_ms)
            .saturating_add(self.apply_total_ms)
            .saturating_add(self.record_finalize_ms)
    }

    pub(crate) fn unknown_or_unattributed_ms(&self) -> u64 {
        self.total_ms.saturating_sub(self.phase_sum_ms())
    }

    pub(crate) fn apply_unknown_ms(&self) -> u64 {
        self.apply_total_ms
            .saturating_sub(self.apply_prepare_ms)
            .saturating_sub(self.db_observation_insert_ms)
            .saturating_sub(self.parent_rebase_ms)
    }

    pub(crate) fn slowest_phase(&self) -> InventoryObservationPhaseDuration {
        let candidates = [
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ExternalLookup.as_str(),
                ms: self.external_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::InitialFillSync.as_str(),
                ms: self.initial_fill_sync_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ApplyTotal.as_str(),
                ms: self.apply_total_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ExecutorLookup.as_str(),
                ms: self.executor_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ConfigLookup.as_str(),
                ms: self.config_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::SnapshotCache.as_str(),
                ms: self.snapshot_cache_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::TokenResultCache.as_str(),
                ms: self.token_result_cache_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::RecordFinalize.as_str(),
                ms: self.record_finalize_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::UnknownOrUnattributed.as_str(),
                ms: self.unknown_or_unattributed_ms(),
            },
        ];
        select_slowest_phase(&candidates)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InventoryObservationRecordTiming {
    pub(crate) external_lookup_ms: u64,
    pub(crate) initial_fill_sync_ms: u64,
    pub(crate) apply_total_ms: u64,
    pub(crate) executor_lookup_ms: u64,
    pub(crate) config_lookup_ms: u64,
    pub(crate) snapshot_cache_ms: u64,
    pub(crate) token_result_cache_ms: u64,
    pub(crate) record_finalize_ms: u64,
}

impl InventoryObservationRecordTiming {
    pub(crate) fn add_phase_ms(&mut self, phase: InventoryObservationPhase, ms: u64) {
        match phase {
            InventoryObservationPhase::ExternalLookup => {
                self.external_lookup_ms = self.external_lookup_ms.saturating_add(ms);
            }
            InventoryObservationPhase::InitialFillSync => {
                self.initial_fill_sync_ms = self.initial_fill_sync_ms.saturating_add(ms);
            }
            InventoryObservationPhase::ApplyTotal => {
                self.apply_total_ms = self.apply_total_ms.saturating_add(ms);
            }
            InventoryObservationPhase::ExecutorLookup => {
                self.executor_lookup_ms = self.executor_lookup_ms.saturating_add(ms);
            }
            InventoryObservationPhase::ConfigLookup => {
                self.config_lookup_ms = self.config_lookup_ms.saturating_add(ms);
            }
            InventoryObservationPhase::SnapshotCache => {
                self.snapshot_cache_ms = self.snapshot_cache_ms.saturating_add(ms);
            }
            InventoryObservationPhase::TokenResultCache => {
                self.token_result_cache_ms = self.token_result_cache_ms.saturating_add(ms);
            }
            InventoryObservationPhase::RecordFinalize => {
                self.record_finalize_ms = self.record_finalize_ms.saturating_add(ms);
            }
            InventoryObservationPhase::DbObservationInsert
            | InventoryObservationPhase::ParentRebase
            | InventoryObservationPhase::UnknownOrUnattributed => {}
        }
    }

    pub(crate) fn phase_sum_ms(&self) -> u64 {
        self.external_lookup_ms
            .saturating_add(self.initial_fill_sync_ms)
            .saturating_add(self.apply_total_ms)
            .saturating_add(self.executor_lookup_ms)
            .saturating_add(self.config_lookup_ms)
            .saturating_add(self.snapshot_cache_ms)
            .saturating_add(self.token_result_cache_ms)
            .saturating_add(self.record_finalize_ms)
    }

    pub(crate) fn slowest_phase(&self, record_total_ms: u64) -> InventoryObservationPhaseDuration {
        let candidates = [
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ExternalLookup.as_str(),
                ms: self.external_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::InitialFillSync.as_str(),
                ms: self.initial_fill_sync_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ApplyTotal.as_str(),
                ms: self.apply_total_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ExecutorLookup.as_str(),
                ms: self.executor_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::ConfigLookup.as_str(),
                ms: self.config_lookup_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::SnapshotCache.as_str(),
                ms: self.snapshot_cache_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::TokenResultCache.as_str(),
                ms: self.token_result_cache_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::RecordFinalize.as_str(),
                ms: self.record_finalize_ms,
            },
            InventoryObservationPhaseDuration {
                phase: InventoryObservationPhase::UnknownOrUnattributed.as_str(),
                ms: record_total_ms.saturating_sub(self.phase_sum_ms()),
            },
        ];
        select_slowest_phase(&candidates)
    }
}

pub(crate) fn millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

pub(crate) struct InventoryObservationTimer {
    started: Instant,
}

impl InventoryObservationTimer {
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
    fn unique_key_count_distinguishes_user_token_pairs() {
        let mut stats = InventoryObservationTimingStats::default();
        let key_a = InventoryObservationCacheKey::new(1, "token").unwrap();
        let key_b = InventoryObservationCacheKey::new(2, "token").unwrap();
        stats.record_attempt(1, "token");
        stats.record_cache_miss(&key_a);
        stats.record_attempt(1, "token");
        stats.record_cache_hit(&key_a);
        stats.record_attempt(2, "token");
        stats.record_cache_miss(&key_b);

        assert_eq!(stats.unique_user_count(), 2);
        assert_eq!(stats.unique_token_count(), 1);
        assert_eq!(stats.unique_key_count(), 2);
        assert_eq!(stats.duplicate_key_count(), 1);
        assert_eq!(stats.cache_miss_count, 2);
        assert_eq!(stats.cache_hit_count, 1);
    }

    #[test]
    fn error_count_includes_external_cached_db_and_rebase_errors() {
        let mut stats = InventoryObservationTimingStats::default();
        stats.record_external_error();
        stats.record_cached_error();
        stats.record_db_insert_error();
        stats.record_parent_rebase_error();

        assert_eq!(stats.external_error_count, 1);
        assert_eq!(stats.cached_error_count, 1);
        assert_eq!(stats.db_insert_error_count, 1);
        assert_eq!(stats.parent_rebase_error_count, 1);
        assert_eq!(stats.error_count, 4);
    }

    #[test]
    fn unknown_can_be_slowest_phase() {
        let stats = InventoryObservationTimingStats {
            total_ms: 1_000,
            external_lookup_ms: 100,
            apply_total_ms: 100,
            initial_fill_sync_ms: 100,
            ..InventoryObservationTimingStats::default()
        };
        let slowest = stats.slowest_phase();
        assert_eq!(
            slowest.phase,
            InventoryObservationPhase::UnknownOrUnattributed.as_str()
        );
        assert_eq!(slowest.ms, 700);
    }

    #[test]
    fn latency_buckets_and_max_record_are_tracked() {
        let mut stats = InventoryObservationTimingStats::default();
        stats.record_latency(101, 1, "btc", "tok-a", 7, "phase_a");
        stats.record_latency(251, 2, "eth", "tok-b", 8, "phase_b");
        stats.record_latency(1_001, 3, "sol", "tok-c", 9, "phase_c");

        assert_eq!(stats.over_100ms_count, 3);
        assert_eq!(stats.over_250ms_count, 2);
        assert_eq!(stats.over_1000ms_count, 1);
        assert_eq!(stats.max_ms, 1_001);
        assert_eq!(stats.max_phase, Some("phase_c"));
        assert_eq!(stats.max_record_id, Some(3));
        assert_eq!(stats.max_market_slug.as_deref(), Some("sol"));
        assert_eq!(stats.max_token_id.as_deref(), Some("tok-c"));
        assert_eq!(stats.max_user_id, Some(9));
    }

    #[test]
    fn uncacheable_records_do_not_inflate_duplicate_key_count() {
        let mut stats = InventoryObservationTimingStats::default();
        let key = InventoryObservationCacheKey::new(1, "tok").unwrap();
        stats.record_attempt(1, "tok");
        stats.record_cache_miss(&key);
        stats.record_attempt(1, "");
        stats.record_uncacheable();

        assert_eq!(stats.unique_key_count(), 1);
        assert_eq!(stats.duplicate_key_count(), 0);
        assert_eq!(stats.uncacheable_count, 1);
    }

    #[test]
    fn positions_snapshot_and_token_lookup_stats_feed_external_lookup_ms() {
        let mut stats = InventoryObservationTimingStats::default();
        stats.record_positions_snapshot_record_miss();
        stats.record_positions_snapshot_fetch(100, 35, 70);
        stats.record_positions_snapshot_record_hit();
        stats.record_token_lookup(4, InventoryObservationReadResult::Visible { qty: 2.0 });
        stats.record_token_lookup(3, InventoryObservationReadResult::NotVisible);
        stats.record_fallback_available_token_qty_ms(20);

        assert_eq!(stats.positions_snapshot_record_miss_count, 1);
        assert_eq!(stats.positions_snapshot_record_hit_count, 1);
        assert_eq!(stats.positions_snapshot_fetch_count, 1);
        assert_eq!(stats.positions_snapshot_fetch_ms, 100);
        assert_eq!(stats.positions_snapshot_row_count, 35);
        assert_eq!(stats.positions_snapshot_alias_count, 70);
        assert_eq!(stats.token_lookup_count, 2);
        assert_eq!(stats.token_lookup_ms, 7);
        assert_eq!(stats.token_visible_count, 1);
        assert_eq!(stats.token_not_visible_count, 1);
        assert_eq!(stats.fallback_available_token_qty_ms, 20);
        assert_eq!(stats.external_lookup_ms, 127);
    }

    #[test]
    fn inventory_unknown_does_not_double_count_apply_nested_phases() {
        let stats = InventoryObservationTimingStats {
            total_ms: 1_000,
            external_lookup_ms: 100,
            apply_total_ms: 500,
            apply_prepare_ms: 100,
            db_observation_insert_ms: 150,
            parent_rebase_ms: 150,
            ..InventoryObservationTimingStats::default()
        };

        assert_eq!(stats.apply_unknown_ms(), 100);
        assert_eq!(stats.unknown_or_unattributed_ms(), 400);
    }

    #[test]
    fn apply_unknown_saturates_when_nested_sum_exceeds_apply_total() {
        let stats = InventoryObservationTimingStats {
            apply_total_ms: 100,
            apply_prepare_ms: 50,
            db_observation_insert_ms: 60,
            parent_rebase_ms: 70,
            ..InventoryObservationTimingStats::default()
        };

        assert_eq!(stats.apply_unknown_ms(), 0);
    }

    #[test]
    fn inventory_initial_fill_sync_phase_reduces_unknown() {
        let mut stats = InventoryObservationTimingStats {
            total_ms: 500,
            ..InventoryObservationTimingStats::default()
        };
        stats.record_initial_fill_sync(300);

        assert_eq!(stats.initial_fill_sync_ms, 300);
        assert_eq!(stats.initial_fill_sync_call_count, 1);
        assert_eq!(stats.initial_fill_sync_user_count, 1);
        assert_eq!(stats.unknown_or_unattributed_ms(), 200);
    }

    #[test]
    fn inventory_executor_lookup_phase_reduces_unknown() {
        let mut stats = InventoryObservationTimingStats {
            total_ms: 500,
            ..InventoryObservationTimingStats::default()
        };
        stats.record_executor_lookup(125);

        assert_eq!(stats.executor_lookup_count, 1);
        assert_eq!(stats.executor_lookup_ms, 125);
        assert_eq!(stats.unknown_or_unattributed_ms(), 375);
    }

    #[test]
    fn inventory_max_phase_tracks_record_dominant_phase() {
        let mut record = InventoryObservationRecordTiming::default();
        record.add_phase_ms(InventoryObservationPhase::ExecutorLookup, 40);
        record.add_phase_ms(InventoryObservationPhase::InitialFillSync, 120);

        let slowest = record.slowest_phase(200);
        assert_eq!(
            slowest.phase,
            InventoryObservationPhase::InitialFillSync.as_str()
        );
        assert_eq!(slowest.ms, 120);
    }

    #[test]
    fn inventory_max_phase_tie_break_is_deterministic() {
        let mut record = InventoryObservationRecordTiming::default();
        record.add_phase_ms(InventoryObservationPhase::ExternalLookup, 50);
        record.add_phase_ms(InventoryObservationPhase::InitialFillSync, 50);
        record.add_phase_ms(InventoryObservationPhase::ApplyTotal, 50);

        let slowest = record.slowest_phase(150);
        assert_eq!(
            slowest.phase,
            InventoryObservationPhase::ExternalLookup.as_str()
        );
    }

    #[test]
    fn positions_snapshot_errors_are_counted_separately_from_cached_errors() {
        let mut stats = InventoryObservationTimingStats::default();
        stats.record_positions_snapshot_error();
        stats.record_external_error();
        stats.record_positions_snapshot_cached_error();
        stats.record_cached_error();

        assert_eq!(stats.positions_snapshot_error_count, 1);
        assert_eq!(stats.positions_snapshot_cached_error_count, 1);
        assert_eq!(stats.external_error_count, 1);
        assert_eq!(stats.cached_error_count, 1);
        assert_eq!(stats.error_count, 2);
    }

    #[test]
    fn due_gate_snapshot_and_updates_are_recorded() {
        let mut stats = InventoryObservationTimingStats::default();
        stats.record_due_gate_snapshot(
            crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueGateSnapshot {
                due_count: 2,
                skipped_not_due_count: 3,
                backoff_active_count: 3,
                backoff_reset_count: 1,
                not_visible_streak_max: 4,
                next_due_min_ms: Some(100),
                next_due_max_ms: Some(2_000),
            },
        );
        stats.record_due_gate_update(
            crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueGateUpdate {
                backoff_active_count: 1,
                backoff_reset_count: 1,
                not_visible_streak_max: 5,
                next_due_ms: Some(500),
            },
        );
        stats.record_initial_fill_sync_skipped_no_due();

        assert_eq!(stats.due_count, 2);
        assert_eq!(stats.skipped_not_due_count, 3);
        assert_eq!(stats.backoff_active_count, 4);
        assert_eq!(stats.backoff_reset_count, 2);
        assert_eq!(stats.not_visible_streak_max, 5);
        assert_eq!(stats.next_due_min_ms, 100);
        assert_eq!(stats.next_due_max_ms, 2_000);
        assert_eq!(stats.initial_fill_sync_skipped_no_due_count, 1);
    }
}
