fn normalize_trade_builder_clob_order_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "FOK" => Some("FOK"),
        "FAK" | "IOC" => Some("FAK"),
        "GTC" => Some("GTC"),
        "GTD" => Some("GTD"),
        _ => None,
    }
}

fn action_place_order_clob_order_type(node: &TradeFlowNode, execution_mode: &str) -> &'static str {
    if node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false)
        && execution_mode.eq_ignore_ascii_case("market")
    {
        return "FOK";
    }
    node_config_string(node, "orderType")
        .and_then(|value| normalize_trade_builder_clob_order_type(&value))
        .unwrap_or_else(|| clob_order_type_for_execution_mode(execution_mode))
}

fn action_place_order_allow_partial_fill(order_type: &str) -> bool {
    !order_type.eq_ignore_ascii_case("FOK")
}

fn trade_builder_flow_created_order_type(payload: Option<&Value>) -> Option<&'static str> {
    payload?
        .get("order_type")
        .and_then(Value::as_str)
        .and_then(normalize_trade_builder_clob_order_type)
}

fn trade_builder_flow_created_post_only(payload: Option<&Value>, order_type: &str) -> bool {
    let Some(payload) = payload else {
        return false;
    };
    let requested = payload
        .get("post_only")
        .or_else(|| payload.get("postOnly"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    requested && matches!(order_type, "GTC" | "GTD")
}

async fn resolve_trade_builder_submit_order_type(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    normalized_execution_mode: &str,
) -> Result<&'static str> {
    let payload = repo
        .load_trade_builder_order_flow_created_payload(order.id)
        .await?;
    Ok(trade_builder_flow_created_order_type(payload.as_ref())
        .unwrap_or_else(|| clob_order_type_for_execution_mode(normalized_execution_mode)))
}

async fn resolve_trade_builder_submit_post_only(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    order_type: &str,
) -> Result<bool> {
    let payload = repo
        .load_trade_builder_order_flow_created_payload(order.id)
        .await?;
    Ok(trade_builder_flow_created_post_only(payload.as_ref(), order_type))
}

#[cfg(test)]
mod action_place_order_order_type_tests {
    use super::*;

    fn node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "order".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn early_stale_market_buy_uses_fok_order_type() {
        let order_type = action_place_order_clob_order_type(
            &node(json!({"priceToBeatEarlyStaleSideEnabled": true})),
            "market",
        );

        assert_eq!(order_type, "FOK");
        assert!(!action_place_order_allow_partial_fill(order_type));
    }

    #[test]
    fn explicit_ioc_order_type_normalizes_to_fak() {
        let order_type =
            action_place_order_clob_order_type(&node(json!({"orderType": "IOC"})), "market");

        assert_eq!(order_type, "FAK");
    }

    #[test]
    fn flow_created_post_only_only_applies_to_resting_orders() {
        let payload = json!({"post_only": true});
        assert!(trade_builder_flow_created_post_only(Some(&payload), "GTC"));
        assert!(!trade_builder_flow_created_post_only(Some(&payload), "FAK"));
    }
}
