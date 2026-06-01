#[cfg(test)]
mod pre_buy_submit_revalidation_semantics_tests {
    use super::*;

    fn submit_revalidation_decision(
        passed: bool,
        terminal: bool,
        reason_code: &'static str,
        payload: Value,
    ) -> LiveGapSubmitRevalidationDecision {
        LiveGapSubmitRevalidationDecision {
            passed,
            terminal,
            reason_code,
            payload,
        }
    }

    fn base_payload() -> Value {
        json!({
            "market_slug": "btc-updown-5m-1777917600",
            "token_id": "tok-up",
            "outcome_label": "Up",
            "effective_fill_price": 0.884,
            "remaining_sec": 24,
            "live_gap_usd": 61.7,
            "required_gap_usd": 24.0,
            "current_price_ts_ms": 1_000
        })
    }

    fn local_path_guard_payload() -> Value {
        json!({
            "profile_source": "missing",
            "profile_lookup_status": "row_missing",
            "fallback_level": "gap_relaxed",
            "profile_lookup_fallback_level": "gap_relaxed",
            "runtime_fallback_source": "local_2m_path",
            "local_path_fallback_source": "local_2m_path",
            "protection": "local_path_applied",
            "ptb_floor_usd": 13.0,
            "reason_code": "local_path_drop_too_high",
            "decision": "block_retry",
            "local_path_history_ms": 42_000,
            "local_path_sample_count": 170,
            "largest_sample_gap_ms": 496,
            "local_path_min_gap_30s": 44.7,
            "local_path_min_gap_60s": 40.1,
            "local_path_min_gap_2m": 40.1,
            "local_path_drop_10s": 6.0,
            "local_path_drop_30s": 13.2,
            "local_path_drop_60s": 13.2,
            "local_path_slope_3s": -0.7,
            "local_path_slope_10s": -0.3,
            "local_path_slope_30s": -0.1,
            "local_path_decision_reason": "local_path_drop_too_high",
            "profile_lookup_key": {
                "target_market_slug": "btc-updown-5m-1777917600",
                "target_window_start": "2026-05-06T12:00:00Z",
                "definition_id": 4320,
                "node_key": "action_0rt6iz",
                "profile_config_hash": "abcdef1234567890",
                "asset": "btc",
                "direction": "down",
                "remaining_bucket": "late",
                "price_bucket": "very_high",
                "gap_bucket": "medium",
                "slope_bucket": "negative",
                "quantile": 0.98,
                "high_late": true
            }
        })
    }

    fn annotate(
        decision: &mut LiveGapSubmitRevalidationDecision,
        candidate_stale: bool,
        floor_invalidated: bool,
    ) {
        annotate_live_gap_submit_revalidation_payload(
            decision,
            &json!({}),
            812,
            1_188,
            500,
            candidate_stale,
            floor_invalidated,
            2_000,
        );
    }

