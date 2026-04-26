use super::current_price::format_current_price_label;
use super::*;

fn format_optional_direction(value: Option<&str>) -> String {
    match value {
        Some("up") => "Up".to_string(),
        Some("down") => "Down".to_string(),
        Some(other) => other.to_string(),
        None => "N/A".to_string(),
    }
}

fn format_optional_guard_number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn normalize_guard_threshold_usd(value: f64, unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "usd" => Some(value),
        "cent" => Some(value / 100.0),
        _ => None,
    }
}

fn format_guard_threshold_summary(value: f64, unit: &str, usd: f64) -> String {
    let unit = unit.trim();
    if unit.eq_ignore_ascii_case("usd") {
        return format!("{value:.8} USD");
    }

    match normalize_guard_threshold_usd(value, unit) {
        Some(normalized_usd) if (normalized_usd - usd).abs() <= 1e-9 => {
            format!("{value:.8} {unit} (~{usd:.8} USD)")
        }
        _ => format!("{usd:.8} USD"),
    }
}

fn format_optional_guard_threshold_summary(
    value: Option<f64>,
    unit: Option<&str>,
    usd: Option<f64>,
) -> Option<String> {
    let value = value?;
    let unit = unit?.trim();
    if unit.is_empty() {
        return None;
    }

    if let Some(usd) = usd.or_else(|| normalize_guard_threshold_usd(value, unit)) {
        return Some(format_guard_threshold_summary(value, unit, usd));
    }

    Some(format!("{value:.8} {unit}"))
}

fn build_stop_loss_bump_summary(evaluation: &PriceToBeatGuardEvaluation) -> Option<String> {
    if evaluation.stop_loss_bump_amount.is_none()
        && evaluation.stop_loss_bump_count <= 0
        && evaluation.stop_loss_bump_usd <= 0.0
        && evaluation.stop_loss_bump_max_value.is_none()
    {
        return None;
    }

    let mut parts = Vec::new();
    if let (Some(amount), Some(unit)) = (
        evaluation.stop_loss_bump_amount,
        evaluation.stop_loss_bump_unit.as_deref(),
    ) {
        parts.push(format!("kademe {amount:.8} {unit}"));
    }
    if evaluation.stop_loss_bump_count > 0 {
        parts.push(format!("sayac {}", evaluation.stop_loss_bump_count));
    }
    if evaluation.stop_loss_bump_applied_count > 0
        || evaluation.stop_loss_bump_current_market_excluded
    {
        parts.push(format!(
            "uygulanan sayac {}",
            evaluation.stop_loss_bump_applied_count
        ));
    }
    if evaluation.stop_loss_bump_amount.is_some() || evaluation.stop_loss_bump_usd > 0.0 {
        parts.push(format!(
            "uygulanan {:.8} USD",
            evaluation.stop_loss_bump_usd
        ));
    }
    if let (Some(max_value), Some(unit)) = (
        evaluation.stop_loss_bump_max_value,
        evaluation.stop_loss_bump_unit.as_deref(),
    ) {
        parts.push(format!("max {:.8} {unit}", max_value));
    }
    if evaluation.stop_loss_bump_capped {
        parts.push("cap uygulandi".to_string());
    }
    if evaluation.stop_loss_bump_max_reached {
        parts.push("max limite ulasti".to_string());
    }
    if evaluation.stop_loss_bump_current_market_excluded {
        parts.push("bu market dislandi".to_string());
    }

    Some(parts.join(", "))
}

fn stop_loss_bump_is_active(evaluation: &PriceToBeatGuardEvaluation) -> bool {
    const PTB_BUMP_ACTIVE_EPSILON: f64 = 1e-9;

    if !evaluation.stop_loss_bump_usd.is_finite() || evaluation.stop_loss_bump_usd <= 0.0 {
        return false;
    }

    let baseline_usd = evaluation
        .base_threshold_usd
        .or(evaluation.auto_threshold_usd);
    let effective_usd = evaluation
        .current_effective_ptb_usd
        .or(Some(evaluation.threshold_usd))
        .filter(|value| value.is_finite());

    match (baseline_usd, effective_usd) {
        (Some(baseline_usd), Some(effective_usd)) => {
            effective_usd > baseline_usd + PTB_BUMP_ACTIVE_EPSILON
        }
        _ => false,
    }
}

