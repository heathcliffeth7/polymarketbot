const AUTO_SCOPE_OFFICIAL_REDEEM_ABS_TOLERANCE: f64 = 0.05;
const AUTO_SCOPE_OFFICIAL_REDEEM_REL_TOLERANCE: f64 = 0.02;
const AUTO_SCOPE_OFFICIAL_PRECISION: f64 = 100_000.0;

#[derive(Debug, Clone, Default)]
struct AutoScopeAnalysisPnlReconciliation {
    data_quality_flags: Vec<String>,
    official_pnl_source: String,
    official_buy_notional_usdc: Option<f64>,
    official_sell_notional_usdc: Option<f64>,
    official_redeem_usdc: Option<f64>,
    official_pnl_usdc: Option<f64>,
    official_market_buy_usdc: Option<f64>,
    official_market_sell_usdc: Option<f64>,
    official_market_redeem_usdc: Option<f64>,
    official_market_pnl_usdc: Option<f64>,
    internal_fallback_pnl_usdc: f64,
    official_delta_usdc: Option<f64>,
}

#[derive(Debug, Clone)]
struct AutoScopeOfficialPnlRows {
    rows: Vec<TradeFlowAutoScopeAnalysisRowInput>,
    sell_allocation_summary: AutoScopeAnalysisSellAllocationSummary,
    reconciliation: AutoScopeAnalysisPnlReconciliation,
}

#[derive(Debug, Clone, Copy, Default)]
struct AutoScopeOfficialLedger {
    buy_qty: f64,
    buy_usdc: f64,
    sell_qty: f64,
    sell_usdc: f64,
    redeem_usdc: f64,
    first_buy_at: Option<DateTime<Utc>>,
    last_sell_at: Option<DateTime<Utc>>,
    last_redeem_at: Option<DateTime<Utc>>,
    sell_price: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct AutoScopeOfficialMarketLedger {
    buy_usdc: f64,
    sell_usdc: f64,
    redeem_usdc: f64,
    pnl_usdc: f64,
}

impl AutoScopeAnalysisPnlReconciliation {
    fn local(internal_fallback_pnl_usdc: f64) -> Self {
        Self {
            official_pnl_source: "local_fallback".to_string(),
            internal_fallback_pnl_usdc,
            ..Self::default()
        }
    }

    fn with_flag(mut self, flag: &str) -> Self {
        trade_builder_analysis_data_flag(&mut self.data_quality_flags, flag);
        self
    }

