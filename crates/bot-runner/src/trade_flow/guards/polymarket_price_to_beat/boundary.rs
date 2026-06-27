use super::*;
use bot_infra::db::{PostgresRepository, TradeFlowPriceBoundarySnapshotInput};
use chrono::TimeZone;

const BOUNDARY_SOURCE: &str = "chainlink_rtds";

fn valid_price(value: Option<f64>) -> Option<f64> {
    value.filter(|price| price.is_finite() && *price > 0.0)
}

fn timestamp_from_millis(timestamp_ms: i64) -> Result<DateTime<Utc>> {
    Utc.timestamp_millis_opt(timestamp_ms)
        .single()
        .ok_or_else(|| anyhow!("invalid timestamp millis: {timestamp_ms}"))
}

fn boundary_input_for_spec(
    spec: &PriceToBeatQuerySpec,
    open_price: Option<f64>,
    open_ts: Option<DateTime<Utc>>,
    high_price: Option<f64>,
    low_price: Option<f64>,
    close_price: Option<f64>,
    close_ts: Option<DateTime<Utc>>,
    sample_count: i32,
) -> TradeFlowPriceBoundarySnapshotInput {
    TradeFlowPriceBoundarySnapshotInput {
        market_slug: spec.market_slug.clone(),
        asset: spec.asset.to_ascii_lowercase(),
        timeframe: spec.timeframe.clone(),
        window_start: spec.start_at,
        window_end: spec.end_at,
        open_price,
        open_ts,
        high_price,
        low_price,
        close_price,
        close_ts,
        sample_count,
        source: BOUNDARY_SOURCE.to_string(),
    }
}

async fn ensure_completed_boundary_from_chainlink(
    repo: &PostgresRepository,
    spec: &PriceToBeatQuerySpec,
) -> Result<()> {
    if repo
        .get_trade_flow_price_boundary_snapshot(&spec.market_slug)
        .await?
        .and_then(|snapshot| valid_price(snapshot.close_price))
        .is_some()
    {
        return Ok(());
    }

    let stats = match super::super::chainlink_price::get_chainlink_price_window_stats(
        &spec.asset,
        spec.start_at.timestamp_millis(),
        spec.end_at.timestamp_millis(),
    ) {
        Ok(stats) => stats,
        Err(err) => {
            tracing::debug!(
                market_slug = %spec.market_slug,
                asset = %spec.asset,
                error = %err,
                "PRICE_TO_BEAT_BOUNDARY_SNAPSHOT_CHAINLINK_PENDING"
            );
            return Ok(());
        }
    };

    let input = boundary_input_for_spec(
        spec,
        Some(stats.open_price),
        Some(spec.start_at),
        Some(stats.high_price),
        Some(stats.low_price),
        Some(stats.close_price),
        Some(spec.end_at),
        stats.sample_count as i32,
    );
    let snapshot = repo
        .upsert_trade_flow_price_boundary_snapshot(&input)
        .await?;
    tracing::info!(
        market_slug = %snapshot.market_slug,
        asset = %snapshot.asset,
        timeframe = %snapshot.timeframe,
        open_price = snapshot.open_price,
        close_price = snapshot.close_price,
        sample_count = snapshot.sample_count,
        source = %snapshot.source,
        "PRICE_TO_BEAT_BOUNDARY_SNAPSHOT_UPSERTED"
    );
    Ok(())
}

pub(super) async fn record_open_boundary_from_chainlink(
    repo: &PostgresRepository,
    market_slug: &str,
    price: f64,
    timestamp_ms: i64,
) -> Result<()> {
    if !price.is_finite() || price <= 0.0 {
        return Ok(());
    }
    let spec = build_price_to_beat_query_spec(market_slug)?;
    let open_ts = timestamp_from_millis(timestamp_ms)?;
    let input = boundary_input_for_spec(
        &spec,
        Some(price),
        Some(open_ts),
        None,
        None,
        None,
        None,
        1,
    );
    let snapshot = repo
        .upsert_trade_flow_price_boundary_snapshot(&input)
        .await?;
    tracing::info!(
        market_slug = %snapshot.market_slug,
        asset = %snapshot.asset,
        timeframe = %snapshot.timeframe,
        open_price = snapshot.open_price,
        open_ts = ?snapshot.open_ts,
        source = %snapshot.source,
        "PRICE_TO_BEAT_BOUNDARY_SNAPSHOT_UPSERTED"
    );
    Ok(())
}

pub(super) async fn seed_from_rtds_previous_close(
    service: &PolymarketPriceToBeatService,
    repo: &PostgresRepository,
    market_slug: &str,
) -> Result<Option<PolymarketPriceToBeatSnapshot>> {
    if let Some(snapshot) = service.current_snapshot(market_slug) {
        if snapshot.is_lookup_ready() {
            return Ok(Some(snapshot));
        }
    }

    let spec = build_price_to_beat_query_spec(market_slug)?;
    let previous_spec = build_previous_price_to_beat_query_spec(&spec)?;
    ensure_completed_boundary_from_chainlink(repo, &previous_spec).await?;

    let Some(previous) = repo
        .get_trade_flow_price_boundary_snapshot(&previous_spec.market_slug)
        .await?
    else {
        return Ok(None);
    };
    let Some(close_price) = valid_price(previous.close_price) else {
        return Ok(None);
    };

    let source_latency_ms = previous.close_ts.map(|close_ts| {
        (close_ts.timestamp_millis() - previous_spec.end_at.timestamp_millis()).abs()
    });
    let seeded = service.seed_snapshot_with_source(
        &spec.market_slug,
        &spec.asset,
        &spec.timeframe,
        close_price,
        PriceToBeatSource::ChainlinkRtdsPreviousClose,
        source_latency_ms,
    );
    if seeded {
        tracing::info!(
            market_slug = %spec.market_slug,
            previous_market_slug = %previous_spec.market_slug,
            asset = %spec.asset,
            timeframe = %spec.timeframe,
            price_to_beat = close_price,
            source_latency_ms,
            "PRICE_TO_BEAT_SEEDED_FROM_RTDS_PREVIOUS_CLOSE"
        );
    }
    Ok(service
        .current_snapshot(&spec.market_slug)
        .filter(PolymarketPriceToBeatSnapshot::is_lookup_ready))
}
