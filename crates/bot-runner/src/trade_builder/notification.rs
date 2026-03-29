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

async fn send_trade_builder_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    notification_type: &str,
    message: &str,
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

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let result = TELEGRAM_HTTP_CLIENT
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message.as_str(),
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            if let Err(err) = repo
                .append_trade_builder_order_event(
                    order.id,
                    "notification_sent",
                    &json!({
                        "notification_type": notification_type,
                        "message": message.as_str(),
                        "chat_id": chat_id,
                        "flow_identity": flow_identity.as_ref().map(|(_, payload)| payload.clone()),
                    }),
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
        }
        Ok(resp) => {
            warn!(
                builder_order_id = order.id,
                notification_type,
                http_status = resp.status().as_u16(),
                "TRADE_BUILDER_NOTIFICATION_FAILED"
            );
            false
        }
        Err(err) => {
            warn!(
                builder_order_id = order.id,
                notification_type,
                error = %err,
                "TRADE_BUILDER_NOTIFICATION_FAILED"
            );
            false
        }
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
    let message = build_order_not_filled_notification_message(order, reason_code, reason_message);
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

    Some((
        notification_type,
        if order.side == "buy" && order.parent_order_id.is_none() {
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
        },
    ))
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
) -> String {
    format!(
        "Max Fiyat Korumasi Bekleme Modu
Sebep: Referans fiyat max fiyat limitini asiyor. Kosullar duzelince order yeniden denenecek.
Market: {}
Outcome: {}
Guncel: {:.4}
Referans ({}): {:.4}
Max: {:.4}",
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

fn build_order_not_filled_notification_message(
    order: &TradeBuilderOrder,
    reason_code: &str,
    reason_message: &str,
) -> String {
    format!(
        "Emir Icra Edilemedi\nSebep Kodu: {}\nSebep: {}\nMarket: {}\nOutcome: {}\nSide: {}",
        reason_code, reason_message, order.market_slug, order.outcome_label, order.side
    )
}
