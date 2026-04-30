fn build_guard_notification_reason(scope: &str, reason_code: &str) -> String {
    format!("{scope}:{reason_code}")
}

fn should_send_guard_transition_notification(
    order: &TradeBuilderOrder,
    candidate_reason: &str,
    notify_flag: bool,
) -> bool {
    notify_flag
        && order.last_guard_notification_reason.as_deref() != Some(candidate_reason)
}

async fn build_trade_builder_flow_identity(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Option<(String, Value)> {
    let definition_id = order.origin_flow_definition_id?;
    let definition_name = repo
        .get_trade_flow_definition(definition_id)
        .await
        .ok()
        .flatten()
        .map(|definition| definition.name)
        .unwrap_or_else(|| "?".to_string());
    let node_key = order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let run_id = order.origin_flow_run_id;
    let mut block = format!("\nFlow: {} (#{definition_id})", definition_name);
    if let Some(run_id) = run_id {
        block.push_str(&format!("\nRun: #{run_id}"));
    }
    if let Some(node_key) = node_key {
        block.push_str(&format!("\nNode: {node_key}"));
    }
    block.push_str(&format!("\nSource Trade: #{}", order.trade_id));

    Some((
        block,
        json!({
            "origin_flow_definition_id": definition_id,
            "origin_flow_name": definition_name,
            "origin_flow_run_id": run_id,
            "origin_flow_node_key": node_key,
            "source_trade_id": order.trade_id,
        }),
    ))
}

async fn build_trade_flow_notification_identity(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
) -> Option<(String, Value)> {
    let definition_name = repo
        .get_trade_flow_definition(run.definition_id)
        .await
        .ok()
        .flatten()
        .map(|definition| definition.name)
        .unwrap_or_else(|| "?".to_string());
    let mut block = format!("\nFlow: {} (#{})", definition_name, run.definition_id);
    block.push_str(&format!("\nRun: #{}", run.id));
    block.push_str(&format!("\nNode: {node_key}"));
    Some((
        block,
        json!({
            "origin_flow_definition_id": run.definition_id,
            "origin_flow_name": definition_name,
            "origin_flow_run_id": run.id,
            "origin_flow_node_key": node_key,
        }),
    ))
}

async fn send_trade_builder_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    notification_type: &str,
    message: &str,
) -> bool {
    send_trade_builder_notification_with_payload(repo, order, notification_type, message, None)
        .await
}

async fn send_trade_builder_notification_with_payload(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    notification_type: &str,
    message: &str,
    extra_payload: Option<Value>,
) -> bool {
    let flow_identity = build_trade_builder_flow_identity(repo, order).await;
    let message = if let Some((block, _)) = flow_identity.as_ref() {
        format!("{message}{block}")
    } else {
        message.to_string()
    };
    let Ok(telegram) = load_user_telegram_config(repo, order.user_id).await else {
        return false;
    };
    let bot_token = telegram.bot_token.trim();
    let chat_id = telegram.chat_id.trim();
    if bot_token.is_empty() || chat_id.is_empty() {
        return false;
    }

    let Ok(bot_token) = decrypt_config_string_if_needed("telegram.bot_token", bot_token) else {
        return false;
    };
    if bot_token.is_empty() {
        return false;
    }

    let send_result = send_telegram_message(
        order.user_id,
        &bot_token,
        chat_id,
        message.as_str(),
        None,
        notification_type,
    )
    .await;

    if send_result.sent {
            let mut event_payload = json!({
                "notification_type": notification_type,
                "message": message.as_str(),
                "chat_id": chat_id,
                "flow_identity": flow_identity.as_ref().map(|(_, payload)| payload.clone()),
            });
            if let (Some(target), Some(extra)) =
                (event_payload.as_object_mut(), extra_payload.and_then(|payload| payload.as_object().cloned()))
            {
                for (key, value) in extra {
                    target.insert(key, value);
                }
            }
            if let Err(err) = repo
                .append_trade_builder_order_event(
                    order.id,
                    "notification_sent",
                    &event_payload,
                )
                .await
            {
                warn!(
                    builder_order_id = order.id,
                    notification_type,
                    error = %err,
                    "TRADE_BUILDER_NOTIFICATION_EVENT_WRITE_FAILED"
                );
            }
            info!(
                builder_order_id = order.id,
                notification_type,
                "TRADE_BUILDER_NOTIFICATION_SENT"
            );
            true
    } else if send_result.skipped_by_backoff {
            warn!(
                builder_order_id = order.id,
                notification_type,
                backoff_until_ms = send_result.backoff_until_ms,
                "TRADE_BUILDER_NOTIFICATION_SKIPPED_TELEGRAM_BACKOFF"
            );
            false
    } else {
            warn!(
                builder_order_id = order.id,
                notification_type,
                http_status = send_result.http_status,
                retry_after_sec = send_result.retry_after_sec,
                error = ?send_result.error_message,
                "TRADE_BUILDER_NOTIFICATION_FAILED"
            );
            false
    }
}

