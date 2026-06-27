use super::iv_cex_open_gap::{CexOpenGapConsensus, PriceToBeatIvCexOpenGapEvaluation};
use super::iv_execution_vwap::PriceToBeatIvExecutionVwapEvaluation;
use serde_json::{json, Map, Value};

const SOURCE_EXECUTION_VWAP: &str = "execution_vwap";
const COMBINE_STRICTEST: &str = "strictest";
const BASE_LANE_MAX_CENT: f64 = 77.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPriceBandGuardConfig {
    pub(crate) enabled: bool,
    pub(crate) requested_source: &'static str,
    pub(crate) combine_mode: &'static str,
    pub(crate) bands: Vec<PriceToBeatIvPriceBand>,
}

impl Default for PriceToBeatIvPriceBandGuardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requested_source: SOURCE_EXECUTION_VWAP,
            combine_mode: COMBINE_STRICTEST,
            bands: default_price_bands(),
        }
    }
}

impl PriceToBeatIvPriceBandGuardConfig {
    pub(crate) fn from_node(node: &crate::TradeFlowNode) -> Self {
        let mut config = Self {
            enabled: crate::node_config_bool(node, "priceToBeatIvPriceBandGuardEnabled")
                .unwrap_or(false),
            ..Self::default()
        };
        config.requested_source =
            match crate::node_config_string(node, "priceToBeatIvPriceBandSource").as_deref() {
                Some(SOURCE_EXECUTION_VWAP) | None => SOURCE_EXECUTION_VWAP,
                Some(_) => SOURCE_EXECUTION_VWAP,
            };
        config.combine_mode =
            match crate::node_config_string(node, "priceToBeatIvPriceBandCombineMode").as_deref() {
                Some(COMBINE_STRICTEST) | None => COMBINE_STRICTEST,
                Some(_) => COMBINE_STRICTEST,
            };
        if let Some(parsed) = node
            .config
            .get("priceToBeatIvPriceBands")
            .and_then(Value::as_array)
            .map(|bands| {
                bands
                    .iter()
                    .filter_map(parse_price_band)
                    .collect::<Vec<_>>()
            })
            .filter(|bands| !bands.is_empty())
        {
            config.bands = parsed;
        }
        config
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPriceBand {
    pub(crate) name: String,
    pub(crate) min_price_cent: f64,
    pub(crate) max_price_cent: f64,
    pub(crate) min_q_cent: Option<f64>,
    pub(crate) min_fair_edge_cent: Option<f64>,
    pub(crate) max_spread_cent: Option<f64>,
    pub(crate) require_clean_cex: bool,
    pub(crate) require_cex_with_direction: bool,
    pub(crate) require_book_confirmation: bool,
    pub(crate) require_no_chainlink_stale_penalty: bool,
    pub(crate) require_no_mixed_cex: bool,
    pub(crate) time_rules: Vec<PriceToBeatIvPriceBandTimeRule>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvPriceBandTimeRule {
    pub(crate) start_remaining_secs: f64,
    pub(crate) end_remaining_secs: f64,
    pub(crate) min_gap_strength: Option<f64>,
    pub(crate) min_q_cent: Option<f64>,
    pub(crate) min_fair_edge_cent: Option<f64>,
    pub(crate) max_spread_cent: Option<f64>,
    pub(crate) require_book_confirmation: Option<bool>,
    pub(crate) require_no_chainlink_stale_penalty: Option<bool>,
    pub(crate) require_no_mixed_cex: Option<bool>,
}

impl PriceToBeatIvPriceBandTimeRule {
    fn matches_seconds_left(self, seconds_left: f64) -> bool {
        seconds_left <= self.start_remaining_secs
            && (seconds_left > self.end_remaining_secs
                || (self.end_remaining_secs == 0.0 && seconds_left >= 0.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPriceBandGuardEvaluation {
    pub(crate) enabled: bool,
    pub(crate) requested_source: &'static str,
    pub(crate) actual_source: Option<&'static str>,
    pub(crate) execution_ref_cent: Option<f64>,
    pub(crate) band_name: Option<String>,
    pub(crate) time_rule_index: Option<usize>,
    pub(crate) required_gap_strength: Option<f64>,
    pub(crate) min_q_cent: Option<f64>,
    pub(crate) q_final_cent: Option<f64>,
    pub(crate) min_fair_edge_cent: Option<f64>,
    pub(crate) fair_edge_cent: Option<f64>,
    pub(crate) max_spread_cent: Option<f64>,
    pub(crate) spread_cent: Option<f64>,
    pub(crate) cex_clean: Option<bool>,
    pub(crate) cex_with_direction: Option<bool>,
    pub(crate) book_confirmation: Option<bool>,
    pub(crate) chainlink_stale_penalty: Option<bool>,
    pub(crate) mixed_cex: Option<bool>,
    pub(crate) result: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) failed_checks: Vec<&'static str>,
}

impl Default for PriceToBeatIvPriceBandGuardEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            requested_source: SOURCE_EXECUTION_VWAP,
            actual_source: None,
            execution_ref_cent: None,
            band_name: None,
            time_rule_index: None,
            required_gap_strength: None,
            min_q_cent: None,
            q_final_cent: None,
            min_fair_edge_cent: None,
            fair_edge_cent: None,
            max_spread_cent: None,
            spread_cent: None,
            cex_clean: None,
            cex_with_direction: None,
            book_confirmation: None,
            chainlink_stale_penalty: None,
            mixed_cex: None,
            result: "disabled",
            block_reason: None,
            failed_checks: Vec::new(),
        }
    }
}

impl PriceToBeatIvPriceBandGuardEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("price_band_guard_enabled".to_string(), json!(self.enabled));
        obj.insert(
            "price_band_requested_source".to_string(),
            json!(self.requested_source),
        );
        obj.insert(
            "price_band_actual_source".to_string(),
            json!(self.actual_source),
        );
        obj.insert(
            "price_band_execution_ref_cent".to_string(),
            json!(self.execution_ref_cent),
        );
        obj.insert("price_band_name".to_string(), json!(self.band_name));
        obj.insert(
            "price_band_time_rule_index".to_string(),
            json!(self.time_rule_index),
        );
        obj.insert(
            "price_band_required_gap_strength".to_string(),
            json!(self.required_gap_strength),
        );
        obj.insert("price_band_min_q_cent".to_string(), json!(self.min_q_cent));
        obj.insert(
            "price_band_q_final_cent".to_string(),
            json!(self.q_final_cent),
        );
        obj.insert(
            "price_band_min_fair_edge_cent".to_string(),
            json!(self.min_fair_edge_cent),
        );
        obj.insert(
            "price_band_fair_edge_cent".to_string(),
            json!(self.fair_edge_cent),
        );
        obj.insert(
            "price_band_max_spread_cent".to_string(),
            json!(self.max_spread_cent),
        );
        obj.insert(
            "price_band_spread_cent".to_string(),
            json!(self.spread_cent),
        );
        obj.insert("price_band_cex_clean".to_string(), json!(self.cex_clean));
        obj.insert(
            "price_band_cex_with_direction".to_string(),
            json!(self.cex_with_direction),
        );
        obj.insert(
            "price_band_book_confirmation".to_string(),
            json!(self.book_confirmation),
        );
        obj.insert(
            "price_band_chainlink_stale_penalty".to_string(),
            json!(self.chainlink_stale_penalty),
        );
        obj.insert("price_band_mixed_cex".to_string(), json!(self.mixed_cex));
        obj.insert("price_band_result".to_string(), json!(self.result));
        obj.insert(
            "price_band_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "price_band_failed_checks".to_string(),
            json!(self.failed_checks),
        );
    }
}

pub(crate) struct PriceToBeatIvPriceBandGuardInput<'a> {
    pub(crate) config: &'a PriceToBeatIvPriceBandGuardConfig,
    pub(crate) seconds_left: f64,
    pub(crate) gap_strength: f64,
    pub(crate) existing_required_gap_strength: f64,
    pub(crate) q_final: Option<f64>,
    pub(crate) spread: Option<f64>,
    pub(crate) execution_vwap: &'a PriceToBeatIvExecutionVwapEvaluation,
    pub(crate) cex_open_gap: &'a PriceToBeatIvCexOpenGapEvaluation,
    pub(crate) book_confirmation: Option<bool>,
    pub(crate) chainlink_stale_penalty: Option<bool>,
}

