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

    format!(
        "{}\nSebep: {}{}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}{}\nLimit: {:.8} {} (~{:.8} USD)",
        "Price to Beat Korumasi Engelledi",
        reason,
        detail_line,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        evaluation.price_to_beat_source.as_deref().unwrap_or("N/A"),
        format_current_price_label(evaluation.current_price_source),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.directional_gap),
        format_optional_guard_number(evaluation.gap_abs),
        metric_line,
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd
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
