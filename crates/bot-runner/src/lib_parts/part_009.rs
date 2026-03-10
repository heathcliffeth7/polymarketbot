async fn create_dual_runtime(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    market_id: i64,
    market_slug: String,
    yes_token_id: String,
    no_token_id: String,
    maker_base_fee: u64,
    cycle_ends_at: DateTime<Utc>,
) -> Result<DualBasketRuntime> {
    let trade_id = repo
        .create_trade_stub_dual(
            market_id,
            cfg.strategy.total_notional_usdc,
            "dual_side_dca",
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        )
        .await?;
    repo.ensure_position_exit_rule_defaults(trade_id, DEFAULT_DROP_SELL_PCT)
        .await?;

    Ok(DualBasketRuntime {
        trade_id,
        user_id: None,
        market_slug,
        maker_base_fee,
        state: TradeState::Idle,
        yes_leg: DualLegRuntime {
            side: LegSide::Yes,
            token_id: yes_token_id,
            qty: 0.0,
            avg_entry: 0.0,
            levels_filled: 0,
            last_fill_price: None,
            last_dca_at: None,
        },
        no_leg: DualLegRuntime {
            side: LegSide::No,
            token_id: no_token_id,
            qty: 0.0,
            avg_entry: 0.0,
            levels_filled: 0,
            last_fill_price: None,
            last_dca_at: None,
        },
        cycle_ends_at,
    })
}

async fn transition_dual(
    repo: &PostgresRepository,
    basket: &mut DualBasketRuntime,
    to: TradeState,
    reason: &str,
) -> Result<()> {
    let from = basket.state;
    StateRepository::transition_trade_state(repo, basket.trade_id, from, to, reason).await?;
    basket.state = to;
    Ok(())
}

fn basket_to_trade_runtime(basket: &DualBasketRuntime) -> TradeRuntime {
    TradeRuntime {
        trade_id: basket.trade_id,
        user_id: basket.user_id,
        market_slug: basket.market_slug.clone(),
        entry_price: 0.5,
        tp_price: 0.5,
        position_size: basket.yes_leg.qty + basket.no_leg.qty,
        state: basket.state,
    }
}

async fn record_paper_leg_fill(
    repo: &PostgresRepository,
    trade_id: i64,
    leg: &mut DualLegRuntime,
    intent: &str,
    side: &str,
    price: f64,
    size: f64,
) -> Result<()> {
    if size <= 0.0 {
        return Ok(());
    }
    let client_order_id = Uuid::new_v4().to_string();
    let order_id = repo
        .append_order_event_with_meta(
            trade_id,
            intent,
            side,
            price,
            size,
            "filled",
            Some(client_order_id.as_str()),
            Some(leg_side_label(leg.side)),
            Some(&leg.token_id),
        )
        .await?;
    repo.append_fill_event(order_id, price, size, 0.0).await?;
    apply_fill_to_leg(leg, side, price, size, true);
    Ok(())
}

async fn place_live_leg_order(
    repo: &PostgresRepository,
    trade_id: i64,
    market_slug: &str,
    leg: &mut DualLegRuntime,
    side: &str,
    intent: &str,
    price: f64,
    size: f64,
    fee_rate_bps: u64,
    client: &dyn OrderExecutor,
    order_meta: &mut HashMap<String, OrderMeta>,
) -> Result<()> {
    if size <= 0.0 {
        return Ok(());
    }
    let client_order_id = Uuid::new_v4().to_string();
    let req = PlaceOrderRequest {
        market: market_slug.to_string(),
        token_id: Some(leg.token_id.clone()),
        side: side.to_string(),
        price,
        size,
        intent: intent.to_string(),
        order_type: "GTC".to_string(),
        client_order_id: client_order_id.clone(),
        leg_side: Some(leg_side_label(leg.side).to_string()),
        fee_rate_bps,
    };

    let ack = client.place(&req).await?;
    let exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let raw = json!({
        "client_order_id": ack.client_order_id,
        "exchange_order_id": ack.exchange_order_id,
        "status": ack.status,
        "reject_reason": ack.reject_reason,
        "raw_status": ack.raw_status,
        "exchange_ts": ack.exchange_ts,
        "intent": intent,
        "leg_side": leg_side_label(leg.side),
        "token_id": &leg.token_id
    });
    repo.upsert_order_by_exchange_id_with_meta(
        trade_id,
        &exchange_order_id,
        Some(&client_order_id),
        intent,
        side,
        price,
        size,
        &ack.status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
        Some(leg_side_label(leg.side)),
        Some(&leg.token_id),
    )
    .await?;
    order_meta.insert(
        exchange_order_id,
        OrderMeta {
            leg_side: leg.side,
            side: side.to_string(),
            intent: intent.to_string(),
        },
    );
    Ok(())
}

