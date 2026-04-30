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
