use super::support::*;
use super::*;

fn loss_table_node() -> TradeFlowNode {
    test_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpMode": "loss_table",
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpLossRules": [
            { "lossUsd": 1.0, "bumpValue": 25.0 },
            { "lossUsd": 2.0, "bumpValue": 50.0 },
            { "lossUsd": 5.0, "bumpValue": 100.0 }
        ],
    }))
}

#[test]
fn ptb_stop_loss_bump_config_parses_loss_table_rules() {
    let node = loss_table_node();
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(&node, "buy")
        .expect("config parse")
        .expect("enabled config");

    assert_eq!(config.mode, ActionPlaceOrderPtbStopLossBumpMode::LossTable);
    assert_eq!(config.amount, None);
    assert_eq!(config.unit.as_str(), "cent");
    assert_eq!(config.loss_rules.len(), 3);
    assert_eq!(config.loss_rules[0].loss_usd, 1.0);
    assert_eq!(config.loss_rules[1].bump_value, 50.0);
    assert!((config.loss_rules[2].bump_usd - 1.0).abs() <= 0.000001);
}

#[test]
fn ptb_stop_loss_bump_loss_table_selects_last_matching_rule() {
    let node = loss_table_node();
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(&node, "buy")
        .expect("config parse")
        .expect("enabled config");

    let one = resolve_action_place_order_ptb_stop_loss_bump_loss_rule(&config.loss_rules, 1.0)
        .expect("1 usd rule");
    let two = resolve_action_place_order_ptb_stop_loss_bump_loss_rule(&config.loss_rules, 2.4)
        .expect("2 usd rule");
    let five = resolve_action_place_order_ptb_stop_loss_bump_loss_rule(&config.loss_rules, 5.0)
        .expect("5 usd rule");

    assert_eq!(one.bump_value, 25.0);
    assert!((one.bump_usd - 0.25).abs() <= 0.000001);
    assert_eq!(two.bump_value, 50.0);
    assert!((two.bump_usd - 0.5).abs() <= 0.000001);
    assert_eq!(five.bump_value, 100.0);
    assert!((five.bump_usd - 1.0).abs() <= 0.000001);
}

#[test]
fn ptb_stop_loss_bump_loss_table_skips_when_loss_below_minimum_rule() {
    let node = loss_table_node();
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(&node, "buy")
        .expect("config parse")
        .expect("enabled config");

    assert!(
        resolve_action_place_order_ptb_stop_loss_bump_loss_rule(&config.loss_rules, 0.99).is_none()
    );
}

#[test]
fn ptb_stop_loss_bump_fixed_mode_keeps_existing_increment_behavior() {
    let node = test_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpUnit": "cent",
    }));
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(&node, "buy")
        .expect("config parse")
        .expect("enabled config");

    assert_eq!(config.mode, ActionPlaceOrderPtbStopLossBumpMode::Fixed);
    assert_eq!(config.amount, Some(10.0));
    assert_eq!(
        resolve_action_place_order_ptb_stop_loss_bump_decay_step_usd(&config),
        0.1
    );
}