    fn with_market_ledger(
        mut self,
        ledger: AutoScopeOfficialMarketLedger,
        root_rows_pnl_usdc: f64,
    ) -> Self {
        self.official_market_buy_usdc = Some(round_trade_builder_official_cash_value(ledger.buy_usdc));
        self.official_market_sell_usdc = Some(round_trade_builder_official_cash_value(ledger.sell_usdc));
        self.official_market_redeem_usdc =
            Some(round_trade_builder_official_cash_value(ledger.redeem_usdc));
        self.official_market_pnl_usdc = Some(round_trade_builder_official_cash_value(ledger.pnl_usdc));
        self.official_delta_usdc = Some(round_trade_builder_official_cash_value(
            ledger.pnl_usdc - root_rows_pnl_usdc,
        ));
        self
    }
}

async fn trade_builder_analysis_try_build_official_pnl_rows(
    repo: &PostgresRepository,
    root_order: &TradeBuilderOrder,
    run: &TradeFlowRun,
    local_rows: &[TradeFlowAutoScopeAnalysisRowInput],
    timing: AutoScopeOfficialPnlTiming,
    internal_fallback_pnl_usdc: f64,
) -> AutoScopeOfficialPnlRows {
    let mut fallback = AutoScopeOfficialPnlRows {
        rows: local_rows.to_vec(),
        sell_allocation_summary: trade_builder_analysis_sell_summary_from_rows(local_rows),
        reconciliation: AutoScopeAnalysisPnlReconciliation::local(internal_fallback_pnl_usdc),
    };

    let root_count = match repo
        .count_trade_builder_filled_roots_for_market_token(
            root_order.user_id,
            &root_order.market_slug,
            &root_order.token_id,
        )
        .await
    {
        Ok(count) => count,
        Err(err) => {
            warn!(
                root_builder_order_id = root_order.id,
                error = %err,
                "AUTO_SCOPE_OFFICIAL_PNL_ROOT_COUNT_FAILED"
            );
            fallback.reconciliation = fallback
                .reconciliation
                .with_flag("official_activity_lookup_failed");
            return fallback;
        }
    };

    let market_root_count = match repo
        .count_trade_builder_filled_roots_for_market(root_order.user_id, &root_order.market_slug)
        .await
    {
        Ok(count) => count,
        Err(err) => {
            warn!(
                root_builder_order_id = root_order.id,
                error = %err,
                "AUTO_SCOPE_OFFICIAL_PNL_MARKET_ROOT_COUNT_FAILED"
            );
            fallback.reconciliation = fallback
                .reconciliation
                .with_flag("official_activity_lookup_failed");
            return fallback;
        }
    };
    if market_root_count > 1 {
        fallback.reconciliation = fallback
            .reconciliation
            .with_flag("official_market_scope_required");
    }

    let cfg = match load_user_app_config_fresh(repo, run.user_id).await {
        Ok(cfg) => cfg,
        Err(err) => {
            warn!(
                user_id = run.user_id,
                root_builder_order_id = root_order.id,
                error = %err,
                "AUTO_SCOPE_OFFICIAL_PNL_CONFIG_FAILED"
            );
            fallback.reconciliation = fallback
                .reconciliation
                .with_flag("official_activity_lookup_failed");
            return fallback;
        }
    };
    let wallet_address = match trade_builder_analysis_position_wallet_address(&cfg) {
        Ok(value) => value,
        Err(err) => {
            warn!(
                user_id = run.user_id,
                root_builder_order_id = root_order.id,
                error = %err,
                "AUTO_SCOPE_OFFICIAL_PNL_WALLET_FAILED"
            );
            fallback.reconciliation = fallback.reconciliation.with_flag("official_wallet_missing");
            return fallback;
        }
    };

    let client = PolymarketDataApiClient::new(cfg.claim.data_api_base_url.clone());
    let activity = match client
        .list_market_activity(
            &wallet_address,
            &root_order.market_slug,
            cfg.claim.positions_page_size,
            cfg.claim.positions_max_pages,
        )
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            warn!(
                user_id = run.user_id,
                root_builder_order_id = root_order.id,
                error = %err,
                "AUTO_SCOPE_OFFICIAL_PNL_ACTIVITY_FAILED"
            );
            fallback.reconciliation = fallback
                .reconciliation
                .with_flag("official_activity_lookup_failed");
            return fallback;
        }
    };

    let market_ledger = trade_builder_analysis_official_market_ledger(&activity);
    fallback.reconciliation = fallback
        .reconciliation
        .clone()
        .with_market_ledger(market_ledger, internal_fallback_pnl_usdc);
    if root_count > 1 {
        fallback.reconciliation = fallback
            .reconciliation
            .with_flag("official_activity_ambiguous");
        return fallback;
    }

    let ledger = trade_builder_analysis_official_ledger(&activity, &root_order.token_id);
    if ledger.buy_qty <= 0.0 || ledger.buy_usdc <= 0.0 {
        fallback.reconciliation = fallback
            .reconciliation
            .with_flag("official_activity_missing_buy");
        return fallback;
    }

    let build = trade_builder_analysis_build_official_rows_from_ledger(
        root_order,
        run,
        local_rows,
        timing,
        ledger,
        internal_fallback_pnl_usdc,
    );
    match build {
        Some(mut rows) => {
            rows.reconciliation = rows
                .reconciliation
                .clone()
                .with_market_ledger(market_ledger, internal_fallback_pnl_usdc);
            if market_root_count > 1 {
                rows.reconciliation = rows
                    .reconciliation
                    .with_flag("official_market_scope_required");
            }
            rows
        }
        None => {
            fallback.reconciliation = fallback
                .reconciliation
                .with_flag("official_redeem_unmatched");
            fallback
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AutoScopeOfficialPnlTiming {
    market_open_at: Option<DateTime<Utc>>,
    triggered_at: Option<DateTime<Utc>>,
    buy_filled_at: Option<DateTime<Utc>>,
    open_to_trigger_ms: Option<i64>,
    trigger_to_buy_fill_ms: Option<i64>,
}

fn trade_builder_analysis_build_official_rows_from_ledger(
    root_order: &TradeBuilderOrder,
    run: &TradeFlowRun,
    local_rows: &[TradeFlowAutoScopeAnalysisRowInput],
    timing: AutoScopeOfficialPnlTiming,
    ledger: AutoScopeOfficialLedger,
    internal_fallback_pnl_usdc: f64,
) -> Option<AutoScopeOfficialPnlRows> {
    let buy_cost_per_share = ledger.buy_usdc / ledger.buy_qty.max(0.0000001);
    let mut rows = Vec::new();
    let sell_qty = ledger.sell_qty.min(ledger.buy_qty);
    let residual_qty = round_trade_builder_official_share_qty((ledger.buy_qty - sell_qty).max(0.0));
    let mut official_pnl_usdc = -ledger.buy_usdc + ledger.sell_usdc;

    if sell_qty > 0.0 {
        let cost_basis = sell_qty * buy_cost_per_share;
        let row_pnl = ledger.sell_usdc - cost_basis;
        rows.push(TradeFlowAutoScopeAnalysisRowInput {
            row_key: format!("official_sell:{}", root_order.id),
            user_id: run.user_id,
            definition_id: run.definition_id,
            run_id: run.id,
            root_builder_order_id: root_order.id,
            exit_builder_order_id: local_rows
                .iter()
                .find(|row| row.row_type == "sell_exit")
                .and_then(|row| row.exit_builder_order_id),
            row_type: "sell_exit".to_string(),
            market_slug: root_order.market_slug.clone(),
            token_id: root_order.token_id.clone(),
            outcome_label: root_order.outcome_label.clone(),
            exit_reason: trade_builder_analysis_official_exit_reason(local_rows),
            market_open_at: timing.market_open_at,
            triggered_at: timing.triggered_at,
            buy_filled_at: ledger.first_buy_at.or(timing.buy_filled_at),
            sell_filled_at: ledger.last_sell_at,
            open_to_trigger_ms: timing.open_to_trigger_ms,
            trigger_to_buy_fill_ms: timing.trigger_to_buy_fill_ms,
            buy_avg_price: Some(round_trade_builder_official_cash_value(buy_cost_per_share)),
            mark_or_sell_price: ledger.sell_price,
            mark_price_captured_at: ledger.last_sell_at,
            row_qty: round_trade_builder_official_share_qty(sell_qty),
            remaining_qty_after_exit: residual_qty,
            row_pnl_usdc: round_trade_builder_official_cash_value(row_pnl),
            buy_notional_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
            buy_fee_usdc: Some(0.0),
            cost_basis_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
            sell_notional_usdc: Some(round_trade_builder_official_cash_value(ledger.sell_usdc)),
            sell_fee_usdc: Some(0.0),
            mark_value_usdc: None,
            net_value_usdc: Some(round_trade_builder_official_cash_value(ledger.sell_usdc)),
            pnl_pct: trade_builder_analysis_pct(row_pnl, cost_basis),
            valuation_kind: "realized".to_string(),
        });
    }

    let market_ended = trade_builder_analysis_market_has_ended(&root_order.market_slug);
    if residual_qty > 0.0 {
        if ledger.redeem_usdc > 0.0 {
            if !trade_builder_analysis_redeem_fits_residual(residual_qty, ledger.redeem_usdc) {
                return None;
            }
            official_pnl_usdc += ledger.redeem_usdc;
            rows.push(trade_builder_analysis_settled_row(
                root_order,
                run,
                timing,
                ledger,
                residual_qty,
                buy_cost_per_share,
                ledger.redeem_usdc,
            ));
        } else if market_ended {
            rows.push(trade_builder_analysis_settled_row(
                root_order,
                run,
                timing,
                ledger,
                residual_qty,
                buy_cost_per_share,
                0.0,
            ));
        } else if let Some(open_row) = trade_builder_analysis_official_open_row(
            root_order,
            run,
            local_rows,
            timing,
            ledger,
            residual_qty,
            buy_cost_per_share,
        ) {
            official_pnl_usdc += open_row.net_value_usdc.unwrap_or_default();
            rows.push(open_row);
        }
    }

    if rows.is_empty() {
        return None;
    }

    let total_pnl = round_trade_builder_official_cash_value(rows.iter().map(|row| row.row_pnl_usdc).sum());
    let reconciliation = AutoScopeAnalysisPnlReconciliation {
        official_pnl_source: "data_api_activity".to_string(),
        official_buy_notional_usdc: Some(round_trade_builder_official_cash_value(ledger.buy_usdc)),
        official_sell_notional_usdc: Some(round_trade_builder_official_cash_value(ledger.sell_usdc)),
        official_redeem_usdc: Some(round_trade_builder_official_cash_value(ledger.redeem_usdc)),
        official_pnl_usdc: Some(round_trade_builder_official_cash_value(official_pnl_usdc)),
        official_market_buy_usdc: None,
        official_market_sell_usdc: None,
        official_market_redeem_usdc: None,
        official_market_pnl_usdc: None,
        internal_fallback_pnl_usdc,
        official_delta_usdc: Some(round_trade_builder_official_cash_value(
            total_pnl - internal_fallback_pnl_usdc,
        )),
        data_quality_flags: Vec::new(),
    };

    Some(AutoScopeOfficialPnlRows {
        rows,
        sell_allocation_summary: AutoScopeAnalysisSellAllocationSummary {
            observed_sell_qty: round_trade_builder_official_share_qty(ledger.sell_qty),
            allocated_sold_qty: round_trade_builder_official_share_qty(sell_qty),
            ignored_sell_qty: round_trade_builder_official_share_qty((ledger.sell_qty - sell_qty).max(0.0)),
        },
        reconciliation,
    })
}

fn trade_builder_analysis_settled_row(
    root_order: &TradeBuilderOrder,
    run: &TradeFlowRun,
    timing: AutoScopeOfficialPnlTiming,
    ledger: AutoScopeOfficialLedger,
    row_qty: f64,
    buy_cost_per_share: f64,
    redeem_usdc: f64,
) -> TradeFlowAutoScopeAnalysisRowInput {
    let cost_basis = row_qty * buy_cost_per_share;
    let row_pnl = redeem_usdc - cost_basis;
    TradeFlowAutoScopeAnalysisRowInput {
        row_key: format!("settled:{}", root_order.id),
        user_id: run.user_id,
        definition_id: run.definition_id,
        run_id: run.id,
        root_builder_order_id: root_order.id,
        exit_builder_order_id: None,
        row_type: "settled_payout".to_string(),
        market_slug: root_order.market_slug.clone(),
        token_id: root_order.token_id.clone(),
        outcome_label: root_order.outcome_label.clone(),
        exit_reason: "other".to_string(),
        market_open_at: timing.market_open_at,
        triggered_at: timing.triggered_at,
        buy_filled_at: ledger.first_buy_at.or(timing.buy_filled_at),
        sell_filled_at: ledger.last_redeem_at,
        open_to_trigger_ms: timing.open_to_trigger_ms,
        trigger_to_buy_fill_ms: timing.trigger_to_buy_fill_ms,
        buy_avg_price: Some(round_trade_builder_official_cash_value(buy_cost_per_share)),
        mark_or_sell_price: None,
        mark_price_captured_at: ledger.last_redeem_at,
        row_qty,
        remaining_qty_after_exit: 0.0,
        row_pnl_usdc: round_trade_builder_official_cash_value(row_pnl),
        buy_notional_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
        buy_fee_usdc: Some(0.0),
        cost_basis_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
        sell_notional_usdc: None,
        sell_fee_usdc: None,
        mark_value_usdc: Some(round_trade_builder_official_cash_value(redeem_usdc)),
        net_value_usdc: Some(round_trade_builder_official_cash_value(redeem_usdc)),
        pnl_pct: trade_builder_analysis_pct(row_pnl, cost_basis),
        valuation_kind: "settled".to_string(),
    }
}

fn trade_builder_analysis_assumed_lost_settled_row(
    root_order: &TradeBuilderOrder,
    run: &TradeFlowRun,
    timing: AutoScopeOfficialPnlTiming,
    row_qty: f64,
    buy_avg_price: f64,
    buy_notional_per_share: f64,
    buy_fee_per_share: f64,
) -> TradeFlowAutoScopeAnalysisRowInput {
    let breakdown =
        trade_builder_analysis_pnl_breakdown(row_qty, buy_notional_per_share, buy_fee_per_share, 0.0);
    TradeFlowAutoScopeAnalysisRowInput {
        row_key: format!("settled:{}", root_order.id),
        user_id: run.user_id,
        definition_id: run.definition_id,
        run_id: run.id,
        root_builder_order_id: root_order.id,
        exit_builder_order_id: None,
        row_type: "settled_payout".to_string(),
        market_slug: root_order.market_slug.clone(),
        token_id: root_order.token_id.clone(),
        outcome_label: root_order.outcome_label.clone(),
        exit_reason: "other".to_string(),
        market_open_at: timing.market_open_at,
        triggered_at: timing.triggered_at,
        buy_filled_at: timing.buy_filled_at,
        sell_filled_at: None,
        open_to_trigger_ms: timing.open_to_trigger_ms,
        trigger_to_buy_fill_ms: timing.trigger_to_buy_fill_ms,
        buy_avg_price: Some(buy_avg_price),
        mark_or_sell_price: None,
        mark_price_captured_at: trade_builder_analysis_market_end_at_from_slug(&root_order.market_slug),
        row_qty,
        remaining_qty_after_exit: 0.0,
        row_pnl_usdc: breakdown.row_pnl_usdc,
        buy_notional_usdc: Some(breakdown.buy_notional_usdc),
        buy_fee_usdc: Some(breakdown.buy_fee_usdc),
        cost_basis_usdc: Some(breakdown.cost_basis_usdc),
        sell_notional_usdc: None,
        sell_fee_usdc: None,
        mark_value_usdc: Some(0.0),
        net_value_usdc: Some(0.0),
        pnl_pct: breakdown.pnl_pct,
        valuation_kind: "settled".to_string(),
    }
}

fn trade_builder_analysis_official_open_row(
    root_order: &TradeBuilderOrder,
    run: &TradeFlowRun,
    local_rows: &[TradeFlowAutoScopeAnalysisRowInput],
    timing: AutoScopeOfficialPnlTiming,
    ledger: AutoScopeOfficialLedger,
    row_qty: f64,
    buy_cost_per_share: f64,
) -> Option<TradeFlowAutoScopeAnalysisRowInput> {
    let local_open = local_rows.iter().find(|row| row.row_type == "open_position")?;
    let mark_price = local_open.mark_or_sell_price?;
    let mark_value = row_qty * mark_price;
    let cost_basis = row_qty * buy_cost_per_share;
    let row_pnl = mark_value - cost_basis;
    Some(TradeFlowAutoScopeAnalysisRowInput {
        row_key: format!("open:{}", root_order.id),
        user_id: run.user_id,
        definition_id: run.definition_id,
        run_id: run.id,
        root_builder_order_id: root_order.id,
        exit_builder_order_id: None,
        row_type: "open_position".to_string(),
        market_slug: root_order.market_slug.clone(),
        token_id: root_order.token_id.clone(),
        outcome_label: root_order.outcome_label.clone(),
        exit_reason: "open_position".to_string(),
        market_open_at: timing.market_open_at,
        triggered_at: timing.triggered_at,
        buy_filled_at: ledger.first_buy_at.or(timing.buy_filled_at),
        sell_filled_at: None,
        open_to_trigger_ms: timing.open_to_trigger_ms,
        trigger_to_buy_fill_ms: timing.trigger_to_buy_fill_ms,
        buy_avg_price: Some(round_trade_builder_official_cash_value(buy_cost_per_share)),
        mark_or_sell_price: Some(mark_price),
        mark_price_captured_at: local_open.mark_price_captured_at,
        row_qty,
        remaining_qty_after_exit: row_qty,
        row_pnl_usdc: round_trade_builder_official_cash_value(row_pnl),
        buy_notional_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
        buy_fee_usdc: Some(0.0),
        cost_basis_usdc: Some(round_trade_builder_official_cash_value(cost_basis)),
        sell_notional_usdc: None,
        sell_fee_usdc: None,
        mark_value_usdc: Some(round_trade_builder_official_cash_value(mark_value)),
        net_value_usdc: Some(round_trade_builder_official_cash_value(mark_value)),
        pnl_pct: trade_builder_analysis_pct(row_pnl, cost_basis),
        valuation_kind: "mark_to_market".to_string(),
    })
}

fn trade_builder_analysis_official_ledger(
    activity: &[DataApiActivity],
    token_id: &str,
) -> AutoScopeOfficialLedger {
    let mut ledger = AutoScopeOfficialLedger::default();
    for row in activity {
        if row.activity_type == "REDEEM" {
            ledger.redeem_usdc += row.usdc_size.max(row.size).max(0.0);
            ledger.last_redeem_at = max_datetime(ledger.last_redeem_at, row.timestamp);
            continue;
        }
        if row.activity_type != "TRADE" || row.asset.as_deref() != Some(token_id) {
            continue;
        }
        match row.side.as_deref() {
            Some("BUY") => {
                ledger.buy_qty += row.size;
                ledger.buy_usdc += row.usdc_size;
                ledger.first_buy_at = min_datetime(ledger.first_buy_at, row.timestamp);
            }
            Some("SELL") => {
                ledger.sell_qty += row.size;
                ledger.sell_usdc += row.usdc_size;
                ledger.last_sell_at = max_datetime(ledger.last_sell_at, row.timestamp);
                if row.price.is_some() {
                    ledger.sell_price = row.price;
                }
            }
            _ => {}
        }
    }
    ledger.buy_qty = round_trade_builder_official_share_qty(ledger.buy_qty);
    ledger.buy_usdc = round_trade_builder_official_cash_value(ledger.buy_usdc);
    ledger.sell_qty = round_trade_builder_official_share_qty(ledger.sell_qty);
    ledger.sell_usdc = round_trade_builder_official_cash_value(ledger.sell_usdc);
    ledger.redeem_usdc = round_trade_builder_official_cash_value(ledger.redeem_usdc);
    ledger
}

fn trade_builder_analysis_official_market_ledger(
    activity: &[DataApiActivity],
) -> AutoScopeOfficialMarketLedger {
    let mut ledger = AutoScopeOfficialMarketLedger::default();
    for row in activity {
        if row.activity_type == "REDEEM" {
            ledger.redeem_usdc += row.usdc_size.max(row.size).max(0.0);
            continue;
        }
        if row.activity_type != "TRADE" {
            continue;
        }
        match row.side.as_deref() {
            Some("BUY") => ledger.buy_usdc += row.usdc_size.max(0.0),
            Some("SELL") => ledger.sell_usdc += row.usdc_size.max(0.0),
            _ => {}
        }
    }
    ledger.buy_usdc = round_trade_builder_official_cash_value(ledger.buy_usdc);
    ledger.sell_usdc = round_trade_builder_official_cash_value(ledger.sell_usdc);
    ledger.redeem_usdc = round_trade_builder_official_cash_value(ledger.redeem_usdc);
    ledger.pnl_usdc = round_trade_builder_official_cash_value(
        ledger.sell_usdc + ledger.redeem_usdc - ledger.buy_usdc,
    );
    ledger
}

fn trade_builder_analysis_sell_summary_from_rows(
    rows: &[TradeFlowAutoScopeAnalysisRowInput],
) -> AutoScopeAnalysisSellAllocationSummary {
    let sold_qty = round_trade_builder_share_qty(
        rows.iter()
            .filter(|row| row.row_type == "sell_exit")
            .map(|row| row.row_qty)
            .sum(),
    );
    AutoScopeAnalysisSellAllocationSummary {
        observed_sell_qty: sold_qty,
        allocated_sold_qty: sold_qty,
        ignored_sell_qty: 0.0,
    }
}

fn trade_builder_analysis_position_wallet_address(cfg: &AppConfig) -> Result<String> {
    if let Some(safe_address) = cfg.exchange.resolve_gnosis_safe_address() {
        return Ok(safe_address);
    }
    let (creds, _) = resolve_api_credentials_with_source(cfg)?;
    Ok(creds.address)
}

async fn load_user_app_config_fresh(repo: &PostgresRepository, user_id: i64) -> Result<AppConfig> {
    let mut cache = HashMap::new();
    load_user_app_config_cached(repo, user_id, &mut cache).await
}

fn trade_builder_analysis_official_exit_reason(
    rows: &[TradeFlowAutoScopeAnalysisRowInput],
) -> String {
    rows.iter()
        .find(|row| row.row_type == "sell_exit")
        .map(|row| row.exit_reason.clone())
        .unwrap_or_else(|| "other".to_string())
}

fn trade_builder_analysis_pct(row_pnl: f64, cost_basis: f64) -> Option<f64> {
    (cost_basis > 0.0).then_some(round_trade_builder_signed_qty((row_pnl / cost_basis) * 100.0))
}

fn round_trade_builder_official_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * AUTO_SCOPE_OFFICIAL_PRECISION).round() / AUTO_SCOPE_OFFICIAL_PRECISION
}

