use super::*;
use crate::trade_flow::guards::price_to_beat::entry_current_hybrid::{
    CexEntryConsensusBasis, CexEntryConsensusConfig, CexEntryConsensusModeConfig,
};

fn seed_hybrid_entry_state(market_slug: &str, asset: &str, ptb: f64, chainlink_current: f64) {
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

fn seed_hybrid_price_to_beat_only(market_slug: &str, asset: &str, ptb: f64) {
    crate::trade_flow::guards::cex_microstructure::clear_cex_microstructure_test_state();
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

fn seed_stale_chainlink_current(asset: &str, price: f64) {
    let stale_ms = chrono::Utc::now().timestamp_millis() - 60_000;
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        asset,
        &[(stale_ms, price)],
    )
    .expect("seed stale chainlink current");
}

fn seed_cex_entry_mid(
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    asset: &str,
    mid: f64,
) {
    seed_cex_entry_mid_at(venue, asset, mid, chrono::Utc::now().timestamp_millis());
}

fn seed_cex_entry_mid_at(
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    asset: &str,
    mid: f64,
    timestamp_ms: i64,
) {
    seed_cex_entry_book_at(venue, asset, mid, timestamp_ms, "ticker");
}

fn seed_cex_entry_book_at(
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    asset: &str,
    mid: f64,
    timestamp_ms: i64,
    source: &'static str,
) {
    crate::trade_flow::guards::cex_microstructure::seed_cex_book_test_sample(
        crate::trade_flow::guards::cex_microstructure::CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms,
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source,
        },
    );
}

