use super::iv_borderline_pump_book_lead::{
    evaluate_price_to_beat_iv_borderline_pump_book_lead, PriceToBeatIvBorderlinePumpBookLeadInput,
};
#[path = "iv_cex_magnitude_guard.rs"]
pub(crate) mod iv_cex_magnitude_guard;
use self::iv_cex_magnitude_guard::{
    evaluate_price_to_beat_iv_cex_magnitude, PriceToBeatIvCexMagnitudeInput,
};
use super::iv_cex_open_gap::{
    cex_lead_override_applies, cex_open_gap_book_mismatch_reason,
    evaluate_price_to_beat_iv_cex_open_gap, PriceToBeatIvCexOpenGapInput,
};
use super::iv_cex_sigma::{blend_cex_sigma, cex_median_mid_sigma};
use super::iv_chainlink_stale_strong_gap_exception::{
    evaluate_chainlink_stale_strong_gap_exception, ChainlinkStaleStrongGapExceptionInput,
};
use super::iv_entry_quality::{evaluate_iv_entry_quality, IvEntryQualityInput};
use super::iv_execution_vwap::{
    evaluate_price_to_beat_iv_execution_vwap, PriceToBeatIvExecutionVwapInput,
};
use super::iv_gap_fail_cex_book_guard::{
    evaluate_price_to_beat_iv_gap_fail_cex_book_guard, PriceToBeatIvGapFailCexBookGuardInput,
};
use super::iv_gap_gate::{
    evaluate_gap_gate, PriceToBeatIvGapGateInput, GAP_GATE_MODE_HARD_BLOCK,
    GAP_GATE_REASON_BELOW_THRESHOLD,
};
use super::iv_high_price_early_reversal::{
    evaluate_price_to_beat_iv_high_price_early_reversal, PriceToBeatIvHighPriceEarlyReversalInput,
    HIGH_PRICE_EARLY_REVERSAL_GAP_REASON,
};
use super::iv_mismatch_adaptive::{
    evaluate_price_to_beat_iv_adaptive_volume, PriceToBeatIvAdaptiveInput,
};
use super::iv_mismatch_binance::{evaluate_binance_disagreement_penalty, evaluate_binance_veto};
use super::iv_mismatch_book::selected_book_mid_for_ptb_movement;
use super::iv_mismatch_depth::evaluate_price_to_beat_iv_depth;
use super::iv_mismatch_edge_helpers::{
    edge_threshold_for_seconds_left, iv_mismatch_seconds_left, iv_mismatch_side, previous_side_gap,
    side_gap, side_gap_at_or_before, sigma_since, time_normalized_price_deltas, valid_probability,
    zero_cross_count,
};
use super::iv_mismatch_expected_move::{
    evaluate_expected_move_floor, PriceToBeatIvExpectedMoveFloorEvaluation,
    PriceToBeatIvExpectedMoveFloorInput, PriceToBeatIvMinExpectedMoveMode,
};
#[path = "iv_mismatch_chainlink_samples.rs"]
mod iv_mismatch_chainlink_samples;
#[path = "iv_mismatch_edge_evaluation.rs"]
mod iv_mismatch_edge_evaluation;
#[path = "iv_mismatch_edge_telemetry.rs"]
mod iv_mismatch_edge_telemetry;
#[cfg(test)]
use super::iv_mismatch_adaptive::PriceToBeatIvAdaptiveVolumeInput;
#[cfg(test)]
pub(crate) use super::iv_mismatch_edge_config::DEFAULT_BINANCE_Q_BUFFER;
pub(crate) use super::iv_mismatch_edge_config::{
    PriceToBeatIvMismatchEdgeConfig, DEFAULT_CHAINLINK_STALE_MS, DEFAULT_FAST_VOL_WINDOW_SECS,
    DEFAULT_MIN_VOL_SAMPLES, DEFAULT_VOL_WINDOW_SECS,
};
#[cfg(test)]
use super::iv_mismatch_math::inverse_normal_cdf;
use super::iv_mismatch_math::{implied_volatility_ratio, normal_cdf, standard_deviation};
#[cfg(test)]
use super::iv_mismatch_protection::PriceToBeatIvBookQuotes;
#[cfg(test)]
use super::PriceToBeatSignalFormulaMarketInput;
#[path = "iv_mismatch_medium_chop_margin.rs"]
pub(crate) mod iv_mismatch_medium_chop_margin;
use self::iv_mismatch_medium_chop_margin::{
    evaluate_price_to_beat_iv_medium_chop_margin, PriceToBeatIvMediumChopMarginInput,
};
use super::iv_mismatch_protection::{
    evaluate_price_to_beat_iv_protection, PriceToBeatIvProtectionInput, PriceToBeatIvProtectionMode,
};
use super::iv_mismatch_ptb_chop::{evaluate_price_to_beat_iv_ptb_chop, PriceToBeatIvPtbChopInput};
use super::iv_mismatch_time_rule::select_time_rule;
#[cfg(test)]
pub(crate) use super::iv_mismatch_time_rule::PriceToBeatIvMismatchTimeRule;
use super::iv_oracle_lag_book_lead::{
    evaluate_price_to_beat_iv_oracle_lag_book_lead, PriceToBeatIvOracleLagBookLeadInput,
};
use super::iv_oracle_tick_jump::{evaluate_oracle_tick_jump_with_state, OracleTickJumpInput};
use super::iv_price_band_guard::{
    evaluate_price_to_beat_iv_price_band_guard, PriceToBeatIvPriceBandGuardInput,
};
use super::iv_pump_shock::{evaluate_price_to_beat_iv_pump_shock, PriceToBeatIvPumpShockInput};
use super::iv_token_crash_cooldown::{
    evaluate_price_to_beat_iv_token_crash_cooldown, PriceToBeatIvTokenCrashCooldownInput,
};
use super::signal_formula::signal_formula_taker_fee;
use crate::trade_flow::guards::chainlink_price::{
    chainlink_price_sample_readiness, chainlink_symbol_for_asset,
};
use chrono::Utc;
use iv_mismatch_chainlink_samples::get_chainlink_price_samples_with_warmup;
pub(crate) use iv_mismatch_edge_evaluation::PriceToBeatIvMismatchEdgeEvaluation;