fn round_trade_builder_official_cash_value(value: f64) -> f64 {
    (value * AUTO_SCOPE_OFFICIAL_PRECISION).round() / AUTO_SCOPE_OFFICIAL_PRECISION
}

fn min_datetime(current: Option<DateTime<Utc>>, ts: Option<i64>) -> Option<DateTime<Utc>> {
    let next = ts.and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))?;
    Some(current.map(|value| value.min(next)).unwrap_or(next))
}

fn max_datetime(current: Option<DateTime<Utc>>, ts: Option<i64>) -> Option<DateTime<Utc>> {
    let next = ts.and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))?;
    Some(current.map(|value| value.max(next)).unwrap_or(next))
}

fn trade_builder_analysis_market_has_ended(market_slug: &str) -> bool {
    let Some(end_at) = trade_builder_analysis_market_end_at_from_slug(market_slug) else {
        return false;
    };
    Utc::now() >= end_at
}

fn trade_builder_analysis_market_end_at_from_slug(market_slug: &str) -> Option<DateTime<Utc>> {
    let open_at = trade_builder_analysis_market_open_at_from_slug(market_slug)?;
    let duration = if market_slug.to_ascii_lowercase().contains("-15m-") {
        ChronoDuration::minutes(15)
    } else {
        ChronoDuration::minutes(5)
    };
    Some(open_at + duration)
}

