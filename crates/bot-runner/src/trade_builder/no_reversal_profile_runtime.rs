const NO_REVERSAL_MAX_TOTAL_WARMUPS: usize = 16;
const NO_REVERSAL_MAX_EXACT_BURST_WARMUPS: usize = 24;
const NO_REVERSAL_MAX_NON_EXACT_WARMUPS: usize = 8;
const NO_REVERSAL_MAX_BACKGROUND_WARMUPS: usize = 2;
const NO_REVERSAL_WARMUP_RETRY_COOLDOWN_MS: i64 = 60_000;

static NO_REVERSAL_PROFILE_WARMUP_LAST_START_MS: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));
static NO_REVERSAL_PROFILE_EXPECTED_KEY_LAST_MS: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));
static NO_REVERSAL_PROFILE_CAPACITY_LIMIT_LAST_MS: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoReversalWarmupSlotStatus {
    Started,
    AlreadyRunning,
    CapacityLimited,
}

impl NoReversalWarmupSlotStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::AlreadyRunning => "already_running",
            Self::CapacityLimited => "queued_capacity_limited",
        }
    }
}

fn no_reversal_warmup_counts(
    warmups: &HashMap<String, NoReversalProfilePrewarmPriority>,
) -> (usize, usize, usize, usize) {
    let total = warmups.len();
    let exact = warmups
        .values()
        .filter(|priority| **priority == NoReversalProfilePrewarmPriority::ExactCurrent)
        .count();
    let nearby = warmups
        .values()
        .filter(|priority| **priority == NoReversalProfilePrewarmPriority::Nearby)
        .count();
    let background = warmups
        .values()
        .filter(|priority| **priority == NoReversalProfilePrewarmPriority::Background)
        .count();
    (total, exact, nearby, background)
}

fn no_reversal_warmup_capacity_allows(
    warmups: &HashMap<String, NoReversalProfilePrewarmPriority>,
    priority: NoReversalProfilePrewarmPriority,
) -> bool {
    let (total, exact, nearby, background) = no_reversal_warmup_counts(warmups);
    let non_exact = nearby + background;
    match priority {
        NoReversalProfilePrewarmPriority::ExactCurrent => {
            total < NO_REVERSAL_MAX_TOTAL_WARMUPS
                || (exact < NO_REVERSAL_MAX_TOTAL_WARMUPS
                    && total < NO_REVERSAL_MAX_EXACT_BURST_WARMUPS)
        }
        NoReversalProfilePrewarmPriority::Nearby => {
            total < NO_REVERSAL_MAX_TOTAL_WARMUPS
                && non_exact < NO_REVERSAL_MAX_NON_EXACT_WARMUPS
        }
        NoReversalProfilePrewarmPriority::Background => {
            total < NO_REVERSAL_MAX_TOTAL_WARMUPS
                && non_exact < NO_REVERSAL_MAX_NON_EXACT_WARMUPS
                && background < NO_REVERSAL_MAX_BACKGROUND_WARMUPS
        }
    }
}

fn no_reversal_profile_lookup_key_payload(query: &NoReversalProfileQuery) -> Value {
    json!({
        "target_market_slug": query.market_slug.clone(),
        "target_window_start": query.target_window_start,
        "definition_id": query.definition_id,
        "node_key": query.node_key.clone(),
        "profile_config_hash": query.profile_config_hash.clone(),
        "asset": query.asset.clone(),
        "direction": query.direction.clone(),
        "remaining_bucket": query.remaining_bucket.label.clone(),
        "price_bucket": query.price_bucket.label.clone(),
        "gap_bucket": query.gap_bucket.label.clone(),
        "slope_bucket": query.slope_bucket.clone(),
        "quantile": query.quantile,
        "high_late": query.high_late,
    })
}

fn no_reversal_profile_lookup_status(
    profile_source: &str,
    profile_reason: &str,
) -> String {
    match profile_reason {
        "precomputed_profile_missing" => "row_missing",
        "insufficient_historical_adverse_data" => "insufficient_samples",
        "precomputed_profile_stale" => "stale",
        "precomputed_profile_invalid" => "invalid",
        "precomputed_profile_lookup_timeout" | "prewarm_query_timeout" => "timed_out",
        "precomputed_profile_error" | "historical_adverse_query_failed" => "lookup_error",
        _ if profile_source == "missing" => "row_missing",
        _ if profile_source == "insufficient" => "insufficient_samples",
        _ if profile_source == "stale" => "stale",
        _ if profile_source == "timeout" => "timed_out",
        _ if profile_source == "error" => "lookup_error",
        _ => profile_source,
    }
    .to_string()
}

