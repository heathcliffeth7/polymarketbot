fn positive_quantity_flip_grid_cycle_window_bounds(
    config: &PositiveQuantityFlipGridConfig,
    duration_sec: i64,
) -> Option<(i64, i64)> {
    let mode = config.cycle_window_mode.as_deref()?;
    match mode {
        "off" => None,
        "first" => {
            let secs = config.cycle_window_secs?.clamp(1, duration_sec);
            Some((0, secs))
        }
        "last" => {
            let secs = config.cycle_window_secs?.clamp(1, duration_sec);
            Some((duration_sec - secs, duration_sec))
        }
        "custom_range" => {
            let start_sec = config.cycle_window_start_sec?;
            let end_sec = config.cycle_window_end_sec?;
            (start_sec >= 0 && start_sec < end_sec && end_sec <= duration_sec)
                .then_some((start_sec, end_sec))
        }
        _ => None,
    }
}

fn positive_quantity_flip_grid_cycle_window_skip_details(
    config: &PositiveQuantityFlipGridConfig,
    duration_sec: i64,
    remaining_sec: i64,
) -> Option<Value> {
    let mode = config.cycle_window_mode.as_deref()?;
    if mode == "off" {
        return None;
    }
    let elapsed_sec = (duration_sec - remaining_sec).clamp(0, duration_sec);
    let Some((start_sec, end_sec)) =
        positive_quantity_flip_grid_cycle_window_bounds(config, duration_sec)
    else {
        return Some(json!({
            "blocked_by": "cycle_window",
            "guard_reason": "invalid_positive_grid_cycle_window",
            "mode": mode,
            "duration_sec": duration_sec,
            "elapsed_sec": elapsed_sec,
            "remaining_sec": remaining_sec,
        }));
    };
    if elapsed_sec < start_sec || elapsed_sec >= end_sec {
        return Some(json!({
            "blocked_by": "cycle_window",
            "guard_reason": "outside_positive_grid_cycle_window",
            "mode": mode,
            "start_sec": start_sec,
            "end_sec": end_sec,
            "duration_sec": duration_sec,
            "elapsed_sec": elapsed_sec,
            "remaining_sec": remaining_sec,
        }));
    }
    None
}

fn positive_quantity_flip_grid_cycle_window_skip_for_market(
    config: &PositiveQuantityFlipGridConfig,
    market_slug: &str,
    remaining_sec: i64,
) -> Option<Value> {
    let (_, _, duration_sec) = resolve_updown_market_cycle_bounds(market_slug)?;
    positive_quantity_flip_grid_cycle_window_skip_details(config, duration_sec, remaining_sec)
}