fn trade_builder_analysis_redeem_fits_residual(residual_qty: f64, redeem_usdc: f64) -> bool {
    let redeem_tolerance =
        AUTO_SCOPE_OFFICIAL_REDEEM_ABS_TOLERANCE.max(redeem_usdc.abs() * AUTO_SCOPE_OFFICIAL_REDEEM_REL_TOLERANCE);
    redeem_usdc <= residual_qty + redeem_tolerance
}

#[cfg(test)]
mod official_pnl_tests {
    use super::*;

    #[test]
    fn official_ledger_uses_activity_cash_values() {
        let rows = vec![
            DataApiActivity {
                activity_type: "TRADE".to_string(),
                side: Some("BUY".to_string()),
                slug: "btc-updown-5m-1777336800".to_string(),
                asset: Some("token-up".to_string()),
                outcome: Some("Up".to_string()),
                size: 10.02238,
                usdc_size: 4.8974,
                price: Some(0.47),
                timestamp: Some(1),
            },
            DataApiActivity {
                activity_type: "TRADE".to_string(),
                side: Some("SELL".to_string()),
                slug: "btc-updown-5m-1777336800".to_string(),
                asset: Some("token-up".to_string()),
                outcome: Some("Up".to_string()),
                size: 9.0,
                usdc_size: 2.739,
                price: Some(0.32),
                timestamp: Some(2),
            },
            DataApiActivity {
                activity_type: "REDEEM".to_string(),
                side: None,
                slug: "btc-updown-5m-1777336800".to_string(),
                asset: None,
                outcome: None,
                size: 1.02238,
                usdc_size: 1.02238,
                price: Some(0.0),
                timestamp: Some(3),
            },
        ];

        let ledger = trade_builder_analysis_official_ledger(&rows, "token-up");
        let market_ledger = trade_builder_analysis_official_market_ledger(&rows);
        let pnl = round_trade_builder_official_cash_value(
            -ledger.buy_usdc + ledger.sell_usdc + ledger.redeem_usdc,
        );

        assert_eq!(ledger.buy_qty, 10.02238);
        assert_eq!(ledger.sell_qty, 9.0);
        assert_eq!(ledger.redeem_usdc, 1.02238);
        assert_eq!(pnl, -1.13602);
        assert_eq!(market_ledger.pnl_usdc, -1.13602);
    }