pub(crate) fn evaluate_price_to_beat_iv_price_band_guard(
    input: PriceToBeatIvPriceBandGuardInput<'_>,
) -> PriceToBeatIvPriceBandGuardEvaluation {
    let mut evaluation = PriceToBeatIvPriceBandGuardEvaluation {
        enabled: input.config.enabled,
        requested_source: input.config.requested_source,
        ..Default::default()
    };
    if !input.config.enabled {
        return evaluation;
    }
    let (actual_source, execution_ref) = execution_reference(input.execution_vwap);
    evaluation.actual_source = actual_source;
    evaluation.execution_ref_cent = execution_ref.map(to_cent);
    let Some(execution_ref) = execution_ref else {
        return block(evaluation, "blocked_price_band_no_reference_price");
    };
    let execution_ref_cent = to_cent(execution_ref);
    if execution_ref_cent <= BASE_LANE_MAX_CENT {
        evaluation.band_name = Some("base_0_77".to_string());
        evaluation.result = "not_applicable_base_lane";
        return evaluation;
    }
    let Some(band) = input.config.bands.iter().find(|band| {
        execution_ref_cent > band.min_price_cent && execution_ref_cent <= band.max_price_cent
    }) else {
        return block(evaluation, "blocked_price_band_no_matching_band");
    };
    evaluation.band_name = Some(band.name.clone());
    let Some((time_rule_index, time_rule)) = band
        .time_rules
        .iter()
        .copied()
        .enumerate()
        .find(|(_, rule)| rule.matches_seconds_left(input.seconds_left))
    else {
        return block(evaluation, "blocked_price_band_no_matching_time_rule");
    };
    evaluation.time_rule_index = Some(time_rule_index);
    let required_gap_strength = input
        .existing_required_gap_strength
        .max(time_rule.min_gap_strength.unwrap_or(0.0));
    let min_q_cent = max_opt(band.min_q_cent, time_rule.min_q_cent);
    let min_fair_edge_cent = max_opt(band.min_fair_edge_cent, time_rule.min_fair_edge_cent);
    let max_spread_cent = min_opt(band.max_spread_cent, time_rule.max_spread_cent);
    let q_final_cent = input.q_final.map(to_cent);
    let fair_edge_cent = q_final_cent.map(|q| q - execution_ref_cent);
    let spread_cent = input.spread.map(to_cent);
    let require_book_confirmation =
        band.require_book_confirmation || time_rule.require_book_confirmation.unwrap_or(false);
    let require_no_chainlink_stale_penalty = band.require_no_chainlink_stale_penalty
        || time_rule
            .require_no_chainlink_stale_penalty
            .unwrap_or(false);
    let require_no_mixed_cex =
        band.require_no_mixed_cex || time_rule.require_no_mixed_cex.unwrap_or(false);
    let cex_clean = cex_clean(input.cex_open_gap);
    let cex_with_direction = cex_with_direction(input.cex_open_gap);
    let mixed_cex = mixed_cex(input.cex_open_gap);
    evaluation.required_gap_strength = Some(required_gap_strength);
    evaluation.min_q_cent = min_q_cent;
    evaluation.q_final_cent = q_final_cent;
    evaluation.min_fair_edge_cent = min_fair_edge_cent;
    evaluation.fair_edge_cent = fair_edge_cent;
    evaluation.max_spread_cent = max_spread_cent;
    evaluation.spread_cent = spread_cent;
    evaluation.cex_clean = cex_clean;
    evaluation.cex_with_direction = cex_with_direction;
    evaluation.book_confirmation = input.book_confirmation;
    evaluation.chainlink_stale_penalty = input.chainlink_stale_penalty;
    evaluation.mixed_cex = mixed_cex;
    if input.gap_strength < required_gap_strength {
        evaluation
            .failed_checks
            .push("blocked_price_band_gap_strength");
    }
    if min_q_cent
        .map(|minimum| q_final_cent.map(|q| q < minimum).unwrap_or(true))
        .unwrap_or(false)
    {
        evaluation
            .failed_checks
            .push("blocked_price_band_q_below_min");
    }
    if min_fair_edge_cent
        .map(|minimum| fair_edge_cent.map(|edge| edge < minimum).unwrap_or(true))
        .unwrap_or(false)
    {
        evaluation
            .failed_checks
            .push("blocked_price_band_fair_edge_below_min");
    }
    if let Some(maximum) = max_spread_cent {
        match spread_cent {
            Some(spread) if spread <= maximum => {}
            Some(_) => evaluation
                .failed_checks
                .push("blocked_price_band_spread_too_wide"),
            None => evaluation
                .failed_checks
                .push("blocked_price_band_spread_missing"),
        }
    }
    if band.require_clean_cex && cex_clean != Some(true) {
        evaluation
            .failed_checks
            .push("blocked_price_band_cex_not_clean");
    }
    if band.require_cex_with_direction && cex_with_direction != Some(true) {
        evaluation
            .failed_checks
            .push("blocked_price_band_cex_not_with_direction");
    }
    if require_book_confirmation && input.book_confirmation != Some(true) {
        evaluation
            .failed_checks
            .push("blocked_price_band_book_confirmation_missing");
    }
    if require_no_chainlink_stale_penalty && input.chainlink_stale_penalty != Some(false) {
        evaluation
            .failed_checks
            .push("blocked_price_band_chainlink_stale_penalty");
    }
    if require_no_mixed_cex && mixed_cex != Some(false) {
        evaluation
            .failed_checks
            .push("blocked_price_band_mixed_cex");
    }
    if let Some(reason) = evaluation.failed_checks.first().copied() {
        return block(evaluation, reason);
    }
    evaluation.result = "pass";
    evaluation
}

