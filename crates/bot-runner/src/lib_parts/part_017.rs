async fn execute_action_resolve_market(
    cfg: &AppConfig,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let legacy_scope = node_config_string(node, "marketScope")
        .or_else(|| flow_context_string(context, "marketScope"))
        .unwrap_or_else(|| cfg.bot.market_scope.clone());
    let legacy_scope_def = find_updown_scope_by_scope(&legacy_scope);
    let asset = node_config_string(node, "asset")
        .or_else(|| legacy_scope_def.map(|def| def.asset.to_string()))
        .unwrap_or_else(|| "btc".to_string());
    let timeframe = node_config_string(node, "timeframe")
        .or_else(|| legacy_scope_def.map(|def| def.timeframe.to_string()))
        .unwrap_or_else(|| "5m".to_string());
    let scope_def = find_updown_scope_by_asset_timeframe(&asset, &timeframe).ok_or_else(|| {
        anyhow::anyhow!(
            "action.resolve_market unsupported asset/timeframe ({asset}/{timeframe}); supported assets: btc, eth, sol, xrp; timeframes: 5m, 15m"
        )
    })?;
    let market_scope = scope_def.scope.to_string();
    let slug_prefix =
        node_config_string(node, "slugPrefix").unwrap_or_else(|| scope_def.slug_prefix.to_string());
    let selection =
        node_config_string(node, "selection").unwrap_or_else(|| "latest_by_slug".to_string());
    let fail_on_missing_market = node_config_bool(node, "failOnMissingMarket").unwrap_or(true);
    let require_yes_no_tokens = node_config_bool(node, "requireYesNoTokens").unwrap_or(true);
    let require_token_id = node_config_bool(node, "requireTokenId").unwrap_or(true);
    let preferred_outcome = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_else(|| "yes".to_string());
    let normalized_outcome = match preferred_outcome.trim().to_ascii_lowercase().as_str() {
        "no" | "false" | "0" => "no",
        _ => "yes",
    };

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let mut markets = list_markets_for_scope(&gamma, &market_scope).await?;
    let candidate_count_before_prefix = markets.len();
    if !slug_prefix.is_empty() {
        markets.retain(|market| market.slug.starts_with(&slug_prefix));
    }
    let candidate_count = markets.len();

    let selected = select_live_market(markets, &selection, require_yes_no_tokens);
    let Some(selected) = selected else {
        let message = format!(
            "action.resolve_market could not find active market (scope={market_scope}, asset={}, timeframe={}, selection={selection}, slugPrefix={slug_prefix})",
            scope_def.asset,
            scope_def.timeframe
        );
        if fail_on_missing_market {
            anyhow::bail!(message);
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "found": false,
                "reason": "market_not_found",
                "market_scope": market_scope,
                "asset": scope_def.asset,
                "timeframe": scope_def.timeframe,
                "selection": selection,
                "slug_prefix": slug_prefix,
                "candidate_count_before_prefix": candidate_count_before_prefix,
                "candidate_count": candidate_count
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    };

    let resolved_token_id = if normalized_outcome == "no" {
        selected.no_token_id.clone()
    } else {
        selected.yes_token_id.clone()
    };
    if require_token_id && resolved_token_id.is_none() {
        let message = format!(
            "action.resolve_market missing tokenId for outcome={normalized_outcome} on market={}",
            selected.slug
        );
        if fail_on_missing_market {
            anyhow::bail!(message);
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "found": false,
                "reason": "token_not_found",
                "market_slug": selected.slug,
                "outcome_label": normalized_outcome,
                "yes_token_id": selected.yes_token_id,
                "no_token_id": selected.no_token_id
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    let selected_slug = selected.slug;
    let yes_token_id = selected.yes_token_id;
    let no_token_id = selected.no_token_id;
    let resolved_outcome_label = if normalized_outcome == "no" {
        "No".to_string()
    } else {
        "Yes".to_string()
    };
    set_flow_context(context, "marketSlug", json!(selected_slug));
    set_flow_context(context, "marketScope", json!(scope_def.scope));
    set_flow_context(context, "marketAsset", json!(scope_def.asset));
    set_flow_context(context, "marketTimeframe", json!(scope_def.timeframe));
    set_flow_context(context, "outcomeLabel", json!(resolved_outcome_label));
    set_flow_context(context, "yesTokenId", json!(yes_token_id));
    set_flow_context(context, "noTokenId", json!(no_token_id));
    set_flow_context(context, "tokenId", json!(resolved_token_id));

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "resolved_market".to_string());
    let resolved_market_slug = flow_context_value(context, "marketSlug").unwrap_or(Value::Null);
    let resolved_outcome = flow_context_value(context, "outcomeLabel").unwrap_or(Value::Null);
    let resolved_token = flow_context_value(context, "tokenId").unwrap_or(Value::Null);
    let resolved_yes_token = flow_context_value(context, "yesTokenId").unwrap_or(Value::Null);
    let resolved_no_token = flow_context_value(context, "noTokenId").unwrap_or(Value::Null);
    let resolved_scope = flow_context_value(context, "marketScope").unwrap_or(Value::Null);
    let resolved_asset = flow_context_value(context, "marketAsset").unwrap_or(Value::Null);
    let resolved_timeframe = flow_context_value(context, "marketTimeframe").unwrap_or(Value::Null);
    set_flow_var(
        context,
        &format!("{var_prefix}_slug"),
        resolved_market_slug.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        resolved_outcome.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        resolved_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_yes_token_id"),
        resolved_yes_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_no_token_id"),
        resolved_no_token.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_scope"),
        resolved_scope.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_asset"),
        resolved_asset.clone(),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_timeframe"),
        resolved_timeframe.clone(),
    );

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "found": true,
            "market_scope": market_scope,
            "asset": scope_def.asset,
            "timeframe": scope_def.timeframe,
            "selection": selection,
            "slug_prefix": slug_prefix,
            "market_slug": flow_context_string(context, "marketSlug"),
            "token_id": flow_context_string(context, "tokenId"),
            "outcome_label": flow_context_string(context, "outcomeLabel"),
            "yes_token_id": flow_context_string(context, "yesTokenId"),
            "no_token_id": flow_context_string(context, "noTokenId"),
            "candidate_count_before_prefix": candidate_count_before_prefix,
            "candidate_count": candidate_count
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_dual_dca(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sourceTradeId"))?;

    let asset = node_config_string(node, "asset")
        .or_else(|| node_config_string(node, "coin"))
        .or_else(|| flow_context_string(context, "marketAsset"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires asset (btc/eth/sol/xrp)"))?
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        matches!(asset.as_str(), "btc" | "eth" | "sol" | "xrp"),
        "action.dual_dca asset must be one of: btc, eth, sol, xrp"
    );

    let timeframe_raw = node_config_string(node, "timeframe")
        .or_else(|| node_config_string(node, "marketPeriod"))
        .or_else(|| flow_context_string(context, "marketTimeframe"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires timeframe (5m/15m)"))?
        .trim()
        .to_ascii_lowercase();
    let timeframe = match timeframe_raw.as_str() {
        "5" | "5m" | "5min" | "5 min" => "5m",
        "15" | "15m" | "15min" | "15 min" => "15m",
        _ => {
            anyhow::bail!("action.dual_dca timeframe must be 5m or 15m");
        }
    };

    let side_mode_raw = node_config_string(node, "sideMode")
        .or_else(|| node_config_string(node, "side"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sideMode (up/down/all)"))?
        .trim()
        .to_ascii_lowercase();
    let side_mode = match side_mode_raw.as_str() {
        "up" => "up",
        "down" => "down",
        "all" => "all",
        _ => {
            anyhow::bail!("action.dual_dca sideMode must be up, down or all");
        }
    };

    let configured_base_shares = node_config_f64(node, "baseShares");
    let configured_base_usdc = node_config_f64(node, "baseUsdc")
        .or_else(|| node_config_f64(node, "sizeUsdc"))
        .or_else(|| node_config_f64(node, "notionalUsdc"));
    let base_sizing_raw = node_config_string(node, "baseSizing")
        .or_else(|| node_config_string(node, "baseSizeMode"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseSizing (shares/usdc)"))?
        .trim()
        .to_ascii_lowercase();
    let base_sizing = match base_sizing_raw.as_str() {
        "shares" => "shares",
        "usdc" => "usdc",
        _ => {
            anyhow::bail!("action.dual_dca baseSizing must be shares or usdc");
        }
    };

    let base_shares = if base_sizing == "shares" {
        let value = configured_base_shares
            .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseShares"))?;
        anyhow::ensure!(value > 0.0, "action.dual_dca baseShares must be > 0");
        Some(value)
    } else {
        None
    };
    let base_usdc = if base_sizing == "usdc" {
        let value = configured_base_usdc
            .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires baseUsdc"))?;
        anyhow::ensure!(value > 0.0, "action.dual_dca baseUsdc must be > 0");
        Some(value)
    } else {
        None
    };

    let base_price_usdc = node_config_f64(node, "basePriceUsdc")
        .or_else(|| node_config_f64(node, "basePrice"))
        .or_else(|| node_config_f64(node, "basePriceCent").map(|v| v / 100.0));
    if let Some(base_price) = base_price_usdc {
        anyhow::ensure!(
            (0.01..=0.99).contains(&base_price),
            "action.dual_dca basePriceUsdc must be in [0.01, 0.99]"
        );
    }

    let dca_levels = node_config_i64(node, "dcaLevels")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires dcaLevels"))?;
    anyhow::ensure!(
        (1..=20).contains(&dca_levels),
        "action.dual_dca dcaLevels must be in [1, 20]"
    );

    let near_step = node_config_f64(node, "nearStep")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires nearStep"))?;
    anyhow::ensure!(
        near_step > 0.0 && near_step < 1.0,
        "action.dual_dca nearStep must be in (0, 1)"
    );
    let step_mult = node_config_f64(node, "stepMult")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires stepMult"))?;
    anyhow::ensure!(step_mult >= 1.0, "action.dual_dca stepMult must be >= 1");
    let size_mult = node_config_f64(node, "sizeMult")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires sizeMult"))?;
    anyhow::ensure!(size_mult > 0.0, "action.dual_dca sizeMult must be > 0");
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires minPriceDistanceCent"))?;
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.dual_dca minPriceDistanceCent must be > 0"
    );
    let cutoff_min = node_config_i64(node, "cutoffMin")
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires cutoffMin"))?;
    anyhow::ensure!(cutoff_min >= 0, "action.dual_dca cutoffMin must be >= 0");

    let tp_profit_usdc = node_config_f64(node, "tpProfitPct")
        .or_else(|| node_config_f64(node, "tpProfit"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires tpProfitPct"))?;
    let sl_loss_usdc = node_config_f64(node, "slLossPct")
        .or_else(|| node_config_f64(node, "slLoss"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires slLossPct"))?;
    let sl_spread_usdc = node_config_f64(node, "slSpreadPct")
        .or_else(|| node_config_f64(node, "slSpread"))
        .ok_or_else(|| anyhow::anyhow!("action.dual_dca requires slSpreadPct"))?;
    anyhow::ensure!(
        tp_profit_usdc >= 0.0 && sl_loss_usdc >= 0.0 && sl_spread_usdc >= 0.0,
        "action.dual_dca risk thresholds must be >= 0"
    );

    let scope_def = find_updown_scope_by_asset_timeframe(&asset, timeframe).ok_or_else(|| {
        anyhow::anyhow!(
            "action.dual_dca unsupported asset/timeframe ({asset}/{timeframe}); supported assets: btc, eth, sol, xrp; timeframes: 5m, 15m"
        )
    })?;

    let job_id = repo
        .upsert_trade_flow_dual_dca_job(
            run.id,
            run.definition_id,
            Some(run.version_id),
            &node.key,
            Some(source_trade_id),
            &asset,
            timeframe,
            side_mode,
            base_sizing,
            base_shares,
            base_usdc,
            base_price_usdc,
            dca_levels as i32,
            near_step,
            step_mult,
            size_mult,
            min_price_distance_cent,
            cutoff_min as i32,
            tp_profit_usdc,
            sl_loss_usdc,
            sl_spread_usdc,
        )
        .await?;

    repo.append_trade_flow_dual_dca_event(
        job_id,
        None,
        "job_upserted",
        &json!({
            "flow_run_id": run.id,
            "flow_definition_id": run.definition_id,
            "flow_version_id": run.version_id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_scope": scope_def.scope,
            "asset": asset,
            "timeframe": timeframe,
            "side_mode": side_mode,
            "base_sizing": base_sizing,
            "base_shares": base_shares,
            "base_usdc": base_usdc,
            "base_price_usdc": base_price_usdc,
            "dca_levels": dca_levels,
            "near_step": near_step,
            "step_mult": step_mult,
            "size_mult": size_mult,
            "min_price_distance_cent": min_price_distance_cent,
            "cutoff_min": cutoff_min,
            "tp_profit_usdc": tp_profit_usdc,
            "sl_loss_usdc": sl_loss_usdc,
            "sl_spread_usdc": sl_spread_usdc
        }),
    )
    .await?;

    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    set_flow_ref(context, &node.key, json!(job_id));
    set_flow_ref(context, &ref_key, json!(job_id));
    set_flow_var(context, &format!("{ref_key}_job_id"), json!(job_id));
    set_flow_context(context, "marketScope", json!(scope_def.scope));
    set_flow_context(context, "marketAsset", json!(scope_def.asset));
    set_flow_context(context, "marketTimeframe", json!(scope_def.timeframe));

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "job_id": job_id,
            "ref_key": ref_key,
            "market_scope": scope_def.scope,
            "asset": scope_def.asset,
            "timeframe": scope_def.timeframe,
            "side_mode": side_mode,
            "base_sizing": base_sizing,
            "base_shares": base_shares,
            "base_usdc": base_usdc,
            "base_price_usdc": base_price_usdc,
            "dca_levels": dca_levels
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