    #[test]
    fn official_market_ledger_includes_counter_buy_cash() {
        let rows = vec![
            DataApiActivity {
                activity_type: "TRADE".to_string(),
                side: Some("BUY".to_string()),
                slug: "btc-updown-5m-1777336200".to_string(),
                asset: Some("down-token".to_string()),
                outcome: Some("Down".to_string()),
                size: 9.09,
                usdc_size: 9.9258,
                price: Some(0.55),
                timestamp: Some(1),
            },
            DataApiActivity {
                activity_type: "TRADE".to_string(),
                side: Some("BUY".to_string()),
                slug: "btc-updown-5m-1777336200".to_string(),
                asset: Some("up-token".to_string()),
                outcome: Some("Up".to_string()),
                size: 8.84,
                usdc_size: 3.2708,
                price: Some(0.37),
                timestamp: Some(2),
            },
            DataApiActivity {
                activity_type: "TRADE".to_string(),
                side: Some("SELL".to_string()),
                slug: "btc-updown-5m-1777336200".to_string(),
                asset: Some("down-token".to_string()),
                outcome: Some("Down".to_string()),
                size: 9.09,
                usdc_size: 12.066308,
                price: Some(0.91),
                timestamp: Some(3),
            },
        ];

        let ledger = trade_builder_analysis_official_market_ledger(&rows);

        assert_eq!(ledger.buy_usdc, 13.1966);
        assert_eq!(ledger.sell_usdc, 12.06631);
        assert_eq!(ledger.pnl_usdc, -1.13029);
    }

    #[test]
    fn redeem_allocation_accepts_fee_reduced_residual_payout() {
        assert!(trade_builder_analysis_redeem_fits_residual(1.42, 1.02238));
        assert!(!trade_builder_analysis_redeem_fits_residual(0.46, 1.02238));
    }

    #[test]
    fn market_has_ended_uses_slug_window_duration() {
        let future_open = Utc::now() + ChronoDuration::hours(1);
        let future_slug = format!("btc-updown-5m-{}", future_open.timestamp());

        assert!(trade_builder_analysis_market_has_ended(
            "btc-updown-5m-1772296200"
        ));
        assert!(!trade_builder_analysis_market_has_ended(&future_slug));
    }
}
