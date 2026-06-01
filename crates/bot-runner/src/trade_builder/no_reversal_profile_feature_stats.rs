const NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES: &str = "materialized_adverse_features";
const NO_REVERSAL_FEATURE_REFRESH_COOLDOWN_MS: i64 = 60_000;
const NO_REVERSAL_FEATURE_REFRESH_LOOKBACK_HOURS: i64 = 6;
const NO_REVERSAL_FEATURE_REFRESH_SAFETY_DELAY_SEC: i64 = 90;

static NO_REVERSAL_ADVERSE_FEATURE_REFRESH_LAST_MS: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone)]
struct NoReversalAdverseFeatureRefreshTelemetry {
    status: String,
    rows_affected: Option<u64>,
    refresh_ms: Option<i64>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct NoReversalFeatureStatsLookup {
    stats_by_fallback: Vec<(NoReversalFallbackLevel, Vec<NoReversalLookbackStat>)>,
    bulk_query_ms: i64,
}

async fn no_reversal_maybe_refresh_recent_adverse_features(
    repo: &PostgresRepository,
    asset: &str,
    now: DateTime<Utc>,
) -> NoReversalAdverseFeatureRefreshTelemetry {
    let asset = asset.trim().to_ascii_lowercase();
    if asset.is_empty() {
        return NoReversalAdverseFeatureRefreshTelemetry {
            status: "skipped_empty_asset".to_string(),
            rows_affected: None,
            refresh_ms: None,
            error: None,
        };
    }

    let now_ms = now.timestamp_millis();
    {
        let mut last_refresh = NO_REVERSAL_ADVERSE_FEATURE_REFRESH_LAST_MS
            .lock()
            .expect("no-reversal adverse feature refresh throttles");
        if let Some(last_ms) = last_refresh.get(&asset) {
            if now_ms - *last_ms < NO_REVERSAL_FEATURE_REFRESH_COOLDOWN_MS {
                return NoReversalAdverseFeatureRefreshTelemetry {
                    status: "cooldown".to_string(),
                    rows_affected: None,
                    refresh_ms: None,
                    error: None,
                };
            }
        }
        last_refresh.insert(asset.clone(), now_ms);
        if last_refresh.len() > 32 {
            let cutoff_ms = now_ms - 10 * NO_REVERSAL_FEATURE_REFRESH_COOLDOWN_MS;
            last_refresh.retain(|_, last_ms| *last_ms >= cutoff_ms);
        }
    }

    let until = now - ChronoDuration::seconds(NO_REVERSAL_FEATURE_REFRESH_SAFETY_DELAY_SEC);
    let since = until - ChronoDuration::hours(NO_REVERSAL_FEATURE_REFRESH_LOOKBACK_HOURS);
    let started = Instant::now();
    match repo
        .refresh_no_reversal_adverse_features(&asset, since, until)
        .await
    {
        Ok(rows_affected) => NoReversalAdverseFeatureRefreshTelemetry {
            status: "refreshed".to_string(),
            rows_affected: Some(rows_affected),
            refresh_ms: Some(started.elapsed().as_millis().min(i64::MAX as u128) as i64),
            error: None,
        },
        Err(err) => NoReversalAdverseFeatureRefreshTelemetry {
            status: "refresh_error".to_string(),
            rows_affected: None,
            refresh_ms: Some(started.elapsed().as_millis().min(i64::MAX as u128) as i64),
            error: Some(err.to_string()),
        },
    }
}

fn no_reversal_feature_refresh_event_payload(
    refresh: &NoReversalAdverseFeatureRefreshTelemetry,
) -> Value {
    json!({
        "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
        "feature_refresh_status": refresh.status,
        "feature_refresh_rows_affected": refresh.rows_affected,
        "feature_refresh_ms": refresh.refresh_ms,
        "feature_refresh_error": refresh.error,
    })
}

fn no_reversal_merge_json_object(target: &mut Value, fields: &Value) {
    if let (Some(target), Some(fields)) = (target.as_object_mut(), fields.as_object()) {
        for (key, value) in fields {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn no_reversal_bulk_feature_lookback_query(
    query_profile: &NoReversalProfileQuery,
    window: NoReversalLookbackWindow,
    fallback_level: NoReversalFallbackLevel,
    historical_until: DateTime<Utc>,
) -> bot_infra::db::TradeBuilderAdverseMoveStatsBulkLookbackQuery {
    let gap_filter = (fallback_level != NoReversalFallbackLevel::GapRelaxed)
        .then_some((query_profile.gap_bucket.min, query_profile.gap_bucket.max));
    let slope_filter = (fallback_level == NoReversalFallbackLevel::Exact)
        .then(|| query_profile.slope_bucket.clone());
    bot_infra::db::TradeBuilderAdverseMoveStatsBulkLookbackQuery {
        fallback_level: no_reversal_fallback_label(fallback_level).to_string(),
        lookback_name: window.name.to_string(),
        hours: window.hours,
        min_samples: window.min_samples,
        min_markets: window.min_markets,
        since: historical_until - ChronoDuration::hours(window.hours),
        until: historical_until,
        gap_min: gap_filter.map(|(min, _)| min),
        gap_max: gap_filter.map(|(_, max)| max),
        slope_bucket: slope_filter,
        quantile: query_profile.quantile,
    }
}

async fn no_reversal_stats_by_fallback_from_features(
    repo: &PostgresRepository,
    query_profile: &NoReversalProfileQuery,
    now: DateTime<Utc>,
) -> Result<NoReversalFeatureStatsLookup> {
    let historical_until = query_profile.target_window_start.unwrap_or(now);
    let fallback_levels = [
        NoReversalFallbackLevel::Exact,
        NoReversalFallbackLevel::SlopeRelaxed,
        NoReversalFallbackLevel::GapRelaxed,
    ];
    let mut lookbacks = Vec::with_capacity(fallback_levels.len() * NO_REVERSAL_LOOKBACK_WINDOWS.len());
    for fallback in fallback_levels {
        for window in NO_REVERSAL_LOOKBACK_WINDOWS {
            lookbacks.push(no_reversal_bulk_feature_lookback_query(
                query_profile,
                window,
                fallback,
                historical_until,
            ));
        }
    }

    let started = Instant::now();
    let rows = repo
        .trade_builder_adverse_move_stats_bulk_from_features(
            &bot_infra::db::TradeBuilderAdverseMoveStatsBulkFromFeaturesQuery {
                asset: query_profile.asset.clone(),
                direction: query_profile.direction.clone(),
                current_market_slug: query_profile.market_slug.clone(),
                remaining_min_sec: query_profile.remaining_bucket.min,
                remaining_max_sec: query_profile.remaining_bucket.max,
                price_min: query_profile.price_bucket.min,
                price_max: query_profile.price_bucket.max,
                lookbacks,
            },
        )
        .await?;
    let bulk_query_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;

    let mut stats_by_fallback = Vec::with_capacity(fallback_levels.len());
    for fallback in fallback_levels {
        let fallback_label = no_reversal_fallback_label(fallback);
        let mut stats = Vec::with_capacity(NO_REVERSAL_LOOKBACK_WINDOWS.len());
        for window in NO_REVERSAL_LOOKBACK_WINDOWS {
            let row = rows
                .iter()
                .find(|row| row.fallback_level == fallback_label && row.lookback_name == window.name);
            let adverse_quantile = row.and_then(|row| row.adverse_quantile);
            let sample_count = row.map(|row| row.sample_count).unwrap_or(0);
            let market_count = row.map(|row| row.market_count).unwrap_or(0);
            let valid = sample_count >= window.min_samples
                && market_count >= window.min_markets
                && adverse_quantile.is_some_and(|value| value.is_finite());
            stats.push(NoReversalLookbackStat {
                name: window.name,
                hours: window.hours,
                min_samples: window.min_samples,
                min_markets: window.min_markets,
                adverse_quantile,
                sample_count,
                market_count,
                valid,
            });
        }
        stats_by_fallback.push((fallback, stats));
    }

    Ok(NoReversalFeatureStatsLookup {
        stats_by_fallback,
        bulk_query_ms,
    })
}
