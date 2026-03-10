use super::*;

pub(crate) fn validate(
    bot: &BotConfig,
    strategy: &StrategyConfig,
    risk: &RiskConfig,
    execution: &ExecutionConfig,
    exchange: &ExchangeConfig,
    claim: &ClaimConfig,
    _telegram: &TelegramConfig,
) -> Result<()> {
    anyhow::ensure!(
        (0.0..=1.0).contains(&strategy.entry_price),
        "entry_price must be in [0,1]"
    );
    anyhow::ensure!(strategy.tp_pct > 0.0, "tp_pct must be > 0");
    anyhow::ensure!(
        strategy.aggressive_sl_pct > 0.0,
        "aggressive_sl_pct must be > 0"
    );

    if strategy.dual_side_enabled {
        anyhow::ensure!(
            strategy.total_notional_usdc > 0.0,
            "total_notional_usdc must be > 0 in dual_side mode"
        );
        anyhow::ensure!(
            strategy.per_leg_initial_notional_usdc > 0.0,
            "per_leg_initial_notional_usdc must be > 0 in dual_side mode"
        );
        anyhow::ensure!(
            strategy.per_leg_initial_notional_usdc * 2.0 <= strategy.total_notional_usdc,
            "per_leg_initial_notional_usdc * 2 must be <= total_notional_usdc"
        );
        anyhow::ensure!(
            strategy.dca_interval_sec > 0,
            "dca_interval_sec must be > 0 in dual_side mode"
        );
        anyhow::ensure!(
            strategy.max_dca_levels_per_leg > 0,
            "max_dca_levels_per_leg must be > 0 in dual_side mode"
        );
        anyhow::ensure!(
            (0.0..=1.0).contains(&strategy.dca_step_pct) && strategy.dca_step_pct > 0.0,
            "dca_step_pct must be in (0,1]"
        );
        anyhow::ensure!(
            (0.0..=1.0).contains(&strategy.leg_tp_pct) && strategy.leg_tp_pct > 0.0,
            "leg_tp_pct must be in (0,1]"
        );
        anyhow::ensure!(
            strategy.basket_tp_usdc > 0.0,
            "basket_tp_usdc must be > 0 in dual_side mode"
        );
        anyhow::ensure!(
            strategy.basket_sl_usdc < 0.0,
            "basket_sl_usdc must be < 0 in dual_side mode"
        );
        anyhow::ensure!(
            strategy.force_flatten_sec_before_close > 0,
            "force_flatten_sec_before_close must be > 0 in dual_side mode"
        );
    }
    anyhow::ensure!(
        risk.max_notional_per_market_usdc > 0.0,
        "max_notional_per_market_usdc must be > 0"
    );
    if !strategy.flow_only {
        for scope in bot.resolve_scopes() {
            anyhow::ensure!(
                SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES
                    .iter()
                    .any(|(s, _)| *s == scope),
                "market_scope '{}' must be one of: {}",
                scope,
                supported_market_scope_names_csv()
            );
        }
    }
    if !bot.market_slug_override.trim().is_empty() {
        let override_lower = bot.market_slug_override.to_ascii_lowercase();
        anyhow::ensure!(
            SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES
                .iter()
                .any(|(_, slug_prefix)| override_lower.contains(slug_prefix)),
            "market_slug_override must contain a supported slug prefix (e.g. btc-updown-5m-, eth-updown-15m-)"
        );
    }
    anyhow::ensure!(
        bot.market_selection == "latest_by_slug",
        "market_selection must be one of: latest_by_slug"
    );
    anyhow::ensure!(
        bot.market_discovery_retry_interval_ms >= 500,
        "market_discovery_retry_interval_ms must be >= 500"
    );
    anyhow::ensure!(bot.loop_interval_ms >= 100, "loop_interval_ms too low");
    anyhow::ensure!(!execution.order_type.is_empty(), "order_type required");
    anyhow::ensure!(
        exchange.gamma_base_url.starts_with("http"),
        "gamma_base_url must start with http"
    );
    anyhow::ensure!(
        exchange.clob_base_url.starts_with("http"),
        "clob_base_url must start with http"
    );
    anyhow::ensure!(
        exchange.clob_ws_url.starts_with("ws"),
        "clob_ws_url must start with ws"
    );
    anyhow::ensure!(exchange.chain_id > 0, "chain_id must be > 0");
    anyhow::ensure!(
        !exchange.ctf_exchange_address.is_empty(),
        "ctf_exchange_address required"
    );
    anyhow::ensure!(
        exchange.resolve_signer_private_key().is_ok(),
        "signer_private_key required"
    );
    let inline_address = exchange.api_address.trim();
    let inline_key = exchange.api_key.trim();
    let inline_secret = exchange.api_secret.trim();
    let inline_passphrase = exchange.api_passphrase.trim();
    let inline_any = !inline_address.is_empty()
        || !inline_key.is_empty()
        || !inline_secret.is_empty()
        || !inline_passphrase.is_empty();

    if inline_any {
        anyhow::ensure!(
            !inline_address.is_empty()
                && !inline_key.is_empty()
                && !inline_secret.is_empty()
                && !inline_passphrase.is_empty(),
            "api_address, api_key, api_secret, api_passphrase must all be set when using direct credentials"
        );
    } else {
        anyhow::ensure!(
            !exchange.api_address_env.is_empty(),
            "api_address_env is required when direct api_address is empty"
        );
        anyhow::ensure!(
            !exchange.api_key_env.is_empty(),
            "api_key_env is required when direct api_key is empty"
        );
        anyhow::ensure!(
            !exchange.api_secret_env.is_empty(),
            "api_secret_env is required when direct api_secret is empty"
        );
        anyhow::ensure!(
            !exchange.api_passphrase_env.is_empty(),
            "api_passphrase_env is required when direct api_passphrase is empty"
        );
    }

    anyhow::ensure!(
        claim.data_api_base_url.starts_with("http"),
        "claim.data_api_base_url must start with http"
    );
    anyhow::ensure!(
        claim.rpc_url.starts_with("http"),
        "claim.rpc_url must start with http"
    );
    anyhow::ensure!(claim.chain_id > 0, "claim.chain_id must be > 0");
    anyhow::ensure!(
        claim.discovery_interval_sec >= 5,
        "claim.discovery_interval_sec must be >= 5"
    );
    anyhow::ensure!(
        claim.positions_page_size > 0,
        "claim.positions_page_size must be > 0"
    );
    anyhow::ensure!(
        claim.positions_max_pages > 0,
        "claim.positions_max_pages must be > 0"
    );
    anyhow::ensure!(
        claim.process_batch_size > 0,
        "claim.process_batch_size must be > 0"
    );
    anyhow::ensure!(claim.max_attempts > 0, "claim.max_attempts must be > 0");
    anyhow::ensure!(
        claim.retry_backoff_ms >= 1000,
        "claim.retry_backoff_ms must be >= 1000"
    );
    anyhow::ensure!(
        is_hex_address(&claim.ctf_contract_address),
        "claim.ctf_contract_address must be a valid 0x address"
    );
    anyhow::ensure!(
        is_hex_address(&claim.collateral_token_address),
        "claim.collateral_token_address must be a valid 0x address"
    );
    if claim.enabled {
        anyhow::ensure!(
            !claim.rpc_url.trim().is_empty() || !claim.rpc_url_env.trim().is_empty(),
            "claim.rpc_url or claim.rpc_url_env is required when claim.enabled=true"
        );
        anyhow::ensure!(
            !claim.user_address.trim().is_empty() || !claim.user_address_env.trim().is_empty(),
            "claim.user_address or claim.user_address_env is required when claim.enabled=true"
        );
        anyhow::ensure!(
            !claim.private_key.trim().is_empty() || !claim.private_key_env.trim().is_empty(),
            "claim.private_key or claim.private_key_env is required when claim.enabled=true"
        );
        validate_claim_user_address(claim)?;
        validate_claim_private_key(claim)?;
    }

    anyhow::ensure!(
        !matches!(risk.kill_switch_mode, KillSwitchMode::Disabled)
            || !risk.manual_kill_switch_active,
        "manual_kill_switch_active cannot be true when kill_switch_mode is disabled"
    );
    Ok(())
}

pub(crate) fn is_hex_address(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with("0x")
        && trimmed.len() == 42
        && trimmed[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn is_hex_private_key(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with("0x")
        && trimmed.len() == 66
        && trimmed[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn validate_claim_user_address(claim: &ClaimConfig) -> Result<()> {
    if !claim.enabled {
        return Ok(());
    }

    let user_address = claim.resolve_user_address()?;
    anyhow::ensure!(
        is_hex_address(&user_address),
        "claim.user_address must be a valid 0x address when provided"
    );
    Ok(())
}

pub(crate) fn validate_claim_private_key(claim: &ClaimConfig) -> Result<()> {
    if !claim.enabled {
        return Ok(());
    }

    let private_key = claim.resolve_private_key()?;
    anyhow::ensure!(
        is_hex_private_key(&private_key),
        "claim.private_key must be a valid 0x private key when provided"
    );
    Ok(())
}
