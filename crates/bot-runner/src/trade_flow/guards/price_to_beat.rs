use super::chainlink_price::get_chainlink_price_cached;
use super::polymarket_price_to_beat::{
    get_price_to_beat_cached, try_price_to_beat_cached_or_spawn, PriceToBeatLookup,
    PriceToBeatSource,
};
use anyhow::Result;
use chrono::Duration as ChronoDuration;
use serde_json::{json, Value};

mod current_price;
mod notification;
#[cfg(test)]
mod tests;

#[cfg(test)]
use self::current_price::{
    map_current_price_error, resolve_current_price_result, CURRENT_PRICE_SOURCE_COINBASE_FALLBACK,
};
use self::current_price::{resolve_price_to_beat_current_price, CURRENT_PRICE_SOURCE_CHAINLINK};
use self::notification::{
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
};

const PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY: &str = "lastGuardNotificationSeed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatDiffUnit {
    Usd,
    Cent,
}

impl PriceToBeatDiffUnit {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "usd" => Some(Self::Usd),
            "cent" => Some(Self::Cent),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Usd => "usd",
            Self::Cent => "cent",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PriceToBeatGuardEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason_code: String,
    pub(crate) reason_detail: Option<String>,
    pub(crate) normalized_outcome_label: Option<String>,
    pub(crate) direction: Option<String>,
    pub(crate) market_slug: String,
    pub(crate) event_url: String,
    pub(crate) timeframe: Option<String>,
    pub(crate) asset: Option<String>,
    pub(crate) price_to_beat: Option<f64>,
    pub(crate) price_to_beat_source: Option<String>,
    pub(crate) price_to_beat_source_latency_ms: Option<i64>,
    pub(crate) current_price: Option<f64>,
    pub(crate) current_price_source: &'static str,
    pub(crate) directional_gap: Option<f64>,
    pub(crate) gap_abs: Option<f64>,
    pub(crate) threshold_value: f64,
    pub(crate) threshold_unit: String,
    pub(crate) threshold_usd: f64,
}

impl PriceToBeatGuardEvaluation {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "passed": self.passed,
            "reason_code": self.reason_code,
            "reason_detail": self.reason_detail,
            "normalized_outcome_label": self.normalized_outcome_label,
            "direction": self.direction,
            "market_slug": self.market_slug,
            "event_url": self.event_url,
            "timeframe": self.timeframe,
            "asset": self.asset,
            "price_to_beat": self.price_to_beat,
            "price_to_beat_source": self.price_to_beat_source,
            "price_to_beat_source_latency_ms": self.price_to_beat_source_latency_ms,
            "current_price": self.current_price,
            "current_price_source": self.current_price_source,
            "directional_gap": self.directional_gap,
            "gap_abs": self.gap_abs,
            "threshold_value": self.threshold_value,
            "threshold_unit": self.threshold_unit,
            "threshold_usd": self.threshold_usd,
        })
    }
}

fn clear_price_to_beat_guard_waiting_context(context: &mut Value) {
    crate::set_flow_context(context, "priceToBeatGuardWaiting", Value::Null);
    // Legacy key cleanup
    crate::set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
}

