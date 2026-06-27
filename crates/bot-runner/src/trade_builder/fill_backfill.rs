const TRADE_BUILDER_FILL_BACKFILL_MAX_RECENT_PAGES: usize = 5;
const TRADE_BUILDER_FILL_BACKFILL_RETRY_DELAYS_MS: [u64; 4] = [0, 250, 750, 1500];

#[derive(Debug, Clone, Default)]
struct TradeBuilderFillPageSyncResult {
    synced_count: usize,
    pages_scanned: usize,
    raw_count: usize,
    first_row_keys: Vec<String>,
    next_cursor_present: bool,
}

impl TradeBuilderFillPageSyncResult {
    fn merge(&mut self, other: TradeBuilderFillPageSyncResult) {
        self.synced_count = self.synced_count.saturating_add(other.synced_count);
        self.pages_scanned = self.pages_scanned.saturating_add(other.pages_scanned);
        self.raw_count = self.raw_count.saturating_add(other.raw_count);
        self.next_cursor_present |= other.next_cursor_present;
        if self.first_row_keys.is_empty() {
            self.first_row_keys = other.first_row_keys;
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TradeBuilderFillBackfillOutcome {
    synced_count: usize,
    attempts: usize,
    pages_scanned: usize,
    raw_count: usize,
    first_row_keys: Vec<String>,
    next_cursor_present: bool,
    associated_trade_ids: Vec<String>,
    aggregate_fill_qty: Option<f64>,
    order_info_filled_size: Option<f64>,
}

impl TradeBuilderFillBackfillOutcome {
    fn merge_sync(&mut self, sync: TradeBuilderFillPageSyncResult) {
        self.synced_count = self.synced_count.saturating_add(sync.synced_count);
        self.pages_scanned = self.pages_scanned.saturating_add(sync.pages_scanned);
        self.raw_count = self.raw_count.saturating_add(sync.raw_count);
        self.next_cursor_present |= sync.next_cursor_present;
        if self.first_row_keys.is_empty() {
            self.first_row_keys = sync.first_row_keys;
        }
    }

    fn actual_fill_qty_and_source(&self) -> Option<(f64, &'static str)> {
        normalize_trade_builder_terminal_fill_qty_candidate(self.aggregate_fill_qty)
            .map(|qty| (qty, "fills_aggregate"))
            .or_else(|| {
                normalize_trade_builder_terminal_fill_qty_candidate(self.order_info_filled_size)
                    .map(|qty| (qty, "order_status_size_matched"))
            })
    }
}

fn resolve_trade_builder_immediate_fill_quantities(
    order: &TradeBuilderOrder,
    submit_size: f64,
    backfill: &TradeBuilderFillBackfillOutcome,
) -> Result<(f64, &'static str, Option<f64>, Option<&'static str>)> {
    let actual_fill = backfill.actual_fill_qty_and_source();
    if trade_builder_should_track_buy_inventory_observation(order) {
        if let Some((qty, source)) = actual_fill {
            return Ok((qty, "actual_fill_qty", Some(qty), Some(source)));
        }
        let (canonical_entry_qty, canonical_entry_qty_source) =
            trade_builder_canonical_entry_qty(order, Some(submit_size)).ok_or_else(|| {
                anyhow::anyhow!("builder order canonical fill qty unresolved after fill backfill")
            })?;
        return Ok((canonical_entry_qty, canonical_entry_qty_source, None, None));
    }

    let (qty, source) = actual_fill.unwrap_or((submit_size, "submitted_order_size"));
    Ok((qty, "actual_fill_qty", Some(qty), Some(source)))
}

async fn aggregate_trade_builder_fill_qty_option(
    repo: &PostgresRepository,
    exchange_order_id: &str,
) -> Result<Option<f64>> {
    Ok(normalize_trade_builder_terminal_fill_qty_candidate(Some(
        repo.aggregate_fill_qty_by_exchange_order_id(exchange_order_id)
            .await?,
    )))
}

async fn sync_trade_builder_fill_page(
    repo: &PostgresRepository,
    page: FillPage,
    target_exchange_order_id: Option<&str>,
    mut stats: Option<&mut crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats>,
) -> Result<usize> {
    use crate::trade_builder_fill_sync_timing::FinalFillSyncTimer;

    let mut synced = 0usize;
    for fill in page.fills {
        if fill.fill_id.trim().is_empty()
            || fill.order_id.trim().is_empty()
            || fill.price <= 0.0
            || fill.size <= 0.0
        {
            continue;
        }
        if target_exchange_order_id
            .map(|target| fill.order_id != target)
            .unwrap_or(false)
        {
            continue;
        }
        let lookup_timer = FinalFillSyncTimer::start();
        let internal_order_id_result = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await;
        if let Some(stats) = stats.as_deref_mut() {
            stats.record_db_order_lookup_ms(lookup_timer.elapsed_ms());
        }
        let Some(internal_order_id) = internal_order_id_result? else {
            continue;
        };
        let raw_fill = fill.raw_payload.clone().unwrap_or_else(|| {
            json!({
                "fill_id": fill.fill_id,
                "order_id": fill.order_id,
                "price": fill.price,
                "size": fill.size,
                "fee": fill.fee,
                "timestamp": fill.ts
            })
        });

        let upsert_timer = FinalFillSyncTimer::start();
        let upsert_result = repo
            .upsert_fill_by_exchange_fill_id(
                internal_order_id,
                &fill.fill_id,
                fill.price,
                fill.size,
                fill.fee.unwrap_or_default(),
                fill.ts,
                &raw_fill,
            )
            .await;
        if let Some(stats) = stats.as_deref_mut() {
            stats.record_db_upsert_ms(upsert_timer.elapsed_ms());
        }
        upsert_result?;
        synced = synced.saturating_add(1);
    }

    Ok(synced)
}

fn trade_builder_cursor_has_next(next_cursor: Option<&str>, current_cursor: &str) -> bool {
    let Some(next_cursor) = next_cursor.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    !matches!(next_cursor, "LTE=" | "END" | "end") && next_cursor != current_cursor
}

async fn sync_trade_builder_fill_pages(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    base_query: TradeQuery,
    target_exchange_order_id: Option<&str>,
    max_pages: usize,
) -> Result<TradeBuilderFillPageSyncResult> {
    sync_trade_builder_fill_pages_with_stats(
        repo,
        client,
        base_query,
        target_exchange_order_id,
        max_pages,
        None,
    )
    .await
}

async fn sync_trade_builder_fill_pages_with_stats(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    base_query: TradeQuery,
    target_exchange_order_id: Option<&str>,
    max_pages: usize,
    mut stats: Option<&mut crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats>,
) -> Result<TradeBuilderFillPageSyncResult> {
    use crate::trade_builder_fill_sync_timing::FinalFillSyncTimer;

    let mut result = TradeBuilderFillPageSyncResult::default();
    let mut next_cursor = base_query.next_cursor.clone();
    let max_pages = max_pages.max(1);

    for _ in 0..max_pages {
        let current_cursor = next_cursor.clone().unwrap_or_else(|| "MA==".to_string());
        let mut query = base_query.clone();
        query.next_cursor = Some(current_cursor.clone());
        let fetch_timer = FinalFillSyncTimer::start();
        let page_result = client.list_fills_page(query).await;
        if let Some(stats) = stats.as_deref_mut() {
            stats.record_fetch_page_ms(fetch_timer.elapsed_ms());
        }
        let page = page_result?;
        let page_next_cursor = page.next_cursor.clone();
        let first_row_keys = page.first_row_keys.clone();
        let raw_count = page.raw_count;
        let next_cursor_present =
            trade_builder_cursor_has_next(page_next_cursor.as_deref(), &current_cursor);
        let apply_timer = FinalFillSyncTimer::start();
        let synced_result = sync_trade_builder_fill_page(
            repo,
            page,
            target_exchange_order_id,
            stats.as_deref_mut(),
        )
        .await;
        if let Some(stats) = stats.as_deref_mut() {
            stats.record_page_apply_ms(apply_timer.elapsed_ms());
        }
        let synced = synced_result?;

        result.synced_count = result.synced_count.saturating_add(synced);
        result.pages_scanned = result.pages_scanned.saturating_add(1);
        result.raw_count = result.raw_count.saturating_add(raw_count);
        result.next_cursor_present |= next_cursor_present;
        if result.first_row_keys.is_empty() {
            result.first_row_keys = first_row_keys;
        }
        if let Some(stats) = stats.as_deref_mut() {
            stats.record_page_counts(raw_count, synced);
        }

        if !next_cursor_present {
            break;
        }
        next_cursor = page_next_cursor;
    }

    Ok(result)
}

async fn sync_trade_builder_associated_trade_ids(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    exchange_order_id: &str,
    associated_trade_ids: &[String],
) -> TradeBuilderFillPageSyncResult {
    let mut result = TradeBuilderFillPageSyncResult::default();
    for trade_id in associated_trade_ids {
        let sync = sync_trade_builder_fill_pages(
            repo,
            client,
            TradeQuery {
                id: Some(trade_id.clone()),
                ..TradeQuery::default()
            },
            Some(exchange_order_id),
            1,
        )
        .await;
        match sync {
            Ok(sync) => result.merge(sync),
            Err(err) => warn!(
                exchange_order_id = %exchange_order_id,
                trade_id = %trade_id,
                error = %err,
                "TRADE_BUILDER_ASSOCIATED_TRADE_FILL_BACKFILL_FAILED"
            ),
        }
    }
    result
}

async fn backfill_trade_builder_fills_for_order(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    builder_order_id: i64,
    exchange_order_id: &str,
) -> Result<TradeBuilderFillBackfillOutcome> {
    let mut outcome = TradeBuilderFillBackfillOutcome {
        aggregate_fill_qty: aggregate_trade_builder_fill_qty_option(repo, exchange_order_id)
            .await?,
        ..TradeBuilderFillBackfillOutcome::default()
    };
    if outcome.aggregate_fill_qty.is_some() {
        return Ok(outcome);
    }

    for (attempt_index, delay_ms) in TRADE_BUILDER_FILL_BACKFILL_RETRY_DELAYS_MS
        .iter()
        .copied()
        .enumerate()
    {
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        let attempt = attempt_index + 1;
        outcome.attempts = attempt;
        repo.append_trade_builder_order_event(
            builder_order_id,
            "fill_backfill_attempt",
            &json!({
                "exchange_order_id": exchange_order_id,
                "attempt": attempt,
                "delay_ms": delay_ms,
            }),
        )
        .await?;

        match client.status(exchange_order_id).await {
            Ok(order_info) => {
                outcome.order_info_filled_size =
                    normalize_trade_builder_terminal_fill_qty_candidate(order_info.filled_size)
                        .or(outcome.order_info_filled_size);
                if outcome.associated_trade_ids.is_empty() {
                    outcome.associated_trade_ids = order_info.associated_trade_ids.clone();
                }
                let associated_sync = sync_trade_builder_associated_trade_ids(
                    repo,
                    client,
                    exchange_order_id,
                    &order_info.associated_trade_ids,
                )
                .await;
                outcome.merge_sync(associated_sync);
            }
            Err(err) => {
                repo.append_trade_builder_order_event(
                    builder_order_id,
                    "fill_backfill_failed",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "attempt": attempt,
                        "stage": "get_order",
                        "error": err.to_string(),
                    }),
                )
                .await?;
                warn!(
                    builder_order_id,
                    exchange_order_id = %exchange_order_id,
                    error = %err,
                    "TRADE_BUILDER_FILL_BACKFILL_GET_ORDER_FAILED"
                );
            }
        }

        let recent_sync = sync_trade_builder_fill_pages(
            repo,
            client,
            TradeQuery::default(),
            Some(exchange_order_id),
            TRADE_BUILDER_FILL_BACKFILL_MAX_RECENT_PAGES,
        )
        .await;
        match recent_sync {
            Ok(sync) => outcome.merge_sync(sync),
            Err(err) => {
                repo.append_trade_builder_order_event(
                    builder_order_id,
                    "fill_backfill_failed",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "attempt": attempt,
                        "stage": "recent_trades",
                        "error": err.to_string(),
                    }),
                )
                .await?;
                warn!(
                    builder_order_id,
                    exchange_order_id = %exchange_order_id,
                    error = %err,
                    "TRADE_BUILDER_FILL_BACKFILL_RECENT_TRADES_FAILED"
                );
            }
        }