fn build_price_to_beat_summary_block(evaluation: &PriceToBeatGuardEvaluation) -> String {
    let mut lines = Vec::new();
    let configured_mode = evaluation
        .configured_threshold_mode
        .as_deref()
        .unwrap_or(evaluation.threshold_mode.as_str());
    lines.push(format!("Configured Mod: {configured_mode}"));
    lines.push(format!(
        "Efektif PTB: {}",
        format_guard_threshold_summary(
            evaluation.threshold_value,
            &evaluation.threshold_unit,
            evaluation.threshold_usd,
        )
    ));

    if let Some(base_threshold_summary) = format_optional_guard_threshold_summary(
        evaluation.base_threshold_value,
        evaluation.base_threshold_unit.as_deref(),
        evaluation.base_threshold_usd,
    ) {
        lines.push(format!("Base PTB: {base_threshold_summary}"));
    }
    if let Some(auto_threshold_usd) = evaluation.auto_threshold_usd {
        lines.push(format!("Auto Threshold: {auto_threshold_usd:.8} USD"));
    }
    if evaluation.reentry_override_active {
        let reentry_override_summary = format_optional_guard_threshold_summary(
            evaluation.reentry_override_value,
            evaluation.reentry_override_unit.as_deref(),
            None,
        )
        .unwrap_or_else(|| "aktif".to_string());
        lines.push(format!("Re-entry Override: {reentry_override_summary}"));
    }
    if stop_loss_bump_is_active(evaluation) {
        if let Some(stop_loss_bump_summary) = build_stop_loss_bump_summary(evaluation) {
            lines.push(format!("SL Bump: {stop_loss_bump_summary}"));
        }
    }

    format!("\n{}", lines.join("\n"))
}

fn build_iv_mismatch_execution_summary(evaluation: &PriceToBeatGuardEvaluation) -> String {
    let Some(iv) = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    let number = |key: &str| iv.get(key).and_then(Value::as_f64);
    let text = |key: &str| iv.get(key).and_then(Value::as_str).unwrap_or("N/A");
    let mut lines = Vec::new();
    if iv.contains_key("depth_guard_result") {
        lines.push("Depth:".to_string());
        lines.push(format!(
            "Best ask: {}",
            format_optional_guard_number(number("depth_best_ask"))
        ));
        lines.push(format!(
            "Target qty: {}",
            format_optional_guard_number(number("intended_qty"))
        ));
        lines.push(format!(
            "Estimated avg fill: {}",
            format_optional_guard_number(number("estimated_avg_fill"))
        ));
        lines.push(format!(
            "VWAP slippage: {}",
            format_optional_guard_number(number("vwap_slippage"))
        ));
        lines.push(format!(
            "Available best ask qty: {}",
            format_optional_guard_number(number("available_qty_at_best_ask"))
        ));
        lines.push(format!(
            "Levels used: {}",
            format_optional_guard_number(number("depth_levels_used"))
        ));
        lines.push(format!("Result: {}", text("depth_guard_result")));
    }
    if iv.contains_key("model_book_gap") {
        lines.push("Model-book:".to_string());
        lines.push(format!(
            "q_final: {}",
            format_optional_guard_number(number("q_final"))
        ));
        lines.push(format!(
            "selected_mid: {}",
            format_optional_guard_number(number("selected_mid"))
        ));
        lines.push(format!(
            "gap: {}",
            format_optional_guard_number(number("model_book_gap"))
        ));
        lines.push(format!(
            "threshold: {}",
            format_optional_guard_number(number("too_good_threshold"))
        ));
        lines.push(format!("Result: {}", text("book_confirmation_result")));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}", lines.join("\n"))
    }
}