fn no_reversal_prewarmer_status_from_record_status(status: &str) -> &'static str {
    match status {
        "ready" => "ready",
        "insufficient" => "insufficient_samples",
        "timed_out" => "expected_key_timed_out",
        "error" => "expected_key_failed",
        "stale" => "stale",
        _ => "unknown",
    }
}

fn no_reversal_attach_prewarmer_diagnostics(
    payload: &mut Value,
    diagnostics: &NoReversalAdverseProfileDiagnostics,
) {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "prewarmer_status".to_string(),
            json!(diagnostics.prewarmer_status),
        );
        obj.insert(
            "prewarmer_diagnostics".to_string(),
            diagnostics.summary_json.clone(),
        );
        if let Some(events) = diagnostics.summary_json.get("events") {
            if let Some(priority) = events.get("latest_priority").cloned() {
                obj.insert("prewarm_priority".to_string(), priority);
            }
            if let Some(slot_status) = events.get("latest_slot_status").cloned() {
                obj.insert("prewarm_slot_status".to_string(), slot_status);
            }
            if let Some(started_at) = events.get("latest_started_at").cloned() {
                obj.insert("prewarm_started_at".to_string(), started_at);
            }
            if let Some(age_ms) = events.get("prewarm_age_ms").cloned() {
                obj.insert("prewarm_age_ms".to_string(), age_ms);
            }
        }
    }
}

fn no_reversal_attach_prewarmer_status(payload: &mut Value, status: &str) {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("prewarmer_status".to_string(), json!(status));
    }
}

fn no_reversal_profile_prewarm_event_payload(
    query: &NoReversalProfileQuery,
    cache_key: &str,
    status: &str,
    extra: Value,
) -> Value {
    let mut payload = json!({
        "status": status,
        "cache_key": cache_key,
        "node_key": query.node_key.clone(),
        "target_market_slug": query.market_slug.clone(),
        "target_window_start": query.target_window_start,
        "direction": query.direction.clone(),
        "profile_config_hash": query.profile_config_hash.clone(),
        "profile_lookup_key": no_reversal_profile_lookup_key_payload(query),
    });
    if let (Some(payload), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            payload.insert(key.clone(), value.clone());
        }
    }
    payload
}

async fn no_reversal_append_profile_prewarm_event(
    repo: &PostgresRepository,
    query: &NoReversalProfileQuery,
    cache_key: &str,
    event_type: &str,
    status: &str,
    extra: Value,
) {
    let payload = no_reversal_profile_prewarm_event_payload(query, cache_key, status, extra);
    if let Err(err) = repo
        .append_trade_flow_event(None, query.definition_id, None, event_type, &payload)
        .await
    {
        debug!(
            error = %err,
            market_slug = %query.market_slug,
            event_type,
            "no-reversal profile prewarm event append failed"
        );
    }
}

fn no_reversal_record_profile_expected_key(
    repo: PostgresRepository,
    query: NoReversalProfileQuery,
    cache_key: String,
    expected_index: usize,
    expected_total: usize,
    priority: NoReversalProfilePrewarmPriority,
) {
    let now_ms = Utc::now().timestamp_millis();
    {
        let mut last_events = NO_REVERSAL_PROFILE_EXPECTED_KEY_LAST_MS
            .lock()
            .expect("no-reversal expected profile key events");
        if let Some(last_ms) = last_events.get(&cache_key) {
            if now_ms - *last_ms < NO_REVERSAL_WARMUP_RETRY_COOLDOWN_MS {
                return;
            }
        }
        last_events.insert(cache_key.clone(), now_ms);
        if last_events.len() > 8_192 {
            let cutoff_ms = now_ms - 10 * 60_000;
            last_events.retain(|_, last_ms| *last_ms >= cutoff_ms);
        }
    }
    tokio::spawn(async move {
        no_reversal_append_profile_prewarm_event(
            &repo,
            &query,
            &cache_key,
            "no_reversal_profile_expected_key",
            "expected",
            json!({
                "expected_at_ms": now_ms,
                "expected_index": expected_index,
                "expected_total": expected_total,
                "priority": priority.label(),
                "slot_status": "expected",
            }),
        )
        .await;
    });
}

