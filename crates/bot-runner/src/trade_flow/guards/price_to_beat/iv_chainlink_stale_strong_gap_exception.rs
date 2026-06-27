use serde_json::{json, Value};

const DEFAULT_MIN_GAP_STRENGTH: f64 = 4.0;
const DEFAULT_MAX_ORACLE_AGE_MS: i64 = 3_500;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkStaleStrongGapExceptionConfig {
    pub(crate) enabled: bool,
    pub(crate) min_gap_strength: f64,
    pub(crate) max_oracle_age_ms: i64,
    pub(crate) require_cex_confirmed: bool,
    pub(crate) require_bybit_hit: bool,
    pub(crate) require_secondary_cex: bool,
}

impl Default for ChainlinkStaleStrongGapExceptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_gap_strength: DEFAULT_MIN_GAP_STRENGTH,
            max_oracle_age_ms: DEFAULT_MAX_ORACLE_AGE_MS,
            require_cex_confirmed: true,
            require_bybit_hit: true,
            require_secondary_cex: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ChainlinkStaleStrongGapRuntimeContext {
    pub(crate) cex_confirmed: bool,
    pub(crate) anchor_venue: Option<String>,
    pub(crate) anchor_hit: bool,
    pub(crate) bybit_hit: bool,
    pub(crate) secondary_confirmed: bool,
    pub(crate) secondary_sources: Vec<String>,
    pub(crate) cex_clean: Option<bool>,
    pub(crate) cex_direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkStaleStrongGapExceptionInput {
    pub(crate) normal_stale_limit_ms: i64,
    pub(crate) oracle_price_age_ms: Option<i64>,
    pub(crate) ws_receipt_age_ms: Option<i64>,
    pub(crate) ws_receipt_age_scope: Option<&'static str>,
    pub(crate) entry_quality_gap_strength: Option<f64>,
    pub(crate) iv_gap_strength: Option<f64>,
    pub(crate) runtime: Option<ChainlinkStaleStrongGapRuntimeContext>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChainlinkStaleStrongGapExceptionDecision {
    pub(crate) applies: bool,
    pub(crate) passed: bool,
    pub(crate) result: &'static str,
    pub(crate) reasons: Vec<&'static str>,
    pub(crate) gap_source: &'static str,
    pub(crate) normal_stale_limit_ms: i64,
    pub(crate) oracle_price_age_ms: Option<i64>,
    pub(crate) ws_receipt_age_ms: Option<i64>,
    pub(crate) ws_receipt_age_scope: Option<&'static str>,
    pub(crate) exception_max_oracle_age_ms: i64,
    pub(crate) required_gap_strength: f64,
    pub(crate) entry_quality_gap_strength: Option<f64>,
    pub(crate) iv_gap_strength: Option<f64>,
    pub(crate) cex_confirmed: bool,
    pub(crate) anchor_venue: Option<String>,
    pub(crate) anchor_hit: bool,
    pub(crate) bybit_hit: bool,
    pub(crate) secondary_confirmed: bool,
    pub(crate) secondary_sources: Vec<String>,
    pub(crate) cex_clean: Option<bool>,
    pub(crate) cex_direction: Option<String>,
}

impl ChainlinkStaleStrongGapExceptionDecision {
    pub(crate) fn disabled(
        cfg: &ChainlinkStaleStrongGapExceptionConfig,
        input: &ChainlinkStaleStrongGapExceptionInput,
    ) -> Self {
        Self::finish(cfg, input, false, false, "disabled", Vec::new())
    }

    fn finish(
        cfg: &ChainlinkStaleStrongGapExceptionConfig,
        input: &ChainlinkStaleStrongGapExceptionInput,
        applies: bool,
        passed: bool,
        result: &'static str,
        reasons: Vec<&'static str>,
    ) -> Self {
        let runtime = input.runtime.clone().unwrap_or_default();
        Self {
            applies,
            passed,
            result,
            reasons,
            gap_source: "entry_quality",
            normal_stale_limit_ms: input.normal_stale_limit_ms,
            oracle_price_age_ms: input.oracle_price_age_ms,
            ws_receipt_age_ms: input.ws_receipt_age_ms,
            ws_receipt_age_scope: input.ws_receipt_age_scope,
            exception_max_oracle_age_ms: cfg.max_oracle_age_ms,
            required_gap_strength: cfg.min_gap_strength,
            entry_quality_gap_strength: input.entry_quality_gap_strength,
            iv_gap_strength: input.iv_gap_strength,
            cex_confirmed: runtime.cex_confirmed,
            anchor_venue: runtime.anchor_venue,
            anchor_hit: runtime.anchor_hit,
            bybit_hit: runtime.bybit_hit,
            secondary_confirmed: runtime.secondary_confirmed,
            secondary_sources: runtime.secondary_sources,
            cex_clean: runtime.cex_clean,
            cex_direction: runtime.cex_direction,
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "applies": self.applies,
            "passed": self.passed,
            "result": self.result,
            "reasons": self.reasons,
            "gap_source": self.gap_source,
            "normal_stale_limit_ms": self.normal_stale_limit_ms,
            "oracle_price_age_ms": self.oracle_price_age_ms,
            "ws_receipt_age_ms": self.ws_receipt_age_ms,
            "ws_receipt_age_scope": self.ws_receipt_age_scope,
            "exception_max_oracle_age_ms": self.exception_max_oracle_age_ms,
            "required_gap_strength": self.required_gap_strength,
            "entry_quality_gap_strength": self.entry_quality_gap_strength,
            "iv_gap_strength": self.iv_gap_strength,
            "cex_confirmed": self.cex_confirmed,
            "anchor_venue": self.anchor_venue,
            "anchor_hit": self.anchor_hit,
            "bybit_hit": self.bybit_hit,
            "secondary_confirmed": self.secondary_confirmed,
            "secondary_sources": self.secondary_sources,
            "cex_clean": self.cex_clean,
            "cex_direction": self.cex_direction,
        })
    }
}

pub(crate) fn evaluate_chainlink_stale_strong_gap_exception(
    cfg: &ChainlinkStaleStrongGapExceptionConfig,
    input: &ChainlinkStaleStrongGapExceptionInput,
) -> ChainlinkStaleStrongGapExceptionDecision {
    if !cfg.enabled {
        return ChainlinkStaleStrongGapExceptionDecision::disabled(cfg, input);
    }

    let Some(oracle_age_ms) = input.oracle_price_age_ms else {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_age_unavailable",
            vec!["blocked_chainlink_stale_age_unavailable"],
        );
    };
    if oracle_age_ms > cfg.max_oracle_age_ms {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_age_too_high",
            vec!["blocked_chainlink_stale_age_too_high"],
        );
    }

    let Some(gap_strength) = input
        .entry_quality_gap_strength
        .filter(|value| value.is_finite())
    else {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_gap_unavailable",
            vec!["blocked_chainlink_stale_gap_unavailable"],
        );
    };
    if gap_strength < cfg.min_gap_strength {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_weak_gap",
            vec!["blocked_chainlink_stale_weak_gap"],
        );
    }

    let runtime = input.runtime.as_ref();
    if cfg.require_cex_confirmed && runtime.map(|ctx| ctx.cex_confirmed) != Some(true) {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_no_cex_confirmation",
            vec!["blocked_chainlink_stale_no_cex_confirmation"],
        );
    }
    if cfg.require_bybit_hit
        && runtime
            .map(|ctx| ctx.anchor_hit || ctx.bybit_hit)
            .unwrap_or(false)
            != true
    {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_no_bybit_hit",
            vec!["blocked_chainlink_stale_no_bybit_hit"],
        );
    }
    if cfg.require_secondary_cex && runtime.map(|ctx| ctx.secondary_confirmed) != Some(true) {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_no_secondary_confirmation",
            vec!["blocked_chainlink_stale_no_secondary_confirmation"],
        );
    }
    if runtime.and_then(|ctx| ctx.cex_clean) != Some(true) {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_cex_not_clean",
            vec!["blocked_chainlink_stale_cex_not_clean"],
        );
    }
    if !matches!(
        runtime.and_then(|ctx| ctx.cex_direction.as_deref()),
        Some("clean") | Some("aligned") | Some("confirmed")
    ) {
        return ChainlinkStaleStrongGapExceptionDecision::finish(
            cfg,
            input,
            true,
            false,
            "blocked_chainlink_stale_cex_not_clean",
            vec!["blocked_chainlink_stale_cex_not_clean"],
        );
    }

    ChainlinkStaleStrongGapExceptionDecision::finish(
        cfg,
        input,
        true,
        true,
        "passed_chainlink_stale_strong_gap_exception",
        vec!["passed_chainlink_stale_strong_gap_exception"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ChainlinkStaleStrongGapExceptionConfig {
        ChainlinkStaleStrongGapExceptionConfig {
            enabled: true,
            ..Default::default()
        }
    }

    fn clean_runtime() -> ChainlinkStaleStrongGapRuntimeContext {
        ChainlinkStaleStrongGapRuntimeContext {
            cex_confirmed: true,
            anchor_venue: Some("bybit".to_string()),
            anchor_hit: true,
            bybit_hit: true,
            secondary_confirmed: true,
            secondary_sources: vec!["binance".to_string()],
            cex_clean: Some(true),
            cex_direction: Some("clean".to_string()),
        }
    }

    fn input() -> ChainlinkStaleStrongGapExceptionInput {
        ChainlinkStaleStrongGapExceptionInput {
            normal_stale_limit_ms: 3_000,
            oracle_price_age_ms: Some(3_200),
            ws_receipt_age_ms: Some(401),
            ws_receipt_age_scope: Some("symbol_specific"),
            entry_quality_gap_strength: Some(10.0),
            iv_gap_strength: None,
            runtime: Some(clean_runtime()),
        }
    }

    #[test]
    fn passes_with_entry_quality_gap_when_iv_gap_missing() {
        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input());

        assert!(decision.passed);
        assert_eq!(
            decision.result,
            "passed_chainlink_stale_strong_gap_exception"
        );
        assert_eq!(decision.gap_source, "entry_quality");
        assert_eq!(decision.entry_quality_gap_strength, Some(10.0));
        assert_eq!(decision.iv_gap_strength, None);
    }

    #[test]
    fn blocks_missing_oracle_age() {
        let mut input = input();
        input.oracle_price_age_ms = None;

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(!decision.passed);
        assert_eq!(decision.result, "blocked_chainlink_stale_age_unavailable");
    }

    #[test]
    fn blocks_weak_gap() {
        let mut input = input();
        input.entry_quality_gap_strength = Some(3.9);

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(!decision.passed);
        assert_eq!(decision.result, "blocked_chainlink_stale_weak_gap");
    }

    #[test]
    fn blocks_dirty_cex() {
        let mut input = input();
        input.runtime.as_mut().unwrap().cex_clean = Some(false);

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(!decision.passed);
        assert_eq!(decision.result, "blocked_chainlink_stale_cex_not_clean");
    }

    #[test]
    fn blocks_clean_pair_without_anchor_when_anchor_required() {
        let mut input = input();
        let runtime = input.runtime.as_mut().unwrap();
        runtime.cex_confirmed = true;
        runtime.anchor_venue = Some("okx".to_string());
        runtime.anchor_hit = false;
        runtime.bybit_hit = false;
        runtime.secondary_confirmed = true;
        runtime.secondary_sources = vec!["binance".to_string(), "coinbase".to_string()];
        runtime.cex_clean = Some(true);

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(!decision.passed);
        assert_eq!(decision.result, "blocked_chainlink_stale_no_bybit_hit");
    }

    #[test]
    fn okx_anchor_hit_satisfies_legacy_require_bybit_alias() {
        let mut input = input();
        let runtime = input.runtime.as_mut().unwrap();
        runtime.anchor_venue = Some("okx".to_string());
        runtime.anchor_hit = true;
        runtime.bybit_hit = false;

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(decision.passed);
        assert_eq!(
            decision.result,
            "passed_chainlink_stale_strong_gap_exception"
        );
        assert_eq!(decision.anchor_venue.as_deref(), Some("okx"));
        assert!(decision.anchor_hit);
    }

    #[test]
    fn gateio_anchor_hit_satisfies_legacy_require_bybit_alias() {
        let mut input = input();
        let runtime = input.runtime.as_mut().unwrap();
        runtime.anchor_venue = Some("gateio".to_string());
        runtime.anchor_hit = true;
        runtime.bybit_hit = false;

        let decision = evaluate_chainlink_stale_strong_gap_exception(&cfg(), &input);

        assert!(decision.passed);
        assert_eq!(
            decision.result,
            "passed_chainlink_stale_strong_gap_exception"
        );
        assert_eq!(decision.anchor_venue.as_deref(), Some("gateio"));
        assert!(decision.anchor_hit);
    }

    #[test]
    fn disabled_is_noop() {
        let decision = evaluate_chainlink_stale_strong_gap_exception(&Default::default(), &input());

        assert!(!decision.applies);
        assert!(!decision.passed);
        assert_eq!(decision.result, "disabled");
    }
}
