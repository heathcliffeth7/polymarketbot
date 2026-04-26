#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatIvProtectionMode {
    Off,
    Soft,
    Hard,
    Adaptive,
}

impl PriceToBeatIvProtectionMode {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "off" => Some(Self::Off),
            "soft" | "balanced" => Some(Self::Soft),
            "hard" => Some(Self::Hard),
            "adaptive" => Some(Self::Adaptive),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Soft => "soft",
            Self::Hard => "hard",
            Self::Adaptive => "adaptive",
        }
    }

    pub(crate) fn is_active(self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvBookQuotes {
    pub(crate) up_bid: Option<f64>,
    pub(crate) up_ask: Option<f64>,
    pub(crate) down_bid: Option<f64>,
    pub(crate) down_ask: Option<f64>,
}

impl PriceToBeatIvBookQuotes {
    pub(crate) fn has_valid_book(self) -> bool {
        quote_mid(self.up_bid, self.up_ask).is_some()
            && quote_mid(self.down_bid, self.down_ask).is_some()
    }

    fn up_mid(self) -> Option<f64> {
        quote_mid(self.up_bid, self.up_ask)
    }

    fn down_mid(self) -> Option<f64> {
        quote_mid(self.down_bid, self.down_ask)
    }

    fn selected_mid(self, selected_side: &str) -> Option<f64> {
        if selected_side == "up" {
            self.up_mid()
        } else {
            self.down_mid()
        }
    }

    fn selected_ask(self, selected_side: &str) -> Option<f64> {
        if selected_side == "up" {
            normalize_quote_price(self.up_ask)
        } else {
            normalize_quote_price(self.down_ask)
        }
    }

    fn opposite_mid(self, selected_side: &str) -> Option<f64> {
        if selected_side == "up" {
            self.down_mid()
        } else {
            self.up_mid()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvProtectionInput<'a> {
    pub(crate) mode: PriceToBeatIvProtectionMode,
    pub(crate) selected_side: &'a str,
    pub(crate) seconds_left: f64,
    pub(crate) book_quotes: Option<PriceToBeatIvBookQuotes>,
    pub(crate) book_lead_guard_enabled: bool,
    pub(crate) book_lead_under_sec: f64,
    pub(crate) book_lead_min_mid_diff: f64,
    pub(crate) opposite_mid_block: Option<f64>,
    pub(crate) block_on_opposite_book_lead: bool,
    pub(crate) require_binance_fresh_under_sec: Option<f64>,
    pub(crate) require_binance_same_direction: bool,
    pub(crate) binance_fresh: bool,
    pub(crate) binance_same_direction: Option<bool>,
    pub(crate) model_book_gap_warn: Option<f64>,
    pub(crate) model_book_gap_hard: Option<f64>,
    pub(crate) model_book_warn_threshold_penalty: f64,
    pub(crate) model_book_warn_gap_strength_penalty: f64,
    pub(crate) depth_block_reason: Option<&'static str>,
    pub(crate) late_high_price_soft_under_sec: f64,
    pub(crate) late_high_price_ask: f64,
    pub(crate) late_high_price_selected_mid_soft: f64,
    pub(crate) late_high_price_threshold_penalty: f64,
    pub(crate) late_high_price_selected_mid_hard: f64,
    pub(crate) late_high_price_min_gap_usd: f64,
    pub(crate) q_final: f64,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) directional_gap: f64,
    pub(crate) required_gap_usd: f64,
    pub(crate) min_gap_strength_margin: Option<f64>,
    pub(crate) min_gap_usd_margin: Option<f64>,
    pub(crate) momentum_enabled: bool,
    pub(crate) gap_velocity: Option<f64>,
    pub(crate) drop_z: f64,
    pub(crate) drop_z_block_threshold: f64,
    pub(crate) soft_threshold_penalty_unit: f64,
    pub(crate) soft_gap_strength_penalty_unit: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvProtectionEvaluation {
    pub(crate) result: &'static str,
    pub(crate) reasons: Vec<&'static str>,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) threshold_penalty: f64,
    pub(crate) gap_strength_penalty: f64,
    pub(crate) up_mid: Option<f64>,
    pub(crate) down_mid: Option<f64>,
    pub(crate) book_side: Option<&'static str>,
    pub(crate) book_mid_diff: Option<f64>,
    pub(crate) opposite_mid: Option<f64>,
    pub(crate) selected_mid: Option<f64>,
    pub(crate) selected_ask: Option<f64>,
    pub(crate) model_book_gap: Option<f64>,
    pub(crate) gap_strength_margin: Option<f64>,
    pub(crate) gap_usd_margin: Option<f64>,
    pub(crate) falling_knife_flag: Option<bool>,
}

impl PriceToBeatIvProtectionEvaluation {
    fn pass(mode: PriceToBeatIvProtectionMode) -> Self {
        Self {
            result: if mode.is_active() { "pass" } else { "off" },
            reasons: Vec::new(),
            block_reason: None,
            threshold_penalty: 0.0,
            gap_strength_penalty: 0.0,
            up_mid: None,
            down_mid: None,
            book_side: None,
            book_mid_diff: None,
            opposite_mid: None,
            selected_mid: None,
            selected_ask: None,
            model_book_gap: None,
            gap_strength_margin: None,
            gap_usd_margin: None,
            falling_knife_flag: None,
        }
    }
}

pub(crate) fn evaluate_price_to_beat_iv_protection(
    input: &PriceToBeatIvProtectionInput<'_>,
) -> PriceToBeatIvProtectionEvaluation {
    let mut evaluation = PriceToBeatIvProtectionEvaluation::pass(input.mode);
    if !input.mode.is_active() {
        return evaluation;
    }

    let has_valid_book = match input.book_quotes {
        Some(book_quotes) if book_quotes.has_valid_book() => {
            let up_mid = book_quotes.up_mid();
            let down_mid = book_quotes.down_mid();
            evaluation.up_mid = up_mid;
            evaluation.down_mid = down_mid;
            evaluation.selected_mid = book_quotes.selected_mid(input.selected_side);
            evaluation.selected_ask = book_quotes.selected_ask(input.selected_side);
            evaluation.opposite_mid = book_quotes.opposite_mid(input.selected_side);
            if let (Some(up_mid), Some(down_mid)) = (up_mid, down_mid) {
                let diff = up_mid - down_mid;
                evaluation.book_mid_diff = Some(diff.abs());
                evaluation.book_side = Some(book_side(diff, input.book_lead_min_mid_diff));
            }
            if let Some(selected_mid) = evaluation.selected_mid {
                evaluation.model_book_gap = Some(input.q_final - selected_mid);
            }
            true
        }
        _ => false,
    };

    let book_active =
        input.book_lead_guard_enabled && input.seconds_left <= input.book_lead_under_sec.max(0.0);
    if book_active {
        if has_valid_book {
            if input.block_on_opposite_book_lead
                && is_opposite_book_side(evaluation.book_side, input.selected_side)
            {
                record_book_risk(&mut evaluation, input, "blocked_book_leads_opposite");
            }
            if let (Some(opposite_mid), Some(block_price)) =
                (evaluation.opposite_mid, input.opposite_mid_block)
            {
                if opposite_mid >= block_price {
                    record_book_risk(&mut evaluation, input, "blocked_opposite_mid_too_high");
                }
            }
        } else {
            record_book_risk(&mut evaluation, input, "blocked_book_guard_unavailable");
        }
    }

    if let Some(under_sec) = input.require_binance_fresh_under_sec {
        if input.seconds_left <= under_sec.max(0.0) {
            if !input.binance_fresh {
                record_critical_risk(&mut evaluation, input, "blocked_binance_required");
            } else if input.require_binance_same_direction
                && input.binance_same_direction != Some(true)
            {
                record_critical_risk(&mut evaluation, input, "blocked_binance_direction_mismatch");
            }
        }
    }

    if let Some(model_book_gap) = evaluation.model_book_gap {
        if input
            .model_book_gap_hard
            .filter(|gap| model_book_gap >= *gap)
            .is_some()
        {
            record_critical_risk(&mut evaluation, input, "blocked_model_book_not_confirmed");
        } else if input
            .model_book_gap_warn
            .filter(|gap| model_book_gap >= *gap)
            .is_some()
        {
            record_soft_penalty(
                &mut evaluation,
                "warn_model_book_not_confirmed",
                input.model_book_warn_threshold_penalty,
                input.model_book_warn_gap_strength_penalty,
            );
        }
    }
    if input.seconds_left <= input.late_high_price_soft_under_sec.max(0.0) {
        let late_high_price = matches!(
            (evaluation.selected_ask, evaluation.selected_mid),
            (Some(selected_ask), Some(selected_mid))
                if selected_ask >= input.late_high_price_ask
                    && selected_mid < input.late_high_price_selected_mid_soft
        );
        if late_high_price {
            record_soft_penalty(
                &mut evaluation,
                "warn_late_high_price_unconfirmed",
                input.late_high_price_threshold_penalty,
                0.0,
            );
        }
        let late_compound_risk = matches!(
            (evaluation.selected_ask, evaluation.selected_mid),
            (Some(selected_ask), Some(selected_mid))
                if selected_ask >= input.late_high_price_ask
                    && selected_mid < input.late_high_price_selected_mid_hard
                    && input.directional_gap < input.late_high_price_min_gap_usd
        );
        if late_compound_risk {
            record_critical_risk(
                &mut evaluation,
                input,
                "blocked_late_high_price_unconfirmed",
            );
        }
    }

    if let Some(reason) = input.depth_block_reason {
        record_critical_risk(&mut evaluation, input, reason);
    }

    let gap_strength_margin = input.gap_strength - input.required_gap_strength;
    let gap_usd_margin = input.directional_gap - input.required_gap_usd;
    evaluation.gap_strength_margin = Some(gap_strength_margin);
    evaluation.gap_usd_margin = Some(gap_usd_margin);
    if let Some(min_margin) = input.min_gap_strength_margin {
        if gap_strength_margin < min_margin {
            record_critical_risk(&mut evaluation, input, "blocked_thin_gap_strength_margin");
        }
    }
    if let Some(min_margin) = input.min_gap_usd_margin {
        if gap_usd_margin < min_margin {
            record_critical_risk(&mut evaluation, input, "blocked_thin_gap_usd_margin");
        }
    }

    let falling_knife_flag = input.momentum_enabled
        && input.gap_velocity.unwrap_or(0.0) < 0.0
        && input.drop_z > input.drop_z_block_threshold;
    evaluation.falling_knife_flag = Some(falling_knife_flag);
    if falling_knife_flag {
        record_critical_risk(&mut evaluation, input, "blocked_falling_knife_protection");
    }

    evaluation
}

fn record_book_risk(
    evaluation: &mut PriceToBeatIvProtectionEvaluation,
    input: &PriceToBeatIvProtectionInput<'_>,
    reason: &'static str,
) {
    match input.mode {
        PriceToBeatIvProtectionMode::Adaptive => {
            evaluation.reasons.push(reason);
            if evaluation.result == "pass" {
                evaluation.result = "adaptive_observed";
            }
        }
        _ => record_critical_risk(evaluation, input, reason),
    }
}

fn record_critical_risk(
    evaluation: &mut PriceToBeatIvProtectionEvaluation,
    input: &PriceToBeatIvProtectionInput<'_>,
    reason: &'static str,
) {
    evaluation.reasons.push(reason);
    match input.mode {
        PriceToBeatIvProtectionMode::Hard | PriceToBeatIvProtectionMode::Adaptive => {
            if evaluation.block_reason.is_none() {
                evaluation.block_reason = Some(reason);
            }
            evaluation.result = "block";
        }
        PriceToBeatIvProtectionMode::Soft => {
            evaluation.threshold_penalty += input.soft_threshold_penalty_unit.max(0.0);
            evaluation.gap_strength_penalty += input.soft_gap_strength_penalty_unit.max(0.0);
            evaluation.result = "soft_penalty";
        }
        PriceToBeatIvProtectionMode::Off => {}
    }
}

fn record_soft_penalty(
    evaluation: &mut PriceToBeatIvProtectionEvaluation,
    reason: &'static str,
    threshold_penalty: f64,
    gap_strength_penalty: f64,
) {
    evaluation.reasons.push(reason);
    evaluation.threshold_penalty += threshold_penalty.max(0.0);
    evaluation.gap_strength_penalty += gap_strength_penalty.max(0.0);
    if evaluation.block_reason.is_none() {
        evaluation.result = "soft_penalty";
    }
}

fn quote_mid(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    let bid = normalize_quote_price(bid)?;
    let ask = normalize_quote_price(ask)?;
    (ask >= bid).then_some((bid + ask) / 2.0)
}

fn normalize_quote_price(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)
}

fn book_side(diff: f64, min_mid_diff: f64) -> &'static str {
    let min_mid_diff = min_mid_diff.max(0.0);
    if diff >= min_mid_diff {
        "up"
    } else if -diff >= min_mid_diff {
        "down"
    } else {
        "neutral"
    }
}

fn is_opposite_book_side(book_side: Option<&str>, selected_side: &str) -> bool {
    matches!(
        (book_side, selected_side),
        (Some("up"), "down") | (Some("down"), "up")
    )
}
