use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvMismatchTimeRule {
    pub(crate) start_remaining_secs: f64,
    pub(crate) end_remaining_secs: f64,
    pub(crate) max_price: Option<f64>,
    pub(crate) min_edge: f64,
    pub(crate) min_gap_strength: f64,
    pub(crate) min_expected_move_usd: Option<f64>,
    pub(crate) min_gap_strength_margin: Option<f64>,
    pub(crate) min_gap_usd_margin: Option<f64>,
}

impl PriceToBeatIvMismatchTimeRule {
    pub(crate) fn matches_seconds_left(self, seconds_left: f64) -> bool {
        seconds_left <= self.start_remaining_secs && seconds_left > self.end_remaining_secs
    }

    pub(crate) fn to_value(self, index: usize) -> Value {
        json!({
            "index": index,
            "start_remaining_secs": self.start_remaining_secs,
            "end_remaining_secs": self.end_remaining_secs,
            "max_price": self.max_price,
            "min_edge": self.min_edge,
            "min_gap_strength": self.min_gap_strength,
            "min_expected_move_usd": self.min_expected_move_usd,
            "min_gap_strength_margin": self.min_gap_strength_margin,
            "min_gap_usd_margin": self.min_gap_usd_margin,
        })
    }
}

pub(crate) fn select_time_rule(
    seconds_left: f64,
    rules: &[PriceToBeatIvMismatchTimeRule],
) -> Option<(usize, PriceToBeatIvMismatchTimeRule)> {
    rules
        .iter()
        .copied()
        .enumerate()
        .find(|(_, rule)| rule.matches_seconds_left(seconds_left))
}
