#[derive(Debug, Clone, Copy)]
struct AutoScopeDiagnosticPath {
    best_price: Option<f64>,
    worst_price: Option<f64>,
    sample_count: usize,
}

#[derive(Debug, Clone)]
struct AutoScopeDiagnosticEvidence {
    total_pnl_usdc: f64,
    fee_drag_usdc: f64,
    cost_basis_usdc: f64,
    entry_slippage_usdc: Option<f64>,
    exit_reason: Option<String>,
    gave_back_usdc: Option<f64>,
    max_adverse_usdc: Option<f64>,
    open_to_trigger_ms: Option<i64>,
    trigger_to_buy_fill_ms: Option<i64>,
    submit_to_fill_ms: Option<i64>,
    thin_liquidity_signal: bool,
    is_open_position: bool,
}

#[derive(Debug, Clone, Copy)]
struct AutoScopeDiagnosisCandidate {
    code: &'static str,
    label: &'static str,
    detail: &'static str,
}

fn trade_builder_analysis_clamp_score(value: f64) -> f64 {
    round_trade_builder_signed_qty(value.clamp(0.0, 100.0))
}

fn trade_builder_analysis_duration_ms(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Option<i64> {
    start
        .zip(end)
        .map(|(start, end)| end.signed_duration_since(start).num_milliseconds().max(0))
}

fn trade_builder_analysis_event_at(
    event: Option<&TradeBuilderOrderEventRecord>,
    keys: &[&str],
) -> Option<DateTime<Utc>> {
    event
        .and_then(|event| trade_builder_analysis_payload_datetime(&event.payload_json, keys))
        .or_else(|| event.map(|event| event.created_at))
}

fn trade_builder_analysis_nested_payload_number(payload: &Value, path: &[&str]) -> Option<f64> {
    let mut current = payload;
    for key in path {
        current = current.get(*key)?;
    }
    match current {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn trade_builder_analysis_event_number(
    event: Option<&TradeBuilderOrderEventRecord>,
    keys: &[&str],
) -> Option<f64> {
    event.and_then(|event| trade_builder_analysis_payload_number(&event.payload_json, keys))
}

fn trade_builder_analysis_event_number_nested(
    event: Option<&TradeBuilderOrderEventRecord>,
    path: &[&str],
) -> Option<f64> {
    event.and_then(|event| trade_builder_analysis_nested_payload_number(&event.payload_json, path))
}

fn trade_builder_analysis_outcome_side(label: &str) -> Option<&'static str> {
    let normalized = label.trim().to_ascii_lowercase();
    if normalized == "yes" || normalized == "up" || normalized.contains(" up") {
        return Some("yes");
    }
    if normalized == "no" || normalized == "down" || normalized.contains(" down") {
        return Some("no");
    }
    None
}

fn trade_builder_analysis_snapshot_exit_price(
    snapshot: &TradeBuilderMarketSecondSnapshot,
    outcome_label: &str,
) -> Option<f64> {
    match trade_builder_analysis_outcome_side(outcome_label)? {
        "yes" => snapshot
            .yes_best_bid
            .or_else(|| snapshot.yes_best_ask)
            .map(clamp_probability),
        "no" => snapshot
            .no_best_bid
            .or_else(|| snapshot.no_best_ask)
            .map(clamp_probability),
        _ => None,
    }
}

fn trade_builder_analysis_price_path(
    snapshots: &[TradeBuilderMarketSecondSnapshot],
    outcome_label: &str,
    hold_start: Option<DateTime<Utc>>,
    hold_end: Option<DateTime<Utc>>,
) -> AutoScopeDiagnosticPath {
    let mut best_price: Option<f64> = None;
    let mut worst_price: Option<f64> = None;
    let mut sample_count = 0;

    for snapshot in snapshots {
        if let Some(start) = hold_start {
            if snapshot.second_ts < start {
                continue;
            }
        }
        if let Some(end) = hold_end {
            if snapshot.second_ts > end {
                continue;
            }
        }
        let Some(price) = trade_builder_analysis_snapshot_exit_price(snapshot, outcome_label)
        else {
            continue;
        };
        best_price = Some(best_price.map(|current| current.max(price)).unwrap_or(price));
        worst_price = Some(worst_price.map(|current| current.min(price)).unwrap_or(price));
        sample_count += 1;
    }

    AutoScopeDiagnosticPath {
        best_price,
        worst_price,
        sample_count,
    }
}

fn trade_builder_analysis_latest_row_time(
    row: &TradeFlowAutoScopeAnalysisRowInput,
) -> Option<DateTime<Utc>> {
    row.sell_filled_at
        .or(row.mark_price_captured_at)
        .or(row.buy_filled_at)
        .or(row.triggered_at)
}

fn trade_builder_analysis_last_exit_row<'a>(
    rows: &'a [TradeFlowAutoScopeAnalysisRowInput],
) -> Option<&'a TradeFlowAutoScopeAnalysisRowInput> {
    rows.iter().max_by(|left, right| {
        trade_builder_analysis_latest_row_time(left)
            .cmp(&trade_builder_analysis_latest_row_time(right))
            .then_with(|| left.row_key.cmp(&right.row_key))
    })
}

