fn emit_flow_housekeeping_slow_log(
    run_id: i64,
    loop_count: u64,
    housekeeping_elapsed_ms: u64,
    housekeeping_timing: &crate::trade_flow::housekeeping_timing::HousekeepingTimingStats,
    step_stats: &FlowStepProcessingStats,
) {
    let classification =
        crate::trade_flow::housekeeping_timing::classify_housekeeping_slow(housekeeping_timing);
    let housekeeping_slow_class = classification.class.as_str();
    let flow_slowest_phase = classification.flow_slowest_phase.unwrap_or("none");
    let flow_slowest_phase_ms = classification.flow_slowest_phase_ms.unwrap_or(0);
    let builder_orders_slowest = housekeeping_timing.builder_orders.slowest_phase();
    let builder_orders_eval_max_order_id = housekeeping_timing
        .builder_orders
        .eval_max_order_id
        .unwrap_or(0);
    let builder_orders_eval_max_market_slug = housekeeping_timing
        .builder_orders
        .eval_max_market_slug
        .as_deref()
        .unwrap_or("none");
    let builder_orders_eval_max_status = housekeeping_timing
        .builder_orders
        .eval_max_status
        .as_deref()
        .unwrap_or("none");
    let inventory = &housekeeping_timing.builder_orders.inventory_observation;
    let inventory_slowest = inventory.slowest_phase();
    let inventory_max_record_id = inventory.max_record_id.unwrap_or(0);
    let inventory_max_market_slug = inventory.max_market_slug.as_deref().unwrap_or("none");
    let inventory_max_token_id = inventory.max_token_id.as_deref().unwrap_or("none");
    let inventory_max_user_id = inventory.max_user_id.unwrap_or(0);
    let inventory_max_phase = inventory.max_phase.unwrap_or("none");
    let inventory_initial_fill_sync = &inventory.initial_fill_sync_detail;
    let final_fill_sync = &housekeeping_timing.builder_orders.final_fill_sync;
    let final_fill_sync_max_user_id = final_fill_sync.max_user_id.unwrap_or(0);
    macro_rules! emit_flow_housekeeping_slow {
        ($log:ident) => {
            $log!(
                run_id,
                loop_count,
                housekeeping_elapsed_ms,
                housekeeping_total_ms = housekeeping_timing.housekeeping_total_ms,
                housekeeping_slow_class,
                slowest_phase = classification.slowest_phase,
                slowest_phase_ms = classification.slowest_phase_ms,
                unknown_or_unattributed_ms = housekeeping_timing.unknown_or_unattributed_ms(),
                flow_cache_prune_ms = housekeeping_timing.flow_cache_prune_ms,
                process_trade_flows_ms = housekeeping_timing.process_trade_flows_ms,
                process_trade_builder_orders_ms =
                    housekeeping_timing.process_trade_builder_orders_ms,
                builder_orders_total_ms = housekeeping_timing.builder_orders.total_ms,
                builder_orders_load_orders_ms = housekeeping_timing.builder_orders.load_orders_ms,
                builder_orders_load_pending_inventory_ms =
                    housekeeping_timing.builder_orders.load_pending_inventory_ms,
                builder_orders_process_loop_ms =
                    housekeeping_timing.builder_orders.process_loop_ms,
                builder_orders_inventory_observation_loop_ms =
                    housekeeping_timing
                        .builder_orders
                        .inventory_observation_loop_ms,
                builder_orders_final_fill_sync_ms =
                    housekeeping_timing.builder_orders.final_fill_sync_ms,
                builder_orders_refresh_armed_cache_ms =
                    housekeeping_timing.builder_orders.refresh_armed_cache_ms,
                builder_orders_refresh_guarded_buy_cache_ms =
                    housekeeping_timing.builder_orders.refresh_guarded_buy_cache_ms,
                builder_orders_market_stream_union_ms =
                    housekeeping_timing.builder_orders.market_stream_union_ms,
                builder_orders_auto_scope_backfill_ms =
                    housekeeping_timing.builder_orders.auto_scope_backfill_ms,
                builder_orders_unknown_or_unattributed_ms =
                    housekeeping_timing.builder_orders.unknown_or_unattributed_ms(),
                builder_orders_slowest_phase = builder_orders_slowest.phase,
                builder_orders_slowest_phase_ms = builder_orders_slowest.ms,
                builder_orders_loaded_count = housekeeping_timing.builder_orders.loaded_count,
                builder_orders_pending_inventory_count =
                    housekeeping_timing.builder_orders.pending_inventory_count,
                builder_orders_processed_count = housekeeping_timing.builder_orders.processed_count,
                builder_orders_processing_error_count =
                    housekeeping_timing.builder_orders.processing_error_count,
                builder_orders_inventory_observed_count =
                    housekeeping_timing.builder_orders.inventory_observed_count,
                builder_orders_inventory_error_count =
                    housekeeping_timing.builder_orders.inventory_error_count,
                builder_orders_fill_sync_user_count =
                    housekeeping_timing.builder_orders.fill_sync_user_count,
                builder_orders_fill_sync_call_count =
                    housekeeping_timing.builder_orders.fill_sync_call_count,
                builder_orders_fill_sync_error_count =
                    housekeeping_timing.builder_orders.fill_sync_error_count,
                builder_orders_armed_cache_count =
                    housekeeping_timing.builder_orders.armed_cache_count,
                builder_orders_guarded_buy_cache_count =
                    housekeeping_timing.builder_orders.guarded_buy_cache_count,
                builder_orders_auto_scope_backfill_error_count =
                    housekeeping_timing
                        .builder_orders
                        .auto_scope_backfill_error_count,
                builder_orders_eval_max_ms = housekeeping_timing.builder_orders.eval_max_ms,
                builder_orders_eval_max_order_id,
                builder_orders_eval_max_market_slug,
                builder_orders_eval_max_status,
                builder_inventory_observation_total_ms = inventory.total_ms,
                builder_inventory_observation_attempted_count = inventory.attempted_count,
                builder_inventory_observation_due_count = inventory.due_count,
                builder_inventory_observation_skipped_not_due_count =
                    inventory.skipped_not_due_count,
                builder_inventory_observation_backoff_active_count =
                    inventory.backoff_active_count,
                builder_inventory_observation_backoff_reset_count =
                    inventory.backoff_reset_count,
                builder_inventory_observation_not_visible_streak_max =
                    inventory.not_visible_streak_max,
                builder_inventory_observation_next_due_min_ms =
                    inventory.next_due_min_ms,
                builder_inventory_observation_next_due_max_ms =
                    inventory.next_due_max_ms,
                builder_inventory_initial_fill_sync_skipped_no_due_count =
                    inventory.initial_fill_sync_skipped_no_due_count,
                builder_inventory_observation_success_count = inventory.success_count,
                builder_inventory_observation_not_visible_count = inventory.not_visible_count,
                builder_inventory_observation_error_count = inventory.error_count,
                builder_inventory_observation_external_error_count =
                    inventory.external_error_count,
                builder_inventory_observation_cached_error_count = inventory.cached_error_count,
                builder_inventory_observation_db_insert_error_count =
                    inventory.db_insert_error_count,
                builder_inventory_observation_parent_rebase_error_count =
                    inventory.parent_rebase_error_count,
                builder_inventory_observation_cache_hit_count = inventory.cache_hit_count,
                builder_inventory_observation_cache_miss_count = inventory.cache_miss_count,
                builder_inventory_observation_unique_user_count =
                    inventory.unique_user_count(),
                builder_inventory_observation_unique_token_count =
                    inventory.unique_token_count(),
                builder_inventory_observation_unique_key_count =
                    inventory.unique_key_count(),
                builder_inventory_observation_duplicate_key_count =
                    inventory.duplicate_key_count(),
                builder_inventory_observation_uncacheable_count = inventory.uncacheable_count,
                builder_inventory_positions_snapshot_record_hit_count =
                    inventory.positions_snapshot_record_hit_count,
                builder_inventory_positions_snapshot_record_miss_count =
                    inventory.positions_snapshot_record_miss_count,
                builder_inventory_positions_snapshot_fetch_count =
                    inventory.positions_snapshot_fetch_count,
                builder_inventory_positions_snapshot_fetch_ms =
                    inventory.positions_snapshot_fetch_ms,
                builder_inventory_positions_snapshot_error_count =
                    inventory.positions_snapshot_error_count,
                builder_inventory_positions_snapshot_cached_error_count =
                    inventory.positions_snapshot_cached_error_count,
                builder_inventory_positions_snapshot_unsupported_count =
                    inventory.positions_snapshot_unsupported_count,
                builder_inventory_positions_snapshot_row_count =
                    inventory.positions_snapshot_row_count,
                builder_inventory_positions_snapshot_alias_count =
                    inventory.positions_snapshot_alias_count,
                builder_inventory_token_lookup_count = inventory.token_lookup_count,
                builder_inventory_token_lookup_ms = inventory.token_lookup_ms,
                builder_inventory_token_visible_count = inventory.token_visible_count,
                builder_inventory_token_not_visible_count = inventory.token_not_visible_count,
                builder_inventory_observation_fallback_available_token_qty_ms =
                    inventory.fallback_available_token_qty_ms,
                builder_inventory_observation_external_lookup_ms =
                    inventory.external_lookup_ms,
                builder_inventory_observation_config_lookup_ms =
                    inventory.config_lookup_ms,
                builder_inventory_observation_config_lookup_count =
                    inventory.config_lookup_count,
                builder_inventory_observation_executor_lookup_ms =
                    inventory.executor_lookup_ms,
                builder_inventory_observation_executor_lookup_count =
                    inventory.executor_lookup_count,
                builder_inventory_observation_initial_fill_sync_ms =
                    inventory.initial_fill_sync_ms,
                builder_inventory_observation_initial_fill_sync_call_count =
                    inventory.initial_fill_sync_call_count,
                builder_inventory_observation_initial_fill_sync_user_count =
                    inventory.initial_fill_sync_user_count,
                builder_inventory_initial_fill_sync_fetch_page_ms =
                    inventory_initial_fill_sync.fetch_page_ms,
                builder_inventory_initial_fill_sync_page_apply_ms =
                    inventory_initial_fill_sync.page_apply_ms,
                builder_inventory_initial_fill_sync_db_order_lookup_ms =
                    inventory_initial_fill_sync.db_order_lookup_ms,
                builder_inventory_initial_fill_sync_db_upsert_ms =
                    inventory_initial_fill_sync.db_upsert_ms,
                builder_inventory_initial_fill_sync_raw_count =
                    inventory_initial_fill_sync.raw_count,
                builder_inventory_initial_fill_sync_synced_count =
                    inventory_initial_fill_sync.synced_count,
                builder_inventory_observation_snapshot_cache_ms =
                    inventory.snapshot_cache_ms,
                builder_inventory_observation_token_result_cache_ms =
                    inventory.token_result_cache_ms,
                builder_inventory_observation_apply_total_ms =
                    inventory.apply_total_ms,
                builder_inventory_observation_apply_prepare_ms =
                    inventory.apply_prepare_ms,
                builder_inventory_observation_apply_unknown_ms =
                    inventory.apply_unknown_ms(),
                builder_inventory_observation_record_finalize_ms =
                    inventory.record_finalize_ms,
                builder_inventory_observation_db_observation_insert_ms =
                    inventory.db_observation_insert_ms,
                builder_inventory_observation_parent_rebase_ms = inventory.parent_rebase_ms,
                builder_inventory_observation_unknown_ms =
                    inventory.unknown_or_unattributed_ms(),
                builder_inventory_observation_slowest_phase = inventory_slowest.phase,
                builder_inventory_observation_slowest_phase_ms = inventory_slowest.ms,
                builder_inventory_observation_max_ms = inventory.max_ms,
                builder_inventory_observation_max_record_id = inventory_max_record_id,
                builder_inventory_observation_max_market_slug = inventory_max_market_slug,
                builder_inventory_observation_max_token_id = inventory_max_token_id,
                builder_inventory_observation_max_user_id = inventory_max_user_id,
                builder_inventory_observation_max_phase = inventory_max_phase,
                builder_inventory_observation_over_100ms_count = inventory.over_100ms_count,
                builder_inventory_observation_over_250ms_count = inventory.over_250ms_count,
                builder_inventory_observation_over_1000ms_count = inventory.over_1000ms_count,
                builder_final_fill_sync_total_ms = final_fill_sync.total_ms,
                builder_final_fill_sync_fetch_page_ms = final_fill_sync.fetch_page_ms,
                builder_final_fill_sync_page_apply_ms = final_fill_sync.page_apply_ms,
                builder_final_fill_sync_db_order_lookup_ms =
                    final_fill_sync.db_order_lookup_ms,
                builder_final_fill_sync_db_upsert_ms = final_fill_sync.db_upsert_ms,
                builder_final_fill_sync_pages_scanned = final_fill_sync.pages_scanned,
                builder_final_fill_sync_raw_count = final_fill_sync.raw_count,
                builder_final_fill_sync_synced_count = final_fill_sync.synced_count,
                builder_final_fill_sync_call_count = final_fill_sync.call_count,
                builder_final_fill_sync_user_count = final_fill_sync.user_count(),
                builder_final_fill_sync_success_count = final_fill_sync.success_count,
                builder_final_fill_sync_error_count = final_fill_sync.error_count,
                builder_final_fill_sync_skipped_fresh_count =
                    final_fill_sync.skipped_fresh_count,
                builder_final_fill_sync_required_count = final_fill_sync.required_count,
                builder_final_fill_sync_max_user_ms = final_fill_sync.max_user_ms,
                builder_final_fill_sync_max_user_id = final_fill_sync_max_user_id,
                builder_final_fill_sync_unknown_ms = final_fill_sync.unknown_ms(),
                process_trade_builder_workflows_ms =
                    housekeeping_timing.process_trade_builder_workflows_ms,
                process_dual_dca_jobs_ms = housekeeping_timing.process_dual_dca_jobs_ms,
                flow_slowest_phase,
                flow_slowest_phase_ms,
                flow_load_definitions_ms = housekeeping_timing.flow.load_definitions_ms,
                flow_sync_definition_runs_ms = housekeeping_timing.flow.sync_definition_runs_ms,
                flow_refresh_ws_fast_path_cache_ms =
                    housekeeping_timing.flow.refresh_ws_fast_path_cache_ms,
                flow_enqueue_ws_open_position_steps_ms =
                    housekeeping_timing.flow.enqueue_ws_open_position_steps_ms,
                flow_process_market_price_timers_ms =
                    housekeeping_timing.flow.process_market_price_timers_ms,
                flow_auto_claim_ms = housekeeping_timing.flow.auto_claim_ms,
                flow_process_ready_steps_ms = housekeeping_timing.flow.process_ready_steps_ms,
                retry_only_slow = housekeeping_timing.retry_only_slow,
                step_processing_run_id = %step_stats.processing_run_id,
                claimed_step_count = step_stats.claimed_step_count,
                ptb_retry_blocked_count = step_stats.ptb_retry_blocked_count,
                ptb_retry_created_count = step_stats.ptb_retry_created_count,
                ptb_retry_same_run_excluded_count = step_stats.ptb_retry_same_run_excluded_count,
                runnable_non_retry_ready_count = step_stats.runnable_non_retry_ready_count,
                clob_book_fetch_hit_count = step_stats.clob_book_fetch_hit_count,
                clob_book_fetch_pass_cache_hit_count =
                    step_stats.clob_book_fetch_pass_cache_hit_count,
                clob_book_fetch_process_ttl_hit_count =
                    step_stats.clob_book_fetch_process_ttl_hit_count,
                clob_book_fetch_miss_count = step_stats.clob_book_fetch_miss_count,
                clob_book_fetch_error_count = step_stats.clob_book_fetch_error_count,
                unique_book_tokens_fetched = step_stats.unique_book_tokens_fetched,
                coalesced_event_suppressed_count = step_stats.coalesced_event_suppressed_count,
                "FLOW_HOUSEKEEPING_SLOW"
            );
        };
    }
    if classification.level == tracing::Level::INFO {
        emit_flow_housekeeping_slow!(info);
    } else {
        emit_flow_housekeeping_slow!(warn);
    }
}