async fn send_trade_flow_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    notification_type: &str,
    message: &str,
) -> bool {
    let flow_identity = build_trade_flow_notification_identity(repo, run, node_key).await;
    let message = if let Some((block, _)) = flow_identity.as_ref() {
        format!("{message}{block}")
    } else {
        message.to_string()
    };
    let Ok(telegram) = load_user_telegram_config(repo, run.user_id).await else {
        return false;
    };
    let bot_token = telegram.bot_token.trim();
    let chat_id = telegram.chat_id.trim();
    if bot_token.is_empty() || chat_id.is_empty() {
        return false;
    }

    let Ok(bot_token) = decrypt_config_string_if_needed("telegram.bot_token", bot_token) else {
        return false;
    };
    if bot_token.is_empty() {
        return false;
    }

    let send_result = send_telegram_message(
        run.user_id,
        &bot_token,
        chat_id,
        message.as_str(),
        None,
        notification_type,
    )
    .await;

    if send_result.sent {
            info!(
                flow_run_id = run.id,
                notification_type,
                node_key,
                "TRADE_FLOW_NOTIFICATION_SENT"
            );
            true
    } else if send_result.skipped_by_backoff {
            warn!(
                flow_run_id = run.id,
                notification_type,
                node_key,
                backoff_until_ms = send_result.backoff_until_ms,
                "TRADE_FLOW_NOTIFICATION_SKIPPED_TELEGRAM_BACKOFF"
            );
            false
    } else {
            warn!(
                flow_run_id = run.id,
                notification_type,
                node_key,
                http_status = send_result.http_status,
                retry_after_sec = send_result.retry_after_sec,
                error = ?send_result.error_message,
                "TRADE_FLOW_NOTIFICATION_FAILED"
            );
            false
    }
}

async fn maybe_send_guard_transition_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    candidate_reason: &str,
    notify_flag: bool,
    notification_type: &str,
    message: &str,
) -> Result<bool> {
    if !should_send_guard_transition_notification(order, candidate_reason, notify_flag) {
        return Ok(false);
    }
    if !send_trade_builder_notification(repo, order, notification_type, message).await {
        return Ok(false);
    }
    repo.update_trade_builder_guard_notification_reason(order.id, Some(candidate_reason))
        .await?;
    Ok(true)
}

fn should_send_order_not_filled_notification(order: &TradeBuilderOrder) -> bool {
    order.notify_on_order_not_filled && order.filled_qty <= 0.0
}

async fn maybe_send_order_not_filled_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason_code: &str,
    reason_message: &str,
) -> bool {
    if !should_send_order_not_filled_notification(order) {
        return false;
    }
    let events = match repo
        .list_trade_builder_order_events_for_orders(&[order.id])
        .await
    {
        Ok(events) => events,
        Err(err) => {
            warn!(
                builder_order_id = order.id,
                error = %err,
                "TRADE_BUILDER_ORDER_NOT_FILLED_REASON_LOAD_FAILED"
            );
            Vec::new()
        }
    };
    let guard_summary = build_order_not_filled_guard_summary(order, &events);
    let message = build_order_not_filled_notification_message_with_guard(
        order,
        reason_code,
        reason_message,
        guard_summary.as_ref(),
    );
    send_trade_builder_notification(repo, order, "order_not_filled", &message).await
}

