async fn record_market_discovery_event(
    repo: &PostgresRepository,
    run_id: i64,
    decision: &str,
    state: MarketDiscoveryState,
    scope: &str,
    selected_market_slug: Option<&str>,
    reason_code: &str,
    message: &str,
) {
    let details = json!({
        "run_id": run_id,
        "state": state.as_str(),
        "market_scope": scope,
        "selected_market_slug": selected_market_slug,
        "reason_code": reason_code,
        "message": message
    })
    .to_string();

    if let Err(err) = repo
        .record_risk_event(None, "market_discovery", decision, &details)
        .await
    {
        warn!(
            run_id,
            error = %err,
            "MARKET_DISCOVERY_EVENT_WRITE_FAILED"
        );
    }
}

fn supported_updown_scope_names_csv() -> String {
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .map(|def| def.scope)
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_updown_scope_by_scope(scope: &str) -> Option<UpdownScopeDef> {
    let normalized = scope.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| def.scope == normalized)
}

pub(crate) fn find_updown_scope_by_asset_timeframe(
    asset: &str,
    timeframe: &str,
) -> Option<UpdownScopeDef> {
    let normalized_asset = asset.trim().to_ascii_lowercase();
    let normalized_timeframe = timeframe.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| def.asset == normalized_asset && def.timeframe == normalized_timeframe)
}

fn find_updown_slug_prefix(raw: &str) -> Option<&'static str> {
    let normalized = raw.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .find(|def| normalized.starts_with(def.slug_prefix))
        .map(|def| def.slug_prefix)
}

fn find_updown_scope_by_slug(slug: &str) -> Option<UpdownScopeDef> {
    let normalized = slug.trim().to_ascii_lowercase();
    SUPPORTED_UPDOWN_SCOPE_DEFS
        .iter()
        .copied()
        .find(|def| normalized.starts_with(def.slug_prefix))
}

fn updown_scope_window_seconds(scope_def: UpdownScopeDef) -> i64 {
    match scope_def.timeframe {
        "15m" => 900,
        _ => 300,
    }
}

fn updown_scope_candidate_slugs(scope_def: UpdownScopeDef, now: DateTime<Utc>) -> Vec<String> {
    let window = updown_scope_window_seconds(scope_def);
    let now_ts = now.timestamp();
    let base = now_ts - now_ts.rem_euclid(window);
    [base - window, base, base + window, base + (2 * window)]
        .into_iter()
        .filter(|ts| *ts > 0)
        .map(|ts| format!("{}{}", scope_def.slug_prefix, ts))
        .collect()
}