fn trade_builder_analysis_data_flag(flags: &mut Vec<String>, flag: &str) {
    if !flags.iter().any(|existing| existing == flag) {
        flags.push(flag.to_string());
    }
}

fn trade_builder_analysis_thin_liquidity_signal(
    root_events: &[TradeBuilderOrderEventRecord],
    submitted_event: Option<&TradeBuilderOrderEventRecord>,
) -> bool {
    let retry_seen = root_events
        .iter()
        .any(|event| event.event_type == "submit_retry_scheduled");
    let depth_levels = trade_builder_analysis_event_number(
        submitted_event,
        &["submit_price_depth_levels_used", "depth_levels_used"],
    )
    .unwrap_or(1.0);
    let requested_qty = trade_builder_analysis_event_number(
        submitted_event,
        &["submit_price_requested_qty", "requested_qty", "size"],
    );
    let visible_qty = trade_builder_analysis_event_number(
        submitted_event,
        &[
            "submit_price_visible_bid_qty",
            "submit_price_visible_ask_qty",
            "visible_qty",
        ],
    );

    retry_seen
        || depth_levels > 1.0
        || requested_qty
            .zip(visible_qty)
            .map(|(requested, visible)| requested > 0.0 && visible < requested * 0.75)
            .unwrap_or(false)
}

fn trade_builder_analysis_quality_scores(
    evidence: &AutoScopeDiagnosticEvidence,
    snapshot_age_ms: Option<i64>,
) -> (Option<f64>, Option<f64>) {
    let cost_basis = evidence.cost_basis_usdc.max(0.000001);
    let entry_penalty = evidence
        .entry_slippage_usdc
        .map(|value| (value.max(0.0) / cost_basis) * 400.0)
        .unwrap_or(0.0);
    let fill_penalty = evidence
        .submit_to_fill_ms
        .map(|value| (value as f64 / 1000.0).min(10.0) * 2.0)
        .unwrap_or(0.0);
    let age_penalty = snapshot_age_ms
        .map(|value| (value as f64 / 1000.0).min(10.0))
        .unwrap_or(0.0);
    let liquidity_penalty = if evidence.thin_liquidity_signal { 12.0 } else { 0.0 };
    let entry_score =
        trade_builder_analysis_clamp_score(100.0 - entry_penalty - fill_penalty - age_penalty - liquidity_penalty);

    let give_back_penalty = evidence
        .gave_back_usdc
        .map(|value| (value.max(0.0) / cost_basis) * 300.0)
        .unwrap_or(0.0);
    let adverse_penalty = evidence
        .max_adverse_usdc
        .map(|value| (value.min(0.0).abs() / cost_basis) * 120.0)
        .unwrap_or(0.0);
    let loss_penalty = if evidence.total_pnl_usdc < 0.0 { 10.0 } else { 0.0 };
    let exit_score =
        trade_builder_analysis_clamp_score(100.0 - give_back_penalty - adverse_penalty - loss_penalty);

    (Some(entry_score), Some(exit_score))
}

