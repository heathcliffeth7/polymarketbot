const NO_REVERSAL_PROFILE_KEYSPACE_MAX_NEARBY_QUERIES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NoReversalProfilePrewarmPriority {
    ExactCurrent,
    Nearby,
    Background,
}

impl NoReversalProfilePrewarmPriority {
    fn label(self) -> &'static str {
        match self {
            Self::ExactCurrent => "exact_current",
            Self::Nearby => "nearby",
            Self::Background => "background",
        }
    }
}

#[derive(Debug, Clone)]
struct NoReversalProfileKeyspaceCandidate {
    query: NoReversalProfileQuery,
    priority: NoReversalProfilePrewarmPriority,
}

#[derive(Debug, Clone)]
struct NoReversalProfileKeyspaceInput {
    market_slug: String,
    target_window_start: Option<DateTime<Utc>>,
    definition_id: i64,
    node_key: String,
    profile_config_hash: String,
    asset: String,
    direction: String,
    current_remaining_sec: i64,
    current_best_ask: f64,
    current_live_gap: f64,
    current_slope_bucket: String,
}

fn no_reversal_keyspace_gap_buckets(input: &NoReversalProfileKeyspaceInput) -> Vec<NoReversalBucket> {
    let mut seen = BTreeSet::new();
    [input.current_live_gap, input.current_live_gap - 5.0, input.current_live_gap + 5.0]
        .into_iter()
        .map(no_reversal_gap_bucket)
        .filter(|bucket| seen.insert(bucket.label.clone()))
        .collect()
}

fn no_reversal_keyspace_slope_buckets(input: &NoReversalProfileKeyspaceInput) -> Vec<String> {
    let current = input.current_slope_bucket.trim().to_ascii_lowercase();
    let opposite = if current == "negative" {
        "non_negative"
    } else {
        "negative"
    };
    let buckets = vec![current, opposite.to_string()];
    let mut seen = BTreeSet::new();
    buckets
        .into_iter()
        .filter(|bucket| !bucket.is_empty())
        .filter(|bucket| seen.insert(bucket.clone()))
        .collect()
}

fn no_reversal_keyspace_high_late(input: &NoReversalProfileKeyspaceInput) -> bool {
    input.current_best_ask >= 0.90 || input.current_remaining_sec <= 30
}

fn no_reversal_keyspace_is_high_late_near(input: &NoReversalProfileKeyspaceInput) -> bool {
    input.current_best_ask >= 0.88 || input.current_remaining_sec <= 45
}

fn no_reversal_profile_keyspace_query_key(query: &NoReversalProfileQuery) -> String {
    format!(
        "{}:{}:{}:{}:{:.3}:{}",
        query.remaining_bucket.label,
        query.price_bucket.label,
        query.gap_bucket.label,
        query.slope_bucket,
        query.quantile,
        query.high_late
    )
}

fn no_reversal_profile_keyspace_query(
    input: &NoReversalProfileKeyspaceInput,
    gap_bucket: NoReversalBucket,
    slope_bucket: String,
    quantile: f64,
    high_late: bool,
) -> NoReversalProfileQuery {
    NoReversalProfileQuery {
        market_slug: input.market_slug.clone(),
        target_window_start: input.target_window_start,
        definition_id: input.definition_id,
        node_key: input.node_key.clone(),
        profile_config_hash: input.profile_config_hash.clone(),
        asset: input.asset.clone(),
        direction: input.direction.clone(),
        slope_bucket,
        remaining_bucket: no_reversal_remaining_bucket(input.current_remaining_sec),
        price_bucket: no_reversal_price_bucket(input.current_best_ask),
        gap_bucket,
        quantile,
        high_late,
    }
}

fn no_reversal_profile_keyspace_candidates(
    input: &NoReversalProfileKeyspaceInput,
) -> Vec<NoReversalProfileKeyspaceCandidate> {
    let high_late = no_reversal_keyspace_high_late(input);
    let exact_quantile = if high_late { 0.98 } else { 0.95 };
    let exact = no_reversal_profile_keyspace_query(
        input,
        no_reversal_gap_bucket(input.current_live_gap),
        input.current_slope_bucket.trim().to_ascii_lowercase(),
        exact_quantile,
        high_late,
    );
    let mut candidates = vec![NoReversalProfileKeyspaceCandidate {
        query: exact,
        priority: NoReversalProfilePrewarmPriority::ExactCurrent,
    }];
    let mut seen = BTreeSet::new();
    seen.insert(no_reversal_profile_keyspace_query_key(&candidates[0].query));

    let mut quantiles = vec![(exact_quantile, high_late)];
    if no_reversal_keyspace_is_high_late_near(input) && !high_late {
        quantiles.push((0.98, true));
    }
    for gap_bucket in no_reversal_keyspace_gap_buckets(input) {
        for slope_bucket in no_reversal_keyspace_slope_buckets(input) {
            for (quantile, high_late) in &quantiles {
                let query = no_reversal_profile_keyspace_query(
                    input,
                    gap_bucket.clone(),
                    slope_bucket.clone(),
                    *quantile,
                    *high_late,
                );
                if seen.insert(no_reversal_profile_keyspace_query_key(&query)) {
                    candidates.push(NoReversalProfileKeyspaceCandidate {
                        query,
                        priority: NoReversalProfilePrewarmPriority::Nearby,
                    });
                    if candidates.len() > NO_REVERSAL_PROFILE_KEYSPACE_MAX_NEARBY_QUERIES {
                        return candidates;
                    }
                }
            }
        }
    }

    candidates
}

#[cfg(test)]
fn no_reversal_profile_keyspace_queries(
    input: &NoReversalProfileKeyspaceInput,
) -> Vec<NoReversalProfileQuery> {
    no_reversal_profile_keyspace_candidates(input)
        .into_iter()
        .map(|candidate| candidate.query)
        .collect()
}