fn scope_candidate_window_markets(
    scope_def: UpdownScopeDef,
    markets: &[GammaMarket],
    now: DateTime<Utc>,
) -> Vec<GammaMarket> {
    let candidate_slugs: HashSet<String> = updown_scope_candidate_slugs(scope_def, now)
        .into_iter()
        .collect();
    markets
        .iter()
        .filter(|market| {
            if !candidate_slugs.contains(&market.slug) {
                return false;
            }
            // Gamma API gecikmesi nedeniyle bitmiş market dönebilir — hariç tut
            let (_, ends_at) = infer_updown_market_window(market);
            ends_at.map(|e| e > now).unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn populate_token_id_cache(markets: &[GammaMarket]) {
    let mut cache = AUTO_SCOPE_TOKEN_ID_CACHE.lock().unwrap();
    for market in markets {
        cache.insert(
            market.slug.clone(),
            CachedMarketTokens {
                yes_token_id: market.yes_token_id.clone(),
                no_token_id: market.no_token_id.clone(),
                maker_base_fee: market.maker_base_fee,
                neg_risk: market.neg_risk,
            },
        );
    }
    let cutoff_ts = Utc::now().timestamp() - 86_400;
    cache.retain(|slug, _| {
        MarketCycleId(slug.clone())
            .start_time()
            .map(|t| t.timestamp() >= cutoff_ts)
            .unwrap_or(false)
    });
}

fn build_synthetic_markets_from_cache(
    scope_def: UpdownScopeDef,
    now: DateTime<Utc>,
) -> Vec<GammaMarket> {
    let candidates = updown_scope_candidate_slugs(scope_def, now);
    let cache = AUTO_SCOPE_TOKEN_ID_CACHE.lock().unwrap();
    candidates
        .into_iter()
        .filter_map(|slug| {
            let cached = cache.get(&slug)?;
            if cached.yes_token_id.is_none() || cached.no_token_id.is_none() {
                return None;
            }
            Some(GammaMarket {
                slug,
                condition_id: None,
                end_date_iso: None,
                active: true,
                closed: false,
                yes_token_id: cached.yes_token_id.clone(),
                no_token_id: cached.no_token_id.clone(),
                maker_base_fee: cached.maker_base_fee,
                neg_risk: cached.neg_risk,
                order_price_min_tick_size: None,
                order_min_size: None,
            })
        })
        .collect()
}

pub(crate) async fn list_markets_for_scope(
    gamma: &GammaHttpClient,
    scope: &str,
) -> Result<Vec<GammaMarket>> {
    let scope_def = find_updown_scope_by_scope(scope).ok_or_else(|| {
        anyhow::anyhow!(
            "unsupported market_scope: {scope} (supported: {})",
            supported_updown_scope_names_csv()
        )
    })?;
    let mut markets: Vec<GammaMarket> = match gamma.list_active_updown_markets().await {
        Ok(all) => {
            let filtered: Vec<GammaMarket> = all
                .into_iter()
                .filter(|market| market.slug.starts_with(scope_def.slug_prefix))
                .collect();
            populate_token_id_cache(&filtered);
            filtered
        }
        Err(gamma_err) => {
            let synthetic = build_synthetic_markets_from_cache(scope_def, Utc::now());
            if synthetic.is_empty() {
                let mut direct = Vec::new();
                for slug in updown_scope_candidate_slugs(scope_def, Utc::now()) {
                    let fetched = match gamma.get_market_by_slug(&slug).await {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let Some(market) = fetched else {
                        continue;
                    };
                    if !market.slug.starts_with(scope_def.slug_prefix) {
                        continue;
                    }
                    if !market.active || market.closed {
                        continue;
                    }
                    direct.push(market);
                }
                if direct.is_empty() {
                    return Err(gamma_err.context(
                        "Gamma API failed and no cached token IDs available for fallback",
                    ));
                }
                populate_token_id_cache(&direct);
                tracing::warn!(
                    scope,
                    direct_count = direct.len(),
                    error = %gamma_err,
                    "AUTO_SCOPE_GAMMA_FALLBACK_DIRECT_SLUG_MARKETS"
                );
                direct
            } else {
                tracing::warn!(
                    scope,
                    synthetic_count = synthetic.len(),
                    error = %gamma_err,
                    "AUTO_SCOPE_GAMMA_FALLBACK_SYNTHETIC_MARKETS"
                );
                synthetic
            }
        }
    };

    if !markets.is_empty() {
        // Prefer markets around the current time window so DCA targets the
        // currently traded 5m/15m market instead of a far-future active slug.
        let now_for_filter = Utc::now();
        let in_window = scope_candidate_window_markets(scope_def, &markets, now_for_filter);
        if !in_window.is_empty() {
            return Ok(in_window);
        }
    }

    let mut seen_slugs: HashSet<String> =
        markets.iter().map(|market| market.slug.clone()).collect();

    for slug in updown_scope_candidate_slugs(scope_def, Utc::now()) {
        if !seen_slugs.insert(slug.clone()) {
            continue;
        }
        let fetched = match gamma.get_market_by_slug(&slug).await {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(market) = fetched else {
            continue;
        };
        if !market.slug.starts_with(scope_def.slug_prefix) {
            continue;
        }
        if !market.active || market.closed {
            continue;
        }
        markets.push(market);
    }

    populate_token_id_cache(&markets);

    let now_for_retry = Utc::now();
    let in_window_retry = scope_candidate_window_markets(scope_def, &markets, now_for_retry);
    if !in_window_retry.is_empty() {
        return Ok(in_window_retry);
    }

    Ok(markets)
}

fn infer_updown_market_window(
    market: &GammaMarket,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let starts_from_slug = MarketCycleId(market.slug.clone()).start_time();
    let ends_from_iso = market.end_date_iso.as_deref().and_then(parse_rfc3339_utc);
    let Some(scope_def) = find_updown_scope_by_slug(&market.slug) else {
        return (starts_from_slug, ends_from_iso);
    };
    let window = ChronoDuration::seconds(updown_scope_window_seconds(scope_def));
    match (starts_from_slug, ends_from_iso) {
        (Some(starts_at), Some(ends_at)) => (Some(starts_at), Some(ends_at)),
        (Some(starts_at), None) => (Some(starts_at), Some(starts_at + window)),
        (None, Some(ends_at)) => (Some(ends_at - window), Some(ends_at)),
        (None, None) => (None, None),
    }
}

fn select_live_market_with_reason(
    market: GammaMarket,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    reason: LiveMarketSelectionReason,
) -> SelectedLiveMarket {
    SelectedLiveMarket {
        slug: market.slug,
        yes_token_id: market.yes_token_id,
        no_token_id: market.no_token_id,
        maker_base_fee: market.maker_base_fee,
        starts_at,
        ends_at,
        selection_reason: reason,
    }
}

pub(crate) fn select_preferred_live_market(
    markets: Vec<GammaMarket>,
    now: DateTime<Utc>,
) -> Option<SelectedLiveMarket> {
    let timed: Vec<(GammaMarket, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> = markets
        .into_iter()
        .map(|market| {
            let (starts_at, ends_at) = infer_updown_market_window(&market);
            (market, starts_at, ends_at)
        })
        .collect();
    if timed.is_empty() {
        return None;
    }

    if let Some((market, starts_at, ends_at)) = timed
        .iter()
        .filter_map(|(market, starts_at, ends_at)| match (starts_at, ends_at) {
            (Some(start), Some(end)) if *start <= now && now < *end => {
                Some((market.clone(), Some(start.clone()), Some(end.clone())))
            }
            _ => None,
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.slug.cmp(&b.0.slug)))
    {
        return Some(select_live_market_with_reason(
            market,
            starts_at,
            ends_at,
            LiveMarketSelectionReason::InWindow,
        ));
    }

    if let Some((market, starts_at, ends_at)) = timed
        .iter()
        .filter_map(|(market, starts_at, ends_at)| match starts_at {
            Some(start) if *start >= now => Some((
                market.clone(),
                Some(start.clone()),
                ends_at.as_ref().cloned(),
            )),
            _ => None,
        })
        .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.slug.cmp(&b.0.slug)))
    {
        return Some(select_live_market_with_reason(
            market,
            starts_at,
            ends_at,
            LiveMarketSelectionReason::NearestFuture,
        ));
    }

    timed
        .into_iter()
        .max_by(|a, b| a.0.slug.cmp(&b.0.slug))
        .map(|(market, starts_at, ends_at)| {
            select_live_market_with_reason(
                market,
                starts_at,
                ends_at,
                LiveMarketSelectionReason::LatestBySlugFallback,
            )
        })
}

fn select_live_market(
    markets: Vec<GammaMarket>,
    selection: &str,
    require_yes_no_tokens: bool,
) -> Option<SelectedLiveMarket> {
    let candidates: Vec<GammaMarket> = markets
        .into_iter()
        .filter(|m| !require_yes_no_tokens || (m.yes_token_id.is_some() && m.no_token_id.is_some()))
        .collect();

    match selection {
        "latest_by_slug" => select_preferred_live_market(candidates, Utc::now()),
        _ => None,
    }
}

fn extract_slug_from_market_override(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let direct = trimmed
        .trim_end_matches('/')
        .split(['?', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    if find_updown_slug_prefix(direct).is_some() {
        return Some(direct.to_ascii_lowercase());
    }

    trimmed.split(['/', '?', '#', '&', '=']).find_map(|part| {
        let normalized = part.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }
        find_updown_slug_prefix(&normalized).map(|_| normalized)
    })
}

fn configured_market_override_slug(cfg: &AppConfig) -> Result<Option<String>> {
    if cfg.bot.market_slug_override.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(extract_slug_from_market_override(&cfg.bot.market_slug_override).ok_or_else(
        || {
            anyhow::anyhow!(
                "market_slug_override must include a supported updown slug (e.g. btc-updown-5m-..., eth-updown-15m-...) or a polymarket event URL"
            )
        },
    )?))
}

fn select_market_from_candidates(
    markets: Vec<GammaMarket>,
    override_slug: Option<&str>,
    selection: &str,
    require_yes_no_tokens: bool,
) -> Option<SelectedLiveMarket> {
    if let Some(forced_slug) = override_slug {
        return markets
            .into_iter()
            .find(|m| {
                m.slug == forced_slug
                    && (!require_yes_no_tokens
                        || (m.yes_token_id.is_some() && m.no_token_id.is_some()))
            })
            .map(|m| {
                let (starts_at, ends_at) = infer_updown_market_window(&m);
                select_live_market_with_reason(
                    m,
                    starts_at,
                    ends_at,
                    LiveMarketSelectionReason::OverrideSlug,
                )
            });
    }

    select_live_market(markets, selection, require_yes_no_tokens)
}

fn detect_trade_builder_stale_market(
    order_market_slug: &str,
    selected: Option<&SelectedLiveMarket>,
) -> Option<TradeBuilderStaleRollingMarket> {
    let normalized_slug = order_market_slug.trim().to_ascii_lowercase();
    let scope_def = find_updown_scope_by_slug(&normalized_slug)?;
    let selected = selected?;
    if selected.slug == normalized_slug {
        return None;
    }

    Some(TradeBuilderStaleRollingMarket {
        detected_scope: scope_def.scope,
        detected_asset: scope_def.asset,
        detected_timeframe: scope_def.timeframe,
        current_live_market_slug: selected.slug.clone(),
        current_live_selection_reason: selected.selection_reason,
    })
}

async fn resolve_trade_builder_stale_market(
    gamma: &GammaHttpClient,
    order_market_slug: &str,
) -> Result<Option<TradeBuilderStaleRollingMarket>> {
    let Some(scope_def) = find_updown_scope_by_slug(order_market_slug) else {
        return Ok(None);
    };

    let markets = list_markets_for_scope(gamma, scope_def.scope).await?;
    let selected = select_preferred_live_market(markets, Utc::now());
    Ok(detect_trade_builder_stale_market(
        order_market_slug,
        selected.as_ref(),
    ))
}

async fn discover_live_market_once(
    cfg: &AppConfig,
    gamma: &GammaHttpClient,
    require_yes_no_tokens: bool,
    override_slug: Option<&str>,
) -> Result<Option<SelectedLiveMarket>> {
    let markets = list_markets_for_scope(gamma, &cfg.bot.market_scope).await?;
    Ok(select_market_from_candidates(
        markets,
        override_slug,
        &cfg.bot.market_selection,
        require_yes_no_tokens,
    ))
}

async fn discover_live_market(
    run_id: i64,
    repo: &PostgresRepository,
    cfg: &AppConfig,
    gamma: &GammaHttpClient,
    require_yes_no_tokens: bool,
) -> Result<SelectedLiveMarket> {
    let override_slug = configured_market_override_slug(cfg)?;

    let retry_interval = Duration::from_millis(cfg.bot.market_discovery_retry_interval_ms);
    let timeout = if cfg.bot.market_discovery_timeout_sec == 0 {
        None
    } else {
        Some(Duration::from_secs(cfg.bot.market_discovery_timeout_sec))
    };
    let started_at = Instant::now();
    let mut waiting_event_emitted = false;

    loop {
        let markets = match list_markets_for_scope(gamma, &cfg.bot.market_scope).await {
            Ok(markets) => markets,
            Err(err) => {
                warn!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    error = %err,
                    "MARKET_DISCOVERY_FETCH_FAILED"
                );
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_discovery_fetch_failed",
                        "Failed to fetch market list. Retrying.",
                    )
                    .await;
                }

                if let Some(max_wait) = timeout {
                    if started_at.elapsed() >= max_wait {
                        let timeout_message = format!(
                            "Market discovery timed out after {}s while fetching market list.",
                            cfg.bot.market_discovery_timeout_sec
                        );
                        error!(
                            run_id,
                            scope = %cfg.bot.market_scope,
                            timeout_sec = cfg.bot.market_discovery_timeout_sec,
                            message = %timeout_message,
                            "MARKET_DISCOVERY_TIMEOUT"
                        );
                        record_market_discovery_event(
                            repo,
                            run_id,
                            "halt",
                            MarketDiscoveryState::Error,
                            &cfg.bot.market_scope,
                            None,
                            "market_discovery_timeout",
                            &timeout_message,
                        )
                        .await;
                        anyhow::bail!(timeout_message);
                    }
                }

                sleep(retry_interval).await;
                continue;
            }
        };

        let selected = select_market_from_candidates(
            markets,
            override_slug.as_deref(),
            &cfg.bot.market_selection,
            require_yes_no_tokens,
        );

        if let Some(selected) = selected {
            info!(
                run_id,
                scope = %cfg.bot.market_scope,
                selection = %cfg.bot.market_selection,
                override_slug = ?override_slug,
                market = %selected.slug,
                selection_reason = selected.selection_reason.as_str(),
                market_start_at = ?selected.starts_at,
                market_end_at = ?selected.ends_at,
                now_utc = %Utc::now(),
                "MARKET_DISCOVERY_FOUND"
            );
            record_market_discovery_event(
                repo,
                run_id,
                "allow",
                MarketDiscoveryState::Ready,
                &cfg.bot.market_scope,
                Some(&selected.slug),
                "market_discovery_ready",
                "Market selected successfully.",
            )
            .await;
            return Ok(selected);
        }

        if !waiting_event_emitted {
            waiting_event_emitted = true;
            let waiting_message = if let Some(forced_slug) = override_slug.as_ref() {
                if require_yes_no_tokens {
                    format!(
                        "Override market not active or missing YES/NO token IDs: {forced_slug}. Retrying."
                    )
                } else {
                    format!("Override market not active yet: {forced_slug}. Retrying.")
                }
            } else if require_yes_no_tokens {
                "No active market with YES/NO token IDs. Retrying.".to_string()
            } else {
                "No active market found. Retrying.".to_string()
            };
            info!(
                run_id,
                scope = %cfg.bot.market_scope,
                selection = %cfg.bot.market_selection,
                override_slug = ?override_slug,
                retry_interval_ms = cfg.bot.market_discovery_retry_interval_ms,
                timeout_sec = cfg.bot.market_discovery_timeout_sec,
                "MARKET_DISCOVERY_WAITING"
            );
            record_market_discovery_event(
                repo,
                run_id,
                "block",
                MarketDiscoveryState::WaitingForMarket,
                &cfg.bot.market_scope,
                None,
                if require_yes_no_tokens {
                    "market_missing_token_ids"
                } else {
                    "market_discovery_waiting"
                },
                &waiting_message,
            )
            .await;
        }

        if let Some(max_wait) = timeout {
            if started_at.elapsed() >= max_wait {
                let timeout_message = format!(
                    "Market discovery timed out after {}s.",
                    cfg.bot.market_discovery_timeout_sec
                );
                error!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    timeout_sec = cfg.bot.market_discovery_timeout_sec,
                    message = %timeout_message,
                    "MARKET_DISCOVERY_TIMEOUT"
                );
                record_market_discovery_event(
                    repo,
                    run_id,
                    "halt",
                    MarketDiscoveryState::Error,
                    &cfg.bot.market_scope,
                    None,
                    "market_discovery_timeout",
                    &timeout_message,
                )
                .await;
                anyhow::bail!(timeout_message);
            }
        }

        sleep(retry_interval).await;
    }
}
