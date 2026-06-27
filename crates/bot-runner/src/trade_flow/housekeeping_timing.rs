use std::{future::Future, time::Duration};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum HousekeepingPhase {
    FlowCachePrune,
    ProcessTradeFlows,
    ProcessTradeBuilderOrders,
    ProcessTradeBuilderWorkflows,
    ProcessDualDcaJobs,
    UnknownOrUnattributed,
}

impl HousekeepingPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FlowCachePrune => "flow_cache_prune",
            Self::ProcessTradeFlows => "process_trade_flows",
            Self::ProcessTradeBuilderOrders => "process_trade_builder_orders",
            Self::ProcessTradeBuilderWorkflows => "process_trade_builder_workflows",
            Self::ProcessDualDcaJobs => "process_dual_dca_jobs",
            Self::UnknownOrUnattributed => "unknown_or_unattributed",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FlowHousekeepingPhase {
    LoadDefinitions,
    SyncDefinitionRuns,
    RefreshWsFastPathCache,
    EnqueueWsOpenPositionSteps,
    ProcessMarketPriceTimers,
    AutoClaim,
    ProcessReadySteps,
}

impl FlowHousekeepingPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LoadDefinitions => "flow_load_definitions",
            Self::SyncDefinitionRuns => "flow_sync_definition_runs",
            Self::RefreshWsFastPathCache => "flow_refresh_ws_fast_path_cache",
            Self::EnqueueWsOpenPositionSteps => "flow_enqueue_ws_open_position_steps",
            Self::ProcessMarketPriceTimers => "flow_process_market_price_timers",
            Self::AutoClaim => "flow_auto_claim",
            Self::ProcessReadySteps => "flow_process_ready_steps",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct HousekeepingPhaseDuration {
    pub(crate) phase: &'static str,
    pub(crate) ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FlowHousekeepingTimingStats {
    pub(crate) load_definitions_ms: u64,
    pub(crate) sync_definition_runs_ms: u64,
    pub(crate) refresh_ws_fast_path_cache_ms: u64,
    pub(crate) enqueue_ws_open_position_steps_ms: u64,
    pub(crate) process_market_price_timers_ms: u64,
    pub(crate) auto_claim_ms: u64,
    pub(crate) process_ready_steps_ms: u64,
}

impl FlowHousekeepingTimingStats {
    pub(crate) fn set_phase_ms(&mut self, phase: FlowHousekeepingPhase, ms: u64) {
        match phase {
            FlowHousekeepingPhase::LoadDefinitions => self.load_definitions_ms = ms,
            FlowHousekeepingPhase::SyncDefinitionRuns => self.sync_definition_runs_ms = ms,
            FlowHousekeepingPhase::RefreshWsFastPathCache => {
                self.refresh_ws_fast_path_cache_ms = ms
            }
            FlowHousekeepingPhase::EnqueueWsOpenPositionSteps => {
                self.enqueue_ws_open_position_steps_ms = ms
            }
            FlowHousekeepingPhase::ProcessMarketPriceTimers => {
                self.process_market_price_timers_ms = ms
            }
            FlowHousekeepingPhase::AutoClaim => self.auto_claim_ms = ms,
            FlowHousekeepingPhase::ProcessReadySteps => self.process_ready_steps_ms = ms,
        }
    }

    pub(crate) fn slowest_phase(&self) -> Option<HousekeepingPhaseDuration> {
        let candidates = [
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::LoadDefinitions.as_str(),
                ms: self.load_definitions_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::SyncDefinitionRuns.as_str(),
                ms: self.sync_definition_runs_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::RefreshWsFastPathCache.as_str(),
                ms: self.refresh_ws_fast_path_cache_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::EnqueueWsOpenPositionSteps.as_str(),
                ms: self.enqueue_ws_open_position_steps_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::ProcessMarketPriceTimers.as_str(),
                ms: self.process_market_price_timers_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::AutoClaim.as_str(),
                ms: self.auto_claim_ms,
            },
            HousekeepingPhaseDuration {
                phase: FlowHousekeepingPhase::ProcessReadySteps.as_str(),
                ms: self.process_ready_steps_ms,
            },
        ];
        candidates.into_iter().max_by_key(|candidate| candidate.ms)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HousekeepingTimingStats {
    pub(crate) housekeeping_total_ms: u64,
    pub(crate) flow_cache_prune_ms: u64,
    pub(crate) process_trade_flows_ms: u64,
    pub(crate) process_trade_builder_orders_ms: u64,
    pub(crate) process_trade_builder_workflows_ms: u64,
    pub(crate) process_dual_dca_jobs_ms: u64,
    pub(crate) flow: FlowHousekeepingTimingStats,
    pub(crate) builder_orders:
        crate::trade_builder_order_housekeeping_timing::TradeBuilderOrderHousekeepingTimingStats,
    pub(crate) retry_only_slow: bool,
    pub(crate) runnable_non_retry_ready_count: i64,
    pub(crate) clob_book_fetch_error_count: u64,
}

impl HousekeepingTimingStats {
    pub(crate) fn set_phase_ms(&mut self, phase: HousekeepingPhase, ms: u64) {
        match phase {
            HousekeepingPhase::FlowCachePrune => self.flow_cache_prune_ms = ms,
            HousekeepingPhase::ProcessTradeFlows => self.process_trade_flows_ms = ms,
            HousekeepingPhase::ProcessTradeBuilderOrders => {
                self.process_trade_builder_orders_ms = ms
            }
            HousekeepingPhase::ProcessTradeBuilderWorkflows => {
                self.process_trade_builder_workflows_ms = ms
            }
            HousekeepingPhase::ProcessDualDcaJobs => self.process_dual_dca_jobs_ms = ms,
            HousekeepingPhase::UnknownOrUnattributed => {}
        }
    }

    pub(crate) fn top_level_phase_sum_ms(&self) -> u64 {
        self.flow_cache_prune_ms
            .saturating_add(self.process_trade_flows_ms)
            .saturating_add(self.process_trade_builder_orders_ms)
            .saturating_add(self.process_trade_builder_workflows_ms)
            .saturating_add(self.process_dual_dca_jobs_ms)
    }

    pub(crate) fn unknown_or_unattributed_ms(&self) -> u64 {
        compute_unknown_or_unattributed_ms(
            self.housekeeping_total_ms,
            self.top_level_phase_sum_ms(),
        )
    }

    pub(crate) fn slowest_phase(&self) -> HousekeepingPhaseDuration {
        let candidates = [
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::FlowCachePrune.as_str(),
                ms: self.flow_cache_prune_ms,
            },
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::ProcessTradeFlows.as_str(),
                ms: self.process_trade_flows_ms,
            },
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::ProcessTradeBuilderOrders.as_str(),
                ms: self.process_trade_builder_orders_ms,
            },
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::ProcessTradeBuilderWorkflows.as_str(),
                ms: self.process_trade_builder_workflows_ms,
            },
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::ProcessDualDcaJobs.as_str(),
                ms: self.process_dual_dca_jobs_ms,
            },
            HousekeepingPhaseDuration {
                phase: HousekeepingPhase::UnknownOrUnattributed.as_str(),
                ms: self.unknown_or_unattributed_ms(),
            },
        ];
        candidates
            .into_iter()
            .max_by_key(|candidate| candidate.ms)
            .unwrap_or(HousekeepingPhaseDuration {
                phase: HousekeepingPhase::UnknownOrUnattributed.as_str(),
                ms: 0,
            })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum HousekeepingSlowClass {
    TradeFlowBacklog,
    ClobFetchSlowOrError,
    RetryOnly,
    TradeFlowNonStep,
    TradeBuilder,
    DualDca,
    UnknownNonTradeflow,
}