fn no_reversal_record_profile_capacity_limited(
    repo: PostgresRepository,
    query: NoReversalProfileQuery,
    cache_key: String,
    priority: NoReversalProfilePrewarmPriority,
    active_total: usize,
    active_exact: usize,
    active_nearby: usize,
    active_background: usize,
) {
    let now_ms = Utc::now().timestamp_millis();
    {
        let mut last_events = NO_REVERSAL_PROFILE_CAPACITY_LIMIT_LAST_MS
            .lock()
            .expect("no-reversal capacity-limited profile events");
        if let Some(last_ms) = last_events.get(&cache_key) {
            if now_ms - *last_ms < NO_REVERSAL_WARMUP_RETRY_COOLDOWN_MS {
                return;
            }
        }
        last_events.insert(cache_key.clone(), now_ms);
        if last_events.len() > 8_192 {
            let cutoff_ms = now_ms - 10 * 60_000;
            last_events.retain(|_, last_ms| *last_ms >= cutoff_ms);
        }
    }
    tokio::spawn(async move {
        no_reversal_append_profile_prewarm_event(
            &repo,
            &query,
            &cache_key,
            "no_reversal_profile_capacity_limited",
            "queued_capacity_limited",
            json!({
                "queued_at_ms": now_ms,
                "priority": priority.label(),
                "slot_status": NoReversalWarmupSlotStatus::CapacityLimited.label(),
                "active_total": active_total,
                "active_exact": active_exact,
                "active_nearby": active_nearby,
                "active_background": active_background,
                "max_total": NO_REVERSAL_MAX_TOTAL_WARMUPS,
                "max_exact_burst": NO_REVERSAL_MAX_EXACT_BURST_WARMUPS,
                "max_non_exact": NO_REVERSAL_MAX_NON_EXACT_WARMUPS,
            }),
        )
        .await;
    });
}

fn no_reversal_profile_stats_totals(stats: &[NoReversalLookbackStat]) -> (i64, i64) {
    let samples = stats
        .iter()
        .map(|stat| stat.sample_count)
        .max()
        .unwrap_or(0);
    let markets = stats
        .iter()
        .map(|stat| stat.market_count)
        .max()
        .unwrap_or(0);
    (samples, markets)
}

async fn no_reversal_upsert_profile_lookup(
    repo: &PostgresRepository,
    query: &NoReversalProfileQuery,
    lookup: NoReversalProfileLookup,
) -> Result<()> {
    let Some(key) = no_reversal_profile_key_from_query(query) else {
        return Ok(());
    };
    let profile_as_of = query.target_window_start.unwrap_or_else(Utc::now);
    let input = if let Some(profile) = lookup.profile {
        let (sample_count, market_count) = no_reversal_profile_stats_totals(&profile.stats);
        NoReversalAdverseProfileInput {
            key,
            status: "ready".to_string(),
            selected_adverse: Some(profile.selected_adverse),
            raw_selected_adverse: Some(profile.raw_selected_adverse),
            fallback_level: Some(no_reversal_fallback_label(profile.fallback_level).to_string()),
            lookbacks_json: no_reversal_stats_payload(&profile.stats),
            sample_count,
            market_count,
            profile_as_of,
            error: None,
        }
    } else {
        let (sample_count, market_count) = no_reversal_profile_stats_totals(&lookup.last_stats);
        NoReversalAdverseProfileInput {
            key,
            status: "insufficient".to_string(),
            selected_adverse: None,
            raw_selected_adverse: None,
            fallback_level: Some(no_reversal_fallback_label(lookup.last_fallback).to_string()),
            lookbacks_json: no_reversal_stats_payload(&lookup.last_stats),
            sample_count,
            market_count,
            profile_as_of,
            error: None,
        }
    };
    repo.upsert_no_reversal_adverse_profile(&input).await
}

