async fn seed_price_to_beat_previous_close_for_warm_slug(
    repo: &PostgresRepository,
    run_id: i64,
    market_slug: &str,
) -> bool {
    match crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_rtds_previous_close(
        repo,
        market_slug,
    )
    .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            warn!(
                run_id,
                market_slug,
                error = %err,
                "PRICE_TO_BEAT_RTDS_PREVIOUS_CLOSE_SEED_FAILED"
            );
            false
        }
    }
}

async fn seed_price_to_beat_for_auto_scope_rotation(
    repo: &PostgresRepository,
    run_id: i64,
    flow_run_id: i64,
    definition_id: i64,
    version_id: i64,
    market_slug: &str,
    asset: &str,
    timeframe: &str,
    expected_market_start: &DateTime<Utc>,
) -> bool {
    let mut seeded_any = match crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_rtds_previous_close(
        repo,
        market_slug,
    )
    .await
    {
        Ok(Some(snapshot)) => {
            info!(
                run_id,
                flow_run_id,
                market_slug,
                source = snapshot.source.as_str(),
                status = snapshot.status(),
                price_to_beat = snapshot.price_to_beat,
                "PRICE_TO_BEAT_SEEDED_FROM_RTDS_PREVIOUS_CLOSE"
            );
            true
        }
        Ok(None) => false,
        Err(err) => {
            warn!(
                run_id,
                flow_run_id,
                market_slug,
                error = %err,
                "PRICE_TO_BEAT_RTDS_PREVIOUS_CLOSE_SEED_FAILED"
            );
            false
        }
    };

    match crate::trade_flow::guards::chainlink_price::get_chainlink_price_start_tick(
        asset,
        expected_market_start.timestamp_millis(),
    ) {
        Ok(snapshot) => {
            let source_latency_ms =
                Some((snapshot.timestamp_ms - expected_market_start.timestamp_millis()).abs());
            let seeded = crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
                market_slug,
                asset,
                timeframe,
                snapshot.price,
                source_latency_ms,
            );
            if seeded {
                seeded_any = true;
                info!(
                    run_id,
                    flow_run_id,
                    market_slug,
                    asset,
                    chainlink_price = snapshot.price,
                    chainlink_tick_ts = snapshot.timestamp_ms,
                    source_latency_ms,
                    "PRICE_TO_BEAT_SEEDED_FROM_CHAINLINK"
                );
            }
            if let Err(err) = crate::trade_flow::guards::polymarket_price_to_beat::record_price_to_beat_open_boundary_from_chainlink(
                repo,
                market_slug,
                snapshot.price,
                snapshot.timestamp_ms,
            )
            .await
            {
                warn!(
                    run_id,
                    flow_run_id,
                    market_slug,
                    asset,
                    error = %err,
                    "PRICE_TO_BEAT_OPEN_BOUNDARY_RECORD_FAILED"
                );
            }
        }
        Err(err) => {
            let err_text = err.to_string();
            warn!(
                run_id,
                flow_run_id,
                market_slug,
                asset,
                error = %err,
                "PRICE_TO_BEAT_CHAINLINK_SEED_FAILED"
            );
            if let Some(details) = crate::trade_flow::guards::chainlink_price::parse_chainlink_near_timestamp_rejection_details(&err_text) {
                let expected_market_start_text = expected_market_start.to_rfc3339();
                warn!(
                    run_id,
                    flow_run_id,
                    market_slug,
                    asset,
                    timeframe,
                    expected_market_start = %expected_market_start_text,
                    gap_ms = details.gap_ms,
                    provider_age_ms = details.provider_age_ms,
                    candidate_timestamp_ms = details.candidate_timestamp_ms,
                    candidate_received_at_ms = details.candidate_received_at_ms,
                    "CHAINLINK_SEED_REJECTED_TOO_OLD"
                );
                let payload = build_chainlink_seed_rejected_too_old_payload(
                    market_slug,
                    asset,
                    timeframe,
                    expected_market_start,
                    &details,
                );
                if let Err(event_err) = repo
                    .append_trade_flow_event(
                        Some(flow_run_id),
                        definition_id,
                        Some(version_id),
                        "chainlink_seed_rejected_too_old",
                        &payload,
                    )
                    .await
                {
                    warn!(
                        run_id,
                        flow_run_id,
                        market_slug,
                        asset,
                        error = %event_err,
                        "CHAINLINK_SEED_REJECTED_TOO_OLD_EVENT_FAILED"
                    );
                }
            }
        }
    }

    crate::trade_flow::guards::polymarket_price_to_beat::warm_price_to_beat_cache_bg(market_slug);
    seeded_any
}