fn trade_builder_analysis_diagnosis_candidates(
    evidence: &AutoScopeDiagnosticEvidence,
) -> Vec<AutoScopeDiagnosisCandidate> {
    let mut candidates = Vec::new();
    let loss_abs = evidence.total_pnl_usdc.min(0.0).abs();
    let cost_basis = evidence.cost_basis_usdc.max(0.000001);
    let entry_slippage = evidence.entry_slippage_usdc.unwrap_or(0.0).max(0.0);
    let gave_back = evidence.gave_back_usdc.unwrap_or(0.0).max(0.0);
    let max_adverse_abs = evidence.max_adverse_usdc.unwrap_or(0.0).min(0.0).abs();

    if evidence.total_pnl_usdc > 0.0001 {
        if evidence.exit_reason.as_deref() == Some("tp") {
            candidates.push(AutoScopeDiagnosisCandidate {
                code: "take_profit_success",
                label: "TP basarili",
                detail: "Pozisyon karda kapanmis ve cikis nedeni TP olarak gorunuyor.",
            });
        } else {
            candidates.push(AutoScopeDiagnosisCandidate {
                code: "clean_win",
                label: "Temiz kar",
                detail: "Trade net karda; belirgin zarar nedeni yok.",
            });
        }
        return candidates;
    }

    if evidence.total_pnl_usdc >= -0.0001 {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "unknown",
            label: "Basa bas",
            detail: "PnL sifira cok yakin; net kar/zarar nedeni ayrismiyor.",
        });
        return candidates;
    }

    if evidence.is_open_position {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "unrealized_mark_loss",
            label: "Acik pozisyon mark zarari",
            detail: "Pozisyon kapanmamis; zarar mevcut mark/canli fiyatla hesaplandi.",
        });
    }
    if entry_slippage >= 0.02 || entry_slippage / cost_basis >= 0.015 {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "bad_entry_price",
            label: "Kotu giris fiyati",
            detail: "Fill fiyati referans fiyata gore pahali kalmis.",
        });
    }
    if evidence
        .open_to_trigger_ms
        .map(|value| value > 90_000)
        .unwrap_or(false)
    {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "late_entry",
            label: "Gec giris",
            detail: "Market acilisindan tetiklenmeye kadar gecikme yuksek.",
        });
    }
    if evidence
        .trigger_to_buy_fill_ms
        .or(evidence.submit_to_fill_ms)
        .map(|value| value > 3_000)
        .unwrap_or(false)
    {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "slow_fill",
            label: "Yavas fill",
            detail: "Tetikten veya submitten fill'e kadar gecen sure yuksek.",
        });
    }
    if evidence.thin_liquidity_signal {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "thin_liquidity",
            label: "Ince likidite",
            detail: "Submit sirasinda derinlik/visible qty sinyali zayif gorunuyor.",
        });
    }
    if evidence.fee_drag_usdc > 0.0
        && (evidence.fee_drag_usdc >= loss_abs * 0.5
            || loss_abs <= evidence.fee_drag_usdc * 1.25)
    {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "fee_drag",
            label: "Fee zarari buyuttu",
            detail: "Toplam fee, zarar tutarinin anlamli bir kismini olusturuyor.",
        });
    }
    if gave_back > 0.02 && gave_back >= loss_abs * 0.8 {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "exit_too_late",
            label: "Cikista gec kalindi",
            detail: "Trade once daha iyi fiyati gormus, kapanista bu avantaji geri vermis.",
        });
    }
    if max_adverse_abs > 0.02 && max_adverse_abs >= loss_abs * 0.8 {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "market_reversal",
            label: "Market ters dondu",
            detail: "Hold sirasinda fiyat entry seviyesine gore belirgin ters hareket etmis.",
        });
    }
    if evidence.exit_reason.as_deref() == Some("sl") {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "stop_loss_expected",
            label: "Stop loss beklenen zarar",
            detail: "Pozisyon SL ile kapanmis; zarar stratejinin stop tarafindan uretilmis.",
        });
    }

    if candidates.is_empty() {
        candidates.push(AutoScopeDiagnosisCandidate {
            code: "unknown",
            label: "Net neden yok",
            detail: "Mevcut kayitlarla zarar nedeni tek bir sinyale indirgenemedi.",
        });
    }

    candidates
}