fn block(
    mut evaluation: PriceToBeatIvPriceBandGuardEvaluation,
    reason: &'static str,
) -> PriceToBeatIvPriceBandGuardEvaluation {
    if evaluation.failed_checks.is_empty() {
        evaluation.failed_checks.push(reason);
    }
    evaluation.block_reason = Some(reason);
    evaluation.result = "block";
    evaluation
}

fn execution_reference(
    execution_vwap: &PriceToBeatIvExecutionVwapEvaluation,
) -> (Option<&'static str>, Option<f64>) {
    if let Some(value) = valid_probability(execution_vwap.execution_vwap) {
        return (Some("execution_vwap"), Some(value));
    }
    if let Some(value) = valid_probability(execution_vwap.execution_best_ask) {
        return (Some("execution_best_ask"), Some(value));
    }
    if let Some(value) = valid_probability(execution_vwap.model_ask) {
        return (Some("model_ask"), Some(value));
    }
    (None, None)
}

fn valid_probability(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)
}

fn cex_clean(cex: &PriceToBeatIvCexOpenGapEvaluation) -> Option<bool> {
    cex.enabled
        .then_some(cex.clean_lane && cex.consensus == CexOpenGapConsensus::Strong)
}

fn cex_with_direction(cex: &PriceToBeatIvCexOpenGapEvaluation) -> Option<bool> {
    cex.enabled.then_some(matches!(
        cex.consensus,
        CexOpenGapConsensus::Strong | CexOpenGapConsensus::Weak
    ))
}

