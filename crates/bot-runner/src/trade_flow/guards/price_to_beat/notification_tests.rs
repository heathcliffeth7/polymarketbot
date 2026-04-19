use super::current_price::CURRENT_PRICE_SOURCE_CHAINLINK;
use super::notification::{
    build_price_to_beat_bump_max_reached_notification_message,
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_recovered_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
    build_price_to_beat_relax_changed_notification_message,
};
use super::*;

fn default_guard_evaluation() -> PriceToBeatGuardEvaluation {
    PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: String::new(),
        reason_detail: None,
        normalized_outcome_label: None,
        direction: None,
        market_slug: String::new(),
        event_url: String::new(),
        timeframe: None,
        asset: None,
        price_to_beat: None,
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_mode: "manual".to_string(),
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        threshold_value: 0.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.0,
        stop_loss_bump_count: 0,
        stop_loss_bump_applied_count: 0,
        stop_loss_bump_amount: None,
        stop_loss_bump_max_value: None,
        stop_loss_bump_unit: None,
        stop_loss_bump_raw_usd: 0.0,
        stop_loss_bump_usd: 0.0,
        stop_loss_bump_capped: false,
        stop_loss_bump_max_reached: false,
        stop_loss_bump_current_market_excluded: false,
        stop_loss_bump_increment_usd: 0.0,
        reentry_generation: 0,
        reentry_override_active: false,
        reentry_override_value: None,
        reentry_override_unit: None,
        max_price_relax: None,
        auto_threshold_usd: None,
        lookback_windows_used: None,
        current_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
        lookback_window_snapshots: None,
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        threshold_pct: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
        threshold_was_clamped: None,
    }
}

#[test]
fn blocked_notification_includes_detailed_ptb_summary_when_metadata_present() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        direction: Some("up".to_string()),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2366.97),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        current_price: Some(2367.12),
        directional_gap: Some(0.15),
        gap_abs: Some(0.15),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: Some(80.0),
        base_threshold_unit: Some("cent".to_string()),
        base_threshold_usd: Some(0.8),
        threshold_value: 90.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 0.9,
        stop_loss_bump_count: 2,
        stop_loss_bump_applied_count: 1,
        stop_loss_bump_amount: Some(10.0),
        stop_loss_bump_max_value: Some(30.0),
        stop_loss_bump_unit: Some("cent".to_string()),
        stop_loss_bump_usd: 0.1,
        stop_loss_bump_capped: true,
        stop_loss_bump_current_market_excluded: true,
        reentry_generation: 1,
        reentry_override_active: true,
        reentry_override_value: Some(5.0),
        reentry_override_unit: Some("cent".to_string()),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 90.00000000 cent (~0.90000000 USD)"));
    assert!(message.contains("Base PTB: 80.00000000 cent (~0.80000000 USD)"));
    assert!(message.contains("Re-entry Override: 5.00000000 cent (~0.05000000 USD)"));
    assert!(message.contains("SL Bump: kademe 10.00000000 cent"));
    assert!(message.contains("uygulanan 0.10000000 USD"));
    assert!(message.contains("bu market dislandi"));
}

#[test]
fn blocked_notification_omits_optional_ptb_summary_lines_when_metadata_missing() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_pending".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 20.00000000 USD"));
    assert!(!message.contains("Base PTB:"));
    assert!(!message.contains("Auto Threshold:"));
    assert!(!message.contains("Re-entry Override:"));
    assert!(!message.contains("SL Bump:"));
}

#[test]
fn waiting_notification_includes_auto_threshold_summary() {
    let evaluation = PriceToBeatGuardEvaluation {
        reason_code: "price_to_beat_pending".to_string(),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        configured_threshold_mode: Some("auto_vol_pct".to_string()),
        threshold_mode: "auto_vol_pct".to_string(),
        threshold_value: 1.2,
        threshold_unit: "usd".to_string(),
        threshold_usd: 1.2,
        auto_threshold_usd: Some(1.2),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Durum: Bekleme moduna alindi"));
    assert!(message.contains("Configured Mod: auto_vol_pct"));
    assert!(message.contains("Efektif PTB: 1.20000000 USD"));
    assert!(message.contains("Auto Threshold: 1.20000000 USD"));
}

#[test]
fn recovered_notification_includes_ptb_summary_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: true,
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776200100".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1776200100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2366.97),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        current_price: Some(2365.50),
        directional_gap: Some(1.47),
        gap_abs: Some(1.47),
        configured_threshold_mode: Some("auto_last_3_avg_excursion".to_string()),
        threshold_mode: "auto_last_3_avg_excursion".to_string(),
        threshold_value: 1.47,
        threshold_unit: "usd".to_string(),
        threshold_usd: 1.47,
        auto_threshold_usd: Some(1.47),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_recovered_notification_message(
        &evaluation,
        "price_to_beat_gap_below_threshold",
    );
    assert!(message.contains("Price to Beat Korumasi Gecti"));
    assert!(message.contains("Configured Mod: auto_last_3_avg_excursion"));
    assert!(message.contains("Efektif PTB: 1.47000000 USD"));
    assert!(message.contains("Auto Threshold: 1.47000000 USD"));
}

#[test]
fn relax_notification_keeps_special_fields_and_adds_shared_summary_block() {
    let evaluation = PriceToBeatGuardEvaluation {
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776198300".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        configured_threshold_mode: Some("manual".to_string()),
        base_threshold_value: Some(100.0),
        base_threshold_unit: Some("cent".to_string()),
        base_threshold_usd: Some(1.0),
        threshold_value: 1.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 1.0,
        auto_threshold_usd: Some(1.0),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_relax_changed_notification_message(
        &evaluation,
        Some(1.0),
        Some(0.67328696),
        1.0,
        Some(0.42328696),
        0.25,
        1.0,
        11,
        &["eth-updown-5m-1776198000".to_string()],
    );

    assert!(message.contains("Onceki Bildirilen Relax PTB: 1.00000000"));
    assert!(message.contains("Ham Relax PTB: 0.67328696"));
    assert!(message.contains("Bu Market Efektif Relax PTB: 1.00000000"));
    assert!(message.contains("Configured Mod: manual"));
    assert!(message.contains("Efektif PTB: 1.00000000 USD"));
    assert!(message.contains("Base PTB: 100.00000000 cent (~1.00000000 USD)"));
    assert!(message.contains("Auto Threshold: 1.00000000 USD"));
}

#[test]
fn bump_max_reached_notification_formats_current_ptb_and_na() {
    let cent_message = build_price_to_beat_bump_max_reached_notification_message(
        "eth-updown-5m-1776200100",
        0.75,
        0.50,
        50.0,
        "cent",
        Some(150.0),
        Some("cent"),
        Some(1.5),
    );
    assert!(cent_message.contains("Guncel PTB: 150.00000000 cent (~1.50000000 USD)"));

    let na_message = build_price_to_beat_bump_max_reached_notification_message(
        "eth-updown-5m-1776200100",
        0.75,
        0.50,
        50.0,
        "cent",
        None,
        None,
        None,
    );
    assert!(na_message.contains("Guncel PTB: N/A"));
}