async fn process_live_dual_fills(
    repo: &PostgresRepository,
    _trade_id: i64,
    client: &dyn OrderExecutor,
    basket: &mut DualBasketRuntime,
    order_meta: &HashMap<String, OrderMeta>,
    seen_fill_ids: &mut HashSet<String>,
    seen_buy_fill_order_ids: &mut HashSet<String>,
) -> Result<()> {
    let fills = client.list_fills(None).await?;
    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            continue;
        }
        if !seen_fill_ids.insert(fill.fill_id.clone()) {
            continue;
        }
        let Some(meta) = order_meta.get(&fill.order_id) else {
            continue;
        };
        let Some(order_id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        else {
            continue;
        };
        let raw = json!({
            "fill_id": fill.fill_id,
            "order_id": fill.order_id,
            "price": fill.price,
            "size": fill.size,
            "fee": fill.fee,
            "timestamp": fill.ts,
            "leg_side": leg_side_label(meta.leg_side),
            "intent": meta.intent
        });
        repo.upsert_fill_by_exchange_fill_id(
            order_id,
            &fill.fill_id,
            fill.price,
            fill.size,
            fill.fee.unwrap_or_default(),
            fill.ts,
            &raw,
        )
        .await?;

        let leg = dual_leg_mut(basket, meta.leg_side);
        let _ = seen_buy_fill_order_ids.insert(fill.order_id.clone());
        apply_fill_to_leg(leg, &meta.side, fill.price, fill.size, false);
    }
    Ok(())
}

fn apply_fill_to_leg(
    leg: &mut DualLegRuntime,
    side: &str,
    price: f64,
    size: f64,
    increment_level: bool,
) {
    if side == "buy" {
        let prev_qty = leg.qty;
        let new_qty = prev_qty + size;
        if new_qty > 0.0 {
            leg.avg_entry = ((leg.avg_entry * prev_qty) + (price * size)) / new_qty;
        }
        leg.qty = new_qty;
        if increment_level {
            leg.levels_filled = leg.levels_filled.saturating_add(1);
        }
    } else {
        leg.qty = (leg.qty - size).max(0.0);
        if leg.qty <= 0.0 {
            leg.avg_entry = 0.0;
        }
    }
    leg.last_fill_price = Some(price);
}

async fn persist_leg_snapshots(
    repo: &PostgresRepository,
    basket: &DualBasketRuntime,
) -> Result<()> {
    repo.upsert_leg_position(
        basket.trade_id,
        basket.yes_leg.side,
        &basket.yes_leg.token_id,
        basket.yes_leg.qty,
        basket.yes_leg.avg_entry,
        basket.yes_leg.levels_filled as i32,
        basket.yes_leg.last_fill_price,
    )
    .await?;
    repo.upsert_leg_position(
        basket.trade_id,
        basket.no_leg.side,
        &basket.no_leg.token_id,
        basket.no_leg.qty,
        basket.no_leg.avg_entry,
        basket.no_leg.levels_filled as i32,
        basket.no_leg.last_fill_price,
    )
    .await?;
    Ok(())
}

