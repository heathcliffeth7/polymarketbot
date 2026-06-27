use std::{future::Future, time::Duration};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TradeBuilderOrdersPhase {
    LoadOrders,
    LoadPendingInventory,
    ProcessLoop,
    InventoryObservationLoop,
    FinalFillSync,
    RefreshArmedCache,
    RefreshGuardedBuyCache,
    MarketStreamUnion,
    AutoScopeBackfill,
    UnknownOrUnattributed,
}

impl TradeBuilderOrdersPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LoadOrders => "builder_orders_load_orders",
            Self::LoadPendingInventory => "builder_orders_load_pending_inventory",
            Self::ProcessLoop => "builder_orders_process_loop",
            Self::InventoryObservationLoop => "builder_orders_inventory_observation_loop",
            Self::FinalFillSync => "builder_orders_final_fill_sync",
            Self::RefreshArmedCache => "builder_orders_refresh_armed_cache",
            Self::RefreshGuardedBuyCache => "builder_orders_refresh_guarded_buy_cache",
            Self::MarketStreamUnion => "builder_orders_market_stream_union",
            Self::AutoScopeBackfill => "builder_orders_auto_scope_backfill",
            Self::UnknownOrUnattributed => "builder_orders_unknown_or_unattributed",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TradeBuilderOrdersPhaseDuration {
    pub(crate) phase: &'static str,
    pub(crate) ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TradeBuilderOrderHousekeepingTimingStats {
    pub(crate) total_ms: u64,
    pub(crate) load_orders_ms: u64,
    pub(crate) load_pending_inventory_ms: u64,
    pub(crate) process_loop_ms: u64,
    pub(crate) inventory_observation_loop_ms: u64,
    pub(crate) final_fill_sync_ms: u64,
    pub(crate) refresh_armed_cache_ms: u64,
    pub(crate) refresh_guarded_buy_cache_ms: u64,
    pub(crate) market_stream_union_ms: u64,
    pub(crate) auto_scope_backfill_ms: u64,
    pub(crate) loaded_count: u64,
    pub(crate) pending_inventory_count: u64,
    pub(crate) processed_count: u64,
    pub(crate) processing_error_count: u64,
    pub(crate) inventory_observed_count: u64,
    pub(crate) inventory_error_count: u64,
    pub(crate) fill_sync_user_count: u64,
    pub(crate) fill_sync_call_count: u64,
    pub(crate) fill_sync_error_count: u64,
    pub(crate) armed_cache_count: u64,
    pub(crate) guarded_buy_cache_count: u64,
    pub(crate) auto_scope_backfill_error_count: u64,
    pub(crate) eval_max_ms: u64,
    pub(crate) eval_max_order_id: Option<i64>,
    pub(crate) eval_max_market_slug: Option<String>,
    pub(crate) eval_max_status: Option<String>,
    pub(crate) inventory_observation:
        crate::trade_builder_inventory_observation_timing::InventoryObservationTimingStats,
    pub(crate) final_fill_sync: crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats,
}

impl TradeBuilderOrderHousekeepingTimingStats {
    pub(crate) fn set_phase_ms(&mut self, phase: TradeBuilderOrdersPhase, ms: u64) {
        match phase {
            TradeBuilderOrdersPhase::LoadOrders => self.load_orders_ms = ms,
            TradeBuilderOrdersPhase::LoadPendingInventory => self.load_pending_inventory_ms = ms,
            TradeBuilderOrdersPhase::ProcessLoop => self.process_loop_ms = ms,
            TradeBuilderOrdersPhase::InventoryObservationLoop => {
                self.inventory_observation_loop_ms = ms
            }
            TradeBuilderOrdersPhase::FinalFillSync => self.final_fill_sync_ms = ms,
            TradeBuilderOrdersPhase::RefreshArmedCache => self.refresh_armed_cache_ms = ms,
            TradeBuilderOrdersPhase::RefreshGuardedBuyCache => {
                self.refresh_guarded_buy_cache_ms = ms
            }
            TradeBuilderOrdersPhase::MarketStreamUnion => self.market_stream_union_ms = ms,
            TradeBuilderOrdersPhase::AutoScopeBackfill => self.auto_scope_backfill_ms = ms,
            TradeBuilderOrdersPhase::UnknownOrUnattributed => {}
        }
    }

    pub(crate) fn phase_sum_ms(&self) -> u64 {
        self.load_orders_ms
            .saturating_add(self.load_pending_inventory_ms)
            .saturating_add(self.process_loop_ms)
            .saturating_add(self.inventory_observation_loop_ms)
            .saturating_add(self.final_fill_sync_ms)
            .saturating_add(self.refresh_armed_cache_ms)
            .saturating_add(self.refresh_guarded_buy_cache_ms)
            .saturating_add(self.market_stream_union_ms)
            .saturating_add(self.auto_scope_backfill_ms)
    }

    pub(crate) fn unknown_or_unattributed_ms(&self) -> u64 {
        compute_builder_orders_unknown_or_unattributed_ms(self.total_ms, self.phase_sum_ms())
    }

    pub(crate) fn slowest_phase(&self) -> TradeBuilderOrdersPhaseDuration {
        let candidates = [
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::LoadOrders.as_str(),
                ms: self.load_orders_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::LoadPendingInventory.as_str(),
                ms: self.load_pending_inventory_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::ProcessLoop.as_str(),
                ms: self.process_loop_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::InventoryObservationLoop.as_str(),
                ms: self.inventory_observation_loop_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::FinalFillSync.as_str(),
                ms: self.final_fill_sync_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::RefreshArmedCache.as_str(),
                ms: self.refresh_armed_cache_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::RefreshGuardedBuyCache.as_str(),
                ms: self.refresh_guarded_buy_cache_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::MarketStreamUnion.as_str(),
                ms: self.market_stream_union_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::AutoScopeBackfill.as_str(),
                ms: self.auto_scope_backfill_ms,
            },
            TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::UnknownOrUnattributed.as_str(),
                ms: self.unknown_or_unattributed_ms(),
            },
        ];
        candidates
            .into_iter()
            .max_by_key(|candidate| candidate.ms)
            .unwrap_or(TradeBuilderOrdersPhaseDuration {
                phase: TradeBuilderOrdersPhase::UnknownOrUnattributed.as_str(),
                ms: 0,
            })
    }

    pub(crate) fn record_order_eval(
        &mut self,
        elapsed_ms: u64,
        order_id: i64,
        market_slug: &str,
        status: &str,
    ) {
        if elapsed_ms >= self.eval_max_ms {
            self.eval_max_ms = elapsed_ms;
            self.eval_max_order_id = Some(order_id);
            self.eval_max_market_slug = Some(market_slug.to_string());
            self.eval_max_status = Some(status.to_string());
        }
    }
}