fn seed_cex_open_gap(
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
    seed_cex_entry_book_at(
        venue,
        asset,
        current_mid,
        chrono::Utc::now().timestamp_millis(),
        "bookTicker",
    );
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

fn hybrid_debug_value(evaluation: &PriceToBeatGuardEvaluation) -> Value {
    evaluation.to_value()
}

#[allow(clippy::too_many_arguments)]
async fn evaluate_price_to_beat_guard_with_current_source(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    current_price_source: PriceToBeatCurrentPriceSource,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> PriceToBeatGuardEvaluation {
    evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        market_slug,
        mode,
        threshold_value,
        threshold_unit,
        outcome_label,
        signal_config,
        current_price_source,
        CexEntryConsensusModeConfig::default(),
        iv_mismatch_config,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    current_price_source: PriceToBeatCurrentPriceSource,
    cex_entry_consensus_mode: CexEntryConsensusModeConfig,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> PriceToBeatGuardEvaluation {
    evaluate_price_to_beat_guard_with_current_source_and_consensus_config(
        market_slug,
        mode,
        threshold_value,
        threshold_unit,
        outcome_label,
        signal_config,
        current_price_source,
        CexEntryConsensusConfig::current_price(cex_entry_consensus_mode),
        iv_mismatch_config,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn evaluate_price_to_beat_guard_with_current_source_and_consensus_config(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    current_price_source: PriceToBeatCurrentPriceSource,
    cex_entry_consensus_config: CexEntryConsensusConfig,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> PriceToBeatGuardEvaluation {
    super::evaluate_price_to_beat_guard_with_current_source(
        market_slug,
        mode,
        threshold_value,
        threshold_unit,
        outcome_label,
        signal_config,
        current_price_source,
        cex_entry_consensus_config,
        iv_mismatch_config,
    )
    .await
}

#[tokio::test]
async fn chainlink_cex_entry_source_allows_when_chainlink_leg_passes() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 112.0);

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(112.0));
    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    let debug = hybrid_debug_value(&evaluation);
    assert_eq!(
        debug
            .get("selected_entry_current_source")
            .and_then(Value::as_str),
        Some("chainlink_live_data_ws")
    );
    assert_eq!(
        debug
            .get("entry_current_source_evaluations")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_allows_when_cex_confirmed_leg_passes() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 105.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        112.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        111.0,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(112.0));
    assert_eq!(
        evaluation.current_price_source,
        "chainlink_cex_consensus_or_hybrid"
    );
    let debug = hybrid_debug_value(&evaluation);
    assert_eq!(
        debug
            .get("selected_entry_current_source")
            .and_then(Value::as_str),
        Some("chainlink_cex_consensus_or_hybrid")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_allows_stale_chainlink_with_fresh_binance_book_only() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(BTC_MARKET_5M, "btc", 100.0);
    seed_stale_chainlink_current("btc", 105.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        112.0,
    );
    seed_cex_entry_book_at(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        111.0,
        chrono::Utc::now().timestamp_millis(),
        "depth5",
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(
        evaluation.current_price_source,
        "chainlink_cex_consensus_or_hybrid"
    );
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        cex_debug
            .get("binance")
            .and_then(|binance| binance.get("ticker_staleness_ms"))
            .and_then(Value::as_i64),
        cex_debug
            .get("binance")
            .and_then(|binance| binance.get("book_staleness_ms"))
            .and_then(Value::as_i64)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_bybit_micro_stale_retry_recovers() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "doge-updown-5m-1774013100";
    seed_hybrid_price_to_beat_only(market_slug, "doge", 100.0);
    let stale_ms = chrono::Utc::now().timestamp_millis() - 820;
    seed_cex_entry_mid_at(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "doge",
        112.0,
        stale_ms,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "doge",
        111.0,
    );
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        seed_cex_entry_mid(
            crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
            "doge",
            112.0,
        );
    });

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        market_slug,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(
        evaluation.current_price_source,
        "chainlink_cex_consensus_or_hybrid"
    );
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    let retry = cex_debug
        .get("bybit")
        .and_then(|bybit| bybit.get("bybit_micro_stale_retry"))
        .expect("bybit retry debug");
    assert_eq!(retry.get("result").and_then(Value::as_str), Some("ready"));
}

#[tokio::test]
async fn chainlink_cex_entry_source_bybit_micro_stale_retry_preserves_unavailable_when_still_stale()
{
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "doge-updown-5m-1774013100";
    seed_hybrid_price_to_beat_only(market_slug, "doge", 100.0);
    let stale_ms = chrono::Utc::now().timestamp_millis() - 820;
    seed_cex_entry_mid_at(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "doge",
        112.0,
        stale_ms,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "doge",
        111.0,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        market_slug,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_bybit_unavailable")
    );
    let retry = cex_debug
        .get("bybit")
        .and_then(|bybit| bybit.get("bybit_micro_stale_retry"))
        .expect("bybit retry debug");
    assert_eq!(retry.get("result").and_then(Value::as_str), Some("timeout"));
}

#[tokio::test]
async fn chainlink_cex_entry_source_blocks_when_cex_leg_is_unconfirmed() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 105.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        112.0,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = debug
        .get("entry_current_source_evaluations")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("source").and_then(Value::as_str) == Some("cex_consensus_bybit_plus_one")
            })
        })
        .expect("cex debug");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_unconfirmed")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_default_mode_allows_binance_coinbase_pair() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("binance_coinbase")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_selects_conservative_up_price() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.01));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("clean_pair")
    );
    assert_eq!(
        cex_debug.get("current_price_venue").and_then(Value::as_str),
        Some("clean_pair_conservative")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_selects_conservative_down_price() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 101.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        101.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        99.02,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Down",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(99.02));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("clean_pair")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_passes_when_bybit_unavailable() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(BTC_MARKET_5M, "btc", 100.0);
    seed_stale_chainlink_current("btc", 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.01));
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_blocks_single_secondary_hit() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_blocks_dislocated_pair() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        101.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.5,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_clean_pair_dislocated")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_clean_pair_blocks_stale_pair_leg() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid_at(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
        chrono::Utc::now().timestamp_millis() - 2_000,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("clean_pair_fresh").and_then(Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_new_mode_keeps_bybit_plus_one_path() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        100.05,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bybit_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("bybit_plus_one")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_unknown_mode_defaults_with_warning() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("bad_value")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug
            .get("cex_entry_consensus_mode")
            .and_then(Value::as_str),
        Some("binance_coinbase")
    );
    assert_eq!(
        cex_debug
            .get("cex_entry_consensus_mode_parse_warning")
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_prefers_chainlink_when_both_legs_pass() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 113.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
        "btc",
        112.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        111.0,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(113.0));
    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    let debug = hybrid_debug_value(&evaluation);
    assert_eq!(
        debug
            .get("selected_entry_current_source")
            .and_then(Value::as_str),
        Some("chainlink_live_data_ws")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_uses_cex_reason_when_chainlink_unavailable() {
    const FUTURE_MARKET_5M: &str = "btc-updown-5m-9999999900";
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(FUTURE_MARKET_5M, "btc", 100.0);
    seed_stale_chainlink_current("btc", 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        112.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        111.0,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        FUTURE_MARKET_5M,
        PriceToBeatMode::IvMismatchEdge,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "blocked_too_early");
    assert_eq!(
        evaluation.current_price_source,
        "chainlink_cex_consensus_or_hybrid"
    );
    assert_ne!(evaluation.reason_code, "current_price_unavailable");
}

#[tokio::test]
async fn chainlink_cex_entry_source_okx_plus_one_passes_with_secondary() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
        "btc",
        100.05,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("okx_plus_one")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
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
async fn chainlink_cex_entry_source_okx_plus_one_blocks_okx_miss() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
        "btc",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("okx_plus_one")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_okx_below_threshold")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_okx_mode_uses_clean_pair_when_okx_unavailable() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(BTC_MARKET_5M, "btc", 100.0);
    seed_stale_chainlink_current("btc", 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("okx_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.01));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("clean_pair")
    );
    assert_eq!(
        cex_debug.get("anchor_hit").and_then(Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_gate_plus_one_passes_with_secondary() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(SOL_MARKET_5M, "sol", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Gateio,
        "sol",
        100.05,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "sol",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        SOL_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("gate_plus_one")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.05));
    let debug = hybrid_debug_value(&evaluation);
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
async fn chainlink_cex_entry_source_gate_plus_one_blocks_gate_miss() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(SOL_MARKET_5M, "sol", 100.0, 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Gateio,
        "sol",
        99.0,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "sol",
        100.03,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        SOL_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("gate_plus_one")),
        None,
    )
    .await;

    assert!(!evaluation.passed);
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("reason_code").and_then(Value::as_str),
        Some("cex_consensus_gateio_below_threshold")
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_gate_mode_uses_clean_pair_when_gate_unavailable() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(SOL_MARKET_5M, "sol", 100.0);
    seed_stale_chainlink_current("sol", 99.0);
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "sol",
        100.03,
    );
    seed_cex_entry_mid(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "sol",
        100.01,
    );

    let evaluation = evaluate_price_to_beat_guard_with_current_source_and_consensus_mode(
        SOL_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        CexEntryConsensusModeConfig::parse(Some("gate_plus_one_or_clean_pair")),
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.current_price, Some(100.01));
    let debug = hybrid_debug_value(&evaluation);
    let cex_debug = entry_current_candidate(&debug, "cex_consensus_bybit_plus_one");
    assert_eq!(
        cex_debug.get("confirmed_via").and_then(Value::as_str),
        Some("clean_pair")
    );
    assert_eq!(
        cex_debug.get("anchor_venue").and_then(Value::as_str),
        Some("gateio")
    );
    assert_eq!(
        cex_debug.get("anchor_hit").and_then(Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn chainlink_cex_entry_source_keeps_pending_when_all_rtds_sources_missing() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(BTC_MARKET_5M, "btc", 100.0);
    seed_stale_chainlink_current("btc", 99.0);

    let evaluation = evaluate_price_to_beat_guard_with_current_source(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(10.0),
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        None,
    )
    .await;

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "price_to_beat_pending");
    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
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

async fn evaluate_own_open_gap(config: CexEntryConsensusConfig) -> PriceToBeatGuardEvaluation {
    evaluate_price_to_beat_guard_with_current_source_and_consensus_config(
        BTC_MARKET_5M,
        PriceToBeatMode::Manual,
        Some(0.30),
        PriceToBeatDiffUnit::Usd,
        "Down",
        None,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensus,
        config,
        None,
    )
    .await
}

#[tokio::test]
async fn own_open_gap_allows_binance_coinbase_and_reports_quality_metrics() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.9);
    seed_cex_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        "btc",
        BTC_MARKET_5M,
        100.0,
        99.55,
    );
    seed_cex_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "btc",
        BTC_MARKET_5M,
        100.0,
        99.00,
    );
    let mut config = own_open_gap_config();
    config.open_gap.spread_floor_usd = 0.55;
    config.open_gap.ratio_min = 0.44;

    let evaluation = evaluate_own_open_gap(config).await;

    assert!(evaluation.passed);
    let result = hybrid_debug_value(&evaluation)
        .get("cex_entry_consensus_result")
        .cloned()
        .expect("result");
    assert_eq!(
        result.get("reason").and_then(Value::as_str),
        Some("open_gap_anchor_pair_pass")
    );
    assert!(
        (result
            .get("gap_spread")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            - 0.55)
            .abs()
            < 0.000001
    );
    assert!(
        (result
            .get("gap_ratio")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            - 0.45)
            .abs()
            < 0.000001
    );
}