fn basket_unrealized_pnl(basket: &DualBasketRuntime, yes_price: f64, no_price: f64) -> f64 {
    let yes = (yes_price - basket.yes_leg.avg_entry) * basket.yes_leg.qty;
    let no = (no_price - basket.no_leg.avg_entry) * basket.no_leg.qty;
    yes + no
}

fn dual_leg_mut(basket: &mut DualBasketRuntime, leg_side: LegSide) -> &mut DualLegRuntime {
    match leg_side {
        LegSide::Yes => &mut basket.yes_leg,
        LegSide::No => &mut basket.no_leg,
    }
}

fn should_leg_take_profit(
    strategy: &impl DualSideStrategy,
    leg: &DualLegRuntime,
    current_price: f64,
    leg_tp_pct: f64,
) -> bool {
    if leg.qty <= 0.0 || leg.avg_entry <= 0.0 {
        return false;
    }
    current_price >= strategy.leg_take_profit_price(leg.avg_entry, leg_tp_pct)
}

async fn maybe_paper_dca(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    trade_id: i64,
    leg: &mut DualLegRuntime,
    current_price: f64,
    level_notional: f64,
    now: DateTime<Utc>,
) -> Result<()> {
    if !can_dca_now(leg, cfg, now) {
        return Ok(());
    }
    if strategy.should_dca_leg(
        current_price,
        leg.last_fill_price,
        cfg.strategy.dca_step_pct,
        leg.levels_filled,
        cfg.strategy.max_dca_levels_per_leg,
    ) {
        let size = calc_level_size(level_notional, current_price);
        record_paper_leg_fill(repo, trade_id, leg, "dca", "buy", current_price, size).await?;
        leg.last_dca_at = Some(now);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn maybe_live_leg_dca(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    trade_id: i64,
    market_slug: &str,
    leg: &mut DualLegRuntime,
    current_price: f64,
    level_notional: f64,
    fee_rate_bps: u64,
    client: &dyn OrderExecutor,
    order_meta: &mut HashMap<String, OrderMeta>,
    now: DateTime<Utc>,
) -> Result<()> {
    if !can_dca_now(leg, cfg, now) {
        return Ok(());
    }
    if strategy.should_dca_leg(
        current_price,
        leg.last_fill_price,
        cfg.strategy.dca_step_pct,
        leg.levels_filled,
        cfg.strategy.max_dca_levels_per_leg,
    ) {
        let size = calc_level_size(level_notional, current_price);
        place_live_leg_order(
            repo,
            trade_id,
            market_slug,
            leg,
            "buy",
            "dca",
            current_price,
            size,
            fee_rate_bps,
            client,
            order_meta,
        )
        .await?;
        leg.last_dca_at = Some(now);
        leg.levels_filled = leg.levels_filled.saturating_add(1);
        leg.last_fill_price = Some(current_price);
    }
    Ok(())
}

async fn maybe_live_leg_tp(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    strategy: &impl DualSideStrategy,
    basket: &mut DualBasketRuntime,
    order_meta: &mut HashMap<String, OrderMeta>,
    client: &dyn OrderExecutor,
    yes_price: f64,
    no_price: f64,
) -> Result<()> {
    if should_leg_take_profit(
        strategy,
        &basket.yes_leg,
        yes_price,
        cfg.strategy.leg_tp_pct,
    ) {
        let key = format!("trade:{}:leg_tp:yes:{:.4}", basket.trade_id, yes_price);
        if repo.try_record_idempotency_key(&key).await? && basket.yes_leg.qty > 0.0 {
            let qty = basket.yes_leg.qty;
            place_live_leg_order(
                repo,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.yes_leg,
                "sell",
                "leg_tp",
                yes_price,
                qty,
                basket.maker_base_fee,
                client,
                order_meta,
            )
            .await?;
        }
    }
    if should_leg_take_profit(strategy, &basket.no_leg, no_price, cfg.strategy.leg_tp_pct) {
        let key = format!("trade:{}:leg_tp:no:{:.4}", basket.trade_id, no_price);
        if repo.try_record_idempotency_key(&key).await? && basket.no_leg.qty > 0.0 {
            let qty = basket.no_leg.qty;
            place_live_leg_order(
                repo,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.no_leg,
                "sell",
                "leg_tp",
                no_price,
                qty,
                basket.maker_base_fee,
                client,
                order_meta,
            )
            .await?;
        }
    }
    Ok(())
}

fn can_dca_now(leg: &DualLegRuntime, cfg: &AppConfig, now: DateTime<Utc>) -> bool {
    let Some(last_dca_at) = leg.last_dca_at else {
        return true;
    };
    now.signed_duration_since(last_dca_at).num_seconds() >= cfg.strategy.dca_interval_sec as i64
}

pub(crate) fn calc_level_size(level_notional: f64, price: f64) -> f64 {
    if price <= 0.0 {
        return 0.0;
    }
    ((level_notional / price) * 100.0).round() / 100.0
}

pub(crate) fn clamp_probability(value: f64) -> f64 {
    value.clamp(0.01, 0.99)
}

fn leg_side_label(leg_side: LegSide) -> &'static str {
    match leg_side {
        LegSide::Yes => "yes",
        LegSide::No => "no",
    }
}

fn compute_pressure_score(
    previous_yes_price: Option<f64>,
    yes_price: f64,
) -> (f64, f64, f64, bool) {
    let Some(prev_yes) = previous_yes_price else {
        return (0.0, 0.0, 0.0, false);
    };

    if prev_yes <= 0.0 {
        return (0.0, 0.0, 0.0, false);
    }

    let drop_pct = ((prev_yes - yes_price) / prev_yes * 100.0).max(0.0);
    let sell_ratio = if yes_price < prev_yes { 1.0 } else { 0.0 };
    let score = drop_pct + (sell_ratio * 0.5);
    let triggered = drop_pct >= PRESSURE_DROP_PCT_THRESHOLD;
    (score, drop_pct, sell_ratio, triggered)
}

async fn detect_drop_sell_reason(
    repo: &PostgresRepository,
    trade_id: i64,
    basket: &DualBasketRuntime,
    yes_price: f64,
    no_price: f64,
) -> Result<Option<&'static str>> {
    let mut yes_drop_pct = DEFAULT_DROP_SELL_PCT;
    let mut no_drop_pct = DEFAULT_DROP_SELL_PCT;
    let mut yes_enabled = true;
    let mut no_enabled = true;

    for rule in repo.load_position_exit_rules(trade_id).await? {
        match rule.leg_side {
            LegSide::Yes => {
                yes_drop_pct = rule.drop_sell_pct;
                yes_enabled = rule.enabled;
            }
            LegSide::No => {
                no_drop_pct = rule.drop_sell_pct;
                no_enabled = rule.enabled;
            }
        }
    }

    if yes_enabled
        && basket.yes_leg.qty > 0.0
        && price_dropped_below_threshold(
            basket
                .yes_leg
                .last_fill_price
                .unwrap_or(basket.yes_leg.avg_entry),
            yes_price,
            yes_drop_pct,
        )
    {
        return Ok(Some("drop_sell_yes"));
    }

    if no_enabled
        && basket.no_leg.qty > 0.0
        && price_dropped_below_threshold(
            basket
                .no_leg
                .last_fill_price
                .unwrap_or(basket.no_leg.avg_entry),
            no_price,
            no_drop_pct,
        )
    {
        return Ok(Some("drop_sell_no"));
    }

    Ok(None)
}

fn price_dropped_below_threshold(
    reference_price: f64,
    current_price: f64,
    drop_sell_pct: f64,
) -> bool {
    if reference_price <= 0.0 || drop_sell_pct <= 0.0 {
        return false;
    }
    current_price <= reference_price * (1.0 - (drop_sell_pct / 100.0))
}