    fn test_order() -> TradeBuilderOrder {
        let now = Utc::now();
        TradeBuilderOrder {
            id: 1,
            trade_id: 108_194,
            user_id: 1,
            kind: "immediate".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1777917600".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "buy".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: None,
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: now,
            updated_at: now,
            parent_order_id: None,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            ptb_current_price_source: "chainlink".to_string(),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn candidate_stale_fresh_pass_allows_clob_submit() {
        let mut decision =
            submit_revalidation_decision(true, false, "retrace_stabilized", base_payload());
        annotate(&mut decision, true, false);

        assert_eq!(
            decision.payload["fresh_revalidation_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS
        );
        assert_eq!(
            decision.payload["decision_reason"],
            LIVE_GAP_SUBMIT_REVALIDATION_DECISION_REASON_PASS
        );
        assert_eq!(
            decision.payload["revalidation_trigger"],
            LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_CANDIDATE_STALE
        );
        assert_eq!(
            decision.payload["candidate_reuse_decision"],
            "reuse_denied_revalidation_required"
        );
        assert_eq!(
            decision.payload["candidate_reuse"],
            "denied, revalidation_required"
        );
        assert_eq!(decision.payload["candidate_created_at_ms"], 812);
        assert_eq!(decision.payload["fresh_snapshot_age_ms"], 1_000);
        assert_eq!(decision.payload["fresh_revalidation_ts_ms"], 2_000);
        assert_eq!(
            decision.payload["clob_submit_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_CLOB_ALLOWED
        );
    }

    #[test]
    fn candidate_stale_fresh_block_uses_real_guard_reason() {
        let mut decision = submit_revalidation_decision(
            false,
            false,
            "effective_fill_above_hard_max",
            base_payload(),
        );
        annotate(&mut decision, true, false);

        assert_eq!(
            decision.payload["fresh_revalidation_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_RETRY
        );
        assert_eq!(
            decision.payload["decision_reason"],
            "effective_fill_above_hard_max"
        );
        assert_eq!(
            decision.payload["revalidation_trigger"],
            LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_CANDIDATE_STALE
        );
        assert_eq!(
            decision.payload["clob_submit_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_CLOB_NOT_SUBMITTED
        );
    }

    #[test]
    fn floor_breach_fresh_pass_allows_clob_submit() {
        let mut decision =
            submit_revalidation_decision(true, false, "retrace_stabilized", base_payload());
        annotate(&mut decision, false, true);

        assert_eq!(
            decision.payload["fresh_revalidation_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS
        );
        assert_eq!(
            decision.payload["revalidation_trigger"],
            LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_FLOOR_BREACH
        );
        assert_eq!(
            decision.payload["candidate_reuse_decision"],
            "reuse_denied_floor_breach"
        );
        assert_eq!(
            decision.payload["clob_submit_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_CLOB_ALLOWED
        );
    }

    #[test]
    fn floor_breach_fresh_block_uses_real_guard_reason() {
        let mut decision =
            submit_revalidation_decision(false, false, "live_gap_below_required", base_payload());
        annotate(&mut decision, false, true);

        assert_eq!(
            decision.payload["fresh_revalidation_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_RETRY
        );
        assert_eq!(decision.payload["decision_reason"], "live_gap_below_required");
        assert_eq!(
            decision.payload["revalidation_trigger"],
            LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_FLOOR_BREACH
        );
        assert_eq!(
            decision.payload["clob_submit_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_CLOB_NOT_SUBMITTED
        );
    }

    #[test]
    fn submit_revalidation_pass_message_has_pass_title() {
        let mut decision =
            submit_revalidation_decision(true, false, "retrace_stabilized", base_payload());
        annotate(&mut decision, true, false);

        let message = build_live_gap_submit_revalidation_notification_message_from_fields(
            "btc-updown-5m-1777917600",
            "Up",
            None,
            &decision.payload,
        );

        assert!(message.contains("Pre-Buy Submit Revalidation Pass"));
        assert!(!message.contains("Pre-Buy Submit Revalidation Block"));
        assert!(message.contains("Decision Reason: fresh_revalidation_passed"));
        assert!(message.contains("CLOB: CLOB_SUBMIT_ALLOWED"));
    }

    #[test]
    fn submit_revalidation_block_message_has_block_title() {
        let mut decision = submit_revalidation_decision(
            false,
            false,
            "effective_fill_above_hard_max",
            base_payload(),
        );
        annotate(&mut decision, true, false);

        let message = build_live_gap_submit_revalidation_notification_message_from_fields(
            "btc-updown-5m-1777917600",
            "Up",
            None,
            &decision.payload,
        );

        assert!(message.contains("Pre-Buy Submit Revalidation Block"));
        assert!(message.contains("Decision Reason: effective_fill_above_hard_max"));
        assert!(message.contains("CLOB: CLOB_NOT_SUBMITTED"));
    }

    #[test]
    fn submit_revalidation_block_message_shows_fresh_timing_and_local_path() {
        let mut payload = base_payload();
        payload
            .as_object_mut()
            .expect("payload object")
            .insert("no_reversal_entry_guard".to_string(), local_path_guard_payload());
        let mut decision =
            submit_revalidation_decision(false, false, "local_path_drop_too_high", payload);
        annotate(&mut decision, true, false);

        let message = build_live_gap_submit_revalidation_notification_message_from_fields(
            "btc-updown-5m-1777917600",
            "Down",
            None,
            &decision.payload,
        );

        assert!(message.contains("Pre-Buy Submit Revalidation Block"));
        assert!(message.contains("Candidate Created At: 812ms"));
        assert!(message.contains("Fresh Snapshot Age: 1000ms"));
        assert!(message.contains("Fresh Revalidation TS: 2000ms"));
        assert!(message.contains("Candidate Reuse: denied, revalidation_required"));
        assert!(message.contains("CLOB: CLOB_NOT_SUBMITTED"));
        assert!(message.contains("No-Reversal:"));
        assert!(message.contains("Profile Lookup Fallback: gap_relaxed"));
        assert!(message.contains("Fallback: local_2m_path"));
        assert!(message.contains("Profile Lookup Key: window_start=2026-05-06T12:00:00Z"));
        assert!(message.contains("hash=abcdef12"));
        assert!(message.contains("Local Path:"));
        assert!(message.contains("History: 42s"));
        assert!(message.contains("Samples: 170"));
        assert!(message.contains("Largest Sample Gap: 496ms"));
        assert!(message.contains("Min Gap 60s: +40.1 USD"));
        assert!(message.contains("Drop10/30/60: 6.0 USD / 13.2 USD / 13.2 USD"));
        assert!(message.contains("Decision: BLOCK"));
        assert!(message.contains("Reason: local_path_drop_too_high"));
    }

    #[test]
    fn smart_dedupe_suppresses_repeated_stale_pass() {
        let order = test_order();
        let mut decision =
            submit_revalidation_decision(true, false, "retrace_stabilized", base_payload());
        annotate(&mut decision, true, false);

        assert!(remember_live_gap_submit_revalidation_notification_state(
            &mut decision.payload,
            &order,
            "smart",
        ));
        assert!(!remember_live_gap_submit_revalidation_notification_state(
            &mut decision.payload,
            &order,
            "smart",
        ));
    }

    #[test]
    fn smart_dedupe_notifies_block_reason_change() {
        let order = test_order();
        let mut decision = submit_revalidation_decision(
            false,
            false,
            "effective_fill_above_hard_max",
            base_payload(),
        );
        annotate(&mut decision, true, false);

        assert!(remember_live_gap_submit_revalidation_notification_state(
            &mut decision.payload,
            &order,
            "smart",
        ));
        if let Some(obj) = decision.payload.as_object_mut() {
            obj.insert(
                "decision_reason".to_string(),
                json!("live_gap_below_required"),
            );
        }
        assert!(remember_live_gap_submit_revalidation_notification_state(
            &mut decision.payload,
            &order,
            "smart",
        ));
    }

    #[test]
    fn late_high_price_notify_only_does_not_block() {
        let mut decision =
            submit_revalidation_decision(true, false, "retrace_stabilized", base_payload());
        annotate(&mut decision, true, false);

        assert_eq!(
            decision.payload["fresh_revalidation_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS
        );
        assert_eq!(
            decision.payload["clob_submit_decision"],
            LIVE_GAP_SUBMIT_REVALIDATION_CLOB_ALLOWED
        );
        assert_eq!(decision.payload["late_high_price_risk"]["mode"], "notify_only");
    }
}
