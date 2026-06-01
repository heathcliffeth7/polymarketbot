#[cfg(test)]
mod no_reversal_entry_guard_tests {
    use super::*;

    fn stat(
        name: &'static str,
        value: Option<f64>,
        samples: i64,
        markets: i64,
    ) -> NoReversalLookbackStat {
        let window = NO_REVERSAL_LOOKBACK_WINDOWS
            .iter()
            .find(|window| window.name == name)
            .copied()
            .expect("lookback window");
        NoReversalLookbackStat {
            name,
            hours: window.hours,
            min_samples: window.min_samples,
            min_markets: window.min_markets,
            adverse_quantile: value,
            sample_count: samples,
            market_count: markets,
            valid: value.is_some()
                && samples >= window.min_samples
                && markets >= window.min_markets,
        }
    }

    fn cfg() -> NoReversalEntryGuardConfig {
        NoReversalEntryGuardConfig {
            enabled: true,
            decision_mode: NO_REVERSAL_DECISION_MODE_HISTORICAL_ADAPTIVE.to_string(),
            lookback_mode: "multi_window_adaptive".to_string(),
            precomputed_profiles_enabled: true,
            allow_cold_profile_query: false,
            baseline_floor_pct: 0.80,
            daily_fallback_floor_pct: 0.70,
            source_mismatch_buffer_usd: None,
            source_mismatch_buffer_floor_ratio: 0.15,
            late_high_extra_buffer_usd: None,
            freeze_per_market: true,
            cache_ttl_sec: 60,
            profile_query_timeout_ms: 500,
            profile_lookup_timeout_ms: 500,
            prewarm_query_timeout_ms: 30_000,
            max_relax_pct_per_window: 0.20,
            max_tighten_pct_per_window: 0.40,
            soft_pass_on_insufficient_data: false,
            use_local_path_fallback_on_missing_profile: true,
            local_path_fallback_enabled: true,
            local_path_lookback_ms: 120_000,
            local_path_lookback_source: "node_config".to_string(),
            local_path_min_history_ms: 30_000,
            local_path_gate_mode: NO_REVERSAL_LOCAL_PATH_GATE_MODE_CLEAN_FLOOR.to_string(),
            local_path_fresh_retrace_window_ms: 10_000,
            local_path_fresh_max_drop_usd: 5.0,
            local_path_fresh_min_history_ms: 1_000,
            block_if_profile_missing_and_local_path_insufficient: true,
            profile_missing_emergency_margin_enabled: true,
            profile_missing_emergency_margin_floor_ratio: 0.9,
            ptb_floor_usd: Some(13.0),
        }
    }