fn format_current_ptb_summary(
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    format_optional_guard_threshold_summary(current_ptb_value, current_ptb_unit, current_ptb_usd)
        .unwrap_or_else(|| "Bilinmiyor".to_string())
}

pub(super) fn build_price_to_beat_guard_blocked_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    let reason = match evaluation.reason_code.as_str() {
        "price_to_beat_gap_below_threshold" => {
            "Secilen yon icin Price to Beat farki gereken minimum seviyenin altinda."
        }
        "price_to_beat_pending" => {
            "Price to Beat verisi henuz hazir degil, cycle-open fiyat snapshot'i bekleniyor."
        }
        "price_to_beat_unavailable" => {
            "Polymarket Price to Beat verisi alinamadigi icin emir engellendi."
        }
        "current_price_unavailable" => "Current price verisi alinamadigi icin emir engellendi.",
        "unsupported_market" => "Bu market Price to Beat guard tarafindan desteklenmiyor.",
        "unsupported_outcome_label" => {
            "Outcome label Up/Down veya Yes/No yonlerinden biri olarak taninamadi."
        }
        _ => "Price to Beat guard emri engelledi.",
    };

    let detail_line = evaluation
        .reason_detail
        .as_deref()
        .map(|detail| format!("\nDetay: {detail}"))
        .unwrap_or_default();
    let metric_line = evaluation
        .direction
        .as_deref()
        .map(|_| {
            "\nKarar Metrigi: Yonsel fark kullanilir. Up=current-price_to_beat, Down=price_to_beat-current. Mutlak fark sadece bilgidir."
        })
        .unwrap_or_default();
    let summary_block = build_price_to_beat_summary_block(evaluation);
    let execution_summary = build_iv_mismatch_execution_summary(evaluation);

    format!(
        "{}\nSebep: {}{}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}{}\nLimit: {:.8} {} (~{:.8} USD){}{}",
        "Price to Beat Korumasi Engelledi",
        reason,
        detail_line,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        evaluation.price_to_beat_status.as_deref().unwrap_or("N/A"),
        evaluation.price_to_beat_source.as_deref().unwrap_or("N/A"),
        format_current_price_label(evaluation.current_price_source),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.directional_gap),
        format_optional_guard_number(evaluation.gap_abs),
        metric_line,
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd,
        summary_block,
        execution_summary,
    )
}

pub(super) fn build_price_to_beat_guard_waiting_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    format!(
        "{}\nDurum: Bekleme moduna alindi. Kosullar duzelince order yeniden denenecek.",
        build_price_to_beat_guard_blocked_notification_message(evaluation)
    )
}

pub(super) fn build_price_to_beat_guard_recovered_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    recovered_from_reason_code: &str,
) -> String {
    let summary_block = build_price_to_beat_summary_block(evaluation);
    let execution_summary = build_iv_mismatch_execution_summary(evaluation);
    format!(
        "{}\nDurum: Price to Beat yeniden uygun hale geldi.\nOnceki Sebep: {}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}\nLimit: {:.8} {} (~{:.8} USD){}{}",
        "Price to Beat Korumasi Gecti",
        recovered_from_reason_code,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        evaluation.price_to_beat_status.as_deref().unwrap_or("N/A"),
        evaluation.price_to_beat_source.as_deref().unwrap_or("N/A"),
        format_current_price_label(evaluation.current_price_source),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.directional_gap),
        format_optional_guard_number(evaluation.gap_abs),
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd,
        summary_block,
        execution_summary,
    )
}

