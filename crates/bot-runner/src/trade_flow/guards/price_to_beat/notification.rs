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
    if let Some(stop_loss_bump_summary) = build_stop_loss_bump_summary(evaluation) {
        lines.push(format!("SL Bump: {stop_loss_bump_summary}"));
    }

    format!("\n{}", lines.join("\n"))
}

fn format_current_ptb_summary(
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    format_optional_guard_threshold_summary(current_ptb_value, current_ptb_unit, current_ptb_usd)
        .unwrap_or_else(|| "N/A".to_string())
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

    format!(
        "{}\nSebep: {}{}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}{}\nLimit: {:.8} {} (~{:.8} USD){}",
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
    format!(
        "{}\nDurum: Price to Beat yeniden uygun hale geldi.\nOnceki Sebep: {}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}\nLimit: {:.8} {} (~{:.8} USD){}",
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

pub(crate) fn build_price_to_beat_bump_increased_notification_message(
    market_slug: &str,
    amount: f64,
    unit: &str,
    count: i64,
    previous_bump_usd: f64,
    next_bump_usd: f64,
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    let current_ptb_summary =
        format_current_ptb_summary(current_ptb_value, current_ptb_unit, current_ptb_usd);
    format!(
        "PTB Stop-Loss Artisi Guncellendi\nMarket: {market_slug}\nKademe: {amount:.8} {unit}\nToplam Artis Sayisi: {count}\nUygulanan Toplam Artis: {previous_bump_usd:.8} USD -> {next_bump_usd:.8} USD\nGuncel PTB: {current_ptb_summary}"
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