impl HousekeepingSlowClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TradeFlowBacklog => "trade_flow_backlog",
            Self::ClobFetchSlowOrError => "clob_fetch_slow_or_error",
            Self::RetryOnly => "retry_only",
            Self::TradeFlowNonStep => "trade_flow_non_step",
            Self::TradeBuilder => "trade_builder",
            Self::DualDca => "dual_dca",
            Self::UnknownNonTradeflow => "unknown_non_tradeflow",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HousekeepingSlowClassification {
    pub(crate) class: HousekeepingSlowClass,
    pub(crate) level: tracing::Level,
    pub(crate) slowest_phase: &'static str,
    pub(crate) slowest_phase_ms: u64,
    pub(crate) flow_slowest_phase: Option<&'static str>,
    pub(crate) flow_slowest_phase_ms: Option<u64>,
}

pub(crate) fn millis_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

pub(crate) fn compute_unknown_or_unattributed_ms(total_ms: u64, top_level_sum_ms: u64) -> u64 {
    total_ms.saturating_sub(top_level_sum_ms)
}

pub(crate) async fn measure_housekeeping_phase<F, T>(
    stats: &mut HousekeepingTimingStats,
    phase: HousekeepingPhase,
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

pub(crate) async fn measure_flow_housekeeping_phase<F, T>(
    stats: &mut FlowHousekeepingTimingStats,
    phase: FlowHousekeepingPhase,
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

pub(crate) fn classify_housekeeping_slow(
    stats: &HousekeepingTimingStats,
) -> HousekeepingSlowClassification {
    let slowest = stats.slowest_phase();
    let flow_slowest = stats.flow.slowest_phase();
    let class = if stats.runnable_non_retry_ready_count > 0 {
        HousekeepingSlowClass::TradeFlowBacklog
    } else if stats.clob_book_fetch_error_count > 0 {
        HousekeepingSlowClass::ClobFetchSlowOrError
    } else if stats.retry_only_slow
        && stats.runnable_non_retry_ready_count == 0
        && stats.clob_book_fetch_error_count == 0
    {
        HousekeepingSlowClass::RetryOnly
    } else if slowest.phase == HousekeepingPhase::ProcessTradeFlows.as_str() {
        HousekeepingSlowClass::TradeFlowNonStep
    } else if slowest.phase == HousekeepingPhase::ProcessTradeBuilderOrders.as_str()
        || slowest.phase == HousekeepingPhase::ProcessTradeBuilderWorkflows.as_str()
    {
        HousekeepingSlowClass::TradeBuilder
    } else if slowest.phase == HousekeepingPhase::ProcessDualDcaJobs.as_str() {
        HousekeepingSlowClass::DualDca
    } else {
        HousekeepingSlowClass::UnknownNonTradeflow
    };
    let level = if class == HousekeepingSlowClass::RetryOnly {
        tracing::Level::INFO
    } else {
        tracing::Level::WARN
    };
    HousekeepingSlowClassification {
        class,
        level,
        slowest_phase: slowest.phase,
        slowest_phase_ms: slowest.ms,
        flow_slowest_phase: flow_slowest.map(|phase| phase.phase),
        flow_slowest_phase_ms: flow_slowest.map(|phase| phase.ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retry_only_stats() -> HousekeepingTimingStats {
        HousekeepingTimingStats {
            housekeeping_total_ms: 1_000,
            process_trade_flows_ms: 900,
            retry_only_slow: true,
            runnable_non_retry_ready_count: 0,
            clob_book_fetch_error_count: 0,
            ..HousekeepingTimingStats::default()
        }
    }

    #[test]
    fn classification_prioritizes_runnable_backlog_over_retry_only() {
        let mut stats = retry_only_stats();
        stats.runnable_non_retry_ready_count = 2;
        let classification = classify_housekeeping_slow(&stats);
        assert_eq!(
            classification.class,
            HousekeepingSlowClass::TradeFlowBacklog
        );
        assert_eq!(classification.level, tracing::Level::WARN);
    }

    #[test]
    fn classification_prioritizes_clob_error_over_retry_only() {
        let mut stats = retry_only_stats();
        stats.clob_book_fetch_error_count = 1;
        let classification = classify_housekeeping_slow(&stats);
        assert_eq!(
            classification.class,
            HousekeepingSlowClass::ClobFetchSlowOrError
        );
        assert_eq!(classification.level, tracing::Level::WARN);
    }

    #[test]
    fn retry_only_without_backlog_or_clob_error_is_info() {
        let classification = classify_housekeeping_slow(&retry_only_stats());
        assert_eq!(classification.class, HousekeepingSlowClass::RetryOnly);
        assert_eq!(classification.level, tracing::Level::INFO);
    }

    #[test]
    fn nested_flow_timings_are_not_included_in_unknown_top_level_sum() {
        let stats = HousekeepingTimingStats {
            housekeeping_total_ms: 1_000,
            process_trade_flows_ms: 100,
            flow: FlowHousekeepingTimingStats {
                load_definitions_ms: 50,
                process_ready_steps_ms: 50,
                ..FlowHousekeepingTimingStats::default()
            },
            ..HousekeepingTimingStats::default()
        };
        assert_eq!(stats.unknown_or_unattributed_ms(), 900);
    }

    #[test]
    fn unknown_or_unattributed_can_be_slowest_phase() {
        let stats = HousekeepingTimingStats {
            housekeeping_total_ms: 5_000,
            process_trade_flows_ms: 50,
            process_trade_builder_orders_ms: 20,
            process_trade_builder_workflows_ms: 10,
            process_dual_dca_jobs_ms: 5,
            ..HousekeepingTimingStats::default()
        };
        let classification = classify_housekeeping_slow(&stats);
        assert_eq!(classification.slowest_phase, "unknown_or_unattributed");
        assert_eq!(classification.slowest_phase_ms, 4_915);
        assert_eq!(
            classification.class,
            HousekeepingSlowClass::UnknownNonTradeflow
        );
    }

    #[test]
    fn flow_slowest_phase_is_reported_when_process_trade_flows_is_slowest() {
        let stats = HousekeepingTimingStats {
            housekeeping_total_ms: 1_000,
            process_trade_flows_ms: 800,
            flow: FlowHousekeepingTimingStats {
                refresh_ws_fast_path_cache_ms: 700,
                process_ready_steps_ms: 10,
                ..FlowHousekeepingTimingStats::default()
            },
            ..HousekeepingTimingStats::default()
        };
        let classification = classify_housekeeping_slow(&stats);
        assert_eq!(classification.slowest_phase, "process_trade_flows");
        assert_eq!(
            classification.flow_slowest_phase,
            Some("flow_refresh_ws_fast_path_cache")
        );
        assert_eq!(classification.flow_slowest_phase_ms, Some(700));
    }

    #[tokio::test]
    async fn early_error_phase_still_records_elapsed_ms() {
        let mut stats = HousekeepingTimingStats::default();
        let result: Result<(), &'static str> = measure_housekeeping_phase(
            &mut stats,
            HousekeepingPhase::ProcessTradeBuilderOrders,
            async { Err("boom") },
        )
        .await;
        assert_eq!(result, Err("boom"));
        assert!(stats.process_trade_builder_orders_ms <= 1);
    }

    #[test]
    fn millis_u64_saturates_and_allows_sub_ms_zero() {
        assert_eq!(millis_u64(Duration::from_nanos(1)), 0);
        assert_eq!(millis_u64(Duration::from_millis(42)), 42);
    }
}
