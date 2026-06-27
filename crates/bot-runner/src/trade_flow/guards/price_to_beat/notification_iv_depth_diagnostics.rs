use serde_json::{Map, Value};

pub(super) fn append_depth_diagnostics(iv: &Map<String, Value>, lines: &mut Vec<String>) {
    let depth_result = text(iv, "depth_guard_result");
    let depth_reason = text(iv, "depth_unavailable_reason");
    if depth_result != Some("unavailable") && depth_reason.is_none() {
        append_book_confirmation_diagnostics(iv, lines);
        return;
    }

    lines.push(format!(
        "Depth Diagnostics: reason={} sizing={} source={} book={} asks={} bids={} valid_asks={}",
        text_or_na(depth_reason),
        text_or_na(text(iv, "depth_sizing_missing_reason")),
        text_or_na(text(iv, "depth_sizing_source")),
        text_or_na(text(iv, "depth_order_book_fetch_status")),
        number_or_na(number(iv, "depth_order_book_asks_len")),
        number_or_na(number(iv, "depth_order_book_bids_len")),
        number_or_na(number(iv, "depth_valid_asks_len"))
    ));
    append_book_confirmation_diagnostics(iv, lines);
}

fn append_book_confirmation_diagnostics(iv: &Map<String, Value>, lines: &mut Vec<String>) {
    let Some(reason) = text(iv, "book_confirmation_missing_reason") else {
        return;
    };
    if !should_render_book_confirmation_diagnostic(reason) {
        return;
    }

    lines.push(format!(
        "Book Confirmation: missing reason={} selected={} opposite={}",
        reason,
        text_or_na(text(iv, "depth_selected_outcome_label")),
        text_or_na(text(iv, "depth_opposite_outcome_label"))
    ));
}

fn should_render_book_confirmation_diagnostic(reason: &str) -> bool {
    matches!(
        reason,
        "outcome_direction_unavailable"
            | "selected_quote_missing"
            | "opposite_quote_missing"
            | "selected_and_opposite_quote_missing"
    )
}

fn text<'a>(iv: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    iv.get(key).and_then(Value::as_str)
}

fn number(iv: &Map<String, Value>, key: &str) -> Option<f64> {
    iv.get(key).and_then(Value::as_f64)
}

fn text_or_na(value: Option<&str>) -> String {
    value.unwrap_or("N/A").to_string()
}

fn number_or_na(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| "N/A".to_string())
}
