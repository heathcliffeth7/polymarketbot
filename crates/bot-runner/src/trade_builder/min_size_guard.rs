#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeBuilderShareSubmitMinSizeDecision {
    Retry,
    Block,
}

const TRADE_BUILDER_MARKET_SPEC_CACHE_TTL_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, Default)]
struct TradeBuilderMarketSpec {
    neg_risk: bool,
    order_price_min_tick_size: Option<f64>,
    order_min_size: Option<f64>,
}

static TRADE_BUILDER_MARKET_SPEC_CACHE: LazyLock<
    StdMutex<HashMap<String, (Instant, TradeBuilderMarketSpec)>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

fn normalize_trade_builder_market_spec_number(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

fn trade_builder_market_spec_slug_candidates(market_slug: &str) -> Vec<String> {
    let normalized = market_slug.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut candidates = vec![normalized.clone()];
    let mut current = normalized.as_str();
    for _ in 0..4 {
        let Some((parent, _)) = current.rsplit_once('-') else { break };
        if parent.len() < 3 { break }
        if !candidates.iter().any(|c| c == parent) {
            candidates.push(parent.to_string());
        }
        current = parent;
    }
    candidates
}

fn trade_builder_market_spec_cache_get(market_slug: &str) -> Option<TradeBuilderMarketSpec> {
    let cache = TRADE_BUILDER_MARKET_SPEC_CACHE.lock().ok()?;
    let (cached_at, spec) = cache.get(market_slug)?;
    if cached_at.elapsed().as_secs() > TRADE_BUILDER_MARKET_SPEC_CACHE_TTL_SECS {
        return None;
    }
    Some(*spec)
}

fn trade_builder_market_spec_cache_put(market_slug: &str, spec: TradeBuilderMarketSpec) {
    if let Ok(mut cache) = TRADE_BUILDER_MARKET_SPEC_CACHE.lock() {
        cache.insert(market_slug.to_string(), (Instant::now(), spec));
    }
}

async fn resolve_trade_builder_market_spec(
    cfg: &AppConfig,
    market_slug: &str,
    token_id: &str,
) -> Option<TradeBuilderMarketSpec> {
    let candidates = trade_builder_market_spec_slug_candidates(market_slug);

    if !candidates.is_empty() {
        for candidate in &candidates {
            if let Some(spec) = trade_builder_market_spec_cache_get(candidate) {
                if !candidates.is_empty() {
                    trade_builder_market_spec_cache_put(&candidates[0], spec);
                }
                return Some(spec);
            }
        }

        let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
        for candidate in &candidates {
            let Ok(Some(market)) = gamma.get_market_spec_by_slug(candidate).await else {
                continue;
            };
            let spec = TradeBuilderMarketSpec {
                neg_risk: market.neg_risk,
                order_price_min_tick_size: normalize_trade_builder_market_spec_number(
                    market.order_price_min_tick_size,
                ),
                order_min_size: normalize_trade_builder_market_spec_number(market.order_min_size),
            };
            trade_builder_market_spec_cache_put(candidate, spec);
            trade_builder_market_spec_cache_put(&candidates[0], spec);
            return Some(spec);
        }
    }

    if !token_id.is_empty() {
        let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
        if let Ok(Some(market)) = gamma.get_market_spec_by_token_id(token_id).await {
            let spec = TradeBuilderMarketSpec {
                neg_risk: market.neg_risk,
                order_price_min_tick_size: normalize_trade_builder_market_spec_number(
                    market.order_price_min_tick_size,
                ),
                order_min_size: normalize_trade_builder_market_spec_number(market.order_min_size),
            };
            if !candidates.is_empty() {
                trade_builder_market_spec_cache_put(&candidates[0], spec);
            }
            return Some(spec);
        }
    }

    warn!(
        market_slug,
        token_id,
        candidates = ?candidates,
        "TRADE_BUILDER_MARKET_SPEC_UNRESOLVED"
    );
    None
}

fn trade_builder_market_min_size(spec: Option<TradeBuilderMarketSpec>) -> Option<f64> {
    spec.and_then(|spec| normalize_trade_builder_market_spec_number(spec.order_min_size))
}

async fn resolve_trade_builder_order_min_size(
    cfg: &AppConfig,
    order: &TradeBuilderOrder,
) -> Option<f64> {
    let now = Utc::now();
    if let Some(order_min_size) = trade_builder_runtime_snapshot_from_order(order)
        .filter(|snapshot| trade_builder_runtime_snapshot_is_fresh(snapshot, now))
        .and_then(|snapshot| trade_builder_market_spec_from_runtime_snapshot(&snapshot))
        .and_then(|spec| trade_builder_market_min_size(Some(spec)))
    {
        return Some(order_min_size);
    }

    trade_builder_market_min_size(
        resolve_trade_builder_market_spec(cfg, &order.market_slug, &order.token_id).await,
    )
}

fn trade_builder_share_submit_min_size_decision(
    requested_qty: Option<f64>,
    submit_qty: f64,
    order_min_size: Option<f64>,
) -> Option<TradeBuilderShareSubmitMinSizeDecision> {
    let order_min_size = normalize_trade_builder_market_spec_number(order_min_size)?;
    if !submit_qty.is_finite() || submit_qty <= 0.0 || submit_qty >= order_min_size {
        return None;
    }

    let requested_qty = requested_qty
        .and_then(|qty| normalize_trade_builder_visible_inventory_qty(Some(qty)))
        .unwrap_or_default();
    Some(if requested_qty >= order_min_size {
        TradeBuilderShareSubmitMinSizeDecision::Retry
    } else {
        TradeBuilderShareSubmitMinSizeDecision::Block
    })
}

fn trade_builder_next_min_size_retry_stage(
    attempt_stage: Option<TradeBuilderExitSubmitStage>,
) -> Option<TradeBuilderExitSubmitStage> {
    match attempt_stage {
        Some(TradeBuilderExitSubmitStage::DynamicGross | TradeBuilderExitSubmitStage::EstimatedVisible) => {
            Some(TradeBuilderExitSubmitStage::VisibleInventory)
        }
        other => other,
    }
}

async fn maybe_handle_trade_builder_share_submit_below_market_min(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_type: &str,
    submit_kind: &str,
    current_price: f64,
    desired_price: f64,
    requested_qty: Option<f64>,
    submit_qty: f64,
    available_qty: Option<f64>,
    order_min_size: Option<f64>,
    attempt_stage: Option<TradeBuilderExitSubmitStage>,
) -> Result<bool> {
    if order.side != "sell"
        || normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES
    {
        return Ok(false);
    }

    let Some(order_min_size) = normalize_trade_builder_market_spec_number(order_min_size) else {
        return Ok(false);
    };
    let Some(decision) = trade_builder_share_submit_min_size_decision(
        requested_qty,
        submit_qty,
        Some(order_min_size),
    ) else {
        return Ok(false);
    };

    let requested_qty = requested_qty
        .or(order.remaining_qty)
        .or(order.target_qty)
        .and_then(|qty| normalize_trade_builder_visible_inventory_qty(Some(qty)))
        .or_else(|| normalize_trade_builder_visible_inventory_qty(Some(submit_qty)));
    let reason = format!(
        "{submit_kind} size ({submit_qty:.2}) below market minimum: {order_min_size:.2}"
    );

    match decision {
        TradeBuilderShareSubmitMinSizeDecision::Retry => {
            schedule_trade_builder_exit_sell_retry(
                repo,
                order,
                event_type,
                &reason,
                current_price,
                desired_price,
                requested_qty,
                available_qty,
                Some(submit_qty),
                requested_qty,
                attempt_stage,
                trade_builder_next_min_size_retry_stage(attempt_stage),
            )
            .await?;
        }
        TradeBuilderShareSubmitMinSizeDecision::Block => {
            let remaining_size = requested_qty.map(|qty| (qty * desired_price).max(0.0));
            repo.set_trade_builder_order_retry_state(
                order.id,
                "blocked",
                Some(&reason),
                remaining_size,
                requested_qty,
            )
            .await?;
            repo.append_trade_builder_order_event(
                order.id,
                event_type,
                &json!({
                    "reason": reason,
                    "reason_code": "below_market_min_size",
                    "status_before": &order.status,
                    "status_after": "blocked",
                    "submit_kind": submit_kind,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "requested_qty": requested_qty,
                    "attempted_qty": submit_qty,
                    "available_qty": available_qty,
                    "order_min_size": order_min_size,
                    "attempt_stage": attempt_stage.map(TradeBuilderExitSubmitStage::as_str),
                    "next_attempt_stage": Value::Null,
                    "size_basis": &order.size_basis,
                    "target_qty": order.target_qty,
                    "remaining_qty": requested_qty,
                }),
            )
            .await?;
        }
    }

    Ok(true)
}
