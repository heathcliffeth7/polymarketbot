use super::support::*;
use super::*;

#[test]
fn apply_fill_buy_increments_when_flag_true() {
    let mut leg = make_leg(0, None);
    apply_fill_to_leg(&mut leg, "buy", 0.50, 10.0, true);
    assert_eq!(leg.levels_filled, 1);
    assert_eq!(leg.last_fill_price, Some(0.50));
}

#[test]
fn apply_fill_buy_no_increment_when_flag_false() {
    let mut leg = make_leg(2, Some(0.50));
    apply_fill_to_leg(&mut leg, "buy", 0.48, 5.0, false);
    assert_eq!(leg.levels_filled, 2);
    assert_eq!(leg.last_fill_price, Some(0.48));
}

#[test]
fn apply_fill_sell_does_not_increment_levels() {
    let mut leg = make_leg(3, Some(0.60));
    apply_fill_to_leg(&mut leg, "sell", 0.65, 5.0, true);
    assert_eq!(leg.levels_filled, 3);
}

#[test]
fn apply_fill_buy_avg_entry_correct() {
    let mut leg = make_leg(0, None);
    leg.qty = 10.0;
    leg.avg_entry = 0.50;
    apply_fill_to_leg(&mut leg, "buy", 0.40, 10.0, false);
    assert!((leg.avg_entry - 0.45).abs() < 1e-9);
    assert_eq!(leg.qty, 20.0);
}

#[test]
fn dca_cap_simulation_max_3_with_leg() {
    use bot_core::DualSideStrategy;
    let strat = bot_core::SymmetricDualDcaStrategy;
    let mut leg = make_leg(0, None);
    let max = 3u32;
    let steps = [0.50f64, 0.48, 0.46];
    for price in steps {
        assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
        leg.levels_filled += 1;
        leg.last_fill_price = Some(price);
    }
    assert!(!strat.should_dca_leg(0.44, leg.last_fill_price, 0.02, leg.levels_filled, max));
    assert!(!strat.should_dca_leg(0.01, leg.last_fill_price, 0.02, leg.levels_filled, max));
}

#[test]
fn dca_cap_simulation_max_1_with_leg() {
    use bot_core::DualSideStrategy;
    let strat = bot_core::SymmetricDualDcaStrategy;
    let mut leg = make_leg(0, None);
    assert!(strat.should_dca_leg(0.50, leg.last_fill_price, 0.02, leg.levels_filled, 1));
    leg.levels_filled += 1;
    leg.last_fill_price = Some(0.50);
    assert!(!strat.should_dca_leg(0.45, leg.last_fill_price, 0.02, leg.levels_filled, 1));
}

#[test]
fn dca_cap_simulation_max_5_with_leg() {
    use bot_core::DualSideStrategy;
    let strat = bot_core::SymmetricDualDcaStrategy;
    let mut leg = make_leg(0, None);
    let max = 5u32;
    let prices = [0.50f64, 0.48, 0.46, 0.44, 0.42];
    for price in prices {
        assert!(strat.should_dca_leg(price, leg.last_fill_price, 0.02, leg.levels_filled, max));
        leg.levels_filled += 1;
        leg.last_fill_price = Some(price);
    }
    assert!(!strat.should_dca_leg(0.40, leg.last_fill_price, 0.02, leg.levels_filled, max));
}