async fn no_reversal_upsert_profile_error(
    repo: &PostgresRepository,
    query: &NoReversalProfileQuery,
    status: &str,
    error: String,
) -> Result<()> {
    let Some(key) = no_reversal_profile_key_from_query(query) else {
        return Ok(());
    };
    repo.upsert_no_reversal_adverse_profile(&NoReversalAdverseProfileInput {
        key,
        status: status.to_string(),
        selected_adverse: None,
        raw_selected_adverse: None,
        fallback_level: None,
        lookbacks_json: json!({}),
        sample_count: 0,
        market_count: 0,
        profile_as_of: query.target_window_start.unwrap_or_else(Utc::now),
        error: Some(error),
    })
    .await
}

fn no_reversal_spawn_profile_warmup(
    repo: PostgresRepository,
    config: NoReversalEntryGuardConfig,
    query: NoReversalProfileQuery,
    cache_key: String,
) {
    no_reversal_spawn_profile_warmup_inner(
        repo,
        config,
        query,
        cache_key,
        NoReversalProfilePrewarmPriority::ExactCurrent,
    );
}

fn no_reversal_spawn_profile_warmup_expected(
    repo: PostgresRepository,
    config: NoReversalEntryGuardConfig,
    query: NoReversalProfileQuery,
    cache_key: String,
    expected_index: usize,
    expected_total: usize,
    priority: NoReversalProfilePrewarmPriority,
) {
    no_reversal_record_profile_expected_key(
        repo.clone(),
        query.clone(),
        cache_key.clone(),
        expected_index,
        expected_total,
        priority,
    );
    no_reversal_spawn_profile_warmup_inner(repo, config, query, cache_key, priority);
}