fn mixed_cex(cex: &PriceToBeatIvCexOpenGapEvaluation) -> Option<bool> {
    cex.enabled.then_some(matches!(
        cex.consensus,
        CexOpenGapConsensus::Mixed
            | CexOpenGapConsensus::Partial
            | CexOpenGapConsensus::Unavailable
    ))
}

fn max_opt(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    left.into_iter().chain(right).max_by(f64::total_cmp)
}

fn min_opt(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    left.into_iter().chain(right).min_by(f64::total_cmp)
}

fn to_cent(value: f64) -> f64 {
    value * 100.0
}

fn parse_price_band(value: &Value) -> Option<PriceToBeatIvPriceBand> {
    let obj = value.as_object()?;
    let min_price_cent = number(obj, &["minPriceCent", "min_price_cent"])?;
    let max_price_cent = number(obj, &["maxPriceCent", "max_price_cent"])?;
    if min_price_cent < 0.0 || max_price_cent <= min_price_cent || max_price_cent > 100.0 {
        return None;
    }
    Some(PriceToBeatIvPriceBand {
        name: obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("custom_price_band")
            .to_string(),
        min_price_cent,
        max_price_cent,
        min_q_cent: optional_non_negative_cent(obj, &["minQCent", "min_q_cent"]),
        min_fair_edge_cent: optional_non_negative_cent(
            obj,
            &["minFairEdgeCent", "min_fair_edge_cent"],
        ),
        max_spread_cent: optional_non_negative_cent(obj, &["maxSpreadCent", "max_spread_cent"]),
        require_clean_cex: bool_field(obj, &["requireCleanCex", "require_clean_cex"])
            .unwrap_or(false),
        require_cex_with_direction: bool_field(
            obj,
            &["requireCexWithDirection", "require_cex_with_direction"],
        )
        .unwrap_or(false),
        require_book_confirmation: bool_field(
            obj,
            &["requireBookConfirmation", "require_book_confirmation"],
        )
        .unwrap_or(false),
        require_no_chainlink_stale_penalty: bool_field(
            obj,
            &[
                "requireNoChainlinkStalePenalty",
                "require_no_chainlink_stale_penalty",
            ],
        )
        .unwrap_or(false),
        require_no_mixed_cex: bool_field(obj, &["requireNoMixedCex", "require_no_mixed_cex"])
            .unwrap_or(false),
        time_rules: obj
            .get("timeRules")
            .or_else(|| obj.get("time_rules"))
            .and_then(Value::as_array)
            .map(|rules| rules.iter().filter_map(parse_time_rule).collect())
            .unwrap_or_default(),
    })
}

