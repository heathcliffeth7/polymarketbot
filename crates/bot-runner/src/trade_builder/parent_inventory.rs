const TRADE_BUILDER_PARENT_POSITION_SOURCE_CURRENT: &str = "trade_builder_parent_positions";
const TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_ACTUAL: &str =
    "inventory_actual_visible_qty";
const TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_EXPECTED: &str =
    "inventory_expected_visible_qty";
const TRADE_BUILDER_PARENT_POSITION_SOURCE_CANONICAL: &str = "canonical_entry_qty";
const TRADE_BUILDER_PARENT_POSITION_SOURCE_LEGACY: &str = "source_trade_leg_position";

fn trade_builder_parent_position_seed_qty(
    seed: Option<&TradeBuilderParentPositionSeed>,
    canonical_entry_qty: f64,
) -> (f64, &'static str) {
    if let Some(actual_visible_qty) = seed
        .and_then(|seed| normalize_trade_builder_visible_inventory_qty(seed.actual_visible_qty))
    {
        return (
            actual_visible_qty,
            TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_ACTUAL,
        );
    }
    if let Some(expected_visible_qty) = seed
        .and_then(|seed| normalize_trade_builder_visible_inventory_qty(seed.expected_visible_qty))
    {
        return (
            expected_visible_qty,
            TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_EXPECTED,
        );
    }
    (
        round_trade_builder_share_qty(canonical_entry_qty.max(0.0)),
        TRADE_BUILDER_PARENT_POSITION_SOURCE_CANONICAL,
    )
}

fn trade_builder_parent_position_fill_source(actual_fill_qty_source: Option<&str>) -> String {
    match actual_fill_qty_source {
        Some(source) if !source.trim().is_empty() => format!("child_fill:{source}"),
        _ => "child_fill:canonical_qty".to_string(),
    }
}

fn build_trade_builder_parent_position_input(
    order: &TradeBuilderOrder,
    baseline_qty: f64,
    current_qty: f64,
    last_fill_qty: Option<f64>,
    last_fill_price: Option<f64>,
    qty_source: &str,
) -> TradeBuilderParentPositionInput {
    TradeBuilderParentPositionInput {
        parent_builder_order_id: order.id,
        user_id: order.user_id,
        source_trade_id: order.trade_id,
        market_slug: order.market_slug.clone(),
        token_id: order.token_id.clone(),
        outcome_label: order.outcome_label.clone(),
        baseline_qty,
        current_qty,
        last_fill_qty,
        last_fill_price,
        qty_source: qty_source.to_string(),
    }
}

async fn ensure_trade_builder_parent_position_from_parent_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    canonical_entry_qty: f64,
    execution_price: f64,
    actual_fill_qty_source: Option<&str>,
) -> Result<TradeBuilderParentPosition> {
    if let Some(existing) = repo.get_trade_builder_parent_position(order.id).await? {
        return Ok(existing);
    }

    let seed = repo.get_trade_builder_parent_position_seed(order.id).await?;
    let (seed_qty, qty_source) =
        trade_builder_parent_position_seed_qty(seed.as_ref(), canonical_entry_qty);
    let fill_qty_source = if qty_source == TRADE_BUILDER_PARENT_POSITION_SOURCE_CANONICAL {
        actual_fill_qty_source
            .map(|source| format!("parent_fill:{source}"))
            .unwrap_or_else(|| qty_source.to_string())
    } else {
        qty_source.to_string()
    };
    let last_fill_price = seed
        .as_ref()
        .and_then(|seed| normalize_trade_builder_reference_price(seed.reference_price))
        .or_else(|| normalize_trade_builder_reference_price(Some(execution_price)));
    let last_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(Some(
        canonical_entry_qty,
    ));
    let input = build_trade_builder_parent_position_input(
        order,
        seed_qty,
        seed_qty,
        last_fill_qty,
        last_fill_price,
        &fill_qty_source,
    );
    repo.upsert_trade_builder_parent_position(&input).await
}