pub(super) fn build_price_to_beat_relax_changed_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    previous_threshold_usd: Option<f64>,
    raw_target_threshold_usd: Option<f64>,
    next_threshold_usd: f64,
    min_gap_usd: Option<f64>,
    buffer_usd: f64,
    floor_usd: f64,
    miss_streak: i64,
    qualified_market_slugs: &[String],
) -> String {
    let qualified_market_summary = if qualified_market_slugs.is_empty() {
        "N/A".to_string()
    } else {
        qualified_market_slugs.join(", ")
    };
    let summary_block = build_price_to_beat_summary_block(evaluation);
    format!(
        "{}\nMarket: {}\nYon: {}\nAsset: {}\nTimeframe: {}\nOnceki Bildirilen Relax PTB: {}\nHam Relax PTB: {}\nBu Market Efektif Relax PTB: {:.8}\nMin Uygun Gap: {}\nTampon: {:.8}\nFloor: {:.8}\nMiss Streak: {}\nQualified Markets: {}{}",
        "Price to Beat Relax Guncellendi",
        evaluation.market_slug,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(previous_threshold_usd),
        format_optional_guard_number(raw_target_threshold_usd),
        next_threshold_usd,
        format_optional_guard_number(min_gap_usd),
        buffer_usd,
        floor_usd,
        miss_streak,
        qualified_market_summary,
        summary_block,
    )
}

pub(super) fn build_price_to_beat_relax_miss_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    previous_miss_streak: Option<i64>,
    next_miss_streak: i64,
    missed_market_slug: Option<&str>,
    tradable_seconds_count: i64,
    max_fillability_score: Option<f64>,
    config_miss_count: i64,
    relax_active: bool,
    effective_target_threshold_usd: Option<f64>,
) -> String {
    let previous_miss_streak = previous_miss_streak
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let missed_market_slug = missed_market_slug.unwrap_or("N/A");
    let relax_status = if relax_active {
        format!(
            "aktif\nGuncel Efektif Relax PTB: {}",
            format_optional_guard_number(effective_target_threshold_usd)
        )
    } else {
        "threshold henuz gevsemedi".to_string()
    };
    let summary_block = build_price_to_beat_summary_block(evaluation);
    format!(
        "{}\nMarket: {}\nMissed Market: {}\nYon: {}\nAsset: {}\nTimeframe: {}\nOnceki Bildirilen Miss Streak: {}\nYeni Miss Streak: {}\nMissed Tradable Seconds: {}\nMissed Max Fillability: {}\nConfigured Miss Count: {}\nRelax Durumu: {}{}",
        "Price to Beat Relax Miss Artti",
        evaluation.market_slug,
        missed_market_slug,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        previous_miss_streak,
        next_miss_streak,
        tradable_seconds_count,
        format_optional_guard_number(max_fillability_score),
        config_miss_count,
        relax_status,
        summary_block,
    )
}

pub(crate) fn build_price_to_beat_bump_increased_notification_message(
    market_slug: &str,
    amount: f64,
    unit: &str,
    count: i64,
    previous_bump_usd: f64,
    next_bump_usd: f64,
    previous_ptb_value: Option<f64>,
    previous_ptb_unit: Option<&str>,
    previous_ptb_usd: Option<f64>,
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    let previous_ptb_summary =
        format_current_ptb_summary(previous_ptb_value, previous_ptb_unit, previous_ptb_usd);
    let current_ptb_summary =
        format_current_ptb_summary(current_ptb_value, current_ptb_unit, current_ptb_usd);
    format!(
        "PTB Stop-Loss Artisi Guncellendi\nMarket: {market_slug}\nKademe: {amount:.8} {unit}\nToplam Artis Sayisi: {count}\nUygulanan Toplam Artis: {previous_bump_usd:.8} USD -> {next_bump_usd:.8} USD\nEfektif PTB: {previous_ptb_summary} -> {current_ptb_summary}\nGuncel PTB: {current_ptb_summary}"
    )
}

pub(crate) fn build_price_to_beat_bump_max_reached_notification_message(
    market_slug: &str,
    raw_bump_usd: f64,
    capped_bump_usd: f64,
    max_value: f64,
    unit: &str,
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    let current_ptb_summary =
        format_current_ptb_summary(current_ptb_value, current_ptb_unit, current_ptb_usd);
    format!(
        "PTB Stop-Loss Artisi Max Limite Ulasti\nMarket: {market_slug}\nHam Artis: {raw_bump_usd:.8} USD\nUygulanan Artis: {capped_bump_usd:.8} USD\nMax Limit: {max_value:.8} {unit}\nGuncel PTB: {current_ptb_summary}"
    )
}