    fn input(current_live_gap: f64) -> NoReversalEntryGuardInput<'static> {
        NoReversalEntryGuardInput {
            market_slug: "btc-updown-5m-test",
            token_id: "tok-up",
            outcome_label: "Up",
            definition_id: 4320,
            node_key: "action",
            asset: "btc",
            direction: "up",
            remaining_sec: 46,
            effective_fill: 0.82,
            current_live_gap,
            regime: "low_clean",
            slope_bucket: "non_negative",
        }
    }

    fn query() -> NoReversalProfileQuery {
        NoReversalProfileQuery {
            market_slug: "btc-updown-5m-test".to_string(),
            target_window_start: None,
            definition_id: 4320,
            node_key: "action".to_string(),
            profile_config_hash: "hash".to_string(),
            asset: "btc".to_string(),
            direction: "up".to_string(),
            slope_bucket: "non_negative".to_string(),
            remaining_bucket: no_reversal_remaining_bucket(46),
            price_bucket: no_reversal_price_bucket(0.82),
            gap_bucket: no_reversal_gap_bucket(23.0),
            quantile: 0.95,
            high_late: false,
        }
    }

    fn profile(selected_adverse: f64) -> NoReversalResolvedProfile {
        NoReversalResolvedProfile {
            selected_adverse,
            raw_selected_adverse: selected_adverse,
            clamp_applied: false,
            previous_selected: None,
            selection: NoReversalSelection {
                selected_adverse,
                recent_risk: Some(selected_adverse),
                session_risk: None,
                session_source: None,
                baseline_floor: None,
                baseline_source: None,
            },
            fallback_level: NoReversalFallbackLevel::Exact,
            stats: Vec::new(),
        }
    }

    #[test]
    fn adverse_move_uses_future_min_gap() {
        assert_eq!(no_reversal_adverse_move(23.0, 7.0), 16.0);
        assert_eq!(no_reversal_adverse_move(23.0, 27.0), 0.0);
    }

    #[test]
    fn multi_lookback_keeps_baseline_floor_when_recent_is_calm() {
        let stats = vec![
            stat("3h", Some(8.0), 100, 30),
            stat("6h", Some(10.0), 140, 40),
            stat("12h", Some(13.0), 200, 60),
            stat("1d", Some(16.0), 300, 100),
            stat("14d", Some(18.0), 600, 200),
        ];
        let selected = no_reversal_select_adverse(&stats, &cfg()).expect("selection");
        assert_eq!(selected.selected_adverse, 14.4);
        assert_eq!(selected.baseline_source, Some("14d_floor"));
    }

    #[test]
    fn multi_lookback_tightens_on_recent_volatility() {
        let stats = vec![
            stat("3h", Some(24.0), 100, 30),
            stat("6h", Some(21.0), 140, 40),
            stat("12h", Some(15.0), 200, 60),
            stat("14d", Some(18.0), 600, 200),
        ];
        let selected = no_reversal_select_adverse(&stats, &cfg()).expect("selection");
        assert_eq!(selected.selected_adverse, 24.0);
    }

    #[test]
    fn daily_p95_is_session_fallback_when_twelve_hour_is_invalid() {
        let stats = vec![
            stat("12h", Some(20.0), 50, 10),
            stat("1d", Some(17.0), 300, 100),
            stat("14d", Some(18.0), 600, 200),
        ];
        let selected = no_reversal_select_adverse(&stats, &cfg()).expect("selection");
        assert_eq!(selected.session_risk, Some(17.0));
        assert_eq!(selected.session_source, Some("1d_fallback"));
    }

    #[test]
    fn source_buffer_scales_by_asset_floor() {
        let cfg = cfg();
        assert_eq!(no_reversal_source_buffer(&cfg, "btc", 13.0, false), 2.0);
        assert!((no_reversal_source_buffer(&cfg, "eth", 1.2, false) - 0.18).abs() < 0.000001);
    }

    #[test]
    fn high_late_profile_adds_asset_scaled_extra_buffer() {
        let cfg = cfg();
        assert_eq!(no_reversal_source_buffer(&cfg, "btc", 13.0, true), 6.0);
        assert!((no_reversal_source_buffer(&cfg, "sol", 0.15, true) - 0.0675).abs() < 0.000001);
    }

    #[test]
    fn window_clamp_slows_relaxing_and_caps_tightening() {
        assert_eq!(
            no_reversal_apply_window_clamp(10.0, Some(20.0), 0.20, 0.40),
            (16.0, true, Some(20.0))
        );
        assert_eq!(
            no_reversal_apply_window_clamp(40.0, Some(20.0), 0.20, 0.40),
            (28.0, true, Some(20.0))
        );
    }

    #[test]
    fn cached_profile_recomputes_decision_from_fresh_live_gap() {
        let cfg = cfg();
        let query = query();
        let profile = profile(14.0);
        let strong = no_reversal_decision_from_profile(
            &cfg,
            &input(40.0),
            &query,
            13.0,
            2.0,
            &profile,
            true,
            Some(100),
            None,
            false,
            "memory_cache",
        );
        let weak = no_reversal_decision_from_profile(
            &cfg,
            &input(20.0),
            &query,
            13.0,
            2.0,
            &profile,
            true,
            Some(100),
            None,
            false,
            "memory_cache",
        );
        assert!(strong.passed);
        assert!(!weak.passed);
        assert_eq!(
            strong.payload["selected_adverse_usd"],
            weak.payload["selected_adverse_usd"]
        );
    }

    #[test]
    fn missing_profile_blocks_when_local_history_is_insufficient() {
        let decision = no_reversal_local_path_decision(
            &cfg(),
            &input(40.0),
            &query(),
            13.0,
            2.0,
            "missing",
            "precomputed_profile_missing",
        );
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "local_path_history_insufficient");
    }

    #[test]
    fn local_path_safe_fallback_payload_includes_diagnostic_windows() {
        let market_slug = "btc-updown-5m-local-path-telemetry";
        let token_id = "tok-local-telemetry";
        let now_ms = Utc::now().timestamp_millis();
        for (offset_ms, gap) in [
            (110_000, 31.0),
            (55_000, 39.5),
            (29_000, 53.4),
            (20_000, 63.6),
            (10_000, 54.0),
            (9_000, 60.8),
            (3_000, 57.0),
            (0, 59.4),
        ] {
            record_pre_buy_collapse_sample_with_retention(
                market_slug,
                token_id,
                "Up",
                PreBuyCollapseSample {
                    ts_ms: now_ms - offset_ms,
                    live_gap: gap,
                    effective_fill: 0.88,
                    best_ask: 0.88,
                    sample_source: "test",
                },
                120_000,
            );
        }
        let input = NoReversalEntryGuardInput {
            market_slug,
            token_id,
            outcome_label: "Up",
            definition_id: 4320,
            node_key: "action",
            asset: "btc",
            direction: "up",
            remaining_sec: 44,
            effective_fill: 0.88,
            current_live_gap: 59.4,
            regime: "high",
            slope_bucket: "non_negative",
        };
        let mut query = query();
        query.market_slug = market_slug.to_string();
        query.target_window_start = Some(Utc::now());
        query.profile_config_hash = "abcdef1234567890".to_string();

        let decision = no_reversal_local_path_decision(
            &cfg(),
            &input,
            &query,
            13.0,
            2.0,
            "missing",
            "precomputed_profile_missing",
        );

        assert!(decision.passed);
        assert_eq!(decision.reason_code, "local_path_safe_fallback");
        assert_eq!(
            decision.payload["local_path_fallback_source"],
            json!("local_2m_path")
        );
        assert_eq!(
            decision.payload["runtime_fallback_source"],
            json!("local_2m_path")
        );
        assert_eq!(
            decision.payload["profile_lookup_fallback_level"],
            json!("gap_relaxed")
        );
        assert_eq!(decision.payload["profile_lookup_status"], json!("row_missing"));
        assert_eq!(
            decision.payload["profile_lookup_key"]["target_market_slug"],
            json!(market_slug)
        );
        assert_eq!(
            decision.payload["profile_lookup_key"]["node_key"],
            json!("action")
        );
        assert_eq!(
            decision.payload["profile_lookup_key"]["profile_config_hash"],
            json!("abcdef1234567890")
        );
        assert_eq!(
            decision.payload["profile_lookup_key"]["remaining_bucket"],
            json!("45_60")
        );
        assert_eq!(
            decision.payload["profile_lookup_key"]["slope_bucket"],
            json!("non_negative")
        );
        assert_eq!(
            decision.payload["local_path_decision_reason"],
            json!("local_path_safe_fallback")
        );
        assert_eq!(decision.payload["local_path_min_gap_2m"], json!(31.0));
        assert_eq!(decision.payload["local_path_min_gap_60s"], json!(39.5));
        assert_eq!(decision.payload["local_path_min_gap_30s"], json!(53.4));
        assert!(decision.payload["local_path_drop_10s"]
            .as_f64()
            .is_some_and(|value| value > 1.0));
        assert!(decision.payload["local_path_slope_30s"]
            .as_f64()
            .is_some_and(|value| value > 0.0));
        assert!(decision.payload["largest_sample_gap_ms"].as_i64().is_some());
    }

    #[test]
    fn local_path_fresh_floor_touch_allows_fresh_touch_and_blocks_stale_retrace() {
        let cases: [(&str, [(i64, f64); 2], bool, &str); 4] = [
            ("first_touch", [(1_000, 19.0), (0, 20.0)], true, "local_path_fresh_floor_touch"),
            ("small_pullback", [(1_000, 21.0), (0, 20.0)], true, "local_path_fresh_floor_touch"),
            (
                "large_retrace",
                [(9_000, 30.0), (0, 20.0)],
                false,
                "local_path_fresh_retrace_too_high",
            ),
            (
                "below_floor",
                [(1_000, 19.5), (0, 19.99)],
                false,
                "local_path_current_gap_below_floor",
            ),
        ];
        for (name, samples, expected_passed, expected_reason) in cases {
            let market_slug = format!("btc-updown-5m-fresh-floor-{name}");
            let token_id = format!("tok-fresh-floor-{name}");
            let now_ms = Utc::now().timestamp_millis();
            for (offset_ms, gap) in samples {
                record_pre_buy_collapse_sample_with_retention(
                    &market_slug,
                    &token_id,
                    "Up",
                    PreBuyCollapseSample {
                        ts_ms: now_ms - offset_ms,
                        live_gap: gap,
                        effective_fill: 0.88,
                        best_ask: 0.88,
                        sample_source: "test",
                    },
                    300_000,
                );
            }
            let mut config = cfg();
            config.local_path_gate_mode =
                NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH.to_string();
            let mut input = input(samples.last().map(|(_, gap)| *gap).unwrap_or(0.0));
            input.market_slug = &market_slug;
            input.token_id = &token_id;
            let mut query = query();
            query.market_slug = market_slug.clone();

            let decision =
                no_reversal_local_path_decision(&config, &input, &query, 20.0, 2.0, "missing", "precomputed_profile_missing");

            assert_eq!(decision.passed, expected_passed, "{name}");
            assert_eq!(decision.reason_code, expected_reason, "{name}");
            assert_eq!(decision.payload["local_path_decision_reason"], json!(expected_reason));
        }
    }

    #[test]
    fn local_path_only_passes_with_primary_reason() {
        let market_slug = "btc-updown-5m-local-path-primary";
        let token_id = "tok-local-primary";
        let now_ms = Utc::now().timestamp_millis();
        for (offset_ms, gap) in [(70_000, 34.0), (45_000, 36.0), (20_000, 38.0), (0, 39.0)] {
            record_pre_buy_collapse_sample_with_retention(
                market_slug,
                token_id,
                "Up",
                PreBuyCollapseSample {
                    ts_ms: now_ms - offset_ms,
                    live_gap: gap,
                    effective_fill: 0.88,
                    best_ask: 0.88,
                    sample_source: "test",
                },
                120_000,
            );
        }
        let mut config = cfg();
        config.decision_mode = NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY.to_string();
        config.precomputed_profiles_enabled = true;
        let mut input = input(39.0);
        input.market_slug = market_slug;
        input.token_id = token_id;
        let mut query = query();
        query.market_slug = market_slug.to_string();

        let decision =
            no_reversal_local_path_primary_decision(&config, &input, &query, 13.0, 2.0);

        assert!(decision.passed);
        assert_eq!(decision.reason_code, "local_path_primary");
        assert_eq!(decision.payload["reason_code"], json!("local_path_primary"));
        assert_eq!(
            decision.payload["decision_mode"],
            json!(NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY)
        );
        assert_eq!(
            decision.payload["profile_source"],
            json!("disabled_by_local_path_only")
        );
        assert_eq!(
            decision.payload["runtime_fallback_source"],
            json!("local_2m_path")
        );
        assert_eq!(decision.payload["protection"], json!("local_path_applied"));
        assert_eq!(
            decision.payload["no_reversal_profile_cache_hit"],
            json!(false)
        );
    }

    #[test]
    fn local_path_only_blocks_when_history_is_insufficient() {
        let mut config = cfg();
        config.decision_mode = NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY.to_string();
        let input = NoReversalEntryGuardInput {
            market_slug: "btc-updown-5m-local-path-empty",
            token_id: "tok-local-path-empty",
            outcome_label: "Up",
            definition_id: 4320,
            node_key: "action",
            asset: "btc",
            direction: "up",
            remaining_sec: 46,
            effective_fill: 0.82,
            current_live_gap: 40.0,
            regime: "low_clean",
            slope_bucket: "non_negative",
        };
        let mut query = query();
        query.market_slug = input.market_slug.to_string();

        let decision =
            no_reversal_local_path_primary_decision(&config, &input, &query, 13.0, 2.0);

        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "local_path_history_insufficient");
        assert_eq!(
            decision.payload["profile_source"],
            json!("disabled_by_local_path_only")
        );
    }

    #[test]
    fn local_path_only_blocks_floor_and_drop_failures() {
        let cases: [(&str, [(i64, f64); 3], &str); 3] = [
            (
                "floor",
                [(45_000, 35.0), (20_000, 12.0), (0, 36.0)],
                "local_path_floor_breached",
            ),
            (
                "drop",
                [(45_000, 40.0), (20_000, 60.0), (0, 39.0)],
                "local_path_drop_too_high",
            ),
            (
                "fast-3s-drop",
                [(45_000, 36.0), (3_000, 46.0), (0, 35.0)],
                "local_path_drop_too_high",
            ),
        ];
        for (name, samples, expected_reason) in cases {
            let market_slug = format!("btc-updown-5m-local-path-{name}");
            let token_id = format!("tok-local-path-{name}");
            let now_ms = Utc::now().timestamp_millis();
            for (offset_ms, gap) in samples {
                record_pre_buy_collapse_sample_with_retention(
                    &market_slug,
                    &token_id,
                    "Up",
                    PreBuyCollapseSample {
                        ts_ms: now_ms - offset_ms,
                        live_gap: gap,
                        effective_fill: 0.88,
                        best_ask: 0.88,
                        sample_source: "test",
                    },
                    120_000,
                );
            }
            let mut config = cfg();
            config.decision_mode = NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY.to_string();
            let input = NoReversalEntryGuardInput {
                market_slug: &market_slug,
                token_id: &token_id,
                outcome_label: "Up",
                definition_id: 4320,
                node_key: "action",
                asset: "btc",
                direction: "up",
                remaining_sec: 44,
                effective_fill: 0.88,
                current_live_gap: samples.last().map(|(_, gap)| *gap).unwrap_or(0.0),
                regime: "low_clean",
                slope_bucket: "non_negative",
            };
            let mut query = query();
            query.market_slug = market_slug.clone();

            let decision =
                no_reversal_local_path_primary_decision(&config, &input, &query, 13.0, 2.0);

            assert!(!decision.passed, "{name} should block");
            assert_eq!(decision.reason_code, expected_reason);
        }
    }

    #[test]
    fn local_path_only_allows_moderate_three_second_drop() {
        let market_slug = "btc-updown-5m-local-path-moderate-3s-drop";
        let token_id = "tok-local-path-moderate-3s-drop";
        let now_ms = Utc::now().timestamp_millis();
        for (offset_ms, gap) in [(45_000, 36.0), (3_000, 40.0), (0, 35.0)] {
            record_pre_buy_collapse_sample_with_retention(
                market_slug,
                token_id,
                "Up",
                PreBuyCollapseSample {
                    ts_ms: now_ms - offset_ms,
                    live_gap: gap,
                    effective_fill: 0.88,
                    best_ask: 0.88,
                    sample_source: "test",
                },
                120_000,
            );
        }
        let mut config = cfg();
        config.decision_mode = NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY.to_string();
        let mut input = input(35.0);
        input.market_slug = market_slug;
        input.token_id = token_id;
        input.effective_fill = 0.88;
        let mut query = query();
        query.market_slug = market_slug.to_string();

        let decision =
            no_reversal_local_path_primary_decision(&config, &input, &query, 13.0, 2.0);

        assert!(decision.passed);
        assert_eq!(decision.reason_code, "local_path_primary");
        assert!(decision.payload["local_path_slope_3s"]
            .as_f64()
            .is_some_and(|value| value < -1.0));
        assert_eq!(decision.payload["local_path_drop_30s"], json!(5.0));
    }

    #[test]
    fn no_reversal_profile_prewarm_event_payload_carries_lookup_key() {
        let mut query = query();
        query.target_window_start = Some(Utc::now());
        query.profile_config_hash = "feedfacecafebeef".to_string();
        let payload = no_reversal_profile_prewarm_event_payload(
            &query,
            "cache-key",
            "started",
            json!({
                "started_at_ms": 1234,
                "timeout_ms": 30_000,
            }),
        );

        assert_eq!(payload["status"], json!("started"));
        assert_eq!(payload["cache_key"], json!("cache-key"));
        assert_eq!(payload["node_key"], json!("action"));
        assert_eq!(payload["profile_config_hash"], json!("feedfacecafebeef"));
        assert_eq!(payload["profile_lookup_key"]["node_key"], json!("action"));
        assert_eq!(
            payload["profile_lookup_key"]["profile_config_hash"],
            json!("feedfacecafebeef")
        );
        assert_eq!(payload["profile_lookup_key"]["direction"], json!("up"));
        assert_eq!(
            payload["profile_lookup_key"]["remaining_bucket"],
            json!("45_60")
        );
        assert_eq!(payload["started_at_ms"], json!(1234));
        assert_eq!(payload["timeout_ms"], json!(30_000));
    }

    #[test]
    fn no_reversal_bulk_feature_queries_preserve_fallback_filters() {
        let mut query = query();
        query.target_window_start = DateTime::from_timestamp(1_778_071_800, 0);
        let window = NO_REVERSAL_LOOKBACK_WINDOWS[0];

        let exact = no_reversal_bulk_feature_lookback_query(
            &query,
            window,
            NoReversalFallbackLevel::Exact,
            query.target_window_start.unwrap(),
        );
        let slope_relaxed = no_reversal_bulk_feature_lookback_query(
            &query,
            window,
            NoReversalFallbackLevel::SlopeRelaxed,
            query.target_window_start.unwrap(),
        );
        let gap_relaxed = no_reversal_bulk_feature_lookback_query(
            &query,
            window,
            NoReversalFallbackLevel::GapRelaxed,
            query.target_window_start.unwrap(),
        );

        assert_eq!(exact.fallback_level, "exact");
        assert_eq!(exact.gap_min, Some(query.gap_bucket.min));
        assert_eq!(exact.gap_max, Some(query.gap_bucket.max));
        assert_eq!(exact.slope_bucket, Some("non_negative".to_string()));
        assert_eq!(slope_relaxed.fallback_level, "slope_relaxed");
        assert_eq!(slope_relaxed.gap_min, Some(query.gap_bucket.min));
        assert_eq!(slope_relaxed.slope_bucket, None);
        assert_eq!(gap_relaxed.fallback_level, "gap_relaxed");
        assert_eq!(gap_relaxed.gap_min, None);
        assert_eq!(gap_relaxed.gap_max, None);
        assert_eq!(gap_relaxed.slope_bucket, None);
    }

    #[test]
    fn no_reversal_feature_refresh_payload_names_materialized_source() {
        let payload =
            no_reversal_feature_refresh_event_payload(&NoReversalAdverseFeatureRefreshTelemetry {
                status: "refreshed".to_string(),
                rows_affected: Some(42),
                refresh_ms: Some(17),
                error: None,
            });

        assert_eq!(
            payload["stats_source"],
            json!("materialized_adverse_features")
        );
        assert_eq!(payload["feature_refresh_status"], json!("refreshed"));
        assert_eq!(payload["feature_refresh_rows_affected"], json!(42));
        assert_eq!(payload["feature_refresh_ms"], json!(17));
    }

    #[test]
    fn no_reversal_profile_keyspace_includes_high_late_runtime_key() {
        let input = NoReversalProfileKeyspaceInput {
            market_slug: "btc-updown-5m-1778061000".to_string(),
            target_window_start: DateTime::from_timestamp(1_778_061_000, 0),
            definition_id: 4320,
            node_key: "action_qontiv".to_string(),
            profile_config_hash: "9325db4031703f49".to_string(),
            asset: "btc".to_string(),
            direction: "up".to_string(),
            current_remaining_sec: 74,
            current_best_ask: 0.91,
            current_live_gap: 52.0,
            current_slope_bucket: "non_negative".to_string(),
        };

        let queries = no_reversal_profile_keyspace_queries(&input);

        assert!(queries.iter().any(|query| {
            query.remaining_bucket.label == "60_90"
                && query.price_bucket.label == "90_93"
                && query.gap_bucket.label == "50_55"
                && query.slope_bucket == "non_negative"
                && (query.quantile - 0.98).abs() < f64::EPSILON
                && query.high_late
        }));
    }

    #[test]
    fn no_reversal_profile_keyspace_exact_key_is_first_and_nearby_is_bounded() {
        let input = NoReversalProfileKeyspaceInput {
            market_slug: "btc-updown-5m-1778064000".to_string(),
            target_window_start: DateTime::from_timestamp(1_778_064_000, 0),
            definition_id: 4320,
            node_key: "action_qontiv".to_string(),
            profile_config_hash: "9325db4031703f49".to_string(),
            asset: "btc".to_string(),
            direction: "up".to_string(),
            current_remaining_sec: 75,
            current_best_ask: 0.88,
            current_live_gap: 55.7,
            current_slope_bucket: "non_negative".to_string(),
        };

        let candidates = no_reversal_profile_keyspace_candidates(&input);
        let exact = &candidates[0];

        assert_eq!(
            exact.priority,
            NoReversalProfilePrewarmPriority::ExactCurrent
        );
        assert_eq!(exact.query.remaining_bucket.label, "60_90");
        assert_eq!(exact.query.price_bucket.label, "85_89");
        assert_eq!(exact.query.gap_bucket.label, "55_60");
        assert_eq!(exact.query.slope_bucket, "non_negative");
        assert_eq!(exact.query.quantile, 0.95);
        assert!(!exact.query.high_late);
        assert!(candidates.len() <= 1 + NO_REVERSAL_PROFILE_KEYSPACE_MAX_NEARBY_QUERIES);
        assert!(candidates
            .iter()
            .skip(1)
            .all(|candidate| candidate.priority == NoReversalProfilePrewarmPriority::Nearby));
    }

    #[test]
    fn no_reversal_profile_warmup_capacity_reserves_slots_for_exact_keys() {
        let mut warmups = HashMap::new();
        for index in 0..NO_REVERSAL_MAX_TOTAL_WARMUPS {
            warmups.insert(
                format!("nearby-{index}"),
                NoReversalProfilePrewarmPriority::Nearby,
            );
        }

        assert!(no_reversal_warmup_capacity_allows(
            &warmups,
            NoReversalProfilePrewarmPriority::ExactCurrent
        ));
        assert!(!no_reversal_warmup_capacity_allows(
            &warmups,
            NoReversalProfilePrewarmPriority::Nearby
        ));
        assert!(!no_reversal_warmup_capacity_allows(
            &warmups,
            NoReversalProfilePrewarmPriority::Background
        ));
    }

    #[test]
    fn no_reversal_prewarmer_status_keeps_non_ready_rows_distinct() {
        assert_eq!(
            no_reversal_prewarmer_status_from_record_status("insufficient"),
            "insufficient_samples"
        );
        assert_eq!(
            no_reversal_prewarmer_status_from_record_status("timed_out"),
            "expected_key_timed_out"
        );
        assert_eq!(
            no_reversal_profile_lookup_status("timeout", "prewarm_query_timeout"),
            "timed_out"
        );
    }

    #[test]
    fn no_reversal_attach_prewarmer_diagnostics_payload() {
        let diagnostics = NoReversalAdverseProfileDiagnostics {
            prewarmer_status: "completed_but_hash_mismatch".to_string(),
            summary_json: json!({
                "table": { "different_hash_rows": 4 },
                "events": {
                    "expected_rows": 1,
                    "latest_priority": "exact_current",
                    "latest_slot_status": "queued_capacity_limited",
                    "prewarm_age_ms": 8000
                }
            }),
        };
        let mut payload = json!({});

        no_reversal_attach_prewarmer_diagnostics(&mut payload, &diagnostics);

        assert_eq!(
            payload["prewarmer_status"],
            json!("completed_but_hash_mismatch")
        );
        assert_eq!(
            payload["prewarmer_diagnostics"]["table"]["different_hash_rows"],
            json!(4)
        );
        assert_eq!(payload["prewarm_priority"], json!("exact_current"));
        assert_eq!(
            payload["prewarm_slot_status"],
            json!("queued_capacity_limited")
        );
        assert_eq!(payload["prewarm_age_ms"], json!(8000));
    }
}
