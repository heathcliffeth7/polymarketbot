const NO_REVERSAL_LOCAL_PATH_DEFAULT_WORKFLOW_LOOKBACK_MS: i64 = 300_000;
const NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS: i64 = 300_000;
const NO_REVERSAL_LOCAL_PATH_GATE_MODE_CLEAN_FLOOR: &str = "clean_floor";
const NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH: &str = "fresh_floor_touch";

fn no_reversal_local_path_gate_mode_from_value(value: Option<&str>) -> &'static str {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(NO_REVERSAL_LOCAL_PATH_GATE_MODE_CLEAN_FLOOR)
    {
        NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH => {
            NO_REVERSAL_LOCAL_PATH_GATE_MODE_FRESH_FLOOR_TOUCH
        }
        _ => NO_REVERSAL_LOCAL_PATH_GATE_MODE_CLEAN_FLOOR,
    }
}

fn live_gap_collector_config_with_workflow_local_path(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    step: &TradeFlowRunStep,
) -> ActionPlaceOrderLiveGapCollectorConfig {
    let (lookback_ms, source) =
        no_reversal_workflow_local_path_lookback_from_input(step.input_json.as_ref());
    live_gap_collector_config_with_local_path_lookback(config, lookback_ms, source)
}

fn live_gap_collector_config_with_trigger_cycle_window(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
) -> ActionPlaceOrderLiveGapCollectorConfig {
    let (lookback_ms, source) = no_reversal_workflow_local_path_lookback(
        cycle_window_mode,
        cycle_window_secs,
        cycle_window_start_sec,
        cycle_window_end_sec,
    );
    live_gap_collector_config_with_local_path_lookback(config, lookback_ms, source)
}

fn live_gap_collector_config_with_local_path_lookback(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    lookback_ms: i64,
    source: &'static str,
) -> ActionPlaceOrderLiveGapCollectorConfig {
    let mut next = config.clone();
    next.no_reversal_entry_guard = no_reversal_entry_guard_config_with_local_path_lookback(
        &config.no_reversal_entry_guard,
        lookback_ms,
        source,
    );
    next.live_gap_history_retention_ms = next
        .live_gap_history_retention_ms
        .max(next.no_reversal_entry_guard.local_path_lookback_ms)
        .clamp(
            PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS,
            NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS,
        );
    next
}

fn no_reversal_entry_guard_config_with_local_path_lookback(
    config: &NoReversalEntryGuardConfig,
    lookback_ms: i64,
    source: &'static str,
) -> NoReversalEntryGuardConfig {
    let mut next = config.clone();
    next.local_path_lookback_ms = lookback_ms;
    next.local_path_lookback_source = source.to_string();
    next.local_path_min_history_ms = next.local_path_min_history_ms.min(lookback_ms).max(1_000);
    next
}

fn no_reversal_workflow_local_path_lookback_from_input(
    input: Option<&Value>,
) -> (i64, &'static str) {
    no_reversal_workflow_local_path_lookback(
        no_reversal_local_path_window_mode(input),
        no_reversal_local_path_input_i64(input, &["cycleWindowSecs", "cycle_window_secs"]),
        no_reversal_local_path_input_i64(input, &["cycleWindowStartSec", "cycle_window_start_sec"]),
        no_reversal_local_path_input_i64(input, &["cycleWindowEndSec", "cycle_window_end_sec"]),
    )
}

fn no_reversal_workflow_local_path_lookback(
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
) -> (i64, &'static str) {
    let is_custom = cycle_window_mode
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("custom_range"));
    if is_custom {
        if let (Some(start_sec), Some(end_sec)) = (cycle_window_start_sec, cycle_window_end_sec) {
            if end_sec > start_sec {
                return (
                    ((end_sec - start_sec) * 1_000)
                        .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
                    "trigger_custom_range",
                );
            }
        }
        if let Some(secs) = cycle_window_secs {
            return (
                (secs * 1_000).clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
                "trigger_custom_range",
            );
        }
    }
    (
        NO_REVERSAL_LOCAL_PATH_DEFAULT_WORKFLOW_LOOKBACK_MS,
        "default_workflow_5m",
    )
}

fn no_reversal_local_path_window_mode(input: Option<&Value>) -> Option<&str> {
    ["windowBoundaryMode", "cycleWindowMode", "cycle_window_mode"]
        .iter()
        .filter_map(|key| input.and_then(|value| value.get(*key)).and_then(Value::as_str))
        .next()
}

fn no_reversal_local_path_input_i64(input: Option<&Value>, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .filter_map(|key| input.and_then(|value| value.get(*key)).and_then(value_as_i64))
        .next()
}

#[cfg(test)]
mod no_reversal_local_path_window_tests {
    use super::*;

    #[test]
    fn local_path_uses_custom_range_duration() {
        let input = json!({
            "windowBoundaryMode": "custom_range",
            "cycleWindowStartSec": 230,
            "cycleWindowEndSec": 290,
        });

        assert_eq!(
            no_reversal_workflow_local_path_lookback_from_input(Some(&input)),
            (60_000, "trigger_custom_range")
        );
    }

    #[test]
    fn local_path_defaults_to_full_workflow_when_trigger_has_no_custom_range() {
        let input = json!({
            "windowBoundaryMode": "last",
            "cycleWindowSecs": 45,
        });

        assert_eq!(
            no_reversal_workflow_local_path_lookback_from_input(Some(&input)),
            (300_000, "default_workflow_5m")
        );
    }
}