fn price_to_beat_guard_notification_seed_reason(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<String> {
    let seed = crate::flow_context_value(context, PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY)?;
    let seed_node_key = seed.get("nodeKey")?.as_str()?;
    let seed_market_slug = seed.get("marketSlug")?.as_str()?;
    let seed_token_id = seed.get("tokenId")?.as_str()?;
    let reason = seed.get("reason")?.as_str()?;
    if seed_node_key != node_key || seed_market_slug != market_slug || seed_token_id != token_id {
        return None;
    }
    Some(reason.to_string())
}

fn set_price_to_beat_guard_notification_seed(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    reason: &str,
) {
    crate::set_flow_context(
        context,
        PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY,
        json!({
            "nodeKey": node_key,
            "marketSlug": market_slug,
            "tokenId": token_id,
            "reason": reason,
        }),
    );
}

pub(crate) fn take_price_to_beat_guard_notification_seed(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<String> {
    let reason =
        price_to_beat_guard_notification_seed_reason(context, node_key, market_slug, token_id)?;
    crate::set_flow_context(
        context,
        PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY,
        Value::Null,
    );
    Some(reason)
}

struct PriceToBeatGuardWaitingState {
    market_slug: String,
    reason_code: String,
}

fn price_to_beat_guard_waiting_state(context: &Value) -> Option<PriceToBeatGuardWaitingState> {
    let obj = crate::flow_context_value(context, "priceToBeatGuardWaiting")?;
    let market_slug = obj.get("marketSlug")?.as_str()?.to_string();
    let reason_code = obj.get("reasonCode")?.as_str()?.to_string();
    if market_slug.is_empty() || reason_code.is_empty() {
        return None;
    }
    Some(PriceToBeatGuardWaitingState {
        market_slug,
        reason_code,
    })
}

fn set_price_to_beat_guard_waiting_state(
    context: &mut Value,
    market_slug: &str,
    reason_code: &str,
) {
    crate::set_flow_context(
        context,
        "priceToBeatGuardWaiting",
        json!({ "marketSlug": market_slug, "reasonCode": reason_code }),
    );
    // Clear legacy key if present
    crate::set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DirectionalGapEvaluation {
    normalized_outcome_label: &'static str,
    direction: &'static str,
    directional_gap: f64,
    passed: bool,
}

fn normalize_outcome_direction(label: &str) -> Option<(&'static str, &'static str)> {
    match label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some(("yes", "up")),
        "no" | "down" | "short" | "bear" => Some(("no", "down")),
        _ => None,
    }
}

fn evaluate_directional_gap(
    current_price: f64,
    price_to_beat: f64,
    threshold_usd: f64,
    outcome_label: &str,
) -> Option<DirectionalGapEvaluation> {
    let (normalized_outcome_label, direction) = normalize_outcome_direction(outcome_label)?;
    let directional_gap = if direction == "up" {
        current_price - price_to_beat
    } else {
        price_to_beat - current_price
    };
    Some(DirectionalGapEvaluation {
        normalized_outcome_label,
        direction,
        directional_gap,
        passed: directional_gap >= threshold_usd,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct PriceToBeatTriggerGateResult {
    pub(crate) passed: bool,
    pub(crate) reason: &'static str,
    pub(crate) directional_gap: Option<f64>,
    pub(crate) price_to_beat: Option<f64>,
    pub(crate) current_price: Option<f64>,
    pub(crate) min_gap: f64,
    pub(crate) max_gap: Option<f64>,
}

impl PriceToBeatTriggerGateResult {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "passed": self.passed,
            "reason": self.reason,
            "directional_gap": self.directional_gap,
            "price_to_beat": self.price_to_beat,
            "current_price": self.current_price,
            "min_gap": self.min_gap,
            "max_gap": self.max_gap,
        })
    }
}

pub(crate) fn evaluate_price_to_beat_trigger_gate(
    market_slug: &str,
    outcome_label: &str,
    min_gap: f64,
    max_gap: Option<f64>,
    unit: PriceToBeatDiffUnit,
) -> PriceToBeatTriggerGateResult {
    let min_gap_usd = normalize_price_to_beat_threshold_usd(min_gap, unit);
    let max_gap_usd = max_gap.map(|value| normalize_price_to_beat_threshold_usd(value, unit));
    let Some(snapshot) = get_price_to_beat_cached(market_slug) else {
        return PriceToBeatTriggerGateResult {
            passed: false,
            reason: "price_to_beat_pending",
            directional_gap: None,
            price_to_beat: None,
            current_price: None,
            min_gap: min_gap_usd,
            max_gap: max_gap_usd,
        };
    };
    let current_price = match get_chainlink_price_cached(&snapshot.asset) {
        Ok(price) => price,
        Err(_) => {
            return PriceToBeatTriggerGateResult {
                passed: true,
                reason: "chainlink_unavailable",
                directional_gap: None,
                price_to_beat: Some(snapshot.price_to_beat),
                current_price: None,
                min_gap: min_gap_usd,
                max_gap: max_gap_usd,
            };
        }
    };
    let Some(directional) = evaluate_directional_gap(
        current_price,
        snapshot.price_to_beat,
        min_gap_usd,
        outcome_label,
    ) else {
        return PriceToBeatTriggerGateResult {
            passed: false,
            reason: "unsupported_outcome_label",
            directional_gap: None,
            price_to_beat: Some(snapshot.price_to_beat),
            current_price: Some(current_price),
            min_gap: min_gap_usd,
            max_gap: max_gap_usd,
        };
    };

    let passed = directional.directional_gap >= min_gap_usd
        && max_gap_usd
            .map(|threshold| directional.directional_gap <= threshold)
            .unwrap_or(true);
    let reason = if directional.directional_gap < min_gap_usd {
        "below_min_gap"
    } else if max_gap_usd
        .map(|threshold| directional.directional_gap > threshold)
        .unwrap_or(false)
    {
        "above_max_gap"
    } else {
        "in_range"
    };

    PriceToBeatTriggerGateResult {
        passed,
        reason,
        directional_gap: Some(directional.directional_gap),
        price_to_beat: Some(snapshot.price_to_beat),
        current_price: Some(current_price),
        min_gap: min_gap_usd,
        max_gap: max_gap_usd,
    }
}

pub(crate) async fn maybe_block_action_place_order_price_to_beat_guard(
    repo: &crate::PostgresRepository,
    run: &crate::TradeFlowRun,
    node: &crate::TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
) -> Result<Option<crate::TradeFlowNodeExecution>> {
    crate::set_flow_context(context, "priceToBeatGuard", Value::Null);

    if side != "buy" {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }

    let guard_enabled = crate::node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false);
    if !guard_enabled {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }
    let retry_on_guard_block =
        crate::node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true);

    let threshold_value = crate::node_config_f64(node, "priceToBeatMaxDiff").unwrap_or(0.0);
    anyhow::ensure!(
        threshold_value.is_finite() && threshold_value > 0.0,
        "action.place_order priceToBeatMaxDiff must be > 0 when guard is enabled"
    );
    let threshold_unit = PriceToBeatDiffUnit::parse(
        crate::node_config_string(node, "priceToBeatMaxDiffUnit").as_deref(),
    )
    .ok_or_else(|| {
        anyhow::anyhow!("action.place_order priceToBeatMaxDiffUnit must be one of: usd, cent")
    })?;

    let evaluation =
        evaluate_price_to_beat_guard(market_slug, threshold_value, threshold_unit, outcome_label)
            .await;
    let evaluation_output = evaluation.to_value();
    crate::set_flow_context(context, "priceToBeatGuard", evaluation_output.clone());
    if evaluation.passed {
        clear_price_to_beat_guard_waiting_context(context);
        return Ok(None);
    }

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pre_order_price_to_beat_blocked",
        &json!({
            "node_key": node.key,
            "node_type": node.node_type,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "price_to_beat_guard": evaluation_output.clone(),
        }),
    )
    .await?;

    let should_notify =
        crate::node_config_bool(node, "notifyOnPriceToBeatGapBlocked").unwrap_or(true);
    let candidate_reason =
        crate::build_guard_notification_reason("price_to_beat", &evaluation.reason_code);
    if retry_on_guard_block {
        let _entered_waiting = match price_to_beat_guard_waiting_state(context) {
            Some(prev) => {
                prev.market_slug != market_slug || prev.reason_code != evaluation.reason_code
            }
            None => true,
        };
        set_price_to_beat_guard_waiting_state(context, market_slug, &evaluation.reason_code);
        if should_notify
            && price_to_beat_guard_notification_seed_reason(
                context,
                &node.key,
                market_slug,
                token_id,
            )
            .as_deref()
                != Some(candidate_reason.as_str())
        {
            let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
            if send_price_to_beat_guard_notification(repo, run.user_id, &message).await {
                set_price_to_beat_guard_notification_seed(
                    context,
                    &node.key,
                    market_slug,
                    token_id,
                    &candidate_reason,
                );
            }
        }
        let repeat_at = crate::Utc::now()
            + ChronoDuration::milliseconds(crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS);
        return Ok(Some(crate::TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "blocked": true,
                "reason": "price_to_beat_guard_blocked",
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "side": side,
                "execution_mode": execution_mode,
                "retrying": true,
                "retry_delay_ms": crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS,
                "price_to_beat_guard": evaluation_output,
            }),
            routes: vec![],
            repeat_at: Some(repeat_at),
            repeat_idempotency_key: None,
        }));
    }
    if should_notify
        && price_to_beat_guard_notification_seed_reason(context, &node.key, market_slug, token_id)
            .as_deref()
            != Some(candidate_reason.as_str())
    {
        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
        if send_price_to_beat_guard_notification(repo, run.user_id, &message).await {
            set_price_to_beat_guard_notification_seed(
                context,
                &node.key,
                market_slug,
                token_id,
                &candidate_reason,
            );
        }
    }

    Ok(Some(crate::TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "reason": "price_to_beat_guard_blocked",
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "price_to_beat_guard": evaluation_output,
        }),
        routes: vec![crate::TradeFlowRouteDecision {
            edge_type: "on_error".to_string(),
            available_at: crate::Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

async fn evaluate_price_to_beat_guard(
    market_slug: &str,
    threshold_value: f64,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
) -> PriceToBeatGuardEvaluation {
    let threshold_usd = normalize_price_to_beat_threshold_usd(threshold_value, threshold_unit);
    let event_url = format!("https://polymarket.com/event/{market_slug}");
    let Some(scope) = crate::find_updown_scope_by_slug(market_slug) else {
        return blocked_price_to_beat_guard_evaluation(
            market_slug,
            event_url,
            threshold_value,
            threshold_unit,
            threshold_usd,
            "unsupported_market",
            Some("market slug is not a supported 5m/15m updown scope".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        );
    };
    if !matches!(scope.timeframe, "5m" | "15m") {
        return blocked_price_to_beat_guard_evaluation(
            market_slug,
            event_url,
            threshold_value,
            threshold_unit,
            threshold_usd,
            "unsupported_market",
            Some(format!("unsupported timeframe: {}", scope.timeframe)),
            Some(scope.timeframe.to_string()),
            Some(scope.asset.to_string()),
            None,
            None,
            None,
            None,
        );
    }

    let snapshot = match try_price_to_beat_cached_or_spawn(market_slug) {
        PriceToBeatLookup::Ready(snapshot) => snapshot,
        PriceToBeatLookup::Pending => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                event_url,
                threshold_value,
                threshold_unit,
                threshold_usd,
                "price_to_beat_pending",
                None,
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                None,
                None,
                None,
                None,
            );
        }
        PriceToBeatLookup::Unavailable(detail) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                event_url,
                threshold_value,
                threshold_unit,
                threshold_usd,
                "price_to_beat_unavailable",
                Some(detail),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                None,
                None,
                None,
                None,
            );
        }
    };

    let (current_price, current_price_source) = match resolve_price_to_beat_current_price(
        snapshot.source,
        market_slug,
        &snapshot.asset,
        snapshot.source_latency_ms,
    )
    .await
    {
        Ok(resolved) => resolved,
        Err((reason_code, reason_detail)) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                snapshot.event_url.clone(),
                threshold_value,
                threshold_unit,
                threshold_usd,
                reason_code,
                Some(reason_detail),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                Some(snapshot.price_to_beat),
                Some(snapshot.source.as_str().to_string()),
                snapshot.source_latency_ms,
                None,
            );
        }
    };

    let gap_abs = (current_price - snapshot.price_to_beat).abs();
    let directional = match evaluate_directional_gap(
        current_price,
        snapshot.price_to_beat,
        threshold_usd,
        outcome_label,
    ) {
        Some(directional) => directional,
        None => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                snapshot.event_url.clone(),
                threshold_value,
                threshold_unit,
                threshold_usd,
                "unsupported_outcome_label",
                Some(format!(
                    "outcome_label '{outcome_label}' is not a recognized direction"
                )),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                Some(snapshot.price_to_beat),
                Some(snapshot.source.as_str().to_string()),
                snapshot.source_latency_ms,
                Some(current_price),
            );
        }
    };
    let passed = directional.passed;
    PriceToBeatGuardEvaluation {
        passed,
        reason_code: if passed {
            "passed".to_string()
        } else {
            "price_to_beat_gap_below_threshold".to_string()
        },
        reason_detail: (!passed).then(|| {
            format!(
                "directional gap {:.8} (direction={}) is below threshold {:.8} {} (~{:.8} usd)",
                directional.directional_gap,
                directional.direction,
                threshold_value,
                threshold_unit.as_str(),
                threshold_usd
            )
        }),
        normalized_outcome_label: Some(directional.normalized_outcome_label.to_string()),
        direction: Some(directional.direction.to_string()),
        market_slug: market_slug.to_string(),
        event_url: snapshot.event_url,
        timeframe: Some(snapshot.timeframe),
        asset: Some(snapshot.asset),
        price_to_beat: Some(snapshot.price_to_beat),
        price_to_beat_source: Some(snapshot.source.as_str().to_string()),
        price_to_beat_source_latency_ms: snapshot.source_latency_ms,
        current_price: Some(current_price),
        current_price_source,
        directional_gap: Some(directional.directional_gap),
        gap_abs: Some(gap_abs),
        threshold_value,
        threshold_unit: threshold_unit.as_str().to_string(),
        threshold_usd,
    }
}

