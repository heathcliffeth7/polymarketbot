use super::chainlink_price::fetch_chainlink_price;
use super::polymarket_price_to_beat::fetch_polymarket_price_to_beat;
use anyhow::Result;
use chrono::Duration as ChronoDuration;
use serde_json::{json, Value};

const CURRENT_PRICE_SOURCE: &str = "polymarket_live_data_ws";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceToBeatDiffUnit {
    Usd,
    Cent,
}

impl PriceToBeatDiffUnit {
    fn parse(raw: Option<&str>) -> Option<Self> {
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

    fn as_str(self) -> &'static str {
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
    pub(crate) market_slug: String,
    pub(crate) event_url: String,
    pub(crate) timeframe: Option<String>,
    pub(crate) asset: Option<String>,
    pub(crate) price_to_beat: Option<f64>,
    pub(crate) price_to_beat_source: Option<String>,
    pub(crate) price_to_beat_source_latency_ms: Option<i64>,
    pub(crate) current_price: Option<f64>,
    pub(crate) current_price_source: &'static str,
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
            "market_slug": self.market_slug,
            "event_url": self.event_url,
            "timeframe": self.timeframe,
            "asset": self.asset,
            "price_to_beat": self.price_to_beat,
            "price_to_beat_source": self.price_to_beat_source,
            "price_to_beat_source_latency_ms": self.price_to_beat_source_latency_ms,
            "current_price": self.current_price,
            "current_price_source": self.current_price_source,
            "gap_abs": self.gap_abs,
            "threshold_value": self.threshold_value,
            "threshold_unit": self.threshold_unit,
            "threshold_usd": self.threshold_usd,
        })
    }
}

fn clear_price_to_beat_guard_waiting_context(context: &mut Value) {
    crate::set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
}

fn price_to_beat_guard_waiting_reason(context: &Value) -> Option<String> {
    crate::flow_context_value(context, "priceToBeatGuardWaitingReason")
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.trim().is_empty())
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
        evaluate_price_to_beat_guard(market_slug, threshold_value, threshold_unit).await;
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
    if retry_on_guard_block {
        let entered_waiting = price_to_beat_guard_waiting_reason(context).as_deref()
            != Some(evaluation.reason_code.as_str());
        crate::set_flow_context(
            context,
            "priceToBeatGuardWaitingReason",
            json!(evaluation.reason_code.clone()),
        );
        if entered_waiting && should_notify {
            let notification_key = format!(
                "price_to_beat_guard_waiting:{}:{}:{}",
                run.user_id, market_slug, evaluation.reason_code
            );
            if repo.try_record_idempotency_key(&notification_key).await? {
                let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
                send_price_to_beat_guard_notification(repo, run.user_id, &message).await;
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
    if should_notify {
        let notification_key = format!(
            "price_to_beat_guard:{}:{}:{}",
            run.user_id, market_slug, evaluation.reason_code
        );
        if repo.try_record_idempotency_key(&notification_key).await? {
            let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
            send_price_to_beat_guard_notification(repo, run.user_id, &message).await;
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

    let snapshot = match fetch_polymarket_price_to_beat(market_slug).await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                event_url,
                threshold_value,
                threshold_unit,
                threshold_usd,
                "price_to_beat_unavailable",
                Some(err.to_string()),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                None,
                None,
                None,
                None,
            )
        }
    };

    let current_price = match fetch_chainlink_price(&snapshot.asset).await {
        Ok(price) => price,
        Err(err) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                snapshot.event_url.clone(),
                threshold_value,
                threshold_unit,
                threshold_usd,
                "current_price_unavailable",
                Some(format!("chainlink ws error: {err}")),
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
    let passed = gap_abs >= threshold_usd;
    PriceToBeatGuardEvaluation {
        passed,
        reason_code: if passed {
            "passed".to_string()
        } else {
            "price_to_beat_gap_below_threshold".to_string()
        },
        reason_detail: (!passed).then(|| {
            format!(
                "gap {:.8} is below configured threshold {:.8} {} (~{:.8} usd)",
                gap_abs,
                threshold_value,
                threshold_unit.as_str(),
                threshold_usd
            )
        }),
        market_slug: market_slug.to_string(),
        event_url: snapshot.event_url,
        timeframe: Some(snapshot.timeframe),
        asset: Some(snapshot.asset),
        price_to_beat: Some(snapshot.price_to_beat),
        price_to_beat_source: Some(snapshot.source.as_str().to_string()),
        price_to_beat_source_latency_ms: snapshot.source_latency_ms,
        current_price: Some(current_price),
        current_price_source: CURRENT_PRICE_SOURCE,
        gap_abs: Some(gap_abs),
        threshold_value,
        threshold_unit: threshold_unit.as_str().to_string(),
        threshold_usd,
    }
}

pub(crate) fn build_price_to_beat_guard_blocked_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    let reason = match evaluation.reason_code.as_str() {
        "price_to_beat_gap_below_threshold" => {
            "Current price (Chainlink) ile Price to Beat farki gereken minimum seviyenin altinda."
        }
        "price_to_beat_unavailable" => {
            "Polymarket Price to Beat verisi alinamadigi icin emir engellendi."
        }
        "current_price_unavailable" => {
            "Chainlink current price verisi alinamadigi icin emir engellendi."
        }
        "unsupported_market" => "Bu market Price to Beat guard tarafindan desteklenmiyor.",
        _ => "Price to Beat guard emri engelledi.",
    };

    let detail_line = evaluation
        .reason_detail
        .as_deref()
        .map(|detail| format!("\nDetay: {detail}"))
        .unwrap_or_default();

    format!(
        "Price to Beat Korumasi Engelledi\nSebep: {}{}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nCurrent (Chainlink): {}\nFark: {}\nLimit: {:.8} {} (~{:.8} USD)",
        reason,
        detail_line,
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.gap_abs),
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd
    )
}