#[tokio::test]
async fn own_open_gap_uses_binance_coinbase_pair_deterministically() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.9);
    for (venue, current_mid) in [
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
            99.90,
        ),
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
            99.50,
        ),
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
            99.49,
        ),
    ] {
        seed_cex_open_gap(venue, "btc", BTC_MARKET_5M, 100.0, current_mid);
    }

    let passed = evaluate_own_open_gap(own_open_gap_config()).await;
    assert!(passed.passed);
    assert_eq!(passed.reason_code, "passed");
    let result = hybrid_debug_value(&passed)
        .get("cex_entry_consensus_result")
        .cloned()
        .expect("result");
    assert_eq!(
        result.get("reason").and_then(Value::as_str),
        Some("open_gap_anchor_pair_pass")
    );

    let mut disabled = own_open_gap_config();
    disabled.open_gap.allow_clean_pair_without_anchor = false;
    let blocked = evaluate_own_open_gap(disabled).await;
    assert!(blocked.passed);
    assert_eq!(blocked.reason_code, "passed");
}

#[tokio::test]
async fn own_open_gap_blocks_fresh_opposite_venue() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_entry_state(BTC_MARKET_5M, "btc", 100.0, 99.9);
    for (venue, current_mid) in [
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
            99.50,
        ),
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
            99.45,
        ),
        (
            crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
            100.70,
        ),
    ] {
        seed_cex_open_gap(venue, "btc", BTC_MARKET_5M, 100.0, current_mid);
    }

    let evaluation = evaluate_own_open_gap(own_open_gap_config()).await;

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "open_gap_opposite_venue_detected");
}

#[tokio::test]
async fn own_open_gap_blocks_when_chainlink_stale_with_sanity_enabled() {
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    let _guard = lock_price_to_beat_test_state();
    seed_hybrid_price_to_beat_only(BTC_MARKET_5M, "doge", 100.0);
    seed_stale_chainlink_current("doge", 99.9);
    seed_cex_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Okx,
        "doge",
        BTC_MARKET_5M,
        100.0,
        99.50,
    );
    seed_cex_open_gap(
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        "doge",
        BTC_MARKET_5M,
        100.0,
        99.48,
    );

    let evaluation = evaluate_own_open_gap(own_open_gap_config()).await;

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "open_gap_chainlink_sanity_fail");
}
