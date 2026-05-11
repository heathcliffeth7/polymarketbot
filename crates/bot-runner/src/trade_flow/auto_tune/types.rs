use super::*;

const DEFAULT_SAMPLE_MARKETS: usize = 12;
const DEFAULT_MIN_ELIGIBLE_MARKETS: usize = 8;
const DEFAULT_COOLDOWN_MARKETS_AFTER_ADVICE: usize = 3;
const DEFAULT_DEDUPE_SAME_ADVICE_FOR_MARKETS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoTuneMode {
    Off,
    Advice,
}

#[derive(Debug, Clone)]
struct AutoTuneCaps {
    execution_floor_min_cent: f64,
    execution_floor_max_cent: f64,
    pair_max_total_min_cent: f64,
    pair_max_total_max_cent: f64,
    counter_leg_max_price_min_cent: f64,
    counter_leg_max_price_max_cent: f64,
    iv_rule_max_price_delta_min_cent: f64,
    iv_rule_max_price_delta_max_cent: f64,
    iv_rule_min_edge_delta_min: f64,
    iv_rule_min_edge_delta_max: f64,
    cycle_window_end_sec_max: i64,
    reentry_max_attempts_max: i64,
}

impl Default for AutoTuneCaps {
    fn default() -> Self {
        Self {
            execution_floor_min_cent: 45.0,
            execution_floor_max_cent: 52.0,
            pair_max_total_min_cent: 93.0,
            pair_max_total_max_cent: 96.0,
            counter_leg_max_price_min_cent: 70.0,
            counter_leg_max_price_max_cent: 80.0,
            iv_rule_max_price_delta_min_cent: -2.0,
            iv_rule_max_price_delta_max_cent: 3.0,
            iv_rule_min_edge_delta_min: -0.01,
            iv_rule_min_edge_delta_max: 0.03,
            cycle_window_end_sec_max: 270,
            reentry_max_attempts_max: 1,
        }
    }
}

#[derive(Debug, Clone)]
struct AutoTuneConfig {
    enabled: bool,
    mode: AutoTuneMode,
    sample_markets: usize,
    min_eligible_markets: usize,
    cooldown_markets_after_advice: usize,
    dedupe_same_advice_for_markets: usize,
    caps: AutoTuneCaps,
}

impl AutoTuneConfig {
    fn from_action_graph_and_run_context(
        action_config: Option<&Value>,
        graph_context: Option<&Value>,
        run_context: &Value,
    ) -> Self {
        if let Some(action_config) = action_config {
            if let Some(settings) = auto_tune_settings(action_config) {
                return Self::from_settings_and_legacy(Some(settings), &[action_config]);
            }
            if auto_tune_has_legacy_values(action_config) {
                return Self::from_settings_and_legacy(None, &[action_config]);
            }
        }
        Self::from_graph_and_run_context(graph_context, run_context)
    }

    fn from_graph_and_run_context(graph_context: Option<&Value>, run_context: &Value) -> Self {
        let settings = graph_context
            .and_then(auto_tune_settings)
            .or_else(|| auto_tune_settings(run_context));
        let mut legacy_sources = Vec::new();
        if let Some(context) = graph_context {
            legacy_sources.push(context);
        }
        legacy_sources.push(run_context);
        Self::from_settings_and_legacy(settings, &legacy_sources)
    }

