fn copy_pair_lock_primary_take_profit_fields(
    node: &TradeFlowNode,
    config: &mut serde_json::Map<String, Value>,
) {
    for key in ["tpEnabled", "tpPrice", "tpPriceCent", "tpRules", "notifyOnTpHit"] {
        if let Some(value) = node.config.get(key) {
            config.insert(key.to_string(), value.clone());
        }
    }
}

fn copy_pair_lock_counter_take_profit_fields(
    node: &TradeFlowNode,
    config: &mut serde_json::Map<String, Value>,
) {
    for (source_key, target_key) in [
        ("counterLegTpEnabled", "tpEnabled"),
        ("counterLegTpPriceCent", "tpPriceCent"),
        ("counterLegNotifyOnTpHit", "notifyOnTpHit"),
    ] {
        if let Some(value) = node.config.get(source_key) {
            config.insert(target_key.to_string(), value.clone());
        } else {
            config.remove(target_key);
        }
    }
    if let Some(value) = node.config.get("counterLegTpRules") {
        config.insert("tpRules".to_string(), value.clone());
    } else {
        config.remove("tpRules");
    }
}
