use super::PriceToBeatGuardEvaluation;
use crate::trade_flow::guards::cex_microstructure::{
    ensure_cex_microstructure_started, get_cex_venue_delta_snapshot, CexVenue,
    CexVenueDeltaSnapshot,
};
use anyhow::Result;
use chrono::Duration as ChronoDuration;
use serde_json::{json, Value};

const DEFAULT_MODE: &str = "bybit_plus_one";
const OKX_MODE: &str = "okx_plus_one";
const GATE_MODE: &str = "gate_plus_one";
const BINANCE_COINBASE_MODE: &str = "binance_coinbase";
const DEFAULT_MAX_BOOK_STALE_MS: i64 = 2_500;
const DEFAULT_MIN_MOVE_USD: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexDirectionGuardConfig {
    pub(crate) enabled: bool,
    pub(crate) mode: String,
    pub(crate) max_book_stale_ms: i64,
    pub(crate) min_move_usd: f64,
    pub(crate) fail_closed: bool,
}

impl CexDirectionGuardConfig {
    pub(crate) fn from_node(node: &crate::TradeFlowNode) -> Self {
        Self {
            enabled: crate::node_config_bool(node, "cexDirectionGuardEnabled").unwrap_or(false),
            mode: crate::node_config_string(node, "cexDirectionGuardMode")
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| {
                    matches!(
                        value.as_str(),
                        DEFAULT_MODE | OKX_MODE | GATE_MODE | BINANCE_COINBASE_MODE
                    )
                })
                .unwrap_or_else(|| DEFAULT_MODE.to_string()),
            max_book_stale_ms: cfg_i64(
                node,
                "cexDirectionGuardMaxStaleMs",
                DEFAULT_MAX_BOOK_STALE_MS,
            ),
            min_move_usd: cfg_f64(node, "cexDirectionGuardMinMoveUsd", DEFAULT_MIN_MOVE_USD),
            fail_closed: crate::node_config_bool(node, "cexDirectionGuardFailClosed")
                .unwrap_or(true),
        }
    }

    pub(crate) fn consensus_stop_loss_defaults() -> Self {
        Self {
            enabled: true,
            mode: DEFAULT_MODE.to_string(),
            max_book_stale_ms: DEFAULT_MAX_BOOK_STALE_MS,
            min_move_usd: DEFAULT_MIN_MOVE_USD,
            fail_closed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexDirectionGuardEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason_code: &'static str,
    pub(crate) reason_detail: Option<String>,
    pub(crate) consensus_side: Option<&'static str>,
    pub(crate) bybit_delta_usd: Option<f64>,
    pub(crate) value: Value,
}

pub(crate) fn apply_action_place_order_cex_direction_guard(
    node: &crate::TradeFlowNode,
    market_slug: &str,
    outcome_label: &str,
    evaluation: &mut PriceToBeatGuardEvaluation,
) {
    let config = CexDirectionGuardConfig::from_node(node);
    if !config.enabled || !evaluation.passed {
        return;
    }
    let guard = evaluate_cex_direction_guard(market_slug, outcome_label, &config);
    evaluation.cex_direction_guard = Some(guard.value.clone());
    if !guard.passed {
        evaluation.passed = false;
        evaluation.reason_code = guard.reason_code.to_string();
        evaluation.reason_detail = guard.reason_detail;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn maybe_block_action_place_order_cex_direction_guard_only(
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
    let config = CexDirectionGuardConfig::from_node(node);
    if !config.enabled {
        return Ok(None);
    }
    let guard = evaluate_cex_direction_guard(market_slug, outcome_label, &config);
    crate::set_flow_context(context, "cexDirectionGuard", guard.value.clone());
    crate::set_flow_context(
        context,
        "priceToBeatGuard",
        json!({
            "passed": guard.passed,
            "reason_code": guard.reason_code,
            "reason_detail": guard.reason_detail,
            "market_slug": market_slug,
            "outcome_label": outcome_label,
            "threshold_mode": "cex_direction_guard",
            "cex_direction_guard": guard.value.clone(),
        }),
    );
    if guard.passed {
        return Ok(None);
    }
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pre_order_cex_direction_guard_blocked",
        &json!({
            "node_key": node.key,
            "node_type": node.node_type,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "cex_direction_guard": guard.value.clone(),
        }),
    )
    .await?;
    let retry_on_guard_block =
        crate::node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true);
    if retry_on_guard_block {
        let retry_delay_ms = crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS;
        return Ok(Some(crate::TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "blocked": true,
                "reason": "cex_direction_guard_blocked",
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "side": side,
                "execution_mode": execution_mode,
                "retrying": true,
                "retry_delay_ms": retry_delay_ms,
                "cex_direction_guard": guard.value.clone(),
            }),
            routes: vec![],
            repeat_at: Some(crate::Utc::now() + ChronoDuration::milliseconds(retry_delay_ms)),
            repeat_idempotency_key: None,
        }));
    }
    Ok(Some(crate::TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "reason": "cex_direction_guard_blocked",
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "cex_direction_guard": guard.value.clone(),
        }),
        routes: vec![crate::TradeFlowRouteDecision {
            edge_type: "on_error".to_string(),
            available_at: crate::Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

pub(crate) fn evaluate_cex_direction_guard(
    market_slug: &str,
    outcome_label: &str,
    config: &CexDirectionGuardConfig,
) -> CexDirectionGuardEvaluation {
    if !config.enabled {
        return pass(
            "cex_direction_guard_disabled",
            None,
            base_value(market_slug, outcome_label, config).with_consensus(
                None,
                None,
                Vec::new(),
                "unknown",
            ),
            None,
            None,
        );
    }
    let Some(selected_side) = normalize_outcome_side(outcome_label) else {
        return unavailable_or_pass(
            config,
            "cex_direction_guard_unsupported_outcome",
            Some(format!("unsupported outcome_label={outcome_label}")),
            base_value(market_slug, outcome_label, config),
            None,
            None,
        );
    };
    let Some(scope) = crate::find_updown_scope_by_slug(market_slug) else {
        return unavailable_or_pass(
            config,
            "cex_direction_guard_unsupported_market",
            Some("market slug is not a supported updown scope".to_string()),
            base_value(market_slug, outcome_label, config),
            None,
            None,
        );
    };
    let Some(window_start) = crate::MarketCycleId(market_slug.to_string()).start_time() else {
        return unavailable_or_pass(
            config,
            "cex_direction_guard_missing_window_start",
            None,
            base_value(market_slug, outcome_label, config),
            None,
            None,
        );
    };

    ensure_cex_microstructure_started(scope.asset);
    let window_start_ms = window_start.timestamp_millis();
    if config.mode.as_str() == BINANCE_COINBASE_MODE {
        let binance = load_delta(scope.asset, CexVenue::Binance, window_start_ms, config);
        let coinbase = load_delta(scope.asset, CexVenue::Coinbase, window_start_ms, config);
        let value = base_value(market_slug, outcome_label, config)
            .with_scope(scope.asset, scope.timeframe, window_start_ms)
            .with_delta("binance", &binance)
            .with_delta("coinbase", &coinbase);

        let Some(binance_delta) = binance.snapshot.as_ref() else {
            return pass(
                "cex_direction_guard_unconfirmed",
                Some(format!(
                    "mode={BINANCE_COINBASE_MODE}; venue=binance unavailable: {}",
                    binance
                        .error
                        .clone()
                        .unwrap_or_else(|| "missing".to_string())
                )),
                value.with_consensus(None, None, Vec::new(), selected_side),
                None,
                None,
            );
        };
        let Some(coinbase_delta) = coinbase.snapshot.as_ref() else {
            return pass(
                "cex_direction_guard_unconfirmed",
                Some(format!(
                    "mode={BINANCE_COINBASE_MODE}; venue=coinbase unavailable: {}",
                    coinbase
                        .error
                        .clone()
                        .unwrap_or_else(|| "missing".to_string())
                )),
                value.with_consensus(None, None, Vec::new(), selected_side),
                None,
                Some(binance_delta.delta_usd),
            );
        };
        let Some(binance_side) = binance_delta.side else {
            return pass(
                "cex_direction_guard_neutral",
                Some(format!("{BINANCE_COINBASE_MODE}; venue=binance")),
                value.with_consensus(None, None, Vec::new(), selected_side),
                None,
                Some(binance_delta.delta_usd),
            );
        };
        let Some(coinbase_side) = coinbase_delta.side else {
            return pass(
                "cex_direction_guard_neutral",
                Some(format!("{BINANCE_COINBASE_MODE}; venue=coinbase")),
                value.with_consensus(None, None, Vec::new(), selected_side),
                None,
                Some(binance_delta.delta_usd),
            );
        };
        if binance_side != coinbase_side {
            return pass(
                "cex_direction_guard_unconfirmed",
                Some(format!(
                    "mode={BINANCE_COINBASE_MODE}; binance_side={binance_side}; coinbase_side={coinbase_side}"
                )),
                value.with_consensus(None, None, Vec::new(), selected_side),
                None,
                Some(binance_delta.delta_usd),
            );
        }

        let consensus_side = binance_side;
        let value = value.with_consensus(
            Some(consensus_side),
            Some(consensus_side),
            vec!["binance", "coinbase"],
            selected_side,
        );
        if consensus_side == opposite_side(selected_side) {
            return block(
                "cex_direction_guard_opposite",
                Some(format!(
                    "selected_side={selected_side}; consensus_side={consensus_side}; mode={BINANCE_COINBASE_MODE}; binance_delta_usd={:.8}; coinbase_delta_usd={:.8}",
                    binance_delta.delta_usd,
                    coinbase_delta.delta_usd
                )),
                value,
                Some(consensus_side),
                Some(binance_delta.delta_usd),
            );
        }
        return pass(
            "cex_direction_guard_passed",
            None,
            value,
            Some(consensus_side),
            Some(binance_delta.delta_usd),
        );
    }

    let anchor_venue = match config.mode.as_str() {
        OKX_MODE => CexVenue::Okx,
        GATE_MODE => CexVenue::Gateio,
        _ => CexVenue::Bybit,
    };
    let anchor_name = anchor_venue.as_str();
    let anchor = load_delta(scope.asset, anchor_venue, window_start_ms, config);
    let binance = load_delta(scope.asset, CexVenue::Binance, window_start_ms, config);
    let coinbase = load_delta(scope.asset, CexVenue::Coinbase, window_start_ms, config);
    let value = base_value(market_slug, outcome_label, config)
        .with_scope(scope.asset, scope.timeframe, window_start_ms)
        .with_delta(anchor_name, &anchor)
        .with_delta("binance", &binance)
        .with_delta("coinbase", &coinbase);

    let Some(anchor_delta) = anchor.snapshot.as_ref() else {
        return unavailable_or_pass(
            config,
            "cex_direction_guard_unavailable",
            anchor.error.clone(),
            value,
            None,
            None,
        );
    };
    let Some(anchor_side) = anchor_delta.side else {
        return pass(
            "cex_direction_guard_neutral",
            Some(format!("anchor_venue={anchor_name}")),
            value.with_consensus(None, Some("neutral"), Vec::new(), selected_side),
            None,
            Some(anchor_delta.delta_usd),
        );
    };

    let confirming_venues = [("binance", &binance), ("coinbase", &coinbase)]
        .into_iter()
        .filter_map(|(venue, delta)| {
            delta
                .snapshot
                .as_ref()
                .and_then(|snapshot| (snapshot.side == Some(anchor_side)).then_some(venue))
        })
        .collect::<Vec<_>>();
    let consensus_side = (!confirming_venues.is_empty()).then_some(anchor_side);
    let value = value.with_consensus(
        consensus_side,
        Some(anchor_side),
        confirming_venues.clone(),
        selected_side,
    );
    let Some(consensus_side) = consensus_side else {
        return pass(
            "cex_direction_guard_unconfirmed",
            None,
            value,
            None,
            Some(anchor_delta.delta_usd),
        );
    };
    if consensus_side == opposite_side(selected_side) {
        return block(
            "cex_direction_guard_opposite",
            Some(format!(
                "selected_side={selected_side}; consensus_side={consensus_side}; anchor_venue={anchor_name}; anchor_delta_usd={:.8}; confirming_venues={}",
                anchor_delta.delta_usd,
                confirming_venues.join(",")
            )),
            value,
            Some(consensus_side),
            Some(anchor_delta.delta_usd),
        );
    }
    pass(
        "cex_direction_guard_passed",
        None,
        value,
        Some(consensus_side),
        Some(anchor_delta.delta_usd),
    )
}

#[derive(Debug, Clone)]
struct DeltaResult {
    snapshot: Option<CexVenueDeltaSnapshot>,
    error: Option<String>,
}

fn load_delta(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
    config: &CexDirectionGuardConfig,
) -> DeltaResult {
    match get_cex_venue_delta_snapshot(
        asset,
        venue,
        window_start_ms,
        config.min_move_usd,
        config.max_book_stale_ms,
    ) {
        Ok(snapshot) => DeltaResult {
            snapshot: Some(snapshot),
            error: None,
        },
        Err(err) => DeltaResult {
            snapshot: None,
            error: Some(err.to_string()),
        },
    }
}

#[derive(Debug, Clone)]
struct GuardValue(Value);

impl GuardValue {
    fn with_scope(mut self, asset: &str, timeframe: &str, window_start_ms: i64) -> Self {
        if let Some(obj) = self.0.as_object_mut() {
            obj.insert("asset".to_string(), json!(asset));
            obj.insert("timeframe".to_string(), json!(timeframe));
            obj.insert("window_start_ms".to_string(), json!(window_start_ms));
        }
        self
    }

    fn with_delta(mut self, key: &str, delta: &DeltaResult) -> Self {
        if let Some(obj) = self.0.as_object_mut() {
            let value = delta
                .snapshot
                .as_ref()
                .map(CexVenueDeltaSnapshot::to_value)
                .unwrap_or_else(|| json!({ "venue": key, "error": delta.error }));
            obj.entry("venue_deltas")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("venue_deltas object")
                .insert(key.to_string(), value);
        }
        self
    }

    fn with_consensus(
        mut self,
        consensus_side: Option<&str>,
        anchor_side: Option<&str>,
        confirming_venues: Vec<&str>,
        selected_side: &str,
    ) -> Value {
        if let Some(obj) = self.0.as_object_mut() {
            obj.insert("selected_side".to_string(), json!(selected_side));
            obj.insert(
                "opposing_side".to_string(),
                json!(opposite_side(selected_side)),
            );
            obj.insert("consensus_side".to_string(), json!(consensus_side));
            obj.insert("anchor_side".to_string(), json!(anchor_side));
            obj.insert("bybit_side".to_string(), json!(anchor_side));
            obj.insert("confirming_venues".to_string(), json!(confirming_venues));
        }
        self.0
    }
}

fn base_value(
    market_slug: &str,
    outcome_label: &str,
    config: &CexDirectionGuardConfig,
) -> GuardValue {
    GuardValue(json!({
        "enabled": config.enabled,
        "mode": config.mode,
        "market_slug": market_slug,
        "outcome_label": outcome_label,
        "max_book_stale_ms": config.max_book_stale_ms,
        "min_move_usd": config.min_move_usd,
        "fail_closed": config.fail_closed,
    }))
}

fn pass(
    reason_code: &'static str,
    reason_detail: Option<String>,
    value: Value,
    consensus_side: Option<&'static str>,
    bybit_delta_usd: Option<f64>,
) -> CexDirectionGuardEvaluation {
    finish(
        true,
        reason_code,
        reason_detail,
        value,
        consensus_side,
        bybit_delta_usd,
    )
}

fn block(
    reason_code: &'static str,
    reason_detail: Option<String>,
    value: Value,
    consensus_side: Option<&'static str>,
    bybit_delta_usd: Option<f64>,
) -> CexDirectionGuardEvaluation {
    finish(
        false,
        reason_code,
        reason_detail,
        value,
        consensus_side,
        bybit_delta_usd,
    )
}

fn unavailable_or_pass(
    config: &CexDirectionGuardConfig,
    reason_code: &'static str,
    reason_detail: Option<String>,
    value: GuardValue,
    consensus_side: Option<&'static str>,
    bybit_delta_usd: Option<f64>,
) -> CexDirectionGuardEvaluation {
    let value = value.with_consensus(consensus_side, None, Vec::new(), "unknown");
    finish(
        !config.fail_closed,
        reason_code,
        reason_detail,
        value,
        consensus_side,
        bybit_delta_usd,
    )
}

fn finish(
    passed: bool,
    reason_code: &'static str,
    reason_detail: Option<String>,
    mut value: Value,
    consensus_side: Option<&'static str>,
    bybit_delta_usd: Option<f64>,
) -> CexDirectionGuardEvaluation {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("passed".to_string(), json!(passed));
        obj.insert("reason_code".to_string(), json!(reason_code));
        obj.insert("reason_detail".to_string(), json!(reason_detail));
    }
    CexDirectionGuardEvaluation {
        passed,
        reason_code,
        reason_detail,
        consensus_side,
        bybit_delta_usd,
        value,
    }
}

fn normalize_outcome_side(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn opposite_side(side: &str) -> &'static str {
    if side == "up" {
        "down"
    } else {
        "up"
    }
}

fn cfg_f64(node: &crate::TradeFlowNode, key: &str, default: f64) -> f64 {
    crate::node_config_f64(node, key)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

fn cfg_i64(node: &crate::TradeFlowNode, key: &str, default: i64) -> i64 {
    crate::node_config_i64(node, key)
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, lock_cex_microstructure_test_state,
        seed_cex_book_test_sample, seed_cex_open_test_sample, CexBookSample,
    };
    use chrono::Utc;

    fn market_slug() -> (String, i64) {
        market_slug_for("btc")
    }

    fn market_slug_for(asset: &str) -> (String, i64) {
        let now = Utc::now().timestamp();
        let start = now - (now % 300);
        (format!("{asset}-updown-5m-{start}"), start * 1_000)
    }

    fn seed_book(venue: CexVenue, ts: i64, mid: f64) {
        seed_book_for("btc", venue, ts, mid);
    }

    fn seed_book_for(asset: &str, venue: CexVenue, ts: i64, mid: f64) {
        seed_cex_book_test_sample(CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: ts,
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        });
    }

    fn seed_open(venue: CexVenue, ts: i64, mid: f64) {
        seed_open_for("btc", venue, ts, mid);
    }

    fn seed_open_for(asset: &str, venue: CexVenue, ts: i64, mid: f64) {
        seed_cex_open_test_sample(CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: ts,
            bid: mid,
            ask: mid,
            bid_size: None,
            ask_size: None,
            source: "rest_open",
        });
    }

    fn config() -> CexDirectionGuardConfig {
        CexDirectionGuardConfig {
            enabled: true,
            mode: DEFAULT_MODE.to_string(),
            max_book_stale_ms: 10_000,
            min_move_usd: 1.0,
            fail_closed: true,
        }
    }

    fn okx_config() -> CexDirectionGuardConfig {
        CexDirectionGuardConfig {
            mode: OKX_MODE.to_string(),
            ..config()
        }
    }

    fn gate_config() -> CexDirectionGuardConfig {
        CexDirectionGuardConfig {
            mode: GATE_MODE.to_string(),
            ..config()
        }
    }

    fn binance_coinbase_config() -> CexDirectionGuardConfig {
        CexDirectionGuardConfig {
            mode: BINANCE_COINBASE_MODE.to_string(),
            ..config()
        }
    }

    #[test]
    fn bybit_plus_binance_opposite_blocks_entry() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Bybit, start_ms, 100.0);
        seed_book(CexVenue::Bybit, now_ms, 90.0);
        seed_open(CexVenue::Binance, start_ms, 200.0);
        seed_book(CexVenue::Binance, now_ms, 190.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &config());

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_opposite");
        assert_eq!(evaluation.consensus_side, Some("down"));
        assert_eq!(evaluation.bybit_delta_usd, Some(-10.0));
    }

    #[test]
    fn bybit_without_confirmation_passes() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Bybit, start_ms, 100.0);
        seed_book(CexVenue::Bybit, now_ms, 90.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &config());

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unconfirmed");
    }

    #[test]
    fn okx_plus_binance_opposite_blocks_entry() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Okx, start_ms, 100.0);
        seed_book(CexVenue::Okx, now_ms, 90.0);
        seed_open(CexVenue::Binance, start_ms, 200.0);
        seed_book(CexVenue::Binance, now_ms, 190.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &okx_config());

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_opposite");
        assert_eq!(evaluation.consensus_side, Some("down"));
        assert_eq!(evaluation.bybit_delta_usd, Some(-10.0));
        assert!(evaluation
            .reason_detail
            .unwrap()
            .contains("anchor_venue=okx"));
    }

    #[test]
    fn okx_without_confirmation_passes() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Okx, start_ms, 100.0);
        seed_book(CexVenue::Okx, now_ms, 90.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &okx_config());

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unconfirmed");
        assert_eq!(
            evaluation.value["venue_deltas"]["okx"]["venue"].as_str(),
            Some("okx")
        );
    }

    #[test]
    fn gate_plus_coinbase_opposite_blocks_entry() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug_for("sol");
        let now_ms = Utc::now().timestamp_millis();
        seed_open_for("sol", CexVenue::Gateio, start_ms, 100.0);
        seed_book_for("sol", CexVenue::Gateio, now_ms, 90.0);
        seed_open_for("sol", CexVenue::Coinbase, start_ms, 200.0);
        seed_book_for("sol", CexVenue::Coinbase, now_ms, 190.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &gate_config());

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_opposite");
        assert_eq!(evaluation.consensus_side, Some("down"));
        assert_eq!(evaluation.bybit_delta_usd, Some(-10.0));
        assert!(evaluation
            .reason_detail
            .unwrap()
            .contains("anchor_venue=gateio"));
        assert_eq!(
            evaluation.value["venue_deltas"]["gateio"]["venue"].as_str(),
            Some("gateio")
        );
    }

    #[test]
    fn gate_without_confirmation_passes() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug_for("sol");
        let now_ms = Utc::now().timestamp_millis();
        seed_open_for("sol", CexVenue::Gateio, start_ms, 100.0);
        seed_book_for("sol", CexVenue::Gateio, now_ms, 90.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &gate_config());

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unconfirmed");
    }

    #[test]
    fn binance_coinbase_opposite_blocks_entry() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Binance, start_ms, 100.0);
        seed_book(CexVenue::Binance, now_ms, 90.0);
        seed_open(CexVenue::Coinbase, start_ms, 200.0);
        seed_book(CexVenue::Coinbase, now_ms, 190.0);

        let evaluation =
            evaluate_cex_direction_guard(&market_slug, "Up", &binance_coinbase_config());

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_opposite");
        assert_eq!(evaluation.consensus_side, Some("down"));
        assert_eq!(evaluation.value["mode"].as_str(), Some("binance_coinbase"));
        assert!(evaluation.value["venue_deltas"]["bybit"].is_null());
        assert!(evaluation
            .reason_detail
            .unwrap()
            .contains("mode=binance_coinbase"));
    }

    #[test]
    fn binance_coinbase_single_missing_passes_unconfirmed() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Binance, start_ms, 100.0);
        seed_book(CexVenue::Binance, now_ms, 90.0);

        let evaluation =
            evaluate_cex_direction_guard(&market_slug, "Up", &binance_coinbase_config());

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unconfirmed");
        assert_eq!(evaluation.consensus_side, None);
    }

    #[test]
    fn binance_coinbase_disagreement_passes_unconfirmed() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_open(CexVenue::Binance, start_ms, 100.0);
        seed_book(CexVenue::Binance, now_ms, 90.0);
        seed_open(CexVenue::Coinbase, start_ms, 200.0);
        seed_book(CexVenue::Coinbase, now_ms, 210.0);

        let evaluation =
            evaluate_cex_direction_guard(&market_slug, "Up", &binance_coinbase_config());

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unconfirmed");
        assert_eq!(evaluation.consensus_side, None);
    }

    #[test]
    fn ws_only_window_books_do_not_create_consensus() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = market_slug();
        let now_ms = Utc::now().timestamp_millis();
        seed_book(CexVenue::Bybit, start_ms, 100.0);
        seed_book(CexVenue::Bybit, now_ms, 90.0);
        seed_book(CexVenue::Binance, start_ms, 200.0);
        seed_book(CexVenue::Binance, now_ms, 190.0);

        let evaluation = evaluate_cex_direction_guard(&market_slug, "Up", &config());

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_guard_unavailable");
        assert_eq!(evaluation.consensus_side, None);
        assert!(evaluation.value["venue_deltas"]["bybit"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("window open book missing"));
    }
}
