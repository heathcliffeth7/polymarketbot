use super::*;

#[test]
fn cycle_window_focus_rejects_time_before_cycle_start() {
    let now = Utc::now().timestamp();
    let slug = format!("sol-updown-5m-{}", now + 120);

    assert!(is_outside_cycle_window_focus(&slug, "last", 60, None, None));
}

#[test]
fn cycle_window_focus_rejects_time_after_cycle_end() {
    let now = Utc::now().timestamp();
    let slug = format!("sol-updown-5m-{}", now - 600);

    assert!(is_outside_cycle_window_focus(&slug, "last", 60, None, None));
}

#[test]
fn cycle_window_focus_preserves_in_cycle_first_and_last_behavior() {
    let now = Utc::now().timestamp();
    let first_inside_slug = format!("sol-updown-5m-{}", now - 30);
    let first_outside_slug = format!("sol-updown-5m-{}", now - 90);
    let last_inside_slug = format!("sol-updown-5m-{}", now - 240);
    let last_outside_slug = format!("sol-updown-5m-{}", now - 120);

    assert!(!is_outside_cycle_window_focus(
        &first_inside_slug,
        "first",
        60,
        None,
        None
    ));
    assert!(is_outside_cycle_window_focus(
        &first_outside_slug,
        "first",
        60,
        None,
        None
    ));
    assert!(!is_outside_cycle_window_focus(
        &last_inside_slug,
        "last",
        60,
        None,
        None
    ));
    assert!(is_outside_cycle_window_focus(
        &last_outside_slug,
        "last",
        60,
        None,
        None
    ));
}

#[test]
fn cycle_window_focus_resolves_absolute_bounds_for_last_minute() {
    let (open_at, end_at) =
        resolve_cycle_window_absolute_bounds("btc-updown-5m-1773319200", "last", 60, None, None)
            .expect("bounds");

    assert_eq!(
        open_at,
        DateTime::<Utc>::from_timestamp(1_773_319_440, 0).expect("open_at")
    );
    assert_eq!(
        end_at,
        DateTime::<Utc>::from_timestamp(1_773_319_500, 0).expect("end_at")
    );
}

#[test]
fn cycle_window_focus_fail_closes_on_incomplete_metadata() {
    let now = Utc::now().timestamp();
    let slug = format!("btc-updown-5m-{}", now - 120);

    assert!(should_skip_for_cycle_window(
        None,
        Some("last"),
        Some(60),
        None,
        None
    ));
    assert!(should_skip_for_cycle_window(
        Some(&slug),
        Some("last"),
        None,
        None,
        None
    ));
    assert!(!should_skip_for_cycle_window(
        Some(&slug),
        None,
        Some(60),
        None,
        None
    ));
}

// --- custom_range tests ---

#[test]
fn cycle_window_custom_range_resolves_absolute_bounds() {
    // 5m cycle starting at 1773319200, custom_range 120-240
    let (open_at, end_at) = resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(120),
        Some(240),
    )
    .expect("bounds");

    assert_eq!(
        open_at,
        DateTime::<Utc>::from_timestamp(1_773_319_320, 0).expect("open_at")
    );
    assert_eq!(
        end_at,
        DateTime::<Utc>::from_timestamp(1_773_319_440, 0).expect("end_at")
    );
}

#[test]
fn cycle_window_custom_range_rejects_invalid_range() {
    // start >= end
    assert!(resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(240),
        Some(120),
    )
    .is_none());
    // start == end
    assert!(resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(120),
        Some(120),
    )
    .is_none());
}

#[test]
fn cycle_window_custom_range_rejects_overflow() {
    // end > 300 (5m cycle duration)
    assert!(resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(200),
        Some(360),
    )
    .is_none());
}

#[test]
fn cycle_window_custom_range_full_cycle() {
    // 0-300 covers the whole 5m cycle
    let (open_at, end_at) = resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(0),
        Some(300),
    )
    .expect("bounds");

    assert_eq!(
        open_at,
        DateTime::<Utc>::from_timestamp(1_773_319_200, 0).expect("open_at")
    );
    assert_eq!(
        end_at,
        DateTime::<Utc>::from_timestamp(1_773_319_500, 0).expect("end_at")
    );
}

#[test]
fn cycle_window_custom_range_skip_logic() {
    let now = Utc::now().timestamp();
    // Cycle started 150s ago → we are at second 150 of a 5m cycle
    let slug = format!("btc-updown-5m-{}", now - 150);

    // custom_range 120-240: we are inside (150 is in [120, 240))
    assert!(!should_skip_for_cycle_window(
        Some(&slug),
        Some("custom_range"),
        None,
        Some(120),
        Some(240),
    ));
    // custom_range 0-60: we are outside (150 > 60)
    assert!(should_skip_for_cycle_window(
        Some(&slug),
        Some("custom_range"),
        None,
        Some(0),
        Some(60),
    ));
    // custom_range with missing end_sec → should skip (incomplete metadata)
    assert!(should_skip_for_cycle_window(
        Some(&slug),
        Some("custom_range"),
        None,
        Some(120),
        None,
    ));
}

#[test]
fn cycle_window_custom_range_15m_bounds() {
    // 15m cycle starting at 1773319200, custom_range 600-900
    let (open_at, end_at) = resolve_cycle_window_absolute_bounds(
        "btc-updown-15m-1773319200",
        "custom_range",
        0,
        Some(600),
        Some(900),
    )
    .expect("bounds");

    assert_eq!(
        open_at,
        DateTime::<Utc>::from_timestamp(1_773_319_800, 0).expect("open_at")
    );
    assert_eq!(
        end_at,
        DateTime::<Utc>::from_timestamp(1_773_320_100, 0).expect("end_at")
    );
}

#[test]
fn cycle_window_custom_range_supports_pair_lock_style_last_minute_band() {
    let (open_at, end_at) = resolve_cycle_window_absolute_bounds(
        "btc-updown-5m-1773319200",
        "custom_range",
        0,
        Some(230),
        Some(290),
    )
    .expect("bounds");

    assert_eq!(
        open_at,
        DateTime::<Utc>::from_timestamp(1_773_319_430, 0).expect("open_at")
    );
    assert_eq!(
        end_at,
        DateTime::<Utc>::from_timestamp(1_773_319_490, 0).expect("end_at")
    );
}