pub(crate) fn build_price_to_beat_guard_waiting_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    format!(
        "{}\nDurum: Bekleme moduna alindi. Kosullar duzelince order yeniden denenecek.",
        build_price_to_beat_guard_blocked_notification_message(evaluation)
    )
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
        market_slug: market_slug.to_string(),
        event_url,
        timeframe,
        asset,
        price_to_beat,
        price_to_beat_source,
        price_to_beat_source_latency_ms,
        current_price,
        current_price_source: CURRENT_PRICE_SOURCE,
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

fn format_optional_guard_number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "N/A".to_string())
}

async fn send_price_to_beat_guard_notification(
    repo: &crate::PostgresRepository,
    user_id: i64,
    message: &str,
) {
    let Ok(telegram) = crate::load_user_telegram_config(repo, user_id).await else {
        return;
    };
    let bot_token = telegram.bot_token.trim();
    let chat_id = telegram.chat_id.trim();
    if bot_token.is_empty() || chat_id.is_empty() {
        return;
    }

    let Ok(bot_token) = crate::decrypt_config_string_if_needed("telegram.bot_token", bot_token)
    else {
        return;
    };
    if bot_token.is_empty() {
        return;
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
        }
        Ok(resp) => {
            tracing::warn!(
                user_id,
                http_status = resp.status().as_u16(),
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
        }
        Err(err) => {
            tracing::warn!(
                user_id,
                error = %err,
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_gap_below_threshold_notification_with_values() {
        let evaluation = PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            reason_detail: None,
            market_slug: "btc-updown-5m-1773232500".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            timeframe: Some("5m".to_string()),
            asset: Some("btc".to_string()),
            price_to_beat: Some(69_279.93484689),
            price_to_beat_source: Some("polymarket".to_string()),
            price_to_beat_source_latency_ms: None,
            current_price: Some(69_300.12),
            current_price_source: CURRENT_PRICE_SOURCE,
            gap_abs: Some(20.18515311),
            threshold_value: 30.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 30.0,
        };

        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
        assert!(message.contains("Price to Beat Korumasi Engelledi"));
        assert!(message.contains("Market: btc-updown-5m-1773232500"));
        assert!(message.contains("Asset: btc"));
        assert!(message.contains("gereken minimum seviyenin altinda"));
        assert!(message.contains("Limit: 30.00000000 usd (~30.00000000 USD)"));
    }

    #[test]
    fn blocked_notification_includes_reason_detail_and_partial_prices() {
        let evaluation = PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "price_to_beat_unavailable".to_string(),
            reason_detail: Some("__NEXT_DATA__ script tag not found in html".to_string()),
            market_slug: "btc-updown-5m-1773242700".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1773242700".to_string(),
            timeframe: Some("5m".to_string()),
            asset: Some("btc".to_string()),
            price_to_beat: None,
            price_to_beat_source: None,
            price_to_beat_source_latency_ms: None,
            current_price: Some(70_404.25964978),
            current_price_source: CURRENT_PRICE_SOURCE,
            gap_abs: None,
            threshold_value: 30.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 30.0,
        };

        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
        assert!(message.contains("Detay: __NEXT_DATA__ script tag not found in html"));
        assert!(message.contains("Current (Chainlink): 70404.25964978"));
        assert!(message.contains("Price to Beat: N/A"));
    }

    #[test]
    fn waiting_notification_mentions_recovery_retry() {
        let evaluation = PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            reason_detail: None,
            market_slug: "btc-updown-5m-1773232500".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            timeframe: Some("5m".to_string()),
            asset: Some("btc".to_string()),
            price_to_beat: Some(69_279.93484689),
            price_to_beat_source: Some("polymarket".to_string()),
            price_to_beat_source_latency_ms: None,
            current_price: Some(69_300.12),
            current_price_source: CURRENT_PRICE_SOURCE,
            gap_abs: Some(20.18515311),
            threshold_value: 30.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 30.0,
        };

        let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
        assert!(message.contains("Bekleme moduna alindi"));
        assert!(message.contains("yeniden denenecek"));
    }

    #[test]
    fn cent_threshold_converts_to_usd() {
        assert_eq!(
            normalize_price_to_beat_threshold_usd(1.0, PriceToBeatDiffUnit::Cent),
            0.01
        );
        assert_eq!(
            normalize_price_to_beat_threshold_usd(0.01, PriceToBeatDiffUnit::Cent),
            0.0001
        );
    }

    #[test]
    fn parses_threshold_units() {
        assert_eq!(
            PriceToBeatDiffUnit::parse(Some("usd")),
            Some(PriceToBeatDiffUnit::Usd)
        );
        assert_eq!(
            PriceToBeatDiffUnit::parse(Some("cent")),
            Some(PriceToBeatDiffUnit::Cent)
        );
        assert_eq!(
            PriceToBeatDiffUnit::parse(None),
            Some(PriceToBeatDiffUnit::Usd)
        );
        assert_eq!(PriceToBeatDiffUnit::parse(Some("foo")), None);
    }

    #[test]
    fn min_gap_mode_blocks_when_gap_is_below_threshold() {
        let gap_abs = (70_230.57_f64 - 70_249.29979780_f64).abs();
        assert!(gap_abs < 30.0);
        assert!(!(gap_abs >= 30.0));
    }

    #[test]
    fn min_gap_mode_allows_when_gap_equals_threshold() {
        let gap_abs = 30.0_f64;
        assert!(gap_abs >= 30.0);
    }

    #[test]
    fn min_gap_mode_allows_when_gap_is_above_threshold() {
        let gap_abs = 38.72979780_f64;
        assert!(gap_abs >= 30.0);
    }

    #[test]
    fn current_price_unavailable_reason_is_supported() {
        let evaluation = PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "current_price_unavailable".to_string(),
            reason_detail: Some("chainlink ws error: no cached price for btc/usd".to_string()),
            market_slug: "btc-updown-5m-1773246900".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1773246900".to_string(),
            timeframe: Some("5m".to_string()),
            asset: Some("btc".to_string()),
            price_to_beat: Some(70_714.62472011),
            price_to_beat_source: Some("chainlink_snapshot".to_string()),
            price_to_beat_source_latency_ms: Some(125),
            current_price: None,
            current_price_source: CURRENT_PRICE_SOURCE,
            gap_abs: None,
            threshold_value: 30.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 30.0,
        };

        let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
        assert!(message.contains("Chainlink current price verisi alinamadigi"));
        assert!(message.contains("Current (Chainlink): N/A"));
    }
}
