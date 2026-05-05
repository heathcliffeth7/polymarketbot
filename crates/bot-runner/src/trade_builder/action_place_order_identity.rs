use crate::trade_flow::guards::cex_microstructure::{
    ensure_cex_microstructure_started, get_cex_microstructure_snapshot,
    CexMicrostructureSnapshotConfig,
};

#[derive(Debug, Clone)]
struct ActionPlaceOrderIdentity {
    market_slug: String,
    token_id: String,
    outcome_label: String,
}

fn resolve_action_place_order_identity(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
    side: &str,
) -> Result<ActionPlaceOrderIdentity> {
    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires marketSlug"))?;
    if let Some(identity) =
        resolve_early_stale_side_action_identity(node, context, &market_slug, side)
    {
        return Ok(identity);
    }

    let token_id = resolve_action_place_order_string(
        node,
        context,
        step,
        "tokenId",
        "tokenId",
        &["triggered_token_id", "tokenId"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires tokenId"))?;
    let outcome_label = resolve_action_place_order_string(
        node,
        context,
        step,
        "outcomeLabel",
        "outcomeLabel",
        &["triggered_outcome_label", "outcomeLabel"],
    )
    .unwrap_or_else(|| token_id.clone());
    Ok(ActionPlaceOrderIdentity {
        market_slug,
        token_id,
        outcome_label,
    })
}

fn resolve_early_stale_side_action_identity(
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    side: &str,
) -> Option<ActionPlaceOrderIdentity> {
    if side != "buy" || !node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false)
    {
        return None;
    }
    let scope = find_updown_scope_by_slug(market_slug)?;
    let yes_token_id = node_auto_scope_yes_token_id(context, &node.key)
        .or_else(|| flow_context_string(context, "yesTokenId"))?;
    let no_token_id = node_auto_scope_no_token_id(context, &node.key)
        .or_else(|| flow_context_string(context, "noTokenId"))?;
    ensure_cex_microstructure_started(scope.asset);
    let (token_id, outcome_label) = match get_cex_microstructure_snapshot(
        scope.asset,
        &CexMicrostructureSnapshotConfig::default(),
    )
    .ok()
    .and_then(|snapshot| snapshot.consensus_side)
    {
        Some("down") => (no_token_id, "Down"),
        _ => (yes_token_id, "Up"),
    };
    Some(ActionPlaceOrderIdentity {
        market_slug: market_slug.to_string(),
        token_id,
        outcome_label: outcome_label.to_string(),
    })
}

#[cfg(test)]
mod action_place_order_identity_tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, seed_cex_book_test_sample,
        seed_cex_trade_test_sample, CexBookSample, CexTradeSample, CexVenue, TakerSide,
    };

    fn node() -> TradeFlowNode {
        TradeFlowNode {
            key: "scout".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        }
    }

    fn book(venue: CexVenue, ts: i64, bid: f64, ask: f64) -> CexBookSample {
        CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: ts,
            bid,
            ask,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        }
    }

    fn trade(venue: CexVenue, ts: i64, price: f64) -> CexTradeSample {
        CexTradeSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: ts,
            price,
            size: 1.0,
            taker_side: TakerSide::Buy,
        }
    }

    #[tokio::test]
    async fn early_stale_identity_selects_up_token_from_cex_consensus() {
        clear_cex_microstructure_test_state();
        let now = Utc::now().timestamp_millis();
        for venue in [CexVenue::Binance, CexVenue::Coinbase] {
            seed_cex_book_test_sample(book(venue, now, 67_520.0, 67_522.0));
            seed_cex_trade_test_sample(trade(venue, now - 14_000, 67_500.0));
            seed_cex_trade_test_sample(trade(venue, now, 67_520.0));
        }
        let context = json!({
            "flowContext": {
                "yesTokenId": "yes-token",
                "noTokenId": "no-token"
            }
        });
        let identity = resolve_early_stale_side_action_identity(
            &node(),
            &context,
            "btc-updown-5m-1773319200",
            "buy",
        )
        .expect("identity");

        assert_eq!(identity.token_id, "yes-token");
        assert_eq!(identity.outcome_label, "Up");
    }
}
