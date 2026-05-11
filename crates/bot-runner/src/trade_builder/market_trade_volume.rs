use bot_infra::db::TradeBuilderMarketTradeTickInput;

#[derive(Debug, Clone)]
struct MarketTradeVolumeTick {
    tick: MarketTradeTick,
}

fn build_market_trade_volume_callback(
    tx: tokio::sync::mpsc::UnboundedSender<MarketTradeVolumeTick>,
) -> MarketTradeCallback {
    Arc::new(move |tick| {
        let _ = tx.send(MarketTradeVolumeTick { tick: tick.clone() });
    })
}

async fn run_market_trade_volume_recorder(
    repo: PostgresRepository,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<MarketTradeVolumeTick>,
) {
    while let Some(volume_tick) = rx.recv().await {
        let tick = volume_tick.tick;
        let contexts = market_second_snapshot_contexts_for_token(&tick.token_id);
        if contexts.is_empty() {
            continue;
        }
        let Some(event_ts) = DateTime::<Utc>::from_timestamp_millis(tick.timestamp_ms) else {
            continue;
        };
        let price = clamp_probability(tick.price);
        let size = tick.size;
        if !price.is_finite() || price <= 0.0 || !size.is_finite() || size <= 0.0 {
            continue;
        }
        let notional_usdc = price * size;
        if !notional_usdc.is_finite() || notional_usdc <= 0.0 {
            continue;
        }
        for context in &contexts {
            let Some((window_start, window_end)) =
                trade_builder_second_snapshot_window(&context.market_slug)
            else {
                continue;
            };
            let dedupe_key = market_trade_tick_dedupe_key(&context.market_slug, &tick);
            let input = TradeBuilderMarketTradeTickInput {
                market_slug: context.market_slug.clone(),
                asset: context.asset.clone(),
                window_start,
                window_end,
                token_id: tick.token_id.clone(),
                outcome_side: context.outcome_side.to_string(),
                event_ts,
                price,
                size,
                notional_usdc,
                side: tick.side.clone(),
                dedupe_key,
            };
            if let Err(err) = repo.insert_trade_builder_market_trade_tick(&input).await {
                warn!(
                    token_id = %tick.token_id,
                    market_slug = %context.market_slug,
                    error = %err,
                    "MARKET_TRADE_TICK_INSERT_FAILED"
                );
            }
        }
    }
}

fn market_trade_tick_dedupe_key(market_slug: &str, tick: &MarketTradeTick) -> String {
    if let Some(source_id) = tick.source_id.as_deref().filter(|value| !value.is_empty()) {
        return format!("{market_slug}:{}:{source_id}", tick.token_id);
    }
    format!(
        "{market_slug}:{}:{}:{:.8}:{:.8}:{}",
        tick.token_id,
        tick.timestamp_ms,
        tick.price,
        tick.size,
        tick.side.as_deref().unwrap_or("")
    )
}