pub(crate) fn millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

pub(crate) fn compute_builder_orders_unknown_or_unattributed_ms(
    total_ms: u64,
    phase_sum_ms: u64,
) -> u64 {
    total_ms.saturating_sub(phase_sum_ms)
}

pub(crate) async fn measure_trade_builder_orders_phase<F, T>(
    stats: &mut TradeBuilderOrderHousekeepingTimingStats,
    phase: TradeBuilderOrdersPhase,
    future: F,
) -> T
where
    F: Future<Output = T>,
{
    let started = std::time::Instant::now();
    let result = future.await;
    stats.set_phase_ms(phase, millis_u64(started.elapsed()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_orders_slowest_phase_is_selected() {
        let stats = TradeBuilderOrderHousekeepingTimingStats {
            total_ms: 1_000,
            process_loop_ms: 250,
            refresh_guarded_buy_cache_ms: 300,
            ..TradeBuilderOrderHousekeepingTimingStats::default()
        };
        let slowest = stats.slowest_phase();
        assert_eq!(slowest.phase, "builder_orders_unknown_or_unattributed");
        assert_eq!(slowest.ms, 450);

        let stats = TradeBuilderOrderHousekeepingTimingStats {
            total_ms: 1_000,
            process_loop_ms: 500,
            refresh_guarded_buy_cache_ms: 300,
            ..TradeBuilderOrderHousekeepingTimingStats::default()
        };
        let slowest = stats.slowest_phase();
        assert_eq!(slowest.phase, "builder_orders_process_loop");
        assert_eq!(slowest.ms, 500);
    }

    #[test]
    fn builder_orders_unknown_or_unattributed_saturates() {
        assert_eq!(
            compute_builder_orders_unknown_or_unattributed_ms(100, 120),
            0
        );
        assert_eq!(
            compute_builder_orders_unknown_or_unattributed_ms(120, 100),
            20
        );
    }

    #[test]
    fn builder_orders_eval_max_tracks_slowest_order() {
        let mut stats = TradeBuilderOrderHousekeepingTimingStats::default();
        stats.record_order_eval(10, 1, "btc-up-down", "triggered");
        stats.record_order_eval(42, 2, "eth-up-down", "armed");
        stats.record_order_eval(5, 3, "sol-up-down", "open");

        assert_eq!(stats.eval_max_ms, 42);
        assert_eq!(stats.eval_max_order_id, Some(2));
        assert_eq!(stats.eval_max_market_slug.as_deref(), Some("eth-up-down"));
        assert_eq!(stats.eval_max_status.as_deref(), Some("armed"));
    }

    #[test]
    fn builder_orders_nested_timings_do_not_affect_housekeeping_unknown() {
        let stats = TradeBuilderOrderHousekeepingTimingStats {
            total_ms: 1_000,
            process_loop_ms: 400,
            final_fill_sync_ms: 100,
            eval_max_ms: 700,
            ..TradeBuilderOrderHousekeepingTimingStats::default()
        };

        assert_eq!(stats.unknown_or_unattributed_ms(), 500);
    }

    #[test]
    fn builder_orders_class_defaults_unknown_when_no_phase_dominates() {
        let stats = TradeBuilderOrderHousekeepingTimingStats {
            total_ms: 900,
            load_orders_ms: 1,
            ..TradeBuilderOrderHousekeepingTimingStats::default()
        };
        let slowest = stats.slowest_phase();
        assert_eq!(slowest.phase, "builder_orders_unknown_or_unattributed");
        assert_eq!(slowest.ms, 899);
    }
}
