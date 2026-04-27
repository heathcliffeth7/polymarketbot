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
    fn from_graph_and_run_context(graph_context: Option<&Value>, run_context: &Value) -> Self {
        let settings = graph_context
            .and_then(auto_tune_settings)
            .or_else(|| auto_tune_settings(run_context));
        let enabled = value_bool(settings, "enabled")
            .or_else(|| {
                graph_context.and_then(|context| {
                    auto_tune_legacy_value(context, "autoTuneEnabled").and_then(Value::as_bool)
                })
            })
            .or_else(|| {
                auto_tune_legacy_value(run_context, "autoTuneEnabled").and_then(Value::as_bool)
            })
            .unwrap_or(false);
        let mode = value_string(settings, "mode")
            .or_else(|| {
                graph_context
                    .and_then(|context| auto_tune_legacy_value(context, "autoTuneMode"))
                    .or_else(|| auto_tune_legacy_value(run_context, "autoTuneMode"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
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
            .unwrap_or(DEFAULT_SAMPLE_MARKETS)
            .max(1);
        let min_eligible_markets = value_usize(settings, "minEligibleMarkets")
            .unwrap_or(DEFAULT_MIN_ELIGIBLE_MARKETS)
            .min(sample_markets)
            .max(1);
        let cooldown_markets_after_advice = value_usize(settings, "cooldownMarketsAfterAdvice")
            .or_else(|| value_usize(settings, "cooldownMarketsAfterChange"))
            .unwrap_or(DEFAULT_COOLDOWN_MARKETS_AFTER_ADVICE);
        let dedupe_same_advice_for_markets = value_usize(settings, "dedupeSameAdviceForMarkets")
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
    let value = settings?.get(key)?;
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