    fn from_settings_and_legacy(settings: Option<&Value>, legacy_sources: &[&Value]) -> Self {
        let enabled = value_bool(settings, "enabled")
            .or_else(|| auto_tune_legacy_bool(legacy_sources, "autoTuneEnabled"))
            .unwrap_or(false);
        let mode = value_string(settings, "mode")
            .or_else(|| auto_tune_legacy_string(legacy_sources, "autoTuneMode"))
            .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                "advice" | "advice_only" => AutoTuneMode::Advice,
                _ => AutoTuneMode::Off,
            })
            .unwrap_or(if enabled {
                AutoTuneMode::Advice
            } else {
                AutoTuneMode::Off
            });
        let sample_markets = value_usize(settings, "sampleMarkets")
            .or_else(|| auto_tune_legacy_usize(legacy_sources, "autoTuneSampleMarkets"))
            .unwrap_or(DEFAULT_SAMPLE_MARKETS)
            .max(1);
        let min_eligible_markets = value_usize(settings, "minEligibleMarkets")
            .or_else(|| auto_tune_legacy_usize(legacy_sources, "autoTuneMinEligibleMarkets"))
            .unwrap_or(DEFAULT_MIN_ELIGIBLE_MARKETS)
            .min(sample_markets)
            .max(1);
        let cooldown_markets_after_advice = value_usize(settings, "cooldownMarketsAfterAdvice")
            .or_else(|| value_usize(settings, "cooldownMarketsAfterChange"))
            .or_else(|| auto_tune_legacy_usize(legacy_sources, "autoTuneCooldownMarketsAfterAdvice"))
            .unwrap_or(DEFAULT_COOLDOWN_MARKETS_AFTER_ADVICE);
        let dedupe_same_advice_for_markets = value_usize(settings, "dedupeSameAdviceForMarkets")
            .or_else(|| auto_tune_legacy_usize(legacy_sources, "autoTuneDedupeSameAdviceForMarkets"))
            .unwrap_or(DEFAULT_DEDUPE_SAME_ADVICE_FOR_MARKETS);
        let caps = AutoTuneCaps::from_settings(settings);
        Self {
            enabled,
            mode,
            sample_markets,
            min_eligible_markets,
            cooldown_markets_after_advice,
            dedupe_same_advice_for_markets,
            caps,
        }
    }

    fn advice_enabled(&self) -> bool {
        self.enabled && self.mode == AutoTuneMode::Advice
    }
}

fn auto_tune_settings(context: &Value) -> Option<&Value> {
    context
        .get("autoTune")
        .or_else(|| context.get("auto_tune"))
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|flow_context| flow_context.get("autoTune"))
        })
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|flow_context| flow_context.get("auto_tune"))
        })
}

fn auto_tune_legacy_value<'a>(context: &'a Value, key: &str) -> Option<&'a Value> {
    context
        .get(key)
        .or_else(|| context.get("flowContext").and_then(|flow_context| flow_context.get(key)))
}

fn auto_tune_has_legacy_values(context: &Value) -> bool {
    [
        "autoTuneEnabled",
        "autoTuneMode",
        "autoTuneSampleMarkets",
        "autoTuneMinEligibleMarkets",
        "autoTuneCooldownMarketsAfterAdvice",
        "autoTuneDedupeSameAdviceForMarkets",
    ]
    .iter()
    .any(|key| auto_tune_legacy_value(context, key).is_some())
}

fn auto_tune_legacy_bool(contexts: &[&Value], key: &str) -> Option<bool> {
    contexts
        .iter()
        .find_map(|context| auto_tune_legacy_value(context, key).and_then(Value::as_bool))
}