fn trade_builder_fill_notification_type(order: &TradeBuilderOrder) -> Option<&'static str> {
    if !order.notify_on_fill {
        return None;
    }

    if order.parent_order_id.is_some() && order.side == "sell" {
        return match order.trigger_condition.as_deref() {
            Some("cross_above") => Some("tp_hit"),
            Some("cross_below") => Some("sl_hit"),
            _ => Some("order_filled"),
        };
    }

    Some("order_filled")
}

fn build_trade_builder_fill_notification(
    order: &TradeBuilderOrder,
    execution_price: f64,
    filled_qty: f64,
    flow_created_payload: Option<&Value>,
    submitted_payload: Option<&Value>,
    fill_execution_analysis: Option<&TradeBuilderFillExecutionAnalysis>,
) -> Option<(&'static str, String)> {
    let notification_type = trade_builder_fill_notification_type(order)?;
    let (title, reason) = match notification_type {
        "tp_hit" => (
            "Take Profit Tetiklendi",
            "Take profit seviyesi goruldugu icin cikis emri dolduruldu.",
        ),
        "sl_hit" => (
            "Stop Loss Tetiklendi",
            "Stop loss seviyesi goruldugu icin cikis emri dolduruldu.",
        ),
        _ => ("Emir Doldu", "Emir basariyla dolduruldu."),
    };

    let mut message = if order.side == "buy"
        && order.parent_order_id.is_none()
        && normalize_trade_builder_size_basis(&order.size_basis) == TRADE_BUILDER_SIZE_BASIS_SHARES
    {
        format!(
            "{title}\nSebep: {}\nMarket: {}\nFiyat: {:.4}\nSize Mode: shares\nTarget Qty: {:.2}\nEstimated Notional: {:.2} USDC\nAdet: {:.2}\nOutcome: {}",
            reason,
            order.market_slug,
            execution_price,
            order.target_qty.unwrap_or(filled_qty),
            order.target_qty.unwrap_or(filled_qty) * execution_price,
            filled_qty,
            order.outcome_label
        )
    } else if order.side == "buy" && order.parent_order_id.is_none() {
        format!(
            "{title}\nSebep: {}\nMarket: {}\nFiyat: {:.4}\nNotional USDC: {:.2}\nAdet: {:.2}\nOutcome: {}",
            reason,
            order.market_slug,
            execution_price,
            order.size_usdc,
            filled_qty,
            order.outcome_label
        )
    } else {
        format!(
            "{title}\nSebep: {}\nMarket: {}\nFiyat: {:.4}\nMiktar: {:.2}\nOutcome: {}",
            reason, order.market_slug, execution_price, filled_qty, order.outcome_label
        )
    };
    if let Some(analysis) = fill_execution_analysis {
        message.push_str(&build_trade_builder_fill_analysis_block(
            order,
            analysis,
            flow_created_payload,
            submitted_payload,
        ));
    }
    if let Some(block) =
        trade_builder_iv_mismatch_fill_formula_block(flow_created_payload, execution_price)
    {
        message.push_str(&block);
    }

    Some((notification_type, message))
}

fn build_trigger_guard_blocked_notification_message(
    order: &TradeBuilderOrder,
    reference_price: f64,
    reference_source: &str,
) -> String {
    format!(
        "Tetik Fiyat Korumasi Engelledi\nSebep: Referans fiyat guard seviyesinin altina dustu.\nMarket: {}\nOutcome: {}\nReferans ({}): {:.4}\nGuard: {:.4}",
        order.market_slug,
        order.outcome_label,
        reference_source,
        reference_price,
        order.guard_trigger_price.unwrap_or(0.0)
    )
}

fn build_trigger_guard_waiting_notification_message(
    order: &TradeBuilderOrder,
    reference_price: f64,
    reference_source: &str,
) -> String {
    format!(
        "Tetik Fiyat Korumasi Bekleme Modu\nSebep: Referans fiyat guard seviyesinin altina dustu. Kosullar duzelince order yeniden denenecek.\nMarket: {}\nOutcome: {}\nReferans ({}): {:.4}\nGuard: {:.4}",
        order.market_slug,
        order.outcome_label,
        reference_source,
        reference_price,
        order.guard_trigger_price.unwrap_or(0.0)
    )
}

