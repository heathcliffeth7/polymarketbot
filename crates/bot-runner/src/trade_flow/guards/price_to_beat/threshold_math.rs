use super::{evaluate_directional_gap, PriceToBeatDiffUnit, PriceToBeatGuardEvaluation};

pub(crate) fn normalize_price_to_beat_threshold_usd(
    threshold_value: f64,
    threshold_unit: PriceToBeatDiffUnit,
) -> f64 {
    match threshold_unit {
        PriceToBeatDiffUnit::Usd => threshold_value,
        PriceToBeatDiffUnit::Cent => threshold_value / 100.0,
    }
}

pub(crate) fn apply_price_to_beat_risk_penalty(
    evaluation: &mut PriceToBeatGuardEvaluation,
    risk_penalty_usd: f64,
) {
    if !risk_penalty_usd.is_finite() || risk_penalty_usd <= 0.0 {
        return;
    }
    evaluation.threshold_usd += risk_penalty_usd;
    evaluation.threshold_value = if evaluation.threshold_unit == "cent" {
        evaluation.threshold_usd * 100.0
    } else {
        evaluation.threshold_usd
    };
    if let (Some(current_price), Some(price_to_beat), Some(outcome_label)) = (
        evaluation.current_price,
        evaluation.price_to_beat,
        evaluation.normalized_outcome_label.as_deref(),
    ) {
        if let Some(direction_evaluation) = evaluate_directional_gap(
            current_price,
            price_to_beat,
            evaluation.threshold_usd,
            outcome_label,
        ) {
            evaluation.passed = direction_evaluation.passed;
            evaluation.directional_gap = Some(direction_evaluation.directional_gap);
            evaluation.gap_abs = Some((current_price - price_to_beat).abs());
            evaluation.reason_code = if direction_evaluation.passed {
                "passed".to_string()
            } else {
                "price_to_beat_gap_below_threshold".to_string()
            };
            evaluation.reason_detail = (!direction_evaluation.passed).then(|| {
                format!(
                    "directional gap {:.8} (direction={}) is below threshold {:.8} {} (~{:.8} usd)",
                    direction_evaluation.directional_gap,
                    direction_evaluation.direction,
                    evaluation.threshold_value,
                    evaluation.threshold_unit,
                    evaluation.threshold_usd
                )
            });
        }
    }
}
