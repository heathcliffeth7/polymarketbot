use super::*;
use crate::trade_flow::guards::price_to_beat::entry_current_hybrid::{
    CexEntryConsensusBasis, CexEntryConsensusConfig, CexEntryConsensusModeConfig,
};

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

fn seed_current_mid(
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    asset: &str,
    mid: f64,
) {
    crate::trade_flow::guards::cex_microstructure::seed_cex_book_test_sample(
        crate::trade_flow::guards::cex_microstructure::CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "bookTicker",
        },
    );
}

fn seed_open_gap(
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    asset: &str,
    market_slug: &str,
    open_mid: f64,
    current_mid: f64,
) {
    let start_ms = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .expect("market start")
        .timestamp_millis();
    crate::trade_flow::guards::cex_microstructure::seed_cex_open_test_sample(
        crate::trade_flow::guards::cex_microstructure::CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: start_ms,
            bid: open_mid - 0.5,
            ask: open_mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "rest_open",
        },
    );
    seed_current_mid(venue, asset, current_mid);
}

async fn evaluate_with_config(
    config: CexEntryConsensusConfig,
    outcome_label: &str,
) -> PriceToBeatGuardEvaluation {
    super::evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.30),
        PriceToBeatDiffUnit::Usd,
        outcome_label,
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        config,
        None,
    )
    .await
}

fn own_open_gap_config() -> CexEntryConsensusConfig {
    let mut config = CexEntryConsensusConfig {
        basis: CexEntryConsensusBasis::OwnOpenGap,
        mode: CexEntryConsensusModeConfig::parse(Some("binance_coinbase")),
        open_gap: Default::default(),
    };
    config.open_gap.threshold_usd_explicit = true;
    config
}

#[tokio::test]
async fn own_open_gap_basis_does_not_run_current_price_evaluator() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.9);
    seed_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        BTC_MARKET_5M,
        100.0,
        99.50,
    );
    seed_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        BTC_MARKET_5M,
        100.0,
        99.48,
    );

    let evaluation = evaluate_with_config(own_open_gap_config(), "Down").await;

    assert!(evaluation.passed);
    let debug = evaluation.to_value();
    let evaluations = debug
        .get("entry_current_source_evaluations")
        .and_then(Value::as_array)
        .expect("entry evaluations");
    assert_eq!(evaluations.len(), 1);
    assert_eq!(
        evaluations[0].get("runtime_source").and_then(Value::as_str),
        Some("own_open_gap")
    );
    assert_eq!(
        debug
            .get("cex_entry_consensus_result")
            .and_then(|result| result.get("basis"))
            .and_then(Value::as_str),
        Some("own_open_gap")
    );
}

#[tokio::test]
async fn current_price_basis_keeps_legacy_evaluator() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.9);
    seed_current_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
        "btc",
        99.50,
    );
    seed_current_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        99.48,
    );

    let evaluation = evaluate_with_config(
        CexEntryConsensusConfig::current_price(CexEntryConsensusModeConfig::parse(Some(
            "okx_plus_one",
        ))),
        "Down",
    )
    .await;

    assert!(evaluation.passed);
    let debug = evaluation.to_value();
    let evaluations = debug
        .get("entry_current_source_evaluations")
        .and_then(Value::as_array)
        .expect("entry evaluations");
    assert_eq!(evaluations.len(), 2);
    assert!(evaluations.iter().any(|item| {
        item.get("source").and_then(Value::as_str) == Some("cex_consensus_bybit_plus_one")
    }));
    assert_eq!(
        debug
            .get("cex_entry_consensus_result")
            .and_then(|result| result.get("basis"))
            .and_then(Value::as_str),
        Some("current_price")
    );
}