        outcome.aggregate_fill_qty =
            aggregate_trade_builder_fill_qty_option(repo, exchange_order_id).await?;
        if outcome.aggregate_fill_qty.is_some() {
            repo.append_trade_builder_order_event(
                builder_order_id,
                "fill_backfill_synced",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "attempt": attempt,
                    "synced_count": outcome.synced_count,
                    "pages_scanned": outcome.pages_scanned,
                    "raw_count": outcome.raw_count,
                    "associated_trade_ids": outcome.associated_trade_ids,
                    "first_row_keys": outcome.first_row_keys,
                    "next_cursor_present": outcome.next_cursor_present,
                    "aggregate_fill_qty": outcome.aggregate_fill_qty,
                    "order_info_filled_size": outcome.order_info_filled_size,
                }),
            )
            .await?;
            return Ok(outcome);
        }
    }

    repo.append_trade_builder_order_event(
        builder_order_id,
        "fill_backfill_zero",
        &json!({
            "exchange_order_id": exchange_order_id,
            "attempts": outcome.attempts,
            "synced_count": outcome.synced_count,
            "pages_scanned": outcome.pages_scanned,
            "raw_count": outcome.raw_count,
            "associated_trade_ids": outcome.associated_trade_ids,
            "first_row_keys": outcome.first_row_keys,
            "next_cursor_present": outcome.next_cursor_present,
            "aggregate_fill_qty": outcome.aggregate_fill_qty,
            "order_info_filled_size": outcome.order_info_filled_size,
        }),
    )
    .await?;

    Ok(outcome)
}
