const PRE_BUY_COLLAPSE_HISTORY_STORE_RESTART_GRACE_MS: i64 = 60_000;

#[derive(Debug, Clone, PartialEq)]
struct PreBuyHistoryContext {
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    node_key: String,
    trigger_node_key: Option<String>,
    market_slug: String,
    token_id: String,
    outcome_label: String,
    asset: String,
    direction: String,
    prewarm_enabled: bool,
    prewarm_start_elapsed_sec: Option<i64>,
    trigger_window_start_sec: Option<i64>,
    action_window_start_sec: Option<i64>,
    action_window_end_sec: Option<i64>,
    sample_ms: i64,
    retention_ms: i64,
    trigger_condition: Option<String>,
    trigger_price: Option<f64>,
    triggered_price: Option<f64>,
    trigger_source: Option<String>,
    registered_at_ms: i64,
}

static PRE_BUY_HISTORY_CONTEXTS: LazyLock<StdMutex<HashMap<String, PreBuyHistoryContext>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn register_pre_buy_history_context(context: PreBuyHistoryContext) {
    let key = pre_buy_collapse_guard_key(
        &context.market_slug,
        &context.token_id,
        &context.outcome_label,
    );
    let mut contexts = PRE_BUY_HISTORY_CONTEXTS
        .lock()
        .expect("pre-buy history contexts");
    contexts.insert(key, context);
    if contexts.len() > 1_024 {
        let cutoff_ms = Utc::now().timestamp_millis() - 90_000;
        contexts.retain(|_, context| context.registered_at_ms >= cutoff_ms);
    }
}

fn pre_buy_history_context(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
) -> Option<PreBuyHistoryContext> {
    let key = pre_buy_collapse_guard_key(market_slug, token_id, outcome_label);
    PRE_BUY_HISTORY_CONTEXTS
        .lock()
        .expect("pre-buy history contexts")
        .get(&key)
        .cloned()
}

fn pre_buy_collapse_history_is_full(metrics: &PreBuyCollapseMetrics) -> bool {
    metrics.gap_1s_available && metrics.gap_3s_available && metrics.gap_5s_available
}

fn pre_buy_collapse_clear_kind(
    reason_code: &'static str,
    metrics: &PreBuyCollapseMetrics,
) -> Option<&'static str> {
    let full_history = pre_buy_collapse_history_is_full(metrics);
    match reason_code {
        "retrace_stabilized" if full_history => Some("retrace_stabilized_full_history"),
        "retrace_stabilized" => Some("retrace_stabilized_short_history"),
        "collapse_guard_clear" if full_history => Some("full_history_clear"),
        "collapse_guard_clear" => Some("short_history_clear"),
        _ => None,
    }
}

fn pre_buy_collapse_missing_reasons(
    input: &PreBuyCollapseGuardInput<'_>,
    config: &PreBuyCollapseGuardConfig,
    metrics: &PreBuyCollapseMetrics,
    history_context: Option<&PreBuyHistoryContext>,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    let add = |reasons: &mut Vec<&'static str>, reason| {
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    };

    if metrics.sample_count <= 1 || metrics.history_age_ms == 0 {
        add(&mut reasons, "history_not_started_yet");
    }
    if metrics.history_store_uptime_ms < PRE_BUY_COLLAPSE_HISTORY_STORE_RESTART_GRACE_MS
        && metrics.history_age_ms < config.history_min_age_ms
    {
        add(&mut reasons, "service_restart_history_reset");
    }
    let trigger_window_start_sec = history_context
        .and_then(|context| context.trigger_window_start_sec)
        .unwrap_or(input.window_start_sec);
    if metrics.history_age_ms < config.history_min_age_ms
        && input.elapsed_sec >= trigger_window_start_sec
    {
        add(&mut reasons, "trigger_armed_late");
    }
    let trigger_condition = input
        .trigger_condition
        .map(str::to_string)
        .or_else(|| history_context.and_then(|context| context.trigger_condition.clone()));
    if trigger_condition
        .as_deref()
        .is_some_and(|condition| condition.trim().eq_ignore_ascii_case("cross_above"))
        && history_context.is_none_or(|context| !context.prewarm_enabled)
        && metrics.history_age_ms < config.history_min_age_ms
    {
        add(&mut reasons, "cross_above_no_prewarm");
    }
    if metrics.history_age_ms < config.history_min_age_ms && metrics.sample_count > 1 {
        add(&mut reasons, "action_started_recently");
    }
    if metrics.sample_count <= 1 && history_context.is_some_and(|context| context.prewarm_enabled)
    {
        add(&mut reasons, "new_market_slug");
    }
    if let Some(context) = history_context {
        if let Some(prewarm_start) = context.prewarm_start_elapsed_sec {
            let expected_age_ms = ((input.elapsed_sec - prewarm_start).max(0)) * 1_000;
            if context.prewarm_enabled
                && expected_age_ms >= config.history_min_age_ms + context.sample_ms
                && metrics
                    .oldest_sample_age_ms
                    .is_some_and(|age| age + context.sample_ms * 4 < expected_age_ms)
            {
                add(&mut reasons, "market_not_resolved_until_late");
            }
        }
    }
    if metrics.history_age_ms >= 1_000 && !metrics.gap_3s_available {
        add(&mut reasons, "retry_loop_short_before_decision");
    }
    let expected_sample_ms = history_context
        .map(|context| context.sample_ms)
        .unwrap_or(input.retry_ms)
        .max(1);
    if metrics
        .largest_sample_gap_ms
        .is_some_and(|gap_ms| gap_ms > expected_sample_ms * 6 && gap_ms > 1_500)
    {
        add(&mut reasons, "sample_source_gap");
    }
    reasons
}

fn pre_buy_collapse_missing_reason_detail(
    primary_reason: Option<&str>,
    input: &PreBuyCollapseGuardInput<'_>,
    metrics: &PreBuyCollapseMetrics,
    history_context: Option<&PreBuyHistoryContext>,
) -> Option<String> {
    let detail = match primary_reason? {
        "history_not_started_yet" => {
            "this is the first sample for this market/outcome".to_string()
        }
        "action_started_recently" => format!(
            "action has only watched this side for {}ms",
            metrics.history_age_ms
        ),
        "trigger_armed_late" => format!(
            "trigger/action armed at elapsed={}s; only {}ms of live-gap history collected",
            input.elapsed_sec, metrics.history_age_ms
        ),
        "new_market_slug" => {
            "rolling history resets per market_slug + token_id + outcome".to_string()
        }
        "service_restart_history_reset" => format!(
            "in-memory rolling history store uptime is {}ms",
            metrics.history_store_uptime_ms
        ),
        "cross_above_no_prewarm" => {
            "history collection began only after cross_above trigger".to_string()
        }
        "retry_loop_short_before_decision" => format!(
            "decision happened with {}ms history; 3s/5s metrics are not fully available",
            metrics.history_age_ms
        ),
        "sample_source_gap" => format!(
            "largest sample gap is {}ms",
            metrics.largest_sample_gap_ms.unwrap_or_default()
        ),
        "market_not_resolved_until_late" => format!(
            "prewarm target resolved after elapsed={}s; expected start was {}s",
            input.elapsed_sec,
            history_context
                .and_then(|context| context.prewarm_start_elapsed_sec)
                .unwrap_or(input.window_start_sec)
        ),
        _ => return None,
    };
    Some(detail)
}