fn parse_time_rule(value: &Value) -> Option<PriceToBeatIvPriceBandTimeRule> {
    let obj = value.as_object()?;
    let start_remaining_secs = number(obj, &["startRemainingSec", "start_remaining_sec"])?;
    let end_remaining_secs = number(obj, &["endRemainingSec", "end_remaining_sec"])?;
    if start_remaining_secs <= end_remaining_secs || end_remaining_secs < 0.0 {
        return None;
    }
    Some(PriceToBeatIvPriceBandTimeRule {
        start_remaining_secs,
        end_remaining_secs,
        min_gap_strength: number(obj, &["minGapStrength", "min_gap_strength"])
            .filter(|value| *value >= 0.0),
        min_q_cent: optional_non_negative_cent(obj, &["minQCent", "min_q_cent"]),
        min_fair_edge_cent: optional_non_negative_cent(
            obj,
            &["minFairEdgeCent", "min_fair_edge_cent"],
        ),
        max_spread_cent: optional_non_negative_cent(obj, &["maxSpreadCent", "max_spread_cent"]),
        require_book_confirmation: bool_field(
            obj,
            &["requireBookConfirmation", "require_book_confirmation"],
        ),
        require_no_chainlink_stale_penalty: bool_field(
            obj,
            &[
                "requireNoChainlinkStalePenalty",
                "require_no_chainlink_stale_penalty",
            ],
        ),
        require_no_mixed_cex: bool_field(obj, &["requireNoMixedCex", "require_no_mixed_cex"]),
    })
}

fn number(obj: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(crate::value_as_f64))
        .filter(|value| value.is_finite())
}

fn optional_non_negative_cent(obj: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    number(obj, keys).filter(|value| *value >= 0.0 && *value <= 100.0)
}

fn bool_field(obj: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(Value::as_bool))
}

