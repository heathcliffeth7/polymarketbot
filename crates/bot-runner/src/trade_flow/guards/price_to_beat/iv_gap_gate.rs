pub(crate) const GAP_GATE_MODE_HARD_BLOCK: &str = "hard_block";
pub(crate) const GAP_GATE_REASON_BELOW_THRESHOLD: &str = "blocked_gap_strength_below_threshold";
pub(crate) const GAP_GATE_WARNING_VISIBLE_NOT_ENFORCED: &str =
    "required_gap_strength_visible_but_not_enforced";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvGapGateInput {
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) min_margin: Option<f64>,
    pub(crate) mode: &'static str,
    pub(crate) enforced: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvGapGateEvaluation {
    pub(crate) mode: &'static str,
    pub(crate) enforced: bool,
    pub(crate) result: &'static str,
    pub(crate) reason: Option<&'static str>,
    pub(crate) actual_strength: Option<f64>,
    pub(crate) required_strength: Option<f64>,
    pub(crate) margin: Option<f64>,
    pub(crate) min_margin: Option<f64>,
    pub(crate) warning: Option<&'static str>,
}

impl PriceToBeatIvGapGateEvaluation {
    pub(crate) fn disabled() -> Self {
        Self {
            mode: GAP_GATE_MODE_HARD_BLOCK,
            enforced: true,
            result: "not_evaluated",
            reason: None,
            actual_strength: None,
            required_strength: None,
            margin: None,
            min_margin: None,
            warning: None,
        }
    }

    pub(crate) fn should_block(&self) -> bool {
        self.enforced && self.reason == Some(GAP_GATE_REASON_BELOW_THRESHOLD)
    }
}

pub(crate) fn evaluate_gap_gate(
    input: PriceToBeatIvGapGateInput,
) -> PriceToBeatIvGapGateEvaluation {
    let min_margin = input.min_margin.unwrap_or(0.0).max(0.0);
    let margin = input.gap_strength - input.required_gap_strength;
    let required_visible = input.required_gap_strength.is_finite();
    if !input.enforced {
        return PriceToBeatIvGapGateEvaluation {
            mode: input.mode,
            enforced: false,
            result: if required_visible && margin < min_margin {
                "fail_observed_only"
            } else {
                "pass_observed_only"
            },
            reason: None,
            actual_strength: Some(input.gap_strength),
            required_strength: Some(input.required_gap_strength),
            margin: Some(margin),
            min_margin: Some(min_margin),
            warning: required_visible.then_some(GAP_GATE_WARNING_VISIBLE_NOT_ENFORCED),
        };
    }
    let failed = !input.gap_strength.is_finite()
        || !input.required_gap_strength.is_finite()
        || margin < min_margin;
    PriceToBeatIvGapGateEvaluation {
        mode: input.mode,
        enforced: true,
        result: if failed { "fail" } else { "pass" },
        reason: failed.then_some(GAP_GATE_REASON_BELOW_THRESHOLD),
        actual_strength: Some(input.gap_strength),
        required_strength: Some(input.required_gap_strength),
        margin: Some(margin),
        min_margin: Some(min_margin),
        warning: None,
    }
}

pub(crate) fn run_gap_gate_startup_self_check() -> Result<(), &'static str> {
    let evaluation = evaluate_gap_gate(PriceToBeatIvGapGateInput {
        gap_strength: 1.1433,
        required_gap_strength: 1.9000,
        min_margin: Some(0.0),
        mode: GAP_GATE_MODE_HARD_BLOCK,
        enforced: true,
    });
    if evaluation.should_block() {
        Ok(())
    } else {
        Err("gap_gate_startup_self_check_failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_block_fails_when_strength_below_required() {
        let evaluation = evaluate_gap_gate(PriceToBeatIvGapGateInput {
            gap_strength: 1.1433,
            required_gap_strength: 1.9000,
            min_margin: Some(0.0),
            mode: GAP_GATE_MODE_HARD_BLOCK,
            enforced: true,
        });

        assert!(evaluation.should_block());
        assert_eq!(evaluation.result, "fail");
        assert_eq!(evaluation.reason, Some(GAP_GATE_REASON_BELOW_THRESHOLD));
        let margin = evaluation.margin.expect("margin");
        assert!((margin - -0.7567).abs() <= 1e-12);
        assert_eq!(evaluation.min_margin, Some(0.0));
    }
}