fn trade_builder_analysis_choose_diagnosis(
    evidence: &AutoScopeDiagnosticEvidence,
) -> (String, Option<String>, String, String) {
    let candidates = trade_builder_analysis_diagnosis_candidates(evidence);
    let primary = candidates.first().copied().unwrap_or(AutoScopeDiagnosisCandidate {
        code: "unknown",
        label: "Net neden yok",
        detail: "Mevcut kayitlarla zarar nedeni tek bir sinyale indirgenemedi.",
    });
    let secondary = candidates
        .iter()
        .skip(1)
        .find(|candidate| candidate.code != primary.code)
        .map(|candidate| candidate.code.to_string());

    (
        primary.code.to_string(),
        secondary,
        primary.label.to_string(),
        primary.detail.to_string(),
    )
}

fn trade_builder_analysis_build_trade_diagnostic(
    root_order: &TradeBuilderOrder,
    rows: &[TradeFlowAutoScopeAnalysisRowInput],
    events_by_order_id: &HashMap<i64, Vec<TradeBuilderOrderEventRecord>>,
    second_snapshots: &[TradeBuilderMarketSecondSnapshot],
    buy_metrics: &AutoScopeAnalysisOrderMetrics,
    sell_allocation_summary: &AutoScopeAnalysisSellAllocationSummary,
    open_to_trigger_ms: Option<i64>,
    trigger_to_buy_fill_ms: Option<i64>,
) -> TradeFlowAutoScopeTradeDiagnosticInput {
    let root_events = events_by_order_id
        .get(&root_order.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let submitted_event = root_events.iter().find(|event| event.event_type == "submitted");
    let filled_event = root_events.iter().find(|event| event.event_type == "filled");
    let flow_created_event = root_events
        .iter()
        .find(|event| event.event_type == "flow_created");
    let last_row = trade_builder_analysis_last_exit_row(rows);
    let hold_end = last_row.and_then(trade_builder_analysis_latest_row_time);
    let is_open_position = rows.iter().any(|row| row.row_type == "open_position");
    let exit_reason = last_row.map(|row| row.exit_reason.clone());
    let exit_price = last_row.and_then(|row| row.mark_or_sell_price);

    let submitted_at = trade_builder_analysis_event_at(submitted_event, &["submit_started_at"]);
    let trigger_to_submit_ms =
        trade_builder_analysis_duration_ms(rows.first().and_then(|row| row.triggered_at), submitted_at);
    let submit_to_fill_ms = trade_builder_analysis_duration_ms(submitted_at, buy_metrics.first_filled_at);
    let hold_ms = trade_builder_analysis_duration_ms(buy_metrics.first_filled_at, hold_end);
    let snapshot_age_ms = trade_builder_analysis_event_number(submitted_event, &["snapshot_age_ms"])
        .map(|value| value.round() as i64);
    let runtime_price_fetch_ms =
        trade_builder_analysis_event_number(submitted_event, &["runtime_price_fetch_ms"])
            .map(|value| value.round() as i64);
    let guard_eval_ms = trade_builder_analysis_event_number(submitted_event, &["guard_eval_ms"])
        .map(|value| value.round() as i64);
    let place_http_ms = trade_builder_analysis_event_number(submitted_event, &["place_http_ms"])
        .map(|value| value.round() as i64);

    let total_pnl_usdc = round_trade_builder_signed_qty(rows.iter().map(|row| row.row_pnl_usdc).sum());
    let realized_pnl_usdc = round_trade_builder_signed_qty(
        rows.iter()
            .filter(|row| row.row_type == "sell_exit")
            .map(|row| row.row_pnl_usdc)
            .sum(),
    );
    let open_pnl_usdc = round_trade_builder_signed_qty(
        rows.iter()
            .filter(|row| row.row_type == "open_position")
            .map(|row| row.row_pnl_usdc)
            .sum(),
    );
    let fee_drag_usdc = round_trade_builder_signed_qty(
        rows.iter()
            .map(|row| row.buy_fee_usdc.unwrap_or(0.0) + row.sell_fee_usdc.unwrap_or(0.0))
            .sum(),
    );
    let cost_basis_usdc = round_trade_builder_signed_qty(
        rows.iter().map(|row| row.cost_basis_usdc.unwrap_or(0.0)).sum(),
    );
    let net_value_usdc = round_trade_builder_signed_qty(
        rows.iter().map(|row| row.net_value_usdc.unwrap_or(0.0)).sum(),
    );
    let pnl_pct = (cost_basis_usdc > 0.0)
        .then_some(round_trade_builder_signed_qty((total_pnl_usdc / cost_basis_usdc) * 100.0));

    let entry_trigger_price = trade_builder_analysis_event_number(
        submitted_event.or(flow_created_event),
        &["guard_trigger_price", "trigger_price", "current_price"],
    )
    .or(root_order.trigger_price)
    .map(clamp_probability);
    let entry_submit_price = trade_builder_analysis_event_number(
        submitted_event,
        &["execution_price", "submitted_dynamic_price", "working_price", "max_price"],
    )
    .or(root_order.working_price)
    .map(clamp_probability);
    let entry_fill_price = buy_metrics.avg_price.map(clamp_probability);
    let entry_reference_price = trade_builder_analysis_event_number(submitted_event, &["best_ask"])
        .or_else(|| {
            trade_builder_analysis_event_number_nested(
                submitted_event.or(flow_created_event),
                &["runtime_snapshot", "best_ask"],
            )
        })
        .or_else(|| {
            trade_builder_analysis_event_number(submitted_event.or(flow_created_event), &["current_price"])
        })
        .map(clamp_probability);
    let entry_slippage_usdc = entry_fill_price
        .zip(entry_reference_price)
        .map(|(fill, reference)| round_trade_builder_signed_qty(((fill - reference).max(0.0)) * buy_metrics.qty));

    let path = trade_builder_analysis_price_path(
        second_snapshots,
        &root_order.outcome_label,
        buy_metrics.first_filled_at,
        hold_end,
    );
    let best_price_during_hold = path.best_price;
    let worst_price_during_hold = path.worst_price;
    let max_favorable_usdc = entry_fill_price.zip(best_price_during_hold).map(|(entry, best)| {
        round_trade_builder_signed_qty(((best - entry).max(0.0)) * buy_metrics.qty)
    });
    let max_adverse_usdc = entry_fill_price.zip(worst_price_during_hold).map(|(entry, worst)| {
        round_trade_builder_signed_qty(((worst - entry).min(0.0)) * buy_metrics.qty)
    });
    let gave_back_usdc = best_price_during_hold.zip(exit_price).map(|(best, exit)| {
        round_trade_builder_signed_qty(((best - exit).max(0.0)) * buy_metrics.qty)
    });

    let thin_liquidity_signal =
        trade_builder_analysis_thin_liquidity_signal(root_events, submitted_event);
    let evidence = AutoScopeDiagnosticEvidence {
        total_pnl_usdc,
        fee_drag_usdc,
        cost_basis_usdc,
        entry_slippage_usdc,
        exit_reason: exit_reason.clone(),
        gave_back_usdc,
        max_adverse_usdc,
        open_to_trigger_ms,
        trigger_to_buy_fill_ms,
        submit_to_fill_ms,
        thin_liquidity_signal,
        is_open_position,
    };
    let (entry_quality_score, exit_quality_score) =
        trade_builder_analysis_quality_scores(&evidence, snapshot_age_ms);
    let (primary_diagnosis_code, secondary_diagnosis_code, diagnosis_label, diagnosis_detail) =
        trade_builder_analysis_choose_diagnosis(&evidence);

    let mut data_quality_flags = Vec::new();
    if rows.first().and_then(|row| row.triggered_at).is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_trigger_event");
    }
    if submitted_event.is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_submitted_event");
    }
    if filled_event.is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_filled_event");
    }
    if buy_metrics.first_filled_at.is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_buy_fill_time");
    }
    if entry_reference_price.is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_entry_reference_price");
    }
    if trade_builder_analysis_outcome_side(&root_order.outcome_label).is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "unknown_outcome_side");
    }
    if path.sample_count == 0 {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "missing_second_snapshots");
    }
    if is_open_position {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "open_position_mark");
    }
    if sell_allocation_summary.ignored_sell_qty > 0.0 {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "oversold_exit_qty");
    }
    if submitted_event.is_none() || flow_created_event.is_none() {
        trade_builder_analysis_data_flag(&mut data_quality_flags, "old_trade_best_effort");
    }

    TradeFlowAutoScopeTradeDiagnosticInput {
        root_builder_order_id: root_order.id,
        user_id: root_order.user_id,
        definition_id: rows.first().map(|row| row.definition_id).unwrap_or_default(),
        run_id: rows.first().map(|row| row.run_id).unwrap_or_default(),
        market_slug: root_order.market_slug.clone(),
        token_id: root_order.token_id.clone(),
        outcome_label: root_order.outcome_label.clone(),
        total_pnl_usdc,
        realized_pnl_usdc,
        open_pnl_usdc,
        pnl_pct,
        fee_drag_usdc,
        cost_basis_usdc,
        net_value_usdc,
        entry_trigger_price,
        entry_submit_price,
        entry_fill_price,
        entry_reference_price,
        entry_slippage_usdc,
        entry_quality_score,
        exit_reason,
        exit_price,
        best_price_during_hold,
        worst_price_during_hold,
        max_favorable_usdc,
        max_adverse_usdc,
        gave_back_usdc,
        exit_quality_score,
        open_to_trigger_ms,
        trigger_to_buy_fill_ms,
        trigger_to_submit_ms,
        submit_to_fill_ms,
        hold_ms,
        snapshot_age_ms,
        runtime_price_fetch_ms,
        guard_eval_ms,
        place_http_ms,
        primary_diagnosis_code,
        secondary_diagnosis_code,
        diagnosis_label,
        diagnosis_detail,
        data_quality_flags,
        compact_metrics_json: json!({
            "pnl_model_version": AUTO_SCOPE_ANALYSIS_PNL_MODEL_VERSION,
            "buy_qty": round_trade_builder_share_qty(buy_metrics.qty),
            "buy_notional_usdc": round_trade_builder_signed_qty(buy_metrics.notional_usdc),
            "buy_fee_usdc": round_trade_builder_signed_qty(buy_metrics.fee_usdc),
            "sold_qty": round_trade_builder_share_qty(
                rows.iter()
                    .filter(|row| row.row_type == "sell_exit")
                    .map(|row| row.row_qty)
                    .sum()
            ),
            "allocated_sold_qty": round_trade_builder_share_qty(
                sell_allocation_summary.allocated_sold_qty
            ),
            "observed_sell_qty": round_trade_builder_share_qty(
                sell_allocation_summary.observed_sell_qty
            ),
            "ignored_sell_qty": round_trade_builder_share_qty(
                sell_allocation_summary.ignored_sell_qty
            ),
            "remaining_qty": round_trade_builder_share_qty(
                rows.iter()
                    .filter(|row| row.row_type == "open_position")
                    .map(|row| row.row_qty)
                    .sum()
            ),
            "path_sample_count": path.sample_count,
            "thin_liquidity_signal": thin_liquidity_signal,
            "submitted_at": submitted_at.map(|value| value.to_rfc3339()),
            "buy_first_filled_at": buy_metrics.first_filled_at.map(|value| value.to_rfc3339()),
            "buy_last_filled_at": buy_metrics.last_filled_at.map(|value| value.to_rfc3339()),
            "hold_end_at": hold_end.map(|value| value.to_rfc3339()),
        }),
    }
}