fn default_price_bands() -> Vec<PriceToBeatIvPriceBand> {
    vec![
        band(
            "premium_lite_77_82",
            77.0,
            82.0,
            86.0,
            3.5,
            4.0,
            true,
            true,
            false,
            false,
            false,
            &[
                (240.0, 180.0, 2.75),
                (180.0, 120.0, 2.50),
                (120.0, 60.0, 2.15),
                (60.0, 25.0, 1.85),
                (25.0, 10.0, 1.65),
                (10.0, 0.0, 1.75),
            ],
        ),
        band(
            "premium_strict_82_86",
            82.0,
            86.0,
            91.0,
            4.5,
            3.0,
            true,
            true,
            false,
            false,
            false,
            &[
                (240.0, 180.0, 3.10),
                (180.0, 120.0, 2.80),
                (120.0, 60.0, 2.40),
                (60.0, 25.0, 2.10),
                (25.0, 10.0, 1.90),
                (10.0, 0.0, 2.00),
            ],
        ),
        certainty_band_86_88(),
        certainty_band_88_90(),
    ]
}

#[allow(clippy::too_many_arguments)]
fn band(
    name: &str,
    min_price_cent: f64,
    max_price_cent: f64,
    min_q_cent: f64,
    min_fair_edge_cent: f64,
    max_spread_cent: f64,
    require_clean_cex: bool,
    require_cex_with_direction: bool,
    require_book_confirmation: bool,
    require_no_chainlink_stale_penalty: bool,
    require_no_mixed_cex: bool,
    rules: &[(f64, f64, f64)],
) -> PriceToBeatIvPriceBand {
    PriceToBeatIvPriceBand {
        name: name.to_string(),
        min_price_cent,
        max_price_cent,
        min_q_cent: Some(min_q_cent),
        min_fair_edge_cent: Some(min_fair_edge_cent),
        max_spread_cent: Some(max_spread_cent),
        require_clean_cex,
        require_cex_with_direction,
        require_book_confirmation,
        require_no_chainlink_stale_penalty,
        require_no_mixed_cex,
        time_rules: rules
            .iter()
            .map(|(start, end, gap)| PriceToBeatIvPriceBandTimeRule {
                start_remaining_secs: *start,
                end_remaining_secs: *end,
                min_gap_strength: Some(*gap),
                min_q_cent: None,
                min_fair_edge_cent: None,
                max_spread_cent: None,
                require_book_confirmation: None,
                require_no_chainlink_stale_penalty: None,
                require_no_mixed_cex: None,
            })
            .collect(),
    }
}

fn certainty_band_86_88() -> PriceToBeatIvPriceBand {
    let mut band = band(
        "certainty_86_88",
        86.0,
        88.0,
        96.5,
        6.0,
        2.0,
        true,
        true,
        true,
        true,
        true,
        &[],
    );
    band.time_rules = vec![
        time_rule(240.0, 180.0, 4.0, 98.0, 8.0, 1.5),
        time_rule(180.0, 120.0, 3.6, 97.5, 7.5, 1.5),
        time_rule(120.0, 60.0, 3.2, 97.2, 7.0, 2.0),
        time_rule(60.0, 25.0, 2.85, 97.0, 6.5, 2.0),
        time_rule(25.0, 10.0, 2.65, 97.0, 6.5, 2.0),
        time_rule(10.0, 0.0, 3.0, 97.5, 7.0, 1.5),
    ];
    band
}

fn certainty_band_88_90() -> PriceToBeatIvPriceBand {
    let mut band = band(
        "certainty_88_90",
        88.0,
        90.0,
        97.5,
        7.0,
        1.5,
        true,
        true,
        true,
        true,
        true,
        &[],
    );
    band.time_rules = vec![
        time_rule(240.0, 180.0, 4.3, 98.8, 9.0, 1.0),
        time_rule(180.0, 120.0, 3.9, 98.5, 8.5, 1.0),
        time_rule(120.0, 60.0, 3.5, 98.0, 8.0, 1.5),
        time_rule(60.0, 25.0, 3.1, 97.8, 7.5, 1.5),
        time_rule(25.0, 10.0, 2.9, 97.8, 7.5, 1.5),
        time_rule(10.0, 0.0, 3.2, 98.2, 8.0, 1.0),
    ];
    band
}

