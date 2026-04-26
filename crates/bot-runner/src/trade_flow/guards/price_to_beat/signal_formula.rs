use crate::trade_flow::guards::chainlink_price::{
    get_chainlink_price_samples, ChainlinkPriceSample,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{json, Value};

pub(crate) const SIGNAL_FORMULA_TAKER_FEE_RATE: f64 = 0.072;
const SIGNAL_FORMULA_DEFAULT_SLIPPAGE_BUFFER: f64 = 0.01;
const SIGNAL_FORMULA_DEFAULT_EDGE_THRESHOLD: f64 = 0.06;
const SIGNAL_FORMULA_DEFAULT_MAX_SPREAD: f64 = 0.04;
const SIGNAL_FORMULA_DEFAULT_VOLATILITY_WINDOW_SECS: i64 = 60;
const SIGNAL_FORMULA_DEFAULT_CHOP_WINDOW_SECS: i64 = 30;
const SIGNAL_FORMULA_DEFAULT_MAX_ZERO_CROSS_COUNT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatSignalFormulaMarketInput {
    pub(crate) best_bid: Option<f64>,
    pub(crate) best_ask: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatSignalFormulaConfig {
    pub(crate) market: PriceToBeatSignalFormulaMarketInput,
    pub(crate) slippage_buffer: f64,
    pub(crate) edge_threshold: f64,
    pub(crate) max_spread: f64,
    pub(crate) volatility_window_secs: i64,
    pub(crate) chop_window_secs: i64,
    pub(crate) max_zero_cross_count: usize,
}

impl PriceToBeatSignalFormulaConfig {
    pub(crate) fn taker(market: PriceToBeatSignalFormulaMarketInput) -> Self {
        Self {
            market,
            slippage_buffer: SIGNAL_FORMULA_DEFAULT_SLIPPAGE_BUFFER,
            edge_threshold: SIGNAL_FORMULA_DEFAULT_EDGE_THRESHOLD,
            max_spread: SIGNAL_FORMULA_DEFAULT_MAX_SPREAD,
            volatility_window_secs: SIGNAL_FORMULA_DEFAULT_VOLATILITY_WINDOW_SECS,
            chop_window_secs: SIGNAL_FORMULA_DEFAULT_CHOP_WINDOW_SECS,
            max_zero_cross_count: SIGNAL_FORMULA_DEFAULT_MAX_ZERO_CROSS_COUNT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatSignalFormulaEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason: &'static str,
    pub(crate) side: Option<&'static str>,
    pub(crate) seconds_left: Option<f64>,
    pub(crate) gap: Option<f64>,
    pub(crate) sigma: Option<f64>,
    pub(crate) expected_move: Option<f64>,
    pub(crate) z: Option<f64>,
    pub(crate) q_up: Option<f64>,
    pub(crate) q_down: Option<f64>,
    pub(crate) q_side: Option<f64>,
    pub(crate) ask: Option<f64>,
    pub(crate) bid: Option<f64>,
    pub(crate) spread: Option<f64>,
    pub(crate) fee: Option<f64>,
    pub(crate) cost: Option<f64>,
    pub(crate) edge: Option<f64>,
    pub(crate) edge_threshold: f64,
    pub(crate) slippage_buffer: f64,
    pub(crate) zero_cross_count: Option<usize>,
    pub(crate) max_zero_cross_count: usize,
    pub(crate) sample_count: Option<usize>,
    pub(crate) delta_count: Option<usize>,
    pub(crate) volatility_window_secs: i64,
    pub(crate) chop_window_secs: i64,
}

impl PriceToBeatSignalFormulaEvaluation {
    fn new(config: PriceToBeatSignalFormulaConfig) -> Self {
        Self {
            passed: false,
            reason: "pending",
            side: None,
            seconds_left: None,
            gap: None,
            sigma: None,
            expected_move: None,
            z: None,
            q_up: None,
            q_down: None,
            q_side: None,
            ask: config.market.best_ask,
            bid: config.market.best_bid,
            spread: None,
            fee: None,
            cost: None,
            edge: None,
            edge_threshold: config.edge_threshold,
            slippage_buffer: config.slippage_buffer,
            zero_cross_count: None,
            max_zero_cross_count: config.max_zero_cross_count,
            sample_count: None,
            delta_count: None,
            volatility_window_secs: config.volatility_window_secs,
            chop_window_secs: config.chop_window_secs,
        }
    }

    fn finish(mut self, passed: bool, reason: &'static str) -> Self {
        self.passed = passed;
        self.reason = reason;
        self
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "passed": self.passed,
            "reason": self.reason,
            "side": self.side,
            "seconds_left": self.seconds_left,
            "gap": self.gap,
            "sigma": self.sigma,
            "expected_move": self.expected_move,
            "z": self.z,
            "q_up": self.q_up,
            "q_down": self.q_down,
            "q_side": self.q_side,
            "ask": self.ask,
            "bid": self.bid,
            "spread": self.spread,
            "fee": self.fee,
            "cost": self.cost,
            "edge": self.edge,
            "edge_threshold": self.edge_threshold,
            "slippage_buffer": self.slippage_buffer,
            "zero_cross_count": self.zero_cross_count,
            "max_zero_cross_count": self.max_zero_cross_count,
            "sample_count": self.sample_count,
            "delta_count": self.delta_count,
            "volatility_window_secs": self.volatility_window_secs,
            "chop_window_secs": self.chop_window_secs,
            "fee_rate": SIGNAL_FORMULA_TAKER_FEE_RATE,
        })
    }
}

pub(crate) fn signal_formula_taker_fee(price: f64) -> f64 {
    SIGNAL_FORMULA_TAKER_FEE_RATE * price * (1.0 - price)
}

#[allow(dead_code)]
pub(crate) fn signal_formula_pair_cost(
    up_price: f64,
    down_price: f64,
    slippage_buffer: f64,
) -> f64 {
    up_price
        + down_price
        + signal_formula_taker_fee(up_price)
        + signal_formula_taker_fee(down_price)
        + slippage_buffer.max(0.0)
}

#[allow(dead_code)]
pub(crate) fn signal_formula_fractional_kelly(q: f64, cost: f64, fraction: f64) -> Option<f64> {
    if !q.is_finite() || !cost.is_finite() || !fraction.is_finite() || cost >= 1.0 {
        return None;
    }
    let full = (q - cost) / (1.0 - cost);
    Some((fraction * full).max(0.0))
}

pub(crate) fn evaluate_price_to_beat_signal_formula(
    market_slug: &str,
    outcome_label: &str,
    asset: &str,
    current_price: f64,
    price_to_beat: f64,
    config: PriceToBeatSignalFormulaConfig,
) -> PriceToBeatSignalFormulaEvaluation {
    let mut evaluation = PriceToBeatSignalFormulaEvaluation::new(config);
    let Some(side) = signal_formula_side(outcome_label) else {
        return evaluation.finish(false, "unsupported_outcome_label");
    };
    evaluation.side = Some(side);

    let Some(seconds_left) = signal_formula_seconds_left(market_slug) else {
        return evaluation.finish(false, "market_window_unavailable");
    };
    if seconds_left <= 0.0 {
        evaluation.seconds_left = Some(0.0);
        return evaluation.finish(false, "market_window_closed");
    }
    evaluation.seconds_left = Some(seconds_left);

    let Some(ask) = config
        .market
        .best_ask
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "ask_unavailable");
    };
    let Some(bid) = config
        .market
        .best_bid
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "bid_unavailable");
    };
    if ask < bid {
        evaluation.spread = Some(ask - bid);
        return evaluation.finish(false, "invalid_spread");
    }
    let spread = ask - bid;
    evaluation.spread = Some(spread);
    if spread > config.max_spread {
        return evaluation.finish(false, "spread_too_wide");
    }

    let now_ms = Utc::now().timestamp_millis();
    let sample_window_secs = config
        .volatility_window_secs
        .max(config.chop_window_secs)
        .max(1);
    let samples =
        match get_chainlink_price_samples(asset, now_ms - sample_window_secs * 1_000, now_ms) {
            Ok(samples) => samples,
            Err(_) => return evaluation.finish(false, "chainlink_samples_unavailable"),
        };
    evaluation.sample_count = Some(samples.len());

    let deltas = price_deltas_since(
        &samples,
        now_ms - config.volatility_window_secs.max(1) * 1_000,
    );
    evaluation.delta_count = Some(deltas.len());
    if deltas.is_empty() {
        return evaluation.finish(false, "insufficient_volatility_samples");
    }
    let sigma = standard_deviation(&deltas);
    if !sigma.is_finite() || sigma <= 0.0 {
        return evaluation.finish(false, "zero_volatility");
    }
    evaluation.sigma = Some(sigma);

    let zero_cross_count = zero_cross_count_since(
        &samples,
        price_to_beat,
        now_ms - config.chop_window_secs.max(1) * 1_000,
    );
    evaluation.zero_cross_count = Some(zero_cross_count);

    let gap = current_price - price_to_beat;
    let expected_move = sigma * seconds_left.sqrt();
    if !expected_move.is_finite() || expected_move <= 0.0 {
        return evaluation.finish(false, "expected_move_unavailable");
    }
    let z = gap / expected_move;
    let q_up = normal_cdf(z);
    let q_down = 1.0 - q_up;
    let q_side = if side == "up" { q_up } else { q_down };
    let fee = signal_formula_taker_fee(ask);
    let cost = ask + fee + config.slippage_buffer.max(0.0);
    let edge = q_side - cost;

    evaluation.gap = Some(gap);
    evaluation.expected_move = Some(expected_move);
    evaluation.z = Some(z);
    evaluation.q_up = Some(q_up);
    evaluation.q_down = Some(q_down);
    evaluation.q_side = Some(q_side);
    evaluation.fee = Some(fee);
    evaluation.cost = Some(cost);
    evaluation.edge = Some(edge);

    if zero_cross_count > config.max_zero_cross_count {
        return evaluation.finish(false, "chop_filter_blocked");
    }
    if edge < config.edge_threshold {
        return evaluation.finish(false, "edge_below_threshold");
    }

    evaluation.finish(true, "edge_passed")
}