fn build_max_price_blocked_notification_message(
    order: &TradeBuilderOrder,
    current_price: f64,
    reference_price: f64,
    reference_source: &str,
) -> String {
    format!(
        "Max Fiyat Korumasi Engelledi\nSebep: Referans fiyat max fiyat limitini asiyor.\nMarket: {}\nOutcome: {}\nGuncel: {:.4}\nReferans ({}): {:.4}\nMax: {:.4}",
        order.market_slug,
        order.outcome_label,
        current_price,
        reference_source,
        reference_price,
        order.max_price.unwrap_or(0.0)
    )
}

fn build_max_price_waiting_notification_message(
    order: &TradeBuilderOrder,
    current_price: f64,
    reference_price: f64,
    reference_source: &str,
    reason_code: Option<&str>,
) -> String {
    let reason = match reason_code.unwrap_or(reference_source) {
        "best_ask_unavailable"
        | "pair_primary_best_ask_unavailable"
        | "pair_counter_best_ask_unavailable" => {
            "Best ask verisi bekleniyor. Max fiyat degerlendirmesi ask verisi gelince yeniden yapilacak."
        }
        _ => "Referans fiyat max fiyat limitini asiyor. Kosullar duzelince order yeniden denenecek.",
    };
    format!(
        "Max Fiyat Korumasi Bekleme Modu
Sebep: {}
Market: {}
Outcome: {}
Guncel: {:.4}
Referans ({}): {:.4}
Max: {:.4}",
        reason,
        order.market_slug,
        order.outcome_label,
        current_price,
        reference_source,
        reference_price,
        order.max_price.unwrap_or(0.0)
    )
}

fn build_max_price_blocked_notification(
    order: &TradeBuilderOrder,
    current_price: f64,
    reference_price: f64,
    reference_source: &str,
) -> Option<(&'static str, String)> {
    order.notify_on_max_price_blocked.then(|| {
        (
            "max_price_blocked",
            build_max_price_blocked_notification_message(
                order,
                current_price,
                reference_price,
                reference_source,
            ),
        )
    })
}

fn build_execution_floor_blocked_notification_message(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
) -> String {
    let reason = match (best_ask, order.best_ask_floor_price) {
        (None, _) => "Best ask verisi alinamadigi icin floor korumasi orderi engelledi.",
        (Some(best_ask), Some(floor)) if best_ask < floor => {
            "Best ask floor seviyesinin altinda kaldigi icin order engellendi."
        }
        _ => "Execution floor korumasi orderi engelledi.",
    };
    format!(
        "Execution Floor Engelledi\nSebep: {}\nMarket: {}\nOutcome: {}\nBest Ask: {}\nFloor: {:.4}",
        reason,
        order.market_slug,
        order.outcome_label,
        best_ask.map_or_else(|| "N/A".to_string(), |value| format!("{value:.4}")),
        order.best_ask_floor_price.unwrap_or(0.0)
    )
}

fn build_execution_floor_waiting_notification_message(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
) -> String {
    let reason = match (best_ask, order.best_ask_floor_price) {
        (None, _) => {
            "Best ask verisi alinamadi. Kosullar duzelince order yeniden denenecek."
        }
        (Some(best_ask), Some(floor)) if best_ask < floor => {
            "Best ask floor seviyesinin altinda. Kosullar duzelince order yeniden denenecek."
        }
        _ => "Execution floor korumasi aktif. Kosullar duzelince order yeniden denenecek.",
    };
    format!(
        "Execution Floor Bekleme Modu\nSebep: {}\nMarket: {}\nOutcome: {}\nBest Ask: {}\nFloor: {:.4}",
        reason,
        order.market_slug,
        order.outcome_label,
        best_ask.map_or_else(|| "N/A".to_string(), |value| format!("{value:.4}")),
        order.best_ask_floor_price.unwrap_or(0.0)
    )
}

fn pair_lock_primary_candidate_outcome(candidate: &Value) -> &str {
    candidate
        .get("outcome_label")
        .and_then(Value::as_str)
        .unwrap_or("N/A")
}

fn pair_lock_primary_candidate_best_ask(candidate: &Value) -> Option<f64> {
    candidate.get("best_ask").and_then(value_as_f64)
}