fn blocked_price_to_beat_guard_evaluation(
    market_slug: &str,
    event_url: String,
    threshold_value: f64,
    threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    reason_code: &str,
    reason_detail: Option<String>,
    timeframe: Option<String>,
    asset: Option<String>,
    price_to_beat: Option<f64>,
    price_to_beat_source: Option<String>,
    price_to_beat_source_latency_ms: Option<i64>,
    current_price: Option<f64>,
) -> PriceToBeatGuardEvaluation {
    PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: reason_code.to_string(),
        reason_detail,
        normalized_outcome_label: None,
        direction: None,
        market_slug: market_slug.to_string(),
        event_url,
        timeframe,
        asset,
        price_to_beat,
        price_to_beat_source,
        price_to_beat_source_latency_ms,
        current_price,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value,
        threshold_unit: threshold_unit.as_str().to_string(),
        threshold_usd,
    }
}

fn normalize_price_to_beat_threshold_usd(
    threshold_value: f64,
    threshold_unit: PriceToBeatDiffUnit,
) -> f64 {
    match threshold_unit {
        PriceToBeatDiffUnit::Usd => threshold_value,
        PriceToBeatDiffUnit::Cent => threshold_value / 100.0,
    }
}

async fn send_price_to_beat_guard_notification(
    repo: &crate::PostgresRepository,
    user_id: i64,
    message: &str,
) -> bool {
    let Ok(telegram) = crate::load_user_telegram_config(repo, user_id).await else {
        return false;
    };
    let bot_token = telegram.bot_token.trim();
    let chat_id = telegram.chat_id.trim();
    if bot_token.is_empty() || chat_id.is_empty() {
        return false;
    }

    let Ok(bot_token) = crate::decrypt_config_string_if_needed("telegram.bot_token", bot_token)
    else {
        return false;
    };
    if bot_token.is_empty() {
        return false;
    }

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let result = crate::TELEGRAM_HTTP_CLIENT
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": message,
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(user_id, "PRICE_TO_BEAT_GUARD_NOTIFICATION_SENT");
            true
        }
        Ok(resp) => {
            tracing::warn!(
                user_id,
                http_status = resp.status().as_u16(),
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
            false
        }
        Err(err) => {
            tracing::warn!(
                user_id,
                error = %err,
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
            false
        }
    }
}
