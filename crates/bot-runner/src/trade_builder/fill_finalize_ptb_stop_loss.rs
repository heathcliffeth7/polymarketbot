async fn create_trade_builder_ptb_stop_loss_child_order(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    parent_order: &TradeBuilderOrder,
    execution_price: f64,
    child_sizing: TradeBuilderExitChildSizing,
    ptb_stop_loss_gap_usd: f64,
    ptb_reference_price: Option<f64>,
    ladder_metadata: Option<(usize, f64)>,
) -> Result<Option<i64>> {
    let child_size_usdc = (child_sizing.target_qty * execution_price).max(0.0);
    let exit_mode = if ladder_metadata.is_some() {
        TRADE_BUILDER_EXIT_MODE_STAGED
    } else {
        TRADE_BUILDER_EXIT_MODE_HARD
    };
    let sibling_policy = if ladder_metadata.is_some() {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
    } else {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL
    };
    let exit_ladder_kind = ladder_metadata.map(|_| TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL);
    let child_id = repo
        .create_trade_builder_order_with_exit_ladders(
            parent_order.trade_id,
            "conditional",
            "armed",
            &parent_order.market_slug,
            &parent_order.token_id,
            &parent_order.outcome_label,
            "sell",
            "market",
            Some("cross_below"),
            None,
            None,
            None,
            None,
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            child_size_usdc,
            Some(child_sizing.target_qty),
            Some(child_sizing.remaining_qty),
            parent_order.min_price_distance_cent,
            parent_order.expires_at,
            None,
            None,
            1,
            Some(parent_order.id),
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            parent_order.fee_rate_bps,
            parent_order.origin_flow_definition_id,
            parent_order.origin_flow_run_id,
            parent_order.origin_flow_node_key.as_deref(),
            Some(ptb_stop_loss_gap_usd),
            ptb_reference_price,
            None,
            parent_order.ptb_stop_loss_time_decay_mode.as_deref(),
            Some(parent_order.ptb_current_price_source.as_str()),
            false,
            None,
            None,
            false,
            false,
            None,
            false,
            0,
            None,
            false,
            parent_order.notify_on_sl_hit,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            exit_ladder_kind,
            ladder_metadata.map(|(index, _)| index as i32),
            ladder_metadata.map(|(_, size_pct)| size_pct),
        )
        .await?;

    if let Ok(Some(mut child_order)) = repo.get_trade_builder_order(child_id).await {
        if let Some(snapshot) = ws.get_market_snapshot(&parent_order.token_id).await {
            if let Some(initial_last_seen_price) =
                trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
            {
                repo.set_trade_builder_last_seen_price(child_id, initial_last_seen_price)
                    .await?;
                child_order.last_seen_price = Some(initial_last_seen_price);
            }
        }
        sync_armed_builder_order_to_cache(child_order).await;
    }

    let child_event_payload = json!({
            "child_order_id": child_id,
            "initial_status": "armed",
            "family": exit_ladder_kind.unwrap_or(TRADE_BUILDER_EXIT_LADDER_KIND_SL),
            "exit_mode": exit_mode,
            "sibling_policy": sibling_policy,
            "trigger_price": Value::Null,
            "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
            "size_pct": ladder_metadata.map(|(_, size_pct)| size_pct).unwrap_or(100.0),
            "target_qty": child_sizing.target_qty,
            "execution_price": execution_price,
            "ptb_stop_loss_gap_usd": ptb_stop_loss_gap_usd,
            "ptb_reference_price": ptb_reference_price,
            "ptb_current_price_source": parent_order.ptb_current_price_source,
    });
    repo.append_trade_builder_order_event(parent_order.id, "sl_sell_created", &child_event_payload)
        .await?;
    let sl_event_id = format!("sl:tb:{}:{child_id}", parent_order.id);
    trade_builder_spawn_decision_log(
        repo,
        parent_order,
        "STOP_LOSS_ARMED",
        json!({
            "sl_event_id": &sl_event_id,
            "sl_child_order_id": child_id.to_string(),
            "sl_type": "ptb",
            "armed_config": child_event_payload,
        }),
        TradeBuilderDecisionLogOptions {
            idempotency_key: Some(format!("STOP_LOSS_ARMED:{}:{child_id}", parent_order.id)),
            sl_event_id: Some(sl_event_id),
            child_order_id: Some(child_id.to_string()),
            ..TradeBuilderDecisionLogOptions::default()
        },
    );

    Ok(Some(child_id))
}