async fn resolve_trade_builder_parent_exit_inventory(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    reason: &str,
) -> Result<Option<(f64, &'static str)>> {
    if let Some(position) = repo
        .get_trade_builder_parent_position(parent_order.id)
        .await?
    {
        let current_qty = round_trade_builder_share_qty(position.current_qty.max(0.0));
        repo.append_trade_builder_order_event(
            parent_order.id,
            "exit_inventory_source_selected",
            &json!({
                "reason": reason,
                "inventory_source": TRADE_BUILDER_PARENT_POSITION_SOURCE_CURRENT,
                "current_parent_qty": current_qty,
                "created_position": false,
            }),
        )
        .await?;
        return Ok(Some((current_qty, TRADE_BUILDER_PARENT_POSITION_SOURCE_CURRENT)));
    }

    if let Some(seed) = repo.get_trade_builder_parent_position_seed(parent_order.id).await? {
        let actual_visible_qty = normalize_trade_builder_visible_inventory_qty(seed.actual_visible_qty);
        let expected_visible_qty =
            normalize_trade_builder_visible_inventory_qty(seed.expected_visible_qty);
        let seed_qty = actual_visible_qty.or(expected_visible_qty).unwrap_or_default();
        if seed_qty > TRADE_BUILDER_EXIT_QTY_TOLERANCE {
            let qty_source = if actual_visible_qty.is_some() {
                TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_ACTUAL
            } else {
                TRADE_BUILDER_PARENT_POSITION_SOURCE_OBSERVED_EXPECTED
            };
            let input = build_trade_builder_parent_position_input(
                parent_order,
                seed_qty,
                seed_qty,
                None,
                normalize_trade_builder_reference_price(seed.reference_price),
                qty_source,
            );
            repo.upsert_trade_builder_parent_position(&input).await?;
            repo.append_trade_builder_order_event(
                parent_order.id,
                "exit_inventory_source_selected",
                &json!({
                    "reason": reason,
                    "inventory_source": qty_source,
                    "current_parent_qty": seed_qty,
                    "created_position": true,
                }),
            )
            .await?;
            return Ok(Some((seed_qty, qty_source)));
        }
    }

    if let Ok((legacy_qty, legacy_price)) =
        load_action_place_order_sell_position(repo, parent_order.trade_id, &parent_order.token_id)
            .await
    {
        let legacy_qty = round_trade_builder_share_qty(legacy_qty.max(0.0));
        if legacy_qty > TRADE_BUILDER_EXIT_QTY_TOLERANCE {
            let input = build_trade_builder_parent_position_input(
                parent_order,
                legacy_qty,
                legacy_qty,
                None,
                legacy_price,
                TRADE_BUILDER_PARENT_POSITION_SOURCE_LEGACY,
            );
            repo.upsert_trade_builder_parent_position(&input).await?;
            repo.append_trade_builder_order_event(
                parent_order.id,
                "exit_inventory_source_selected",
                &json!({
                    "reason": reason,
                    "inventory_source": TRADE_BUILDER_PARENT_POSITION_SOURCE_LEGACY,
                    "current_parent_qty": legacy_qty,
                    "created_position": true,
                }),
            )
            .await?;
            return Ok(Some((legacy_qty, TRADE_BUILDER_PARENT_POSITION_SOURCE_LEGACY)));
        }
    }

    repo.append_trade_builder_order_event(
        parent_order.id,
        "exit_inventory_source_selected",
        &json!({
            "reason": reason,
            "inventory_source": "none",
            "current_parent_qty": Value::Null,
            "created_position": false,
        }),
    )
    .await?;
    Ok(None)
}

async fn apply_trade_builder_parent_position_child_fill(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    fill_qty: Option<f64>,
    execution_price: f64,
    actual_fill_qty_source: Option<&str>,
) -> Result<Option<TradeBuilderParentPosition>> {
    let Some(fill_qty) = normalize_trade_builder_terminal_fill_qty_candidate(fill_qty) else {
        return Ok(None);
    };
    if repo
        .get_trade_builder_parent_position(parent_order.id)
        .await?
        .is_none()
    {
        let _ = resolve_trade_builder_parent_exit_inventory(repo, parent_order, "child_fill_seed")
            .await?;
    }

    let qty_source = trade_builder_parent_position_fill_source(actual_fill_qty_source);
    repo.apply_trade_builder_parent_position_fill(
        parent_order.id,
        fill_qty,
        normalize_trade_builder_reference_price(Some(execution_price)),
        &qty_source,
    )
    .await
}

async fn maybe_rebase_trade_builder_parent_position_from_first_visible_inventory(
    repo: &PostgresRepository,
    parent_builder_order_id: i64,
) -> Result<()> {
    let Some(parent_order) = repo.get_trade_builder_order(parent_builder_order_id).await? else {
        return Ok(());
    };
    let Some(seed) = repo
        .get_trade_builder_parent_position_seed(parent_builder_order_id)
        .await?
    else {
        return Ok(());
    };
    let (seed_qty, qty_source) = trade_builder_parent_position_seed_qty(Some(&seed), 0.0);
    if seed_qty <= TRADE_BUILDER_EXIT_QTY_TOLERANCE {
        return Ok(());
    }

    let existing = repo
        .get_trade_builder_parent_position(parent_builder_order_id)
        .await?;
    let can_rebase_existing = existing.as_ref().is_none_or(|position| {
        (position.current_qty - position.baseline_qty).abs() < TRADE_BUILDER_EXIT_QTY_TOLERANCE
            && (position.qty_source == TRADE_BUILDER_PARENT_POSITION_SOURCE_CANONICAL
                || position.qty_source.starts_with("parent_fill:"))
    });
    if !can_rebase_existing {
        return Ok(());
    }

    let last_fill_qty = existing.as_ref().and_then(|position| position.last_fill_qty);
    let last_fill_price = normalize_trade_builder_reference_price(seed.reference_price)
        .or_else(|| existing.as_ref().and_then(|position| position.last_fill_price));
    let input = build_trade_builder_parent_position_input(
        &parent_order,
        seed_qty,
        seed_qty,
        last_fill_qty,
        last_fill_price,
        qty_source,
    );
    repo.upsert_trade_builder_parent_position(&input).await?;
    repo.append_trade_builder_order_event(
        parent_builder_order_id,
        "parent_position_rebased_from_first_visible_inventory",
        &json!({
            "baseline_qty": seed_qty,
            "current_qty": seed_qty,
            "qty_source": qty_source,
        }),
    )
    .await?;
    Ok(())
}