fn auto_tune_legacy_string(contexts: &[&Value], key: &str) -> Option<String> {
    contexts.iter().find_map(|context| {
        auto_tune_legacy_value(context, key)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn auto_tune_legacy_usize(contexts: &[&Value], key: &str) -> Option<usize> {
    contexts
        .iter()
        .find_map(|context| auto_tune_legacy_value(context, key).and_then(value_as_usize))
}

impl AutoTuneCaps {
    fn from_settings(settings: Option<&Value>) -> Self {
        let defaults = Self::default();
        let caps = settings.and_then(|value| value.get("hardCaps"));
        Self {
            execution_floor_min_cent: value_f64(caps, "executionFloorPriceCentMin")
                .unwrap_or(defaults.execution_floor_min_cent),
            execution_floor_max_cent: value_f64(caps, "executionFloorPriceCentMax")
                .unwrap_or(defaults.execution_floor_max_cent),
            pair_max_total_min_cent: value_f64(caps, "pairMaxTotalCentMin")
                .unwrap_or(defaults.pair_max_total_min_cent),
            pair_max_total_max_cent: value_f64(caps, "pairMaxTotalCentMax")
                .unwrap_or(defaults.pair_max_total_max_cent),
            counter_leg_max_price_min_cent: value_f64(caps, "counterLegMaxPriceCentMin")
                .unwrap_or(defaults.counter_leg_max_price_min_cent),
            counter_leg_max_price_max_cent: value_f64(caps, "counterLegMaxPriceCentMax")
                .unwrap_or(defaults.counter_leg_max_price_max_cent),
            iv_rule_max_price_delta_min_cent: value_f64(caps, "maxIvRulePriceDeltaCentMin")
                .unwrap_or(defaults.iv_rule_max_price_delta_min_cent),
            iv_rule_max_price_delta_max_cent: value_f64(caps, "maxIvRulePriceDeltaCentMax")
                .unwrap_or(defaults.iv_rule_max_price_delta_max_cent),
            iv_rule_min_edge_delta_min: value_f64(caps, "minEdgeDeltaMin")
                .unwrap_or(defaults.iv_rule_min_edge_delta_min),
            iv_rule_min_edge_delta_max: value_f64(caps, "minEdgeDeltaMax")
                .unwrap_or(defaults.iv_rule_min_edge_delta_max),
            cycle_window_end_sec_max: value_i64(caps, "cycleWindowEndSecMax")
                .unwrap_or(defaults.cycle_window_end_sec_max),
            reentry_max_attempts_max: value_i64(caps, "reentryMaxAttemptsMax")
                .unwrap_or(defaults.reentry_max_attempts_max),
        }
    }
}

#[derive(Debug, Clone)]
struct AutoTuneAdviceDraft {
    advice_kind: String,
    advice_action: String,
    target_key_path: Option<String>,
    current_value_json: Option<Value>,
    suggested_value_json: Option<Value>,
    clamped: bool,
    hard_cap_min_json: Option<Value>,
    hard_cap_max_json: Option<Value>,
    reason_code: String,
    reason_text: String,
    dominant_blocker: Option<String>,
    metrics_json: Value,
}

#[derive(Debug, Clone)]
struct AutoTuneGuardDecision {
    scope: Option<String>,
    code: Option<String>,
    node_key: Option<String>,
    at: Option<DateTime<Utc>>,
}

fn value_bool(settings: Option<&Value>, key: &str) -> Option<bool> {
    settings?.get(key).and_then(Value::as_bool)
}

fn value_string(settings: Option<&Value>, key: &str) -> Option<String> {
    settings?
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn value_usize(settings: Option<&Value>, key: &str) -> Option<usize> {
    value_as_usize(settings?.get(key)?)
}

fn value_as_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| {
            value
                .as_i64()
                .filter(|value| *value >= 0)
                .and_then(|value| usize::try_from(value as u64).ok())
        })
}

fn value_i64(settings: Option<&Value>, key: &str) -> Option<i64> {
    settings?.get(key).and_then(Value::as_i64)
}

fn value_f64(settings: Option<&Value>, key: &str) -> Option<f64> {
    settings?.get(key).and_then(value_as_f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_auto_tune_config_overrides_graph_context() {
        let action_config = serde_json::json!({
            "autoTune": {
                "enabled": false,
                "mode": "advice"
            }
        });
        let graph_context = serde_json::json!({
            "autoTune": {
                "enabled": true,
                "mode": "advice"
            }
        });
        let run_context = serde_json::json!({});

        let cfg = AutoTuneConfig::from_action_graph_and_run_context(
            Some(&action_config),
            Some(&graph_context),
            &run_context,
        );

        assert!(!cfg.advice_enabled());
    }

    #[test]
    fn action_nested_auto_tune_config_sets_advice_window() {
        let action_config = serde_json::json!({
            "autoTune": {
                "enabled": true,
                "mode": "advice",
                "sampleMarkets": 6,
                "minEligibleMarkets": 4,
                "cooldownMarketsAfterAdvice": 2,
                "dedupeSameAdviceForMarkets": 4
            }
        });
        let run_context = serde_json::json!({});

        let cfg = AutoTuneConfig::from_action_graph_and_run_context(
            Some(&action_config),
            None,
            &run_context,
        );

        assert!(cfg.advice_enabled());
        assert_eq!(cfg.sample_markets, 6);
        assert_eq!(cfg.min_eligible_markets, 4);
        assert_eq!(cfg.cooldown_markets_after_advice, 2);
        assert_eq!(cfg.dedupe_same_advice_for_markets, 4);
    }

    #[test]
    fn action_legacy_flat_auto_tune_config_is_supported() {
        let action_config = serde_json::json!({
            "autoTuneEnabled": true,
            "autoTuneMode": "advice",
            "autoTuneSampleMarkets": 3,
            "autoTuneMinEligibleMarkets": 2
        });
        let run_context = serde_json::json!({});

        let cfg = AutoTuneConfig::from_action_graph_and_run_context(
            Some(&action_config),
            None,
            &run_context,
        );

        assert!(cfg.advice_enabled());
        assert_eq!(cfg.sample_markets, 3);
        assert_eq!(cfg.min_eligible_markets, 2);
    }
}