fn no_reversal_spawn_profile_warmup_inner(
    repo: PostgresRepository,
    config: NoReversalEntryGuardConfig,
    query: NoReversalProfileQuery,
    cache_key: String,
    priority: NoReversalProfilePrewarmPriority,
) {
    let started_at_ms = Utc::now().timestamp_millis();
    let (slot_status, active_total, active_exact, active_nearby, active_background) = {
        let mut last_starts = NO_REVERSAL_PROFILE_WARMUP_LAST_START_MS
            .lock()
            .expect("no-reversal profile warmup starts");
        if let Some(last_ms) = last_starts.get(&cache_key) {
            if started_at_ms - *last_ms < NO_REVERSAL_WARMUP_RETRY_COOLDOWN_MS {
                return;
            }
        }
        let mut warmups = NO_REVERSAL_PROFILE_WARMUPS
            .lock()
            .expect("no-reversal profile warmups");
        if warmups.contains_key(&cache_key) {
            let (total, exact, nearby, background) = no_reversal_warmup_counts(&warmups);
            (
                NoReversalWarmupSlotStatus::AlreadyRunning,
                total,
                exact,
                nearby,
                background,
            )
        } else if !no_reversal_warmup_capacity_allows(&warmups, priority) {
            let (total, exact, nearby, background) = no_reversal_warmup_counts(&warmups);
            (
                NoReversalWarmupSlotStatus::CapacityLimited,
                total,
                exact,
                nearby,
                background,
            )
        } else {
            last_starts.insert(cache_key.clone(), started_at_ms);
            if last_starts.len() > 4_096 {
                let cutoff_ms = started_at_ms - 10 * 60_000;
                last_starts.retain(|_, last_ms| *last_ms >= cutoff_ms);
            }
            warmups.insert(cache_key.clone(), priority);
            let (total, exact, nearby, background) = no_reversal_warmup_counts(&warmups);
            (
                NoReversalWarmupSlotStatus::Started,
                total,
                exact,
                nearby,
                background,
            )
        }
    };
    if slot_status == NoReversalWarmupSlotStatus::CapacityLimited {
        no_reversal_record_profile_capacity_limited(
            repo,
            query,
            cache_key,
            priority,
            active_total,
            active_exact,
            active_nearby,
            active_background,
        );
        return;
    }
    if slot_status != NoReversalWarmupSlotStatus::Started {
        return;
    }
    tokio::spawn(async move {
        let refresh_started_at = Utc::now();
        let feature_refresh =
            no_reversal_maybe_refresh_recent_adverse_features(&repo, &query.asset, refresh_started_at)
                .await;
        let feature_refresh_payload = no_reversal_feature_refresh_event_payload(&feature_refresh);
        let now = Utc::now();
        let now_ms = now.timestamp_millis();
        let mut started_extra = json!({
            "started_at_ms": now_ms,
            "timeout_ms": config.prewarm_query_timeout_ms,
            "priority": priority.label(),
            "slot_status": slot_status.label(),
            "active_total": active_total,
            "active_exact": active_exact,
            "active_nearby": active_nearby,
            "active_background": active_background,
        });
        no_reversal_merge_json_object(&mut started_extra, &feature_refresh_payload);
        no_reversal_append_profile_prewarm_event(
            &repo,
            &query,
            &cache_key,
            "no_reversal_profile_prewarm_started",
            "started",
            started_extra,
        )
        .await;
        let resolved = tokio::time::timeout(
            Duration::from_millis(config.prewarm_query_timeout_ms as u64),
            no_reversal_resolve_profile(&repo, &config, &query, now),
        )
        .await;
        match resolved {
            Ok(Ok(lookup)) => {
                let profile = lookup.profile.clone();
                let stats_source = lookup.stats_source;
                let bulk_query_ms = lookup.bulk_query_ms;
                let (profile_status, fallback_level, sample_count, market_count) =
                    if let Some(profile) = profile.as_ref() {
                        let (sample_count, market_count) =
                            no_reversal_profile_stats_totals(&profile.stats);
                        (
                            "ready",
                            no_reversal_fallback_label(profile.fallback_level),
                            sample_count,
                            market_count,
                        )
                    } else {
                        let (sample_count, market_count) =
                            no_reversal_profile_stats_totals(&lookup.last_stats);
                        (
                            "insufficient_samples",
                            no_reversal_fallback_label(lookup.last_fallback),
                            sample_count,
                            market_count,
                        )
                    };
                if let Some(profile) = profile {
                    no_reversal_store_cached_profile(cache_key.clone(), now_ms, &profile);
                }
                let upsert_result = no_reversal_upsert_profile_lookup(&repo, &query, lookup).await;
                let mut event_extra = json!({
                    "completed_at_ms": Utc::now().timestamp_millis(),
                    "profile_status": profile_status,
                    "fallback_level": fallback_level,
                    "sample_count": sample_count,
                    "market_count": market_count,
                    "priority": priority.label(),
                    "slot_status": "completed",
                    "stats_source": stats_source,
                    "bulk_query_ms": bulk_query_ms,
                });
                no_reversal_merge_json_object(&mut event_extra, &feature_refresh_payload);
                if let Err(err) = upsert_result {
                    if let Some(obj) = event_extra.as_object_mut() {
                        obj.insert("upsert_error".to_string(), json!(err.to_string()));
                    }
                    debug!(
                        error = %err,
                        market_slug = %query.market_slug,
                        "no-reversal background profile upsert failed"
                    );
                }
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    "no_reversal_profile_prewarm_completed",
                    profile_status,
                    event_extra.clone(),
                )
                .await;
                let profile_event_type = if profile_status == "ready" {
                    "no_reversal_profile_written"
                } else {
                    "no_reversal_profile_insufficient"
                };
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    profile_event_type,
                    profile_status,
                    event_extra,
                )
                .await;
            }
            Ok(Err(err)) => {
                let error = err.to_string();
                let _ = no_reversal_upsert_profile_error(&repo, &query, "error", error.clone()).await;
                let mut failed_extra = json!({
                    "completed_at_ms": Utc::now().timestamp_millis(),
                    "error": error,
                    "priority": priority.label(),
                    "slot_status": "failed",
                    "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                });
                no_reversal_merge_json_object(&mut failed_extra, &feature_refresh_payload);
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    "no_reversal_profile_prewarm_failed",
                    "lookup_error",
                    failed_extra,
                )
                .await;
                let mut failed_summary = json!({
                    "completed_at_ms": Utc::now().timestamp_millis(),
                    "error": "profile_lookup_error",
                    "priority": priority.label(),
                    "slot_status": "failed",
                    "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                });
                no_reversal_merge_json_object(&mut failed_summary, &feature_refresh_payload);
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    "no_reversal_profile_failed",
                    "lookup_error",
                    failed_summary,
                )
                .await;
                debug!(
                    error = %err,
                    market_slug = %query.market_slug,
                    "no-reversal background profile warmup failed"
                );
            }
            Err(_) => {
                let _ = no_reversal_upsert_profile_error(
                    &repo,
                    &query,
                    "timed_out",
                    "prewarm_query_timeout".to_string(),
                )
                .await;
                let mut timed_out_extra = json!({
                    "completed_at_ms": Utc::now().timestamp_millis(),
                    "error": "prewarm_query_timeout",
                    "priority": priority.label(),
                    "slot_status": "timed_out",
                    "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                });
                no_reversal_merge_json_object(&mut timed_out_extra, &feature_refresh_payload);
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    "no_reversal_profile_prewarm_timed_out",
                    "timed_out",
                    timed_out_extra.clone(),
                )
                .await;
                no_reversal_append_profile_prewarm_event(
                    &repo,
                    &query,
                    &cache_key,
                    "no_reversal_profile_timed_out",
                    "timed_out",
                    timed_out_extra,
                )
                .await;
            }
        }
        NO_REVERSAL_PROFILE_WARMUPS
            .lock()
            .expect("no-reversal profile warmups")
            .remove(&cache_key);
    });
}

fn no_reversal_profile_from_precomputed(
    record: &bot_infra::db::NoReversalAdverseProfileRecord,
) -> Option<NoReversalResolvedProfile> {
    let selected_adverse = record.selected_adverse.filter(|value| value.is_finite())?;
    Some(NoReversalResolvedProfile {
        selected_adverse,
        raw_selected_adverse: record
            .raw_selected_adverse
            .filter(|value| value.is_finite())
            .unwrap_or(selected_adverse),
        clamp_applied: false,
        previous_selected: None,
        selection: NoReversalSelection {
            selected_adverse,
            recent_risk: None,
            session_risk: None,
            session_source: None,
            baseline_floor: None,
            baseline_source: None,
        },
        fallback_level: no_reversal_fallback_from_label(record.fallback_level.as_deref()),
        stats: Vec::new(),
    })
}

fn no_reversal_local_path_fallback_source(lookback_ms: i64) -> String {
    let lookback_sec = (lookback_ms.max(1) + 999) / 1_000;
    if lookback_sec % 60 == 0 {
        format!("local_{}m_path", lookback_sec / 60)
    } else {
        format!("local_{lookback_sec}s_path")
    }
}

fn no_reversal_local_path_min_gap_since(
    samples: &[PreBuyCollapseSample],
    now_ms: i64,
    window_ms: i64,
) -> Option<f64> {
    samples
        .iter()
        .filter(|sample| now_ms - sample.ts_ms <= window_ms)
        .map(|sample| sample.live_gap)
        .filter(|value| value.is_finite())
        .min_by(f64::total_cmp)
}

fn no_reversal_local_path_peak_gap_since(
    samples: &[PreBuyCollapseSample],
    now_ms: i64,
    window_ms: i64,
) -> Option<f64> {
    samples
        .iter()
        .filter(|sample| now_ms - sample.ts_ms <= window_ms)
        .map(|sample| sample.live_gap)
        .filter(|value| value.is_finite())
        .max_by(f64::total_cmp)
}

fn no_reversal_local_path_slope_since(
    samples: &VecDeque<PreBuyCollapseSample>,
    now_ms: i64,
    current_live_gap: f64,
    window_ms: i64,
) -> Option<f64> {
    pre_buy_collapse_sample_at_or_before(samples, now_ms - window_ms)
        .map(|sample| (current_live_gap - sample.live_gap) / (window_ms as f64 / 1_000.0))
        .filter(|value| value.is_finite())
}

fn no_reversal_local_path_decision(
    config: &NoReversalEntryGuardConfig,
    input: &NoReversalEntryGuardInput<'_>,
    query: &NoReversalProfileQuery,
    ptb_floor: f64,
    source_buffer: f64,
    profile_source: &'static str,
    base_reason: &'static str,
) -> NoReversalEntryGuardDecision {
    let mut payload = json!({
        "enabled": config.enabled,
        "lookback_mode": config.lookback_mode,
        "market_slug": input.market_slug,
        "asset": input.asset,
        "direction": input.direction,
        "remaining_sec": input.remaining_sec,
        "effective_fill": input.effective_fill,
        "current_live_gap_usd": input.current_live_gap,
        "ptb_floor_usd": ptb_floor,
        "source_buffer_usd": source_buffer,
        "quantile": query.quantile,
        "high_late_profile": query.high_late,
        "fallback_level": no_reversal_fallback_label(NoReversalFallbackLevel::GapRelaxed),
        "profile_lookup_fallback_level": no_reversal_fallback_label(NoReversalFallbackLevel::GapRelaxed),
        "profile_lookup_status": no_reversal_profile_lookup_status(profile_source, base_reason),
        "profile_lookup_key": no_reversal_profile_lookup_key_payload(query),
        "profile_source": profile_source,
        "profile_reason": base_reason,
        "protection": "not_applied",
        "local_path_lookback_ms": config.local_path_lookback_ms,
        "local_path_lookback_source": config.local_path_lookback_source,
        "local_path_gate_mode": config.local_path_gate_mode,
        "runtime_fallback_source": no_reversal_local_path_fallback_source(config.local_path_lookback_ms),
        "bucket": {
            "remaining_bucket": &query.remaining_bucket.label,
            "price_bucket": &query.price_bucket.label,
            "gap_bucket": &query.gap_bucket.label,
            "slope_bucket": &query.slope_bucket,
        },
        "sources": { "historical": "precomputed_profile", "live": "binance_live" },
    });
    if !(config.use_local_path_fallback_on_missing_profile && config.local_path_fallback_enabled) {
        return no_reversal_unapplied_decision(config, base_reason, payload);
    }

    let now_ms = Utc::now().timestamp_millis();
    let key = pre_buy_collapse_guard_key(input.market_slug, input.token_id, input.outcome_label);
    let samples = PRE_BUY_COLLAPSE_HISTORY
        .lock()
        .expect("pre-buy collapse history")
        .get(&key)
        .cloned()
        .unwrap_or_default();
    let cutoff_ms = now_ms - config.local_path_lookback_ms;
    let recent: Vec<_> = samples
        .iter()
        .copied()
        .filter(|sample| sample.ts_ms >= cutoff_ms)
        .collect();
    let recent_samples = VecDeque::from(recent.clone());
    let history_age_ms = recent
        .first()
        .and_then(|first| recent.last().map(|last| last.ts_ms - first.ts_ms))
        .unwrap_or(0)
        .max(0);
    let largest_sample_gap_ms = recent
        .iter()
        .zip(recent.iter().skip(1))
        .map(|(prev, next)| (next.ts_ms - prev.ts_ms).max(0))
        .max();
    let min_gap = recent
        .iter()
        .map(|sample| sample.live_gap)
        .filter(|value| value.is_finite())
        .min_by(f64::total_cmp);
    let min_gap_30s = no_reversal_local_path_min_gap_since(&recent, now_ms, 30_000);
    let min_gap_60s = no_reversal_local_path_min_gap_since(&recent, now_ms, 60_000);
    let min_gap_2m = no_reversal_local_path_min_gap_since(&recent, now_ms, 120_000);
    let peak_10s = no_reversal_local_path_peak_gap_since(&recent, now_ms, 10_000);
    let peak_30s = no_reversal_local_path_peak_gap_since(&recent, now_ms, 30_000);
    let peak_60s = no_reversal_local_path_peak_gap_since(&recent, now_ms, 60_000);
    let slope_3s =
        no_reversal_local_path_slope_since(&recent_samples, now_ms, input.current_live_gap, 3_000);
    let slope_10s =
        no_reversal_local_path_slope_since(&recent_samples, now_ms, input.current_live_gap, 10_000);
    let slope_30s =
        no_reversal_local_path_slope_since(&recent_samples, now_ms, input.current_live_gap, 30_000);
    let drop_10s = peak_10s.map(|peak| (peak - input.current_live_gap).max(0.0));
    let drop_30s = peak_30s.map(|peak| (peak - input.current_live_gap).max(0.0));
    let drop_60s = peak_60s.map(|peak| (peak - input.current_live_gap).max(0.0));
    let fresh_peak = no_reversal_local_path_peak_gap_since(
        &recent,
        now_ms,
        config.local_path_fresh_retrace_window_ms,
    );
    let fresh_drop = fresh_peak.map(|peak| (peak - input.current_live_gap).max(0.0));
    let emergency_gap =
        ptb_floor.abs() * (1.0 + config.profile_missing_emergency_margin_floor_ratio);
    let max_drop = ptb_floor.abs() * 0.75;
    let hard_negative_slope = -(ptb_floor.abs() * 0.25).max(0.000001);

    if let Some(obj) = payload.as_object_mut() {
        obj.insert("local_path_history_ms".to_string(), json!(history_age_ms));
        obj.insert("local_path_sample_count".to_string(), json!(recent.len()));
        obj.insert(
            "largest_sample_gap_ms".to_string(),
            json!(largest_sample_gap_ms),
        );
        obj.insert(
            "local_path_fallback_source".to_string(),
            json!(no_reversal_local_path_fallback_source(
                config.local_path_lookback_ms
            )),
        );
        obj.insert("local_path_min_gap".to_string(), json!(min_gap));
        obj.insert("local_path_min_gap_30s".to_string(), json!(min_gap_30s));
        obj.insert("local_path_min_gap_60s".to_string(), json!(min_gap_60s));
        obj.insert("local_path_min_gap_2m".to_string(), json!(min_gap_2m));
        obj.insert("local_path_peak_30s_gap".to_string(), json!(peak_30s));
        obj.insert("local_path_drop_10s".to_string(), json!(drop_10s));
        obj.insert("local_path_drop_30s".to_string(), json!(drop_30s));
        obj.insert("local_path_drop_60s".to_string(), json!(drop_60s));
        obj.insert("local_path_slope_3s".to_string(), json!(slope_3s));
        obj.insert("local_path_slope_10s".to_string(), json!(slope_10s));
        obj.insert("local_path_slope_30s".to_string(), json!(slope_30s));
        obj.insert(
            "local_path_required_current_gap".to_string(),
            json!(emergency_gap),
        );
        obj.insert("local_path_max_drop_30s".to_string(), json!(max_drop));
        obj.insert(
            "local_path_fresh_retrace_window_ms".to_string(),
            json!(config.local_path_fresh_retrace_window_ms),
        );
        obj.insert(
            "local_path_fresh_max_drop_usd".to_string(),
            json!(config.local_path_fresh_max_drop_usd),
        );
        obj.insert(
            "local_path_fresh_min_history_ms".to_string(),
            json!(config.local_path_fresh_min_history_ms),
        );
        obj.insert("local_path_fresh_peak_gap".to_string(), json!(fresh_peak));
        obj.insert("local_path_fresh_drop".to_string(), json!(fresh_drop));
    }

    let fresh_floor_touch =
        config.local_path_gate_mode == NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH;
    let reason = if fresh_floor_touch {
        if history_age_ms < config.local_path_fresh_min_history_ms || fresh_peak.is_none() {
            Some("local_path_fresh_history_insufficient")
        } else if input.current_live_gap < ptb_floor {
            Some("local_path_current_gap_below_floor")
        } else if fresh_drop.is_some_and(|drop| drop > config.local_path_fresh_max_drop_usd) {
            Some("local_path_fresh_retrace_too_high")
        } else {
            None
        }
    } else {
        if history_age_ms < config.local_path_min_history_ms || min_gap.is_none() {
            Some("local_path_history_insufficient")
        } else if min_gap.is_some_and(|gap| gap < ptb_floor) {
            Some("local_path_floor_breached")
        } else if config.profile_missing_emergency_margin_enabled
            && input.current_live_gap < emergency_gap
        {
            Some("no_reversal_profile_missing_margin_too_low")
        } else if drop_30s.is_some_and(|drop| drop > max_drop) {
            Some("local_path_drop_too_high")
        } else if slope_3s.is_some_and(|slope| slope < hard_negative_slope) {
            Some("local_path_slope_negative")
        } else {
            None
        }
    };
    if let Some(reason) = reason {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("local_path_decision_reason".to_string(), json!(reason));
        }
        if config.block_if_profile_missing_and_local_path_insufficient {
            return no_reversal_block(reason, payload);
        }
        return no_reversal_unapplied_decision(config, reason, payload);
    }
    let pass_reason = if fresh_floor_touch {
        "local_path_fresh_floor_touch"
    } else {
        "local_path_safe_fallback"
    };
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("decision".to_string(), json!("pass"));
        obj.insert("reason".to_string(), json!(pass_reason));
        obj.insert("reason_code".to_string(), json!(pass_reason));
        obj.insert(
            "local_path_decision_reason".to_string(),
            json!(pass_reason),
        );
        obj.insert("protection".to_string(), json!("local_path_applied"));
    }
    NoReversalEntryGuardDecision {
        passed: true,
        reason_code: pass_reason,
        payload,
    }
}