fn signal_formula_side(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn signal_formula_seconds_left(market_slug: &str) -> Option<f64> {
    let scope = crate::find_updown_scope_by_slug(market_slug)?;
    let start = crate::MarketCycleId(market_slug.to_string()).start_time()?;
    let end = start + ChronoDuration::seconds(crate::updown_scope_window_seconds(scope));
    Some(
        end.signed_duration_since(Utc::now())
            .num_milliseconds()
            .max(0) as f64
            / 1_000.0,
    )
}

fn valid_probability(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

fn price_deltas_since(samples: &[ChainlinkPriceSample], start_ms: i64) -> Vec<f64> {
    let mut previous = None;
    let mut deltas = Vec::new();
    for sample in samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= start_ms)
    {
        if let Some(previous_price) = previous {
            deltas.push(sample.price - previous_price);
        }
        previous = Some(sample.price);
    }
    deltas
}

fn standard_deviation(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn zero_cross_count_since(
    samples: &[ChainlinkPriceSample],
    price_to_beat: f64,
    start_ms: i64,
) -> usize {
    let mut previous = None;
    let mut count = 0;
    for sample in samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= start_ms)
    {
        let sign = gap_sign(sample.price - price_to_beat);
        if let Some(previous_sign) = previous {
            if sign != previous_sign {
                count += 1;
            }
        }
        previous = Some(sign);
    }
    count
}

fn gap_sign(gap: f64) -> i8 {
    if gap > 0.0 {
        1
    } else if gap < 0.0 {
        -1
    } else {
        0
    }
}

fn normal_cdf(z: f64) -> f64 {
    let x = z.abs();
    let t = 1.0 / (1.0 + 0.231_641_9 * x);
    let poly = t
        * (0.319_381_530
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    let density = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - density * poly;
    if z >= 0.0 {
        cdf
    } else {
        1.0 - cdf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taker_fee_matches_polymarket_crypto_formula() {
        let fee = signal_formula_taker_fee(0.68);
        assert!((fee - 0.015_667_2).abs() < 0.000_001);
    }

    #[test]
    fn pair_cost_includes_both_crypto_taker_fees() {
        let pair_cost = signal_formula_pair_cost(0.50, 0.40, 0.0);
        assert!((pair_cost - 0.935_28).abs() < 0.000_001);
    }

    #[test]
    fn fractional_kelly_uses_binary_share_formula() {
        let stake = signal_formula_fractional_kelly(0.76, 0.70, 0.25).expect("stake");
        assert!((stake - 0.05).abs() < 0.000_001);
    }

    #[test]
    fn normal_cdf_has_expected_shape() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 0.000_001);
        assert!((normal_cdf(1.0) - 0.841_34).abs() < 0.000_5);
    }
}