fn pair_lock_primary_candidate_current_price(candidate: &Value) -> Option<f64> {
    candidate.get("current_price").and_then(value_as_f64)
}

fn pair_lock_primary_candidate_quote_source(candidate: &Value) -> &str {
    candidate
        .get("quote_source_detail")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn pair_lock_primary_candidate_reason_summary(candidate: &Value) -> String {
    format!(
        "{} -> {}",
        pair_lock_primary_candidate_outcome(candidate),
        candidate
            .get("reason_code")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    )
}

fn pair_lock_primary_secondary_reason_line(secondary_candidate: Option<&Value>) -> String {
    secondary_candidate
        .map(|candidate| format!("\nDiger Aday: {}", pair_lock_primary_candidate_reason_summary(candidate)))
        .unwrap_or_default()
}

fn build_pair_lock_primary_trigger_guard_notification_message(
    market_slug: &str,
    candidate: &Value,
    guard_trigger_price: Option<f64>,
    waiting: bool,
    secondary_candidate: Option<&Value>,
) -> String {
    let state_line = if waiting {
        "Tetik Fiyat Korumasi Bekleme Modu\nDurum: Kosullar duzelince ilk bacak secimi yeniden denenecek."
    } else {
        "Tetik Fiyat Korumasi Engelledi\nDurum: Ilk bacak secimi tetik fiyat korumasina takildi."
    };
    format!(
        "{state_line}\nMarket: {market_slug}\nOutcome: {}\nReferans ({}): {}\nGuard: {}{}",
        pair_lock_primary_candidate_outcome(candidate),
        pair_lock_primary_candidate_quote_source(candidate),
        pair_lock_primary_candidate_current_price(candidate)
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        guard_trigger_price
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        pair_lock_primary_secondary_reason_line(secondary_candidate),
    )
}

fn build_pair_lock_primary_execution_floor_notification_message(
    market_slug: &str,
    candidate: &Value,
    best_ask_floor_price: Option<f64>,
    waiting: bool,
    secondary_candidate: Option<&Value>,
) -> String {
    let reason = match pair_lock_primary_candidate_best_ask(candidate) {
        None => "Best ask verisi alinamadi.",
        Some(best_ask) if best_ask_floor_price.is_some_and(|floor| best_ask < floor) => {
            "Best ask floor seviyesinin altinda."
        }
        _ => "Execution floor korumasi aktif.",
    };
    let state_line = if waiting {
        "Execution Floor Bekleme Modu\nDurum: Kosullar duzelince ilk bacak secimi yeniden denenecek."
    } else {
        "Execution Floor Engelledi\nDurum: Ilk bacak secimi execution floor korumasina takildi."
    };
    format!(
        "{state_line}\nSebep: {reason}\nMarket: {market_slug}\nOutcome: {}\nBest Ask: {}\nFloor: {}{}",
        pair_lock_primary_candidate_outcome(candidate),
        pair_lock_primary_candidate_best_ask(candidate)
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        best_ask_floor_price
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        pair_lock_primary_secondary_reason_line(secondary_candidate),
    )
}

fn build_pair_lock_primary_max_price_notification_message(
    market_slug: &str,
    candidate: &Value,
    max_price: Option<f64>,
    waiting: bool,
    secondary_candidate: Option<&Value>,
) -> String {
    let reference_price = candidate
        .pointer("/max_price_guard/details/reference_price")
        .and_then(value_as_f64)
        .or_else(|| pair_lock_primary_candidate_best_ask(candidate));
    let reference_source = candidate
        .pointer("/max_price_guard/details/reference_price_source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let state_line = if waiting {
        "Max Fiyat Korumasi Bekleme Modu\nDurum: Kosullar duzelince ilk bacak secimi yeniden denenecek."
    } else {
        "Max Fiyat Korumasi Engelledi\nDurum: Ilk bacak secimi max fiyat korumasina takildi."
    };
    format!(
        "{state_line}\nMarket: {market_slug}\nOutcome: {}\nGuncel: {}\nReferans ({}): {}\nMax: {}{}",
        pair_lock_primary_candidate_outcome(candidate),
        pair_lock_primary_candidate_current_price(candidate)
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        reference_source,
        reference_price
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        max_price
            .map(|value| format!("{value:.4}"))
            .unwrap_or_else(|| "N/A".to_string()),
        pair_lock_primary_secondary_reason_line(secondary_candidate),
    )
}

fn build_pair_lock_primary_price_to_beat_notification_message(
    market_slug: &str,
    candidate: &Value,
    waiting: bool,
    secondary_candidate: Option<&Value>,
) -> String {
    let ptb_guard = candidate.get("price_to_beat_guard").unwrap_or(&Value::Null);
    let reason_detail = ptb_guard
        .get("reason_detail")
        .and_then(Value::as_str)
        .unwrap_or("Price to Beat guard bekleme durumunda.");
    let current_price_source = ptb_guard
        .get("current_price_source")
        .and_then(Value::as_str)
        .unwrap_or("N/A");
    let state_line = if waiting {
        "Price to Beat Korumasi Bekleme Modu\nDurum: Kosullar duzelince ilk bacak secimi yeniden denenecek."
    } else {
        "Price to Beat Korumasi Engelledi\nDurum: Ilk bacak secimi Price to Beat korumasina takildi."
    };
    format!(
        "{state_line}\nSebep: {reason_detail}\nMarket: {market_slug}\nOutcome: {}\nPrice to Beat: {}\nCurrent ({}): {}\nYonsel Fark: {}\nLimit: {}{}",
        pair_lock_primary_candidate_outcome(candidate),
        ptb_guard
            .get("price_to_beat")
            .and_then(value_as_f64)
            .map(|value| format!("{value:.8}"))
            .unwrap_or_else(|| "N/A".to_string()),
        current_price_source,
        ptb_guard
            .get("current_price")
            .and_then(value_as_f64)
            .map(|value| format!("{value:.8}"))
            .unwrap_or_else(|| "N/A".to_string()),
        ptb_guard
            .get("directional_gap")
            .and_then(value_as_f64)
            .map(|value| format!("{value:.8}"))
            .unwrap_or_else(|| "N/A".to_string()),
        ptb_guard
            .get("threshold_usd")
            .and_then(value_as_f64)
            .map(|value| format!("{value:.8} USD"))
            .unwrap_or_else(|| "N/A".to_string()),
        pair_lock_primary_secondary_reason_line(secondary_candidate),
    )
}

fn build_pair_lock_primary_guard_recovered_notification_message(
    market_slug: &str,
    scope: &str,
    previous_reason_code: &str,
) -> String {
    let title = match scope {
        "trigger_price" => "Tetik Fiyat Korumasi Gecti",
        "execution_floor" => "Execution Floor Korumasi Gecti",
        "max_price" => "Max Fiyat Korumasi Gecti",
        "price_to_beat" => "Price to Beat Korumasi Gecti",
        _ => "Pair Lock Ilk Bacak Korumasi Gecti",
    };
    format!(
        "{title}\nDurum: Kosullar yeniden uygun hale geldi.\nMarket: {market_slug}\nOnceki Sebep: {previous_reason_code}"
    )
}

#[cfg(test)]
fn build_order_not_filled_notification_message(
    order: &TradeBuilderOrder,
    reason_code: &str,
    reason_message: &str,
) -> String {
    build_order_not_filled_notification_message_with_guard(order, reason_code, reason_message, None)
}

fn build_order_not_filled_notification_message_with_guard(
    order: &TradeBuilderOrder,
    reason_code: &str,
    reason_message: &str,
    guard_summary: Option<&TradeBuilderNoFillReasonSummary>,
) -> String {
    let title = if reason_code == "sl_submitted_but_unfilled_before_market_close" {
        "Stop Loss Doldurulamadi"
    } else {
        "Emir Icra Edilemedi"
    };
    let mut message = format!(
        "{title}\nSebep Kodu: {}\nSebep: {}\nMarket: {}\nOutcome: {}\nSide: {}",
        reason_code, reason_message, order.market_slug, order.outcome_label, order.side
    );
    if let Some(summary) = guard_summary {
        message.push_str(&build_no_fill_guard_summary_block(summary));
    } else if matches!(
        reason_code,
        "outside_cycle_window" | "stale_market_cycle" | "ttl_expired"
    ) {
        message.push_str(build_no_fill_missing_guard_block());
    }
    message
}
