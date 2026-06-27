use super::*;
use crate::trade_flow::guards::cex_microstructure::{CexBookSample, CexVenue};
use crate::trade_flow::guards::price_to_beat::entry_current_hybrid::{
    CexEntryConsensusConfig, CexEntryConsensusModeConfig,
};
use serde_json::Value;

const HYPE_MARKET_5M: &str = "hype-updown-5m-1774013100";

fn seed_entry_state(market_slug: &str, asset: &str, ptb: f64, chainlink_current: f64) {
    let now_ms = chrono::Utc::now().timestamp_millis();
    crate::trade_flow::guards::cex_microstructure::clear_cex_microstructure_test_state();
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        asset,
        &[(now_ms, chainlink_current)],
    )
    .expect("seed chainlink current");
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            market_slug,
            asset,
            "5m",
            ptb,
            None,
        )
    );
}

fn seed_mid(venue: CexVenue, asset: &str, mid: f64) {
    crate::trade_flow::guards::cex_microstructure::seed_cex_book_test_sample(CexBookSample {
        venue,
        asset: asset.to_string(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        bid: mid - 0.01,
        ask: mid + 0.01,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: "ticker",
    });
}

fn entry_current_candidate<'a>(debug: &'a Value, source: &str) -> &'a Value {
    debug
        .get("entry_current_source_evaluations")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("source").and_then(Value::as_str) == Some(source))
        })
        .expect("entry current candidate")
}

async fn evaluate_asset_auto(market_slug: &str, outcome_label: &str) -> PriceToBeatGuardEvaluation {
    super::evaluate_price_to_beat_guard_with_current_source(
        market_slug,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        outcome_label,
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusConfig::current_price(CexEntryConsensusModeConfig::parse(Some(
            "asset_auto_plus_one_or_clean_pair",
        ))),
        None,
    )
    .await
}

#[tokio::test]
async fn asset_auto_current_price_uses_okx_for_btc() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_mid(CexVenue::Okx, "btc", 100.05);
    seed_mid(CexVenue::Binance, "btc", 100.03);

    let evaluation = evaluate_asset_auto(BTC_MARKET_5M, "Up").await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = evaluation.to_value();
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug
            .get("cex_entry_consensus_mode")
            .and_then(Value::as_str),
        Some("asset_auto_plus_one_or_clean_pair")
    );
    assert_eq!(
        cex_debug.get("anchor_venue").and_then(Value::as_str),
        Some("okx")
    );
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("okx_plus_one")
    );
}

#[tokio::test]
async fn asset_auto_current_price_uses_gateio_for_sol_without_okx() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(SOL_MARKET_5M, "sol", 100.0, 99.0);
    seed_mid(CexVenue::Gateio, "sol", 100.05);
    seed_mid(CexVenue::Binance, "sol", 100.03);

    let evaluation = evaluate_asset_auto(SOL_MARKET_5M, "Up").await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = evaluation.to_value();
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("anchor_venue").and_then(Value::as_str),
        Some("gateio")
    );
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("gate_plus_one")
    );
}

#[tokio::test]
async fn asset_auto_current_price_uses_hyperliquid_coinbase_for_hype() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(HYPE_MARKET_5M, "hype", 100.0, 99.0);
    seed_mid(CexVenue::Hyperliquid, "hype", 100.05);
    seed_mid(CexVenue::Coinbase, "hype", 100.03);

    let evaluation = evaluate_asset_auto(HYPE_MARKET_5M, "Up").await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = evaluation.to_value();
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("anchor_venue").and_then(Value::as_str),
        Some("hyperliquid")
    );
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("hyperliquid_plus_one")
    );
    assert_eq!(
        cex_debug
            .get("binance")
            .and_then(|value| value.get("skip_reason"))
            .and_then(Value::as_str),
        Some("not_required_for_asset_auto")
    );
}

#[tokio::test]
async fn explicit_okx_mode_still_blocks_sol_when_okx_missing() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(SOL_MARKET_5M, "sol", 100.0, 99.0);
    seed_mid(CexVenue::Gateio, "sol", 100.05);
    seed_mid(CexVenue::Binance, "sol", 100.03);

    let evaluation = super::evaluate_price_to_beat_guard_with_current_source(
        SOL_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusConfig::current_price(CexEntryConsensusModeConfig::parse(Some(
            "okx_plus_one_or_clean_pair",
        ))),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = evaluation.to_value();
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_okx_unavailable")
    );
}