pub(crate) fn evaluate_price_to_beat_iv_mismatch_edge(
    market_slug: &str,
    outcome_label: &str,
    asset: &str,
    current_price: f64,
    price_to_beat: f64,
    config: PriceToBeatIvMismatchEdgeConfig,
) -> PriceToBeatIvMismatchEdgeEvaluation {
    let mut evaluation = PriceToBeatIvMismatchEdgeEvaluation::new(&config);
    let Some(side) = iv_mismatch_side(outcome_label) else {
        return evaluation.finish(false, "unsupported_outcome_label");
    };
    evaluation.candidate_side = Some(side);

    let Some(seconds_left) = iv_mismatch_seconds_left(market_slug) else {
        return evaluation.finish(false, "market_window_unavailable");
    };
    evaluation.seconds_left = Some(seconds_left);
    let selected_time_rule = select_time_rule(seconds_left, &config.time_rules);
    if !config.time_rules.is_empty() && selected_time_rule.is_none() {
        return evaluation.finish(false, "blocked_no_matching_time_rule");
    }
    if let Some((index, rule)) = selected_time_rule {
        evaluation.selected_time_rule_index = Some(index);
        evaluation.selected_time_rule = Some(rule);
        evaluation.time_rule_max_price = rule.max_price;
    }
    let price_band_guard_max_price = config.price_band_guard.enabled.then_some(0.90);
    let node_max_price = match (config.node_max_price, price_band_guard_max_price) {
        (Some(node_max_price), Some(guard_max_price)) => Some(node_max_price.max(guard_max_price)),
        (None, Some(guard_max_price)) => Some(guard_max_price),
        (node_max_price, None) => node_max_price,
    };
    if let Some(guard_max_price) = price_band_guard_max_price {
        evaluation.time_rule_max_price = evaluation
            .time_rule_max_price
            .map(|rule_max_price| rule_max_price.max(guard_max_price));
    }
    evaluation.effective_max_price = match (node_max_price, evaluation.time_rule_max_price) {
        (Some(node_max_price), Some(rule_max_price)) => Some(node_max_price.min(rule_max_price)),
        (Some(node_max_price), None) => Some(node_max_price),
        (None, Some(rule_max_price)) => Some(rule_max_price),
        (None, None) => None,
    };
    let threshold = match edge_threshold_for_seconds_left(
        seconds_left,
        selected_time_rule,
        config.no_new_trade_under_secs,
        config.no_new_trade_over_secs,
        config.edge_threshold_30_90_secs,
        config.edge_threshold_15_30_secs,
        config.edge_threshold_8_15_secs,
    ) {
        Ok(threshold) => threshold,
        Err(reason) => return evaluation.finish(false, reason),
    };
    evaluation.threshold = Some(threshold);

    let Some(ask) = config
        .market
        .best_ask
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "orderbook_unavailable");
    };
    let Some(bid) = config
        .market
        .best_bid
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "orderbook_unavailable");
    };
    if ask < bid {
        evaluation.spread = Some(ask - bid);
        return evaluation.finish(false, "invalid_spread");
    }
    let spread = ask - bid;
    evaluation.spread = Some(spread);
    if spread > config.max_spread {
        return evaluation.finish(false, "blocked_spread_wide");
    }
    let time_rule_price_blocked = !config.entry_quality.eq77_risk_cap_enabled
        && evaluation
            .time_rule_max_price
            .map(|max_price| ask > max_price)
            .unwrap_or(false);

    let mut now_ms = Utc::now().timestamp_millis();
    let sample_window_secs = config.vol_window_secs.max(1);
    let mut sample_window_start_ms = now_ms - sample_window_secs * 1_000;
    evaluation.chainlink_symbol = chainlink_symbol_for_asset(asset).map(ToString::to_string);
    evaluation.sample_window_start_ms = Some(sample_window_start_ms);
    evaluation.sample_window_end_ms = Some(now_ms);
    evaluation.sample_window_secs = Some(sample_window_secs);
    let samples = match get_chainlink_price_samples_with_warmup(asset, sample_window_secs, now_ms) {
        Ok(sample_fetch) => {
            now_ms = sample_fetch.end_ms;
            sample_window_start_ms = sample_fetch.start_ms;
            evaluation.sample_window_start_ms = Some(sample_window_start_ms);
            evaluation.sample_window_end_ms = Some(now_ms);
            sample_fetch.samples
        }
        Err(error) => {
            let readiness = chainlink_price_sample_readiness(asset, sample_window_start_ms, now_ms);
            evaluation.chainlink_symbol = readiness.symbol;
            evaluation.sample_count = readiness.sample_count;
            evaluation.delta_count = readiness.delta_count;
            evaluation.last_symbol_tick_age_ms = readiness.last_symbol_tick_age_ms;
            evaluation.last_symbol_received_age_ms = readiness.last_symbol_received_age_ms;
            evaluation.vol_sample_error = readiness.error.or_else(|| Some(error.to_string()));
            return evaluation.finish(false, "blocked_insufficient_vol_samples");
        }
    };
    evaluation.sample_count = Some(samples.len());
    if samples.len() < config.min_vol_samples {
        return evaluation.finish(false, "blocked_insufficient_vol_samples");
    }
    let latest_timestamp_ms = samples
        .iter()
        .map(|sample| sample.timestamp_ms)
        .max()
        .unwrap_or(now_ms);
    let staleness_ms = now_ms.saturating_sub(latest_timestamp_ms);
    evaluation.chainlink_staleness_ms = Some(staleness_ms);
    let readiness = chainlink_price_sample_readiness(asset, sample_window_start_ms, now_ms);
    evaluation.last_symbol_tick_age_ms = readiness.last_symbol_tick_age_ms;
    evaluation.last_symbol_received_age_ms = readiness.last_symbol_received_age_ms;
    let chainlink_stale_candidate = is_chainlink_stale(staleness_ms, config.chainlink_stale_ms);
    evaluation.chainlink_stale_tolerance_result = Some(chainlink_stale_tolerance_result(
        staleness_ms,
        config.chainlink_stale_ms,
    ));
    if chainlink_stale_candidate && !config.chainlink_stale_strong_gap_exception.enabled {
        evaluation.chainlink_stale_tolerance_result = Some("blocked_chainlink_stale");
        return evaluation.finish(false, "chainlink_provider_stale_global");
    }

    let deltas = time_normalized_price_deltas(&samples);
    evaluation.delta_count = Some(deltas.len());
    if deltas.len() + 1 < config.min_vol_samples {
        return evaluation.finish(false, "blocked_insufficient_vol_samples");
    }
    let sigma = standard_deviation(&deltas);
    if !sigma.is_finite() || sigma <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    evaluation.sigma = Some(sigma);

    let zero_cross_count = zero_cross_count(&samples, price_to_beat);
    evaluation.zero_cross_count = Some(zero_cross_count);

    let gap = current_price - price_to_beat;
    let expected_move = sigma * seconds_left.sqrt();
    if !expected_move.is_finite() || expected_move <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    let z = gap / expected_move;
    let q_up = normal_cdf(z);
    let q_down = 1.0 - q_up;
    let q = if side == "up" { q_up } else { q_down };
    let depth = evaluate_price_to_beat_iv_depth(
        config.depth_order_book.as_ref(),
        ask,
        config.depth_intended_qty,
        config.depth_max_slippage,
        config.depth_guard_enabled
            || config.execution_vwap_guard.enabled
            || config.execution_vwap_guard.limit_by_vwap_enabled,
    );
    let effective_fill_price = depth.estimated_avg_fill.unwrap_or(ask);
    let fee = signal_formula_taker_fee(effective_fill_price);
    let cost = effective_fill_price + fee + config.buffer.max(0.0);
    let edge = q - cost;
    let iv_ratio = implied_volatility_ratio(cost, gap.abs(), seconds_left, sigma);
    let sigma_15 = sigma_since(
        &samples,
        latest_timestamp_ms - config.fast_vol_window_secs.max(1) * 1_000,
    );
    let chainlink_sigma_eff = sigma_15
        .map(|fast| sigma.max(config.fast_vol_multiplier.max(0.0) * fast))
        .unwrap_or(sigma);
    let cex_sigma = cex_median_mid_sigma(asset, config.fast_vol_window_secs);
    let (sigma_eff, sigma_eff_source) =
        blend_cex_sigma(chainlink_sigma_eff, cex_sigma, config.cex_sigma_blend_weight);
    let x_now = side_gap(side, current_price, price_to_beat);
    let latency_horizon_secs =
        (staleness_ms as f64 / 1_000.0) + config.latency_buffer_secs.max(0.0);
    let (x_prev, gap_velocity) =
        previous_side_gap(&samples, side, price_to_beat, latest_timestamp_ms)
            .map(|(sample_ts, sample_gap)| {
                let dt_secs = ((latest_timestamp_ms - sample_ts) as f64 / 1_000.0).max(0.001);
                (Some(sample_gap), Some((x_now - sample_gap) / dt_secs))
            })
            .unwrap_or((None, None));
    let x_eff_velocity_adj =
        gap_velocity.unwrap_or(0.0).min(0.0) * latency_horizon_secs.max(0.0).sqrt();
    let x_eff = (x_now + x_eff_velocity_adj).max(0.0);
    let expected_move_model = sigma_eff * seconds_left.sqrt();
    if !expected_move_model.is_finite() || expected_move_model <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    let z_before_floor = x_eff / expected_move_model;
    let q_before_floor = normal_cdf(z_before_floor);
    let time_rule_expected_move_floor = selected_time_rule
        .and_then(|(_, rule)| rule.min_expected_move_usd)
        .filter(|value| value.is_finite() && *value > 0.0);
    let entry_quality_expected_move_floor = config
        .entry_quality
        .enabled
        .then(|| config.entry_quality.expected_move_floor(current_price))
        .filter(|value| value.is_finite() && *value > 0.0);
    let expected_move_floor = [
        time_rule_expected_move_floor,
        entry_quality_expected_move_floor,
    ]
    .into_iter()
    .flatten()
    .max_by(f64::total_cmp);
    let base_expected_move_eff = expected_move_floor
        .map(|floor| expected_move_model.max(floor))
        .unwrap_or(expected_move_model);
    let expected_move_floor_debug =
        if config.expected_move_floor.mode == PriceToBeatIvMinExpectedMoveMode::Adaptive {
            let base_q_chain_adj = normal_cdf(x_eff / base_expected_move_eff);
            let preliminary_binance_adjustment = evaluate_binance_veto(
                asset,
                side,
                price_to_beat,
                base_expected_move_eff,
                base_q_chain_adj,
                now_ms,
                &config,
            );
            let preliminary_disagreement = evaluate_binance_disagreement_penalty(
                base_q_chain_adj,
                preliminary_binance_adjustment.q_binance,
                &config,
            );
            evaluate_expected_move_floor(
                &config.expected_move_floor,
                PriceToBeatIvExpectedMoveFloorInput {
                    current_price,
                    spread,
                    source_staleness_ms: staleness_ms.max(
                        preliminary_binance_adjustment
                            .binance_staleness_ms
                            .unwrap_or(0),
                    ),
                    sigma_fast: sigma_15,
                    sigma_eff,
                    disagreement_abs: preliminary_disagreement.absolute,
                },
            )
        } else {
            PriceToBeatIvExpectedMoveFloorEvaluation::fixed()
        };
    let expected_move_floor = [expected_move_floor, expected_move_floor_debug.floor_usd]
        .into_iter()
        .flatten()
        .max_by(f64::total_cmp);
    let expected_move_eff = expected_move_floor
        .map(|floor| expected_move_model.max(floor))
        .unwrap_or(expected_move_model);
    let z_adj = x_eff / expected_move_eff;
    let q_after_floor = normal_cdf(z_adj);
    let q_chain_adj = q_after_floor;
    let gap_strength = x_now / expected_move_eff;
    if chainlink_stale_candidate {
        let ws_receipt_age_scope = evaluation
            .last_symbol_received_age_ms
            .map(|_| "symbol_specific");
        let stale_exception = evaluate_chainlink_stale_strong_gap_exception(
            &config.chainlink_stale_strong_gap_exception,
            &ChainlinkStaleStrongGapExceptionInput {
                normal_stale_limit_ms: config.chainlink_stale_ms,
                oracle_price_age_ms: evaluation.chainlink_staleness_ms,
                ws_receipt_age_ms: evaluation.last_symbol_received_age_ms,
                ws_receipt_age_scope,
                entry_quality_gap_strength: Some(gap_strength),
                iv_gap_strength: None,
                runtime: config.chainlink_stale_strong_gap_context.clone(),
            },
        );
        let stale_exception_passed = stale_exception.passed;
        let stale_exception_result = stale_exception.result;
        evaluation.chainlink_stale_exception_passed = stale_exception_passed;
        evaluation.chainlink_stale_strong_gap_exception = Some(stale_exception);
        evaluation.chainlink_stale_tolerance_result = Some(if stale_exception_passed {
            "pass_stale_exception"
        } else {
            "blocked_chainlink_stale_exception"
        });
        if !stale_exception_passed {
            return evaluation.finish(false, stale_exception_result);
        }
    }
    let drop_z = side_gap_at_or_before(&samples, side, price_to_beat, latest_timestamp_ms - 3_000)
        .map(|x_ago| {
            let denominator = sigma_eff * 3.0_f64.sqrt();
            if denominator > 0.0 {
                (x_ago - x_now) / denominator
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);
    let high_price_penalty = if ask >= config.high_price_penalty_threshold {
        config.high_price_penalty.max(0.0)
    } else {
        0.0
    };
    let stale_penalty = if staleness_ms > config.stale_penalty_ms {
        config.stale_penalty.max(0.0)
    } else {
        0.0
    };
    // Hem dusus (drop_z>0) hem yukselis spike'i (drop_z<0) ceza alir; rising start daha gec
    let drop_penalty = config.drop_penalty_per_z.max(0.0)
        * ((drop_z - config.drop_penalty_start_z).max(0.0)
            + (-drop_z - config.rising_drop_penalty_start_z).max(0.0));
    let gap_strength_stale_penalty = if staleness_ms > config.stale_gap_strength_penalty_ms {
        config.stale_gap_strength_penalty.max(0.0)
    } else {
        0.0
    };
    let gap_strength_velocity_penalty = if gap_velocity.unwrap_or(0.0) < 0.0 {
        config.negative_velocity_gap_strength_penalty.max(0.0)
    } else {
        0.0
    };
    let raw_required_gap_strength = selected_time_rule
        .map(|(_, rule)| rule.min_gap_strength)
        .unwrap_or(0.0)
        + gap_strength_stale_penalty
        + gap_strength_velocity_penalty;
    let binance_adjustment = evaluate_binance_veto(
        asset,
        side,
        price_to_beat,
        expected_move_eff,
        q_chain_adj,
        now_ms,
        &config,
    );
    if config.binance_hard_block_stale && binance_adjustment.status == "fail_open_stale" {
        evaluation.binance_staleness_ms = binance_adjustment.binance_staleness_ms;
        evaluation.binance_veto_status = Some(binance_adjustment.status.clone());
        let reason = "blocked_binance_stale";
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    let binance_missing_penalty =
        if ask >= config.binance_missing_ask_threshold && binance_adjustment.is_missing() {
            config.binance_missing_penalty.max(0.0)
        } else {
            0.0
        };
    let binance_disagreement =
        evaluate_binance_disagreement_penalty(q_chain_adj, binance_adjustment.q_binance, &config);
    let base_dynamic_threshold = threshold
        + high_price_penalty
        + stale_penalty
        + drop_penalty
        + binance_missing_penalty
        + binance_disagreement.penalty;
    let mut cex_open_gap = evaluate_price_to_beat_iv_cex_open_gap(PriceToBeatIvCexOpenGapInput {
        config: config.cex_open_gap,
        market_slug,
        asset,
        selected_side: side,
        current_price,
        chainlink_signed_gap: x_now,
        expected_move_eff,
        q_final_before: binance_adjustment.q_final,
    });
    let cex_open_gap_block_reason = cex_open_gap.block_reason;
    let divergence_block_reason = cex_open_gap.divergence_block_reason;
    let cex_lead_override = cex_lead_override_applies(&config.cex_open_gap, &cex_open_gap, x_now);
    let decision_gap_effective_usd = if config.cex_open_gap.decision_gap_enabled {
        cex_open_gap.decision_gap_usd.unwrap_or(x_now)
    } else {
        x_now
    };
    let decision_gap_block_reason: Option<&'static str> =
        if config.cex_open_gap.decision_gap_enabled && cex_open_gap.decision_gap_usd.is_none() {
            Some("blocked_decision_gap_cex_unavailable")
        } else {
            None
        };
    let gap_strength = if config.cex_open_gap.decision_gap_enabled {
        cex_open_gap.decision_gap_strength.unwrap_or(gap_strength)
    } else {
        gap_strength
    };
    let oracle_tick_jump = evaluate_oracle_tick_jump_with_state(
        market_slug,
        &OracleTickJumpInput {
            config: config.oracle_tick_jump.clone(),
            x_now,
            x_prev: x_prev.unwrap_or(x_now),
            expected_move_eff,
            conservative_cex_gap: cex_open_gap.conservative_cex_gap,
        },
    );
    let oracle_tick_jump_block_reason = oracle_tick_jump.block_reason;
    let q_final = cex_open_gap
        .q_final_after_cex_consensus
        .unwrap_or(binance_adjustment.q_final);
    let binance_same_direction = binance_adjustment
        .binance_price
        .map(|price| side_gap(side, price, price_to_beat) > 0.0);
    let base_min_gap_strength_margin = selected_time_rule
        .and_then(|(_, rule)| rule.min_gap_strength_margin)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let base_min_gap_usd_margin = selected_time_rule
        .and_then(|(_, rule)| rule.min_gap_usd_margin)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let adaptive = (config.protection_mode == PriceToBeatIvProtectionMode::Adaptive).then(|| {
        evaluate_price_to_beat_iv_adaptive_volume(&PriceToBeatIvAdaptiveInput {
            selected_side: side,
            seconds_left,
            book_quotes: config.book_quotes,
            book_lead_guard_enabled: config.book_lead_guard_enabled,
            book_lead_under_sec: config.book_lead_under_sec,
            book_lead_min_mid_diff: config.book_lead_min_mid_diff,
            binance_same_direction,
            zero_cross_count,
            chop_zero_cross_limit: config.chop_zero_cross_limit,
            base_min_edge: base_dynamic_threshold,
            base_gap_strength: raw_required_gap_strength,
            base_gap_usd_margin: base_min_gap_usd_margin,
            volume: config.adaptive_volume.as_ref(),
            config: &config.adaptive,
        })
    });
    let adaptive_edge_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.edge_delta)
        .unwrap_or(0.0);
    let adaptive_gap_strength_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.gap_strength_delta)
        .unwrap_or(0.0);
    let adaptive_gap_usd_margin_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.gap_usd_margin_delta)
        .unwrap_or(0.0);
    let base_required_gap_strength_before_chop =
        raw_required_gap_strength + adaptive_gap_strength_delta.max(-raw_required_gap_strength);
    let cex_magnitude_required_gap_usd = base_required_gap_strength_before_chop * expected_move_eff;
    let ptb_movement_model_book_dislocation = config
        .book_quotes
        .and_then(|quotes| selected_book_mid_for_ptb_movement(quotes, side))
        .map(|selected_mid| q_final - selected_mid);
    let ptb_chop = evaluate_price_to_beat_iv_ptb_chop(PriceToBeatIvPtbChopInput {
        config: &config.ptb_chop,
        asset,
        selected_side: side,
        samples: &samples,
        price_to_beat,
        current_price,
        latest_timestamp_ms,
        expected_move_eff,
        gap_strength,
        required_gap_strength: base_required_gap_strength_before_chop,
        cex_consensus: config
            .cex_open_gap
            .enabled
            .then_some(cex_open_gap.consensus),
        model_book_dislocation: ptb_movement_model_book_dislocation,
    });
    let base_required_gap_strength =
        base_required_gap_strength_before_chop + ptb_chop.gap_strength_penalty.max(0.0);
    let base_required_gap_usd = base_required_gap_strength * expected_move_eff;
    let gap_cap = required_gap_cap_for_asset(&config, asset);
    let (base_required_gap_usd, _, _) = apply_required_gap_cap(base_required_gap_usd, gap_cap);
    let min_gap_strength_margin = base_min_gap_strength_margin;
    let min_gap_usd_margin = base_min_gap_usd_margin
        .map(|value| (value + adaptive_gap_usd_margin_delta.max(-value)).max(0.0));
    let base_dynamic_threshold = base_dynamic_threshold + adaptive_edge_delta;
    let protection_depth_block_reason = if depth.block_kind == Some("slippage_too_high") {
        None
    } else {
        depth.block_reason
    };
    let protection = evaluate_price_to_beat_iv_protection(&PriceToBeatIvProtectionInput {
        mode: config.protection_mode,
        selected_side: side,
        seconds_left,
        book_quotes: config.book_quotes,
        book_lead_guard_enabled: config.book_lead_guard_enabled,
        book_lead_under_sec: config.book_lead_under_sec,
        book_lead_min_mid_diff: config.book_lead_min_mid_diff,
        opposite_mid_block: config.opposite_mid_block,
        block_on_opposite_book_lead: config.block_on_opposite_book_lead,
        require_binance_fresh_under_sec: config.require_binance_fresh_under_sec,
        require_binance_same_direction: config.require_binance_same_direction,
        binance_fresh: binance_adjustment.is_fresh(),
        binance_same_direction,
        model_book_gap_warn: config.model_book_gap_warn,
        model_book_gap_hard: config.too_good_to_be_true_gap,
        model_book_warn_threshold_penalty: config.model_book_warn_threshold_penalty,
        model_book_warn_gap_strength_penalty: config.model_book_warn_gap_strength_penalty,
        depth_block_reason: protection_depth_block_reason,
        late_high_price_soft_under_sec: config.late_high_price_soft_under_sec,
        late_high_price_ask: config.late_high_price_ask,
        late_high_price_selected_mid_soft: config.late_high_price_selected_mid_soft,
        late_high_price_threshold_penalty: config.late_high_price_threshold_penalty,
        late_high_price_selected_mid_hard: config.late_high_price_selected_mid_hard,
        late_high_price_min_gap_usd: config.late_high_price_min_gap_usd,
        q_final,
        gap_strength,
        required_gap_strength: base_required_gap_strength,
        directional_gap: decision_gap_effective_usd,
        required_gap_usd: base_required_gap_usd,
        min_gap_strength_margin,
        min_gap_usd_margin,
        momentum_enabled: config.momentum_protection_enabled,
        gap_velocity,
        drop_z,
        drop_z_block_threshold: config.protection_drop_z_block_threshold,
        soft_threshold_penalty_unit: config.protection_soft_threshold_penalty,
        soft_gap_strength_penalty_unit: config.protection_soft_gap_strength_penalty,
    });
    let mut required_gap_strength =
        base_required_gap_strength + protection.gap_strength_penalty.max(0.0);
    let mut required_gap_usd = required_gap_strength * expected_move_eff;
    let (capped_required_gap_usd, applied_gap_cap, mut required_gap_usd_capped) =
        apply_required_gap_cap(required_gap_usd, gap_cap);
    required_gap_usd = capped_required_gap_usd;
    let dynamic_threshold_before_participation =
        base_dynamic_threshold + protection.threshold_penalty.max(0.0);
    let participation_credit =
        super::iv_mismatch_participation::price_to_beat_iv_participation_threshold_credit(&config);
    let dynamic_threshold = if participation_credit > 0.0 {
        (dynamic_threshold_before_participation - participation_credit)
            .max(config.participation_min_threshold.max(0.0))
    } else {
        dynamic_threshold_before_participation
    };
    let telemetry_cost = cost;
    let edge_adjusted_telemetry = q_final - telemetry_cost;
    let adjusted_margin = edge_adjusted_telemetry - dynamic_threshold;
    let min_adjusted_margin = config.min_adjusted_margin.max(0.0);
    let min_final_q = config
        .min_final_q
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    let thin_margin_flag = adjusted_margin < min_adjusted_margin;
    let confidence_score = (adjusted_margin - min_adjusted_margin).max(0.0);

    evaluation.q = Some(q);
    evaluation.q_up = Some(q_up);
    evaluation.q_down = Some(q_down);
    evaluation.depth = depth;
    evaluation.fee = Some(fee);
    evaluation.cost = Some(cost);
    evaluation.edge = Some(edge);
    evaluation.telemetry_cost = Some(telemetry_cost);
    evaluation.iv_ratio = iv_ratio;
    evaluation.expected_move = Some(expected_move);
    evaluation.z = Some(z);
    evaluation.x_now = Some(x_now);
    evaluation.x_prev = x_prev;
    evaluation.gap_velocity = gap_velocity;
    evaluation.latency_horizon_secs = Some(latency_horizon_secs);
    evaluation.x_eff = Some(x_eff);
    evaluation.sigma_15 = sigma_15;
    evaluation.cex_sigma = cex_sigma;
    evaluation.sigma_eff = Some(sigma_eff);
    evaluation.sigma_eff_source = Some(sigma_eff_source);
    evaluation.expected_move_model = Some(expected_move_model);
    evaluation.expected_move_floor = expected_move_floor;
    evaluation.expected_move_eff = Some(expected_move_eff);
    evaluation.q_before_floor = Some(q_before_floor);
    evaluation.q_after_floor = Some(q_after_floor);
    evaluation.q_chain_adj = Some(q_chain_adj);
    evaluation.binance_price = binance_adjustment.binance_price;
    evaluation.binance_staleness_ms = binance_adjustment.binance_staleness_ms;
    evaluation.q_binance = binance_adjustment.q_binance;
    evaluation.q_final = Some(q_final);
    evaluation.edge_adj = Some(edge_adjusted_telemetry);
    evaluation.edge_adjusted_telemetry = Some(edge_adjusted_telemetry);
    evaluation.adjusted_margin = Some(adjusted_margin);
    evaluation.min_adjusted_margin = Some(min_adjusted_margin);
    evaluation.thin_margin_flag = Some(thin_margin_flag);
    evaluation.min_final_q = min_final_q;
    evaluation.q_disagreement = binance_disagreement.adverse;
    evaluation.q_disagreement_abs = binance_disagreement.absolute;
    evaluation.q_disagreement_bucket = binance_disagreement.bucket;
    evaluation.dynamic_threshold_before_participation =
        Some(dynamic_threshold_before_participation);
    evaluation.dynamic_threshold = Some(dynamic_threshold);
    evaluation.participation_credit = Some(participation_credit);
    evaluation.participation_last_fill_age_minutes = config.participation_last_fill_age_minutes;
    evaluation.high_price_penalty_applied = Some(high_price_penalty);
    evaluation.stale_penalty_applied = Some(stale_penalty);
    evaluation.drop_penalty_applied = Some(drop_penalty);
    evaluation.binance_missing_penalty_applied = Some(binance_missing_penalty);
    evaluation.binance_disagreement_penalty_applied = Some(binance_disagreement.penalty);
    evaluation.confidence_score = Some(confidence_score);
    evaluation.drop_z = Some(drop_z);
    evaluation.binance_veto_status = Some(binance_adjustment.status.clone());
    evaluation.gap_strength = Some(gap_strength);
    evaluation.required_gap_strength = Some(required_gap_strength);
    evaluation.required_gap_usd = Some(required_gap_usd);
    evaluation.required_gap_usd_cap = applied_gap_cap;
    evaluation.required_gap_usd_capped = Some(required_gap_usd_capped);
    evaluation.gap_strength_stale_penalty = Some(gap_strength_stale_penalty);
    evaluation.gap_strength_velocity_penalty = Some(gap_strength_velocity_penalty);
    evaluation.protection_result = Some(protection.result);
    evaluation.protection_reasons = protection.reasons.clone();
    evaluation.protection_threshold_penalty = Some(protection.threshold_penalty);
    evaluation.protection_gap_strength_penalty = Some(protection.gap_strength_penalty);
    evaluation.up_mid = protection.up_mid;
    evaluation.down_mid = protection.down_mid;
    evaluation.book_side = protection.book_side;
    evaluation.book_mid_diff = protection.book_mid_diff;
    evaluation.opposite_mid = protection.opposite_mid;
    evaluation.selected_mid = protection.selected_mid;
    evaluation.selected_ask = protection.selected_ask;
    evaluation.model_book_gap = protection.model_book_gap;
    evaluation.book_confirmation_result = if protection
        .reasons
        .contains(&"blocked_model_book_not_confirmed")
    {
        Some("blocked_model_book_not_confirmed")
    } else if protection
        .reasons
        .contains(&"warn_model_book_not_confirmed")
    {
        Some("warn_model_book_not_confirmed")
    } else if protection.selected_mid.is_some() {
        Some("pass")
    } else {
        None
    };
    evaluation.gap_strength_margin = Some(gap_strength - required_gap_strength);
    evaluation.gap_usd_margin = Some(decision_gap_effective_usd - required_gap_usd);
    evaluation.min_gap_strength_margin = min_gap_strength_margin;
    evaluation.min_gap_usd_margin = min_gap_usd_margin;
    evaluation.binance_same_direction = binance_same_direction;
    evaluation.falling_knife_flag = protection.falling_knife_flag;
    evaluation.expected_move_floor_debug = expected_move_floor_debug;
    evaluation.ptb_chop = ptb_chop.clone();
    evaluation.adaptive = adaptive.clone();
    let execution_vwap =
        evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
            config: config.execution_vwap_guard,
            time_rule_price_blocked,
            time_rule_max_price: evaluation.time_rule_max_price,
            model_ask: ask,
            depth: &evaluation.depth,
            effective_max_price: evaluation.effective_max_price,
            q_final,
            dynamic_threshold,
            safety_buffer: config.buffer,
        });
    let execution_vwap_block_reason = execution_vwap.block_reason;
    evaluation.execution_vwap = execution_vwap;
    let high_price_early_reversal = evaluate_price_to_beat_iv_high_price_early_reversal(
        PriceToBeatIvHighPriceEarlyReversalInput {
            config: &config.high_price_early_reversal,
            decision_ref: evaluation.execution_vwap.raw_execution_cost.or(Some(ask)),
            seconds_left: Some(seconds_left),
            q_final: Some(q_final),
            q_binance: binance_adjustment.q_binance,
            binance_fail_open: binance_adjustment.is_missing(),
            chainlink_staleness_ms: Some(staleness_ms),
            gap_strength: Some(gap_strength),
            base_required_gap_strength: required_gap_strength,
            cex_consensus: config
                .cex_open_gap
                .enabled
                .then_some(cex_open_gap.consensus),
            cex_clean_lane: config
                .cex_open_gap
                .enabled
                .then_some(cex_open_gap.clean_lane),
        },
    );
    if high_price_early_reversal.applies {
        if let Some(effective_required_gap_strength) =
            high_price_early_reversal.effective_required_gap_strength
        {
            required_gap_strength = effective_required_gap_strength;
            required_gap_usd = required_gap_strength * expected_move_eff;
            let (recapped, _, recapped_flag) = apply_required_gap_cap(required_gap_usd, gap_cap);
            required_gap_usd = recapped;
            if recapped_flag {
                required_gap_usd_capped = true;
            }
            evaluation.required_gap_strength = Some(required_gap_strength);
            evaluation.required_gap_usd = Some(required_gap_usd);
            evaluation.required_gap_usd_cap = applied_gap_cap;
            evaluation.required_gap_usd_capped = Some(required_gap_usd_capped);
            evaluation.gap_strength_margin = Some(gap_strength - required_gap_strength);
            evaluation.gap_usd_margin = Some(decision_gap_effective_usd - required_gap_usd);
        }
    }
    let high_price_early_block_reason = high_price_early_reversal.block_reason;
    evaluation.high_price_early_reversal = high_price_early_reversal;
    let price_band_guard =
        evaluate_price_to_beat_iv_price_band_guard(PriceToBeatIvPriceBandGuardInput {
            config: &config.price_band_guard,
            seconds_left,
            gap_strength,
            existing_required_gap_strength: required_gap_strength,
            q_final: Some(q_final),
            spread: Some(spread),
            execution_vwap: &evaluation.execution_vwap,
            cex_open_gap: &cex_open_gap,
            book_confirmation: Some(ptb_movement_model_book_dislocation.is_some()),
            chainlink_stale_penalty: Some(stale_penalty > 0.0 || gap_strength_stale_penalty > 0.0),
        });
    let price_band_block_reason = price_band_guard.block_reason;
    evaluation.price_band_guard = price_band_guard;
    let executable_all_in_cost = evaluation
        .execution_vwap
        .execution_cost_for_edge
        .filter(|value| value.is_finite())
        .filter(|_| evaluation.execution_vwap.cost_source == Some("execution_vwap"));
    let (decision_cost, edge_cost_source, edge_cost_warning) =
        if let Some(executable_all_in_cost) = executable_all_in_cost {
            (executable_all_in_cost, "executable_all_in_cost", None)
        } else {
            (
                telemetry_cost,
                "telemetry_cost",
                Some("executable_cost_unavailable_or_not_used"),
            )
        };
    let edge = q_final - decision_cost;
    let edge_adj = q_final - decision_cost;
    let adjusted_margin = edge_adj - dynamic_threshold;
    let medium_chop_margin =
        evaluate_price_to_beat_iv_medium_chop_margin(PriceToBeatIvMediumChopMarginInput {
            config: &config.medium_chop_margin,
            movement_mode: evaluation.ptb_chop.movement_mode,
            decision_ref: evaluation.execution_vwap.raw_execution_cost.or(Some(ask)),
            adjusted_margin,
            binance_fail_open: binance_adjustment.is_missing(),
            chainlink_staleness_ms: staleness_ms,
        });
    let min_adjusted_margin =
        min_adjusted_margin.max(medium_chop_margin.required_margin.unwrap_or(0.0));
    let thin_margin_flag = adjusted_margin < min_adjusted_margin;
    let confidence_score = (adjusted_margin - min_adjusted_margin).max(0.0);
    evaluation.cost = Some(decision_cost);
    evaluation.edge = Some(edge);
    evaluation.decision_cost = Some(decision_cost);
    evaluation.executable_all_in_cost = executable_all_in_cost;
    evaluation.edge_cost_source = Some(edge_cost_source);
    evaluation.edge_cost_warning = edge_cost_warning;
    evaluation.edge_adj = Some(edge_adj);
    evaluation.edge_adjusted_decision = Some(edge_adj);
    evaluation.adjusted_margin = Some(adjusted_margin);
    evaluation.min_adjusted_margin = Some(min_adjusted_margin);
    evaluation.thin_margin_flag = Some(thin_margin_flag);
    evaluation.confidence_score = Some(confidence_score);
    evaluation.medium_chop_margin = medium_chop_margin;
    let cex_magnitude = evaluate_price_to_beat_iv_cex_magnitude(PriceToBeatIvCexMagnitudeInput {
        config: config.cex_magnitude,
        gap_strength,
        required_gap_strength,
        required_gap_usd_for_ratio: cex_magnitude_required_gap_usd,
        same_side_age_seconds: evaluation.ptb_chop.same_side_age_seconds,
        book_confirmation_available: ptb_movement_model_book_dislocation.is_some(),
        cex_open_gap: &cex_open_gap,
    });
    let cex_magnitude_block_reason = cex_magnitude.block_reason;
    if cex_magnitude.is_shallow() {
        cex_open_gap.clean_lane = false;
    }
    evaluation.cex_magnitude = cex_magnitude;
    let gap_fail_cex_book =
        evaluate_price_to_beat_iv_gap_fail_cex_book_guard(PriceToBeatIvGapFailCexBookGuardInput {
            config: config.gap_fail_cex_book,
            seconds_left,
            gap_strength,
            required_gap_strength,
            q_raw: q,
            book_confirmation_available: ptb_movement_model_book_dislocation.is_some(),
            cex_open_gap: &cex_open_gap,
            execution_vwap: &evaluation.execution_vwap,
        });
    let gap_fail_cex_book_block_reason = gap_fail_cex_book.block_reason;
    evaluation.gap_fail_cex_book = gap_fail_cex_book;
    let oracle_lag_book_lead =
        evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
            config: config.oracle_lag_book_lead,
            seconds_left,
            q_final,
            execution_vwap: &evaluation.execution_vwap,
            spread: Some(spread),
            cex_consensus: config
                .cex_open_gap
                .enabled
                .then_some(cex_open_gap.consensus),
            cex_lead_override_applies: cex_lead_override,
        });
    let oracle_lag_block_reason = oracle_lag_book_lead.block_reason;
    evaluation.oracle_lag_book_lead = oracle_lag_book_lead;
    let chainlink_cex_book_mismatch_block_reason = cex_open_gap_book_mismatch_reason(
        &config.cex_open_gap,
        &cex_open_gap,
        evaluation.oracle_lag_book_lead.dislocation,
    );
    cex_open_gap.chainlink_cex_book_mismatch_reason = chainlink_cex_book_mismatch_block_reason;
    let pump_hold_gap = side_gap_at_or_before(
        &samples,
        side,
        price_to_beat,
        latest_timestamp_ms - config.pump_shock.min_hold_ms.max(0),
    );
    let pump_shock = evaluate_price_to_beat_iv_pump_shock(PriceToBeatIvPumpShockInput {
        config: config.pump_shock,
        seconds_left,
        x_now,
        x_prev,
        expected_move_eff,
        same_side_gap_at_hold: pump_hold_gap,
        model_book_dislocation: evaluation.oracle_lag_book_lead.dislocation,
        dislocation_red: config.oracle_lag_book_lead.dislocation_red,
        cex_consensus: config
            .cex_open_gap
            .enabled
            .then_some(cex_open_gap.consensus),
        execution_ref_reliable: Some(
            evaluation.oracle_lag_book_lead.reference_status == "reliable",
        ),
        token_price_confirming: Some(gap_velocity.unwrap_or(0.0) >= 0.0),
        book_dislocation_improving: None,
    });
    let pump_shock_block_reason = pump_shock.block_reason;
    evaluation.pump_shock = pump_shock;
    evaluation.oracle_tick_jump = oracle_tick_jump;
    evaluation.cex_open_gap = cex_open_gap;
    let borderline_pump_book_lead = evaluate_price_to_beat_iv_borderline_pump_book_lead(
        PriceToBeatIvBorderlinePumpBookLeadInput {
            config: config.borderline_pump_book_lead,
            seconds_left,
            gap_strength,
            required_gap_strength_raw: required_gap_strength,
            q_final,
            oracle_lag_book_lead: &evaluation.oracle_lag_book_lead,
            pump_shock: &evaluation.pump_shock,
        },
    );
    let borderline_pump_book_lead_block_reason = borderline_pump_book_lead.block_reason;
    evaluation.borderline_pump_book_lead = borderline_pump_book_lead;

    let token_crash_cooldown = evaluate_price_to_beat_iv_token_crash_cooldown(
        PriceToBeatIvTokenCrashCooldownInput {
            config: &config.token_crash_cooldown,
            market_slug,
            outcome_label,
            now: chrono::Utc::now(),
        },
        &[],
    );
    let token_crash_block_reason = token_crash_cooldown.block_reason;
    evaluation.token_crash_cooldown = token_crash_cooldown;

    let mut eq77_entry_quality_gap_override_requested = false;
    let mut entry_quality_block_reason = None;
    if config.entry_quality.enabled {
        let entry_quality = evaluate_iv_entry_quality(IvEntryQualityInput {
            config: &config.entry_quality,
            side,
            price_to_beat,
            current_price,
            samples: &samples,
            latest_timestamp_ms,
            seconds_left,
            ask,
            spread,
            chainlink_age_ms: Some(staleness_ms),
            expected_move_raw: expected_move_model,
            expected_move_eff,
            q_final: Some(q_final),
            fee,
            buffer: config.buffer,
            dynamic_threshold,
            configured_max_price: evaluation.effective_max_price,
            gap_velocity,
            cex_price: binance_adjustment.binance_price,
            cex_fresh: binance_adjustment.is_fresh(),
            cex_same_direction: binance_same_direction,
            rule_required_gap_strength: Some(required_gap_strength),
            rule_gap_strength_margin: min_gap_strength_margin,
        });
        let block_reason = entry_quality.primary_reason.map(|reason| reason.as_str());
        eq77_entry_quality_gap_override_requested =
            config.entry_quality.eq77_risk_cap_enabled && entry_quality.allowed;
        evaluation.entry_quality = Some(entry_quality);
        entry_quality_block_reason = block_reason;
    }
    evaluation
        .cex_magnitude
        .record_eq77_gap_override(eq77_entry_quality_gap_override_requested);

    if time_rule_price_blocked {
        evaluation.all_reasons.push("blocked_time_rule_max_price");
        if let Some(reason) = cex_open_gap_block_reason {
            evaluation.all_reasons.push(reason);
        }
        for reason in &evaluation.gap_fail_cex_book.all_reasons {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = chainlink_cex_book_mismatch_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = oracle_lag_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = token_crash_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = borderline_pump_book_lead_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = pump_shock_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = divergence_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = decision_gap_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = oracle_tick_jump_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = execution_vwap_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = price_band_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = high_price_early_block_reason {
            evaluation.all_reasons.push(reason);
            for reason in &evaluation.high_price_early_reversal.reasons {
                evaluation.all_reasons.push(reason);
            }
        }
        if let Some(reason) = cex_magnitude_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = entry_quality_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = evaluation.ptb_chop.block_reason {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, "blocked_time_rule_max_price");
    }
    if let Some(reason) = high_price_early_block_reason {
        evaluation.all_reasons.push(reason);
        for reason in &evaluation.high_price_early_reversal.reasons {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = cex_open_gap_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = divergence_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = decision_gap_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = oracle_tick_jump_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = evaluation
        .gap_fail_cex_book
        .all_reasons
        .iter()
        .copied()
        .find(|reason| *reason == "blocked_chainlink_cex_mixed_gap_fail")
    {
        for reason in &evaluation.gap_fail_cex_book.all_reasons {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = cex_magnitude_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = gap_fail_cex_book_block_reason {
        for reason in &evaluation.gap_fail_cex_book.all_reasons {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = chainlink_cex_book_mismatch_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = oracle_lag_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = token_crash_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = borderline_pump_book_lead_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = pump_shock_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = execution_vwap_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = price_band_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = entry_quality_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = evaluation.ptb_chop.block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if config.depth_guard_enabled && config.depth_guard_hard_block_enabled {
        if let Some(reason) = evaluation.depth.block_reason {
            if evaluation.depth.block_kind == Some("slippage_too_high") {
                evaluation.depth.slippage_deferred_to_execution_vwap = true;
            } else {
                evaluation.all_reasons.push(reason);
                return evaluation.finish(false, reason);
            }
        }
    }
    if config.reject_no_opp_depth {
        let no_visible_ask = evaluation
            .depth
            .visible_ask_qty
            .map(|qty| qty <= 0.0)
            .unwrap_or(true);
        if no_visible_ask {
            let reason = "blocked_no_executable_opposite_depth";
            evaluation.all_reasons.push(reason);
            return evaluation.finish(false, reason);
        }
    }

    if let Some(reason) = protection.block_reason.filter(|reason| {
        matches!(
            *reason,
            "blocked_depth_qty_insufficient"
                | "blocked_depth_guard_unavailable"
                | "blocked_model_book_not_confirmed"
                | "blocked_late_high_price_unconfirmed"
        )
    }) {
        return evaluation.finish(false, reason);
    }
    evaluation.gap_gate = evaluate_gap_gate(PriceToBeatIvGapGateInput {
        gap_strength,
        required_gap_strength,
        min_margin: Some(0.0),
        mode: GAP_GATE_MODE_HARD_BLOCK,
        enforced: true,
    });
    if evaluation.gap_gate.should_block() {
        let reason = if evaluation.high_price_early_reversal.applies {
            evaluation.high_price_early_reversal.result = HIGH_PRICE_EARLY_REVERSAL_GAP_REASON;
            evaluation.high_price_early_reversal.block_reason =
                Some(HIGH_PRICE_EARLY_REVERSAL_GAP_REASON);
            evaluation
                .high_price_early_reversal
                .reasons
                .push("gap_below_effective_required");
            HIGH_PRICE_EARLY_REVERSAL_GAP_REASON
        } else {
            evaluation
                .gap_gate
                .reason
                .unwrap_or(GAP_GATE_REASON_BELOW_THRESHOLD)
        };
        evaluation.all_reasons.push(reason);
        if reason == HIGH_PRICE_EARLY_REVERSAL_GAP_REASON {
            for reason in &evaluation.high_price_early_reversal.reasons {
                evaluation.all_reasons.push(reason);
            }
        }
        return evaluation.finish(false, reason);
    }
    if iv_ratio
        .map(|ratio| ratio < config.iv_ratio_block_favorite_below)
        .unwrap_or(false)
    {
        return evaluation.finish(false, "blocked_iv_ratio_low");
    }
    if zero_cross_count >= config.chop_zero_cross_limit && edge_adj < config.chop_value_edge {
        return evaluation.finish(false, "blocked_chop");
    }
    if drop_z > config.falling_knife_drop_z {
        return evaluation.finish(false, "blocked_falling_knife_drop");
    }
    if -drop_z > config.rising_knife_drop_z {
        return evaluation.finish(false, "blocked_rising_knife_spike");
    }
    if gap_velocity.unwrap_or(0.0) < 0.0
        && edge_adj < dynamic_threshold.max(config.recovery_edge_threshold)
    {
        return evaluation.finish(false, "blocked_waiting_recovery");
    }
    if let Some(reason) = adaptive.and_then(|evaluation| evaluation.block_reason) {
        return evaluation.finish(false, reason);
    }
    if min_final_q
        .map(|minimum| q_final < minimum)
        .unwrap_or(false)
    {
        return evaluation.finish(false, "blocked_low_final_q");
    }
    if edge_adj < dynamic_threshold {
        return evaluation.finish(false, "blocked_edge_below_threshold");
    }
    if adjusted_margin < min_adjusted_margin {
        return evaluation.finish(false, "blocked_thin_adjusted_margin");
    }
    if let Some(reason) = protection.block_reason {
        return evaluation.finish(false, reason);
    }

    evaluation.finish(true, "selected_edge_passed")
}

fn chainlink_stale_tolerance_result(staleness_ms: i64, effective_stale_ms: i64) -> &'static str {
    if is_chainlink_stale(staleness_ms, effective_stale_ms) {
        "stale_candidate"
    } else if staleness_ms > DEFAULT_CHAINLINK_STALE_MS
        && effective_stale_ms > DEFAULT_CHAINLINK_STALE_MS
    {
        "pass_stale_tolerance"
    } else {
        "pass_fresh"
    }
}

fn is_chainlink_stale(staleness_ms: i64, effective_stale_ms: i64) -> bool {
    staleness_ms > effective_stale_ms
}

// Asset-bazli required_gap_usd tavani (tail guard). SOL gibi sigma patlamasi yasanan
// assetlerde sisik gap'i sinirlar. None => cap devre disi. deadband_min_usd_* pattern'i
// ile ayni: btc/eth/sol ayri, bilinmeyen asset sol default'una duser.
fn required_gap_cap_for_asset(
    config: &PriceToBeatIvMismatchEdgeConfig,
    asset: &str,
) -> Option<f64> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => config.required_gap_usd_max_btc,
        "eth" => config.required_gap_usd_max_eth,
        "sol" => config.required_gap_usd_max_sol,
        _ => config.required_gap_usd_max_sol,
    }
    .filter(|value| value.is_finite() && *value > 0.0)
}

// required_gap_usd'ye asset cap uygular. Cap devreye girerse Some(cap) ve true doner;
// girmezse ham deger ve false doner. Telemetri icin (capped, applied_cap) donus.
fn apply_required_gap_cap(
    required_gap_usd: f64,
    cap: Option<f64>,
) -> (f64, Option<f64>, bool) {
    match cap {
        Some(cap) if required_gap_usd.is_finite() && required_gap_usd > cap => (cap, Some(cap), true),
        _ => (required_gap_usd, cap, false),
    }
}

#[cfg(test)]
static IV_MISMATCH_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

#[cfg(test)]
#[path = "iv_mismatch_edge_adaptive_tests.rs"]
mod adaptive_tests;
#[cfg(test)]
#[path = "iv_mismatch_edge_core_tests.rs"]
mod core_tests;
#[cfg(test)]
#[path = "iv_mismatch_edge_protection_tests.rs"]
mod protection_tests;
#[cfg(test)]
#[path = "iv_mismatch_edge_quality_tests.rs"]
mod quality_tests;
