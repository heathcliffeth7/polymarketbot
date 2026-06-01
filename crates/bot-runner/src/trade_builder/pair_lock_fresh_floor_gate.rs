const PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_RETRACE_WINDOW_MS: i64 = 10_000;
const PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_MAX_DROP_USD: f64 = 5.0;
const PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_MIN_HISTORY_MS: i64 = 1_000;
const PAIR_LOCK_FRESH_FLOOR_GATE_RETENTION_FLOOR_MS: i64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PairLockFreshFloorGateConfig {
    enabled: bool,
    mode: &'static str,
    retrace_window_ms: i64,
    max_drop_usd: f64,
    min_history_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PairLockFreshFloorGateSample {
    ts_ms: i64,
    directional_gap: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct PairLockFreshFloorGateDecision {
    passed: bool,
    reason_code: &'static str,
    payload: Value,
}

static PAIR_LOCK_FRESH_FLOOR_GATE_HISTORY: LazyLock<
    StdMutex<HashMap<String, VecDeque<PairLockFreshFloorGateSample>>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

fn resolve_pair_lock_fresh_floor_gate_config(
    node: &TradeFlowNode,
) -> PairLockFreshFloorGateConfig {
    let mode_string = node_config_string(node, "noReversalLocalPathGateMode");
    let mode = no_reversal_local_path_gate_mode_from_value(mode_string.as_deref());
    PairLockFreshFloorGateConfig {
        enabled: mode == NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH,
        mode,
        retrace_window_ms: node_config_i64(node, "noReversalLocalPathFreshRetraceWindowMs")
            .unwrap_or(PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_RETRACE_WINDOW_MS)
            .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
        max_drop_usd: node_config_f64(node, "noReversalLocalPathFreshMaxDropUsd")
            .unwrap_or(PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_MAX_DROP_USD)
            .max(0.0),
        min_history_ms: node_config_i64(node, "noReversalLocalPathFreshMinHistoryMs")
            .unwrap_or(PAIR_LOCK_FRESH_FLOOR_GATE_DEFAULT_MIN_HISTORY_MS)
            .clamp(0, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
    }
}

fn pair_lock_fresh_floor_gate_key(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
) -> String {
    format!(
        "{market_slug}:{token_id}:{}",
        outcome_label.trim().to_ascii_lowercase()
    )
}

fn pair_lock_fresh_floor_gate_retention_ms(config: &PairLockFreshFloorGateConfig) -> i64 {
    config
        .retrace_window_ms
        .max(config.min_history_ms)
        .max(PAIR_LOCK_FRESH_FLOOR_GATE_RETENTION_FLOOR_MS)
        .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS)
}

fn record_pair_lock_fresh_floor_gate_sample(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    sample: PairLockFreshFloorGateSample,
    retention_ms: i64,
) {
    if !sample.directional_gap.is_finite() {
        return;
    }
    let key = pair_lock_fresh_floor_gate_key(market_slug, token_id, outcome_label);
    let mut history = PAIR_LOCK_FRESH_FLOOR_GATE_HISTORY
        .lock()
        .expect("pair lock fresh floor gate history");
    let bucket = history.entry(key).or_default();
    bucket.push_back(sample);
    while bucket
        .front()
        .is_some_and(|oldest| sample.ts_ms - oldest.ts_ms > retention_ms)
    {
        bucket.pop_front();
    }
    if history.len() > 512 {
        let cutoff_ms =
            sample.ts_ms - retention_ms.max(PAIR_LOCK_FRESH_FLOOR_GATE_RETENTION_FLOOR_MS);
        history.retain(|_, samples| samples.back().is_some_and(|last| last.ts_ms >= cutoff_ms));
    }
}

fn pair_lock_fresh_floor_gate_recent_samples(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    now_ms: i64,
    window_ms: i64,
) -> Vec<PairLockFreshFloorGateSample> {
    let key = pair_lock_fresh_floor_gate_key(market_slug, token_id, outcome_label);
    PAIR_LOCK_FRESH_FLOOR_GATE_HISTORY
        .lock()
        .expect("pair lock fresh floor gate history")
        .get(&key)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|sample| now_ms - sample.ts_ms <= window_ms)
        .collect()
}

fn pair_lock_fresh_floor_gate_reason_detail(
    reason_code: &'static str,
    current_gap: f64,
    floor_usd: f64,
    peak_gap: Option<f64>,
    drop_usd: Option<f64>,
    config: &PairLockFreshFloorGateConfig,
) -> String {
    match reason_code {
        "local_path_fresh_history_insufficient" => format!(
            "fresh floor touch icin local path gecmisi yetersiz; en az {}ms gerekiyor.",
            config.min_history_ms
        ),
        "local_path_current_gap_below_floor" => format!(
            "current gap {current_gap:.8} USD, floor {floor_usd:.8} USD altinda."
        ),
        "local_path_fresh_retrace_too_high" => format!(
            "son {}ms peak-current drop {:.8} USD; izin verilen maksimum {:.8} USD.",
            config.retrace_window_ms,
            drop_usd.unwrap_or_default(),
            config.max_drop_usd
        ),
        _ => format!(
            "fresh floor touch gecti; current gap {current_gap:.8} USD, peak {:?}.",
            peak_gap
        ),
    }
}

fn evaluate_pair_lock_fresh_floor_gate(
    node: &TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    current_gap: Option<f64>,
    floor_usd: f64,
) -> Option<PairLockFreshFloorGateDecision> {
    let config = resolve_pair_lock_fresh_floor_gate_config(node);
    if !config.enabled {
        return None;
    }
    let current_gap = current_gap.filter(|value| value.is_finite())?;
    if !floor_usd.is_finite() {
        return None;
    }
    Some(evaluate_pair_lock_fresh_floor_gate_at(
        &config,
        market_slug,
        token_id,
        outcome_label,
        current_gap,
        floor_usd,
        Utc::now().timestamp_millis(),
    ))
}

fn evaluate_pair_lock_fresh_floor_gate_at(
    config: &PairLockFreshFloorGateConfig,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    current_gap: f64,
    floor_usd: f64,
    now_ms: i64,
) -> PairLockFreshFloorGateDecision {
    record_pair_lock_fresh_floor_gate_sample(
        market_slug,
        token_id,
        outcome_label,
        PairLockFreshFloorGateSample {
            ts_ms: now_ms,
            directional_gap: current_gap,
        },
        pair_lock_fresh_floor_gate_retention_ms(config),
    );
    let recent = pair_lock_fresh_floor_gate_recent_samples(
        market_slug,
        token_id,
        outcome_label,
        now_ms,
        config.retrace_window_ms,
    );
    let first_ts = recent.iter().map(|sample| sample.ts_ms).min();
    let last_ts = recent.iter().map(|sample| sample.ts_ms).max();
    let history_age_ms = first_ts
        .zip(last_ts)
        .map(|(first, last)| (last - first).max(0))
        .unwrap_or(0);
    let peak_gap = recent
        .iter()
        .map(|sample| sample.directional_gap)
        .filter(|value| value.is_finite())
        .max_by(f64::total_cmp);
    let drop_usd = peak_gap.map(|peak| (peak - current_gap).max(0.0));
    let reason_code =
        if history_age_ms < config.min_history_ms || peak_gap.is_none() {
            "local_path_fresh_history_insufficient"
        } else if current_gap < floor_usd {
            "local_path_current_gap_below_floor"
        } else if drop_usd.is_some_and(|drop| drop > config.max_drop_usd) {
            "local_path_fresh_retrace_too_high"
        } else {
            "local_path_fresh_floor_touch"
        };
    let passed = reason_code == "local_path_fresh_floor_touch";
    let reason_detail = pair_lock_fresh_floor_gate_reason_detail(
        reason_code,
        current_gap,
        floor_usd,
        peak_gap,
        drop_usd,
        config,
    );
    let payload = json!({
        "enabled": true,
        "source": "pair_lock_price_to_beat_guard",
        "local_path_gate_mode": config.mode,
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "current_live_gap_usd": current_gap,
        "ptb_floor_usd": floor_usd,
        "local_path_fresh_retrace_window_ms": config.retrace_window_ms,
        "local_path_fresh_max_drop_usd": config.max_drop_usd,
        "local_path_fresh_min_history_ms": config.min_history_ms,
        "local_path_history_ms": history_age_ms,
        "local_path_sample_count": recent.len(),
        "local_path_fresh_peak_gap": peak_gap,
        "local_path_fresh_drop": drop_usd,
        "decision": if passed { "pass" } else { "block" },
        "reason": reason_code,
        "reason_detail": reason_detail,
    });
    PairLockFreshFloorGateDecision {
        passed,
        reason_code,
        payload,
    }
}

#[cfg(test)]
fn reset_pair_lock_fresh_floor_gate_history() {
    PAIR_LOCK_FRESH_FLOOR_GATE_HISTORY
        .lock()
        .expect("pair lock fresh floor gate history")
        .clear();
}

#[cfg(test)]
mod pair_lock_fresh_floor_gate_tests {
    use super::*;

    static PAIR_LOCK_FRESH_FLOOR_GATE_TEST_LOCK: LazyLock<StdMutex<()>> =
        LazyLock::new(|| StdMutex::new(()));

    fn config() -> PairLockFreshFloorGateConfig {
        PairLockFreshFloorGateConfig {
            enabled: true,
            mode: NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH,
            retrace_window_ms: 10_000,
            max_drop_usd: 5.0,
            min_history_ms: 1_000,
        }
    }

    fn seed(ts_ms: i64, gap: f64) {
        record_pair_lock_fresh_floor_gate_sample(
            "pair97-market",
            "yes",
            "Up",
            PairLockFreshFloorGateSample {
                ts_ms,
                directional_gap: gap,
            },
            30_000,
        );
    }

    #[test]
    fn pair_lock_fresh_floor_allows_first_upward_touch() {
        let _guard = PAIR_LOCK_FRESH_FLOOR_GATE_TEST_LOCK
            .lock()
            .expect("pair lock fresh floor gate test lock");
        reset_pair_lock_fresh_floor_gate_history();
        seed(1_000, 19.0);

        let decision = evaluate_pair_lock_fresh_floor_gate_at(
            &config(),
            "pair97-market",
            "yes",
            "Up",
            20.0,
            20.0,
            2_000,
        );

        assert!(decision.passed);
        assert_eq!(decision.reason_code, "local_path_fresh_floor_touch");
    }

    #[test]
    fn pair_lock_fresh_floor_allows_small_pullback_to_floor() {
        let _guard = PAIR_LOCK_FRESH_FLOOR_GATE_TEST_LOCK
            .lock()
            .expect("pair lock fresh floor gate test lock");
        reset_pair_lock_fresh_floor_gate_history();
        seed(1_000, 21.0);

        let decision = evaluate_pair_lock_fresh_floor_gate_at(
            &config(),
            "pair97-market",
            "yes",
            "Up",
            20.0,
            20.0,
            2_000,
        );

        assert!(decision.passed);
        assert_eq!(decision.reason_code, "local_path_fresh_floor_touch");
    }

    #[test]
    fn pair_lock_fresh_floor_blocks_large_recent_retrace() {
        let _guard = PAIR_LOCK_FRESH_FLOOR_GATE_TEST_LOCK
            .lock()
            .expect("pair lock fresh floor gate test lock");
        reset_pair_lock_fresh_floor_gate_history();
        seed(1_000, 30.0);

        let decision = evaluate_pair_lock_fresh_floor_gate_at(
            &config(),
            "pair97-market",
            "yes",
            "Up",
            20.0,
            20.0,
            11_000,
        );

        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "local_path_fresh_retrace_too_high");
    }

    #[test]
    fn pair_lock_fresh_floor_blocks_below_floor() {
        let _guard = PAIR_LOCK_FRESH_FLOOR_GATE_TEST_LOCK
            .lock()
            .expect("pair lock fresh floor gate test lock");
        reset_pair_lock_fresh_floor_gate_history();
        seed(1_000, 20.5);

        let decision = evaluate_pair_lock_fresh_floor_gate_at(
            &config(),
            "pair97-market",
            "yes",
            "Up",
            19.99,
            20.0,
            2_000,
        );

        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "local_path_current_gap_below_floor");
    }
}
