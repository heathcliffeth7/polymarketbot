#[derive(Debug, Clone, PartialEq)]
struct LiveGapCollectorPriceSnapshot {
    price: f64,
    timestamp_ms: Option<i64>,
    staleness_ms: i64,
    source: String,
    fallback_reason: Option<String>,
    binance_staleness_ms: Option<i64>,
}

fn live_gap_collector_open_price_snapshot(
    asset: &str,
    market_slug: &str,
    window_start_ms: i64,
    now_ms: i64,
) -> Result<LiveGapCollectorPriceSnapshot> {
    match trade_flow::guards::chainlink_price::get_chainlink_price_start_tick(
        asset,
        window_start_ms,
    ) {
        Ok(snapshot) => Ok(LiveGapCollectorPriceSnapshot {
            price: snapshot.price,
            timestamp_ms: Some(snapshot.timestamp_ms),
            staleness_ms: (now_ms - snapshot.timestamp_ms).max(0),
            source: "chainlink_rtds_start_tick".to_string(),
            fallback_reason: None,
            binance_staleness_ms: None,
        }),
        Err(err) => {
            let Some(snapshot) =
                trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(market_slug)
            else {
                return Err(err);
            };
            Ok(LiveGapCollectorPriceSnapshot {
                price: snapshot.price_to_beat,
                timestamp_ms: Some(snapshot.fetched_at.timestamp_millis()),
                staleness_ms: (now_ms - snapshot.fetched_at.timestamp_millis()).max(0),
                source: format!("price_to_beat_cached_{}", snapshot.source.as_str()),
                fallback_reason: Some(err.to_string()),
                binance_staleness_ms: None,
            })
        }
    }
}

fn live_gap_collector_current_price_snapshot(
    asset: &str,
    now_ms: i64,
    max_stale_ms: i64,
) -> Result<LiveGapCollectorPriceSnapshot> {
    match trade_flow::guards::binance_price::get_binance_price_snapshot(asset, now_ms) {
        Ok(snapshot) if snapshot.staleness_ms <= max_stale_ms => {
            Ok(LiveGapCollectorPriceSnapshot {
                price: snapshot.price,
                timestamp_ms: Some(snapshot.timestamp_ms),
                staleness_ms: snapshot.staleness_ms,
                source: "binance_live_data_ws".to_string(),
                fallback_reason: None,
                binance_staleness_ms: Some(snapshot.staleness_ms),
            })
        }
        Ok(snapshot) => {
            match trade_flow::guards::chainlink_price::get_chainlink_price_near_timestamp(
                asset, now_ms,
            ) {
                Ok(chainlink) => Ok(LiveGapCollectorPriceSnapshot {
                    price: chainlink.price,
                    timestamp_ms: Some(chainlink.timestamp_ms),
                    staleness_ms: (now_ms - chainlink.timestamp_ms).max(0),
                    source: "chainlink_live_cached_binance_stale_fallback".to_string(),
                    fallback_reason: Some(format!("binance_stale:{}ms", snapshot.staleness_ms)),
                    binance_staleness_ms: Some(snapshot.staleness_ms),
                }),
                Err(_) => Ok(LiveGapCollectorPriceSnapshot {
                    price: snapshot.price,
                    timestamp_ms: Some(snapshot.timestamp_ms),
                    staleness_ms: snapshot.staleness_ms,
                    source: "binance_live_data_ws".to_string(),
                    fallback_reason: None,
                    binance_staleness_ms: Some(snapshot.staleness_ms),
                }),
            }
        }
        Err(err) => match trade_flow::guards::chainlink_price::get_chainlink_price_near_timestamp(
            asset, now_ms,
        ) {
            Ok(chainlink) => Ok(LiveGapCollectorPriceSnapshot {
                price: chainlink.price,
                timestamp_ms: Some(chainlink.timestamp_ms),
                staleness_ms: (now_ms - chainlink.timestamp_ms).max(0),
                source: "chainlink_live_cached_binance_unavailable_fallback".to_string(),
                fallback_reason: Some(err.to_string()),
                binance_staleness_ms: None,
            }),
            Err(_) => Err(err),
        },
    }
}

fn live_gap_collector_regime_staleness_ms(snapshot: &LiveGapCollectorPriceSnapshot) -> i64 {
    snapshot.staleness_ms
}

fn live_gap_collector_volatility_usd(asset: &str, now_ms: i64) -> Option<f64> {
    let samples = trade_flow::guards::chainlink_price::get_chainlink_price_samples(
        asset,
        now_ms - 15_000,
        now_ms,
    )
    .ok()?;
    let mut min_price = f64::INFINITY;
    let mut max_price = f64::NEG_INFINITY;
    for sample in samples {
        min_price = min_price.min(sample.price);
        max_price = max_price.max(sample.price);
    }
    (min_price.is_finite() && max_price.is_finite()).then_some(max_price - min_price)
}