fn time_rule(
    start_remaining_secs: f64,
    end_remaining_secs: f64,
    min_gap_strength: f64,
    min_q_cent: f64,
    min_fair_edge_cent: f64,
    max_spread_cent: f64,
) -> PriceToBeatIvPriceBandTimeRule {
    PriceToBeatIvPriceBandTimeRule {
        start_remaining_secs,
        end_remaining_secs,
        min_gap_strength: Some(min_gap_strength),
        min_q_cent: Some(min_q_cent),
        min_fair_edge_cent: Some(min_fair_edge_cent),
        max_spread_cent: Some(max_spread_cent),
        require_book_confirmation: None,
        require_no_chainlink_stale_penalty: None,
        require_no_mixed_cex: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestGuardInput {
        config: PriceToBeatIvPriceBandGuardConfig,
        execution_vwap: PriceToBeatIvExecutionVwapEvaluation,
        cex_open_gap: PriceToBeatIvCexOpenGapEvaluation,
        seconds_left: f64,
        gap_strength: f64,
        q_final: Option<f64>,
        spread: Option<f64>,
        book_confirmation: Option<bool>,
        chainlink_stale_penalty: Option<bool>,
    }

    impl TestGuardInput {
        fn evaluate(&self) -> PriceToBeatIvPriceBandGuardEvaluation {
            evaluate_price_to_beat_iv_price_band_guard(PriceToBeatIvPriceBandGuardInput {
                config: &self.config,
                seconds_left: self.seconds_left,
                gap_strength: self.gap_strength,
                existing_required_gap_strength: 0.0,
                q_final: self.q_final,
                spread: self.spread,
                execution_vwap: &self.execution_vwap,
                cex_open_gap: &self.cex_open_gap,
                book_confirmation: self.book_confirmation,
                chainlink_stale_penalty: self.chainlink_stale_penalty,
            })
        }
    }

    fn input(execution_ref: f64, seconds_left: f64, q_final: Option<f64>) -> TestGuardInput {
        TestGuardInput {
            config: PriceToBeatIvPriceBandGuardConfig {
                enabled: true,
                ..Default::default()
            },
            execution_vwap: PriceToBeatIvExecutionVwapEvaluation {
                execution_vwap: Some(execution_ref),
                execution_best_ask: Some((execution_ref - 0.01).max(0.01)),
                model_ask: Some((execution_ref - 0.02).max(0.01)),
                ..Default::default()
            },
            cex_open_gap: PriceToBeatIvCexOpenGapEvaluation {
                enabled: true,
                consensus: CexOpenGapConsensus::Strong,
                clean_lane: true,
                ..Default::default()
            },
            seconds_left,
            gap_strength: 10.0,
            q_final,
            spread: Some(0.01),
            book_confirmation: Some(true),
            chainlink_stale_penalty: Some(false),
        }
    }

    fn assert_blocks(evaluation: PriceToBeatIvPriceBandGuardEvaluation, reason: &'static str) {
        assert_eq!(evaluation.block_reason, Some(reason));
        assert!(evaluation.failed_checks.contains(&reason));
    }

    #[test]
    fn boundary_prices_select_expected_bands() {
        let cases = [
            (0.77, "not_applicable_base_lane", Some("base_0_77")),
            (0.7701, "pass", Some("premium_lite_77_82")),
            (0.82, "pass", Some("premium_lite_77_82")),
            (0.8201, "pass", Some("premium_strict_82_86")),
            (0.86, "pass", Some("premium_strict_82_86")),
            (0.8601, "pass", Some("certainty_86_88")),
            (0.88, "pass", Some("certainty_86_88")),
            (0.8801, "pass", Some("certainty_88_90")),
            (0.90, "pass", Some("certainty_88_90")),
        ];
        for (price, result, band) in cases {
            let evaluation = input(price, 44.0, Some(0.99)).evaluate();
            assert_eq!(evaluation.result, result, "{price}");
            assert_eq!(evaluation.band_name.as_deref(), band, "{price}");
        }
        assert_blocks(
            input(0.9001, 44.0, Some(0.99)).evaluate(),
            "blocked_price_band_no_matching_band",
        );
    }

    #[test]
    fn blocks_no_matching_time_rule_above_240_seconds() {
        let evaluation = input(0.89, 270.0, Some(0.99)).evaluate();
        assert_eq!(
            evaluation.block_reason,
            Some("blocked_price_band_no_matching_time_rule")
        );
    }

    #[test]
    fn uses_execution_vwap_before_best_ask_and_model_ask() {
        let evaluation = input(0.89, 44.0, Some(0.99)).evaluate();
        assert_eq!(evaluation.actual_source, Some("execution_vwap"));
        assert_eq!(evaluation.execution_ref_cent, Some(89.0));
    }

    #[test]
    fn falls_back_to_best_ask_then_model_ask() {
        let mut with_best = input(0.89, 44.0, Some(0.99));
        with_best.execution_vwap.execution_vwap = None;
        let evaluation = with_best.evaluate();
        assert_eq!(evaluation.actual_source, Some("execution_best_ask"));

        let mut with_model = input(0.89, 44.0, Some(0.99));
        with_model.execution_vwap.execution_vwap = None;
        with_model.execution_vwap.execution_best_ask = None;
        let evaluation = with_model.evaluate();
        assert_eq!(evaluation.actual_source, Some("model_ask"));
    }

    #[test]
    fn blocks_without_reference_price() {
        let mut input = input(0.89, 44.0, Some(0.99));
        input.execution_vwap.execution_vwap = None;
        input.execution_vwap.execution_best_ask = None;
        input.execution_vwap.model_ask = None;
        let evaluation = input.evaluate();
        assert_eq!(
            evaluation.block_reason,
            Some("blocked_price_band_no_reference_price")
        );
    }

    #[test]
    fn fair_edge_can_block_even_when_q_passes() {
        let evaluation = input(0.90, 200.0, Some(0.989)).evaluate();
        assert!(evaluation
            .failed_checks
            .contains(&"blocked_price_band_fair_edge_below_min"));
        assert!(!evaluation
            .failed_checks
            .contains(&"blocked_price_band_q_below_min"));
    }

    #[test]
    fn certainty_checks_block_low_gap_q_and_spread() {
        let mut low_gap = input(0.89, 44.0, Some(0.99));
        low_gap.gap_strength = 3.0;
        assert_blocks(low_gap.evaluate(), "blocked_price_band_gap_strength");

        let low_q = input(0.89, 44.0, Some(0.976));
        assert_blocks(low_q.evaluate(), "blocked_price_band_q_below_min");

        let mut missing_spread = input(0.89, 44.0, Some(0.99));
        missing_spread.spread = None;
        assert_blocks(
            missing_spread.evaluate(),
            "blocked_price_band_spread_missing",
        );

        let mut wide_spread = input(0.89, 44.0, Some(0.99));
        wide_spread.spread = Some(0.02);
        assert_blocks(wide_spread.evaluate(), "blocked_price_band_spread_too_wide");
    }

    #[test]
    fn certainty_checks_fail_closed_on_dirty_context() {
        let mut dirty_cex = input(0.89, 44.0, Some(0.99));
        dirty_cex.cex_open_gap.clean_lane = false;
        assert_blocks(dirty_cex.evaluate(), "blocked_price_band_cex_not_clean");

        let mut against_cex = input(0.89, 44.0, Some(0.99));
        against_cex.cex_open_gap.consensus = CexOpenGapConsensus::Against;
        let evaluation = against_cex.evaluate();
        assert!(evaluation
            .failed_checks
            .contains(&"blocked_price_band_cex_not_with_direction"));

        let mut missing_book = input(0.89, 44.0, Some(0.99));
        missing_book.book_confirmation = None;
        assert_blocks(
            missing_book.evaluate(),
            "blocked_price_band_book_confirmation_missing",
        );

        let mut stale = input(0.89, 44.0, Some(0.99));
        stale.chainlink_stale_penalty = Some(true);
        assert_blocks(
            stale.evaluate(),
            "blocked_price_band_chainlink_stale_penalty",
        );

        let mut mixed = input(0.89, 44.0, Some(0.99));
        mixed.cex_open_gap.consensus = CexOpenGapConsensus::Mixed;
        let evaluation = mixed.evaluate();
        assert!(evaluation
            .failed_checks
            .contains(&"blocked_price_band_mixed_cex"));
    }
}
