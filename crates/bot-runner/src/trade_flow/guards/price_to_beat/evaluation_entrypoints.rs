use super::entry_current_hybrid::CexEntryConsensusConfig;
use super::*;

pub(crate) async fn evaluate_price_to_beat_guard(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
) -> PriceToBeatGuardEvaluation {
    evaluate_price_to_beat_guard_with_iv_mismatch_config(
        market_slug,
        mode,
        threshold_value,
        threshold_unit,
        outcome_label,
        signal_config,
        None,
    )
    .await
}

pub(crate) async fn evaluate_price_to_beat_guard_with_iv_mismatch_config(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> PriceToBeatGuardEvaluation {
    evaluate_price_to_beat_guard_with_current_source(
        market_slug,
        mode,
        threshold_value,
        threshold_unit,
        outcome_label,
        signal_config,
        PriceToBeatCurrentPriceSource::Chainlink,
        CexEntryConsensusConfig::default(),
        iv_mismatch_config,
    )
    .await
}
