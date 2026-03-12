use super::*;

#[test]
fn cycle_window_focus_rejects_time_before_cycle_start() {
    let now = Utc::now().timestamp();
    let slug = format!("sol-updown-5m-{}", now + 120);

    assert!(is_outside_cycle_window_focus(&slug, "last", 60));
}

#[test]
fn cycle_window_focus_rejects_time_after_cycle_end() {
    let now = Utc::now().timestamp();
    let slug = format!("sol-updown-5m-{}", now - 600);

    assert!(is_outside_cycle_window_focus(&slug, "last", 60));
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
        60
    ));
    assert!(is_outside_cycle_window_focus(
        &first_outside_slug,
        "first",
        60
    ));
    assert!(!is_outside_cycle_window_focus(
        &last_inside_slug,
        "last",
        60
    ));
    assert!(is_outside_cycle_window_focus(
        &last_outside_slug,
        "last",
        60
    ));
}

#[test]
fn cycle_window_focus_resolves_absolute_bounds_for_last_minute() {
    let (open_at, end_at) =
        resolve_cycle_window_absolute_bounds("btc-updown-5m-1773319200", "last", 60)
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

    assert!(should_skip_for_cycle_window(None, Some("last"), Some(60)));
    assert!(should_skip_for_cycle_window(
        Some(&slug),
        Some("last"),
        None
    ));
    assert!(!should_skip_for_cycle_window(Some(&slug), None, Some(60)));
}
