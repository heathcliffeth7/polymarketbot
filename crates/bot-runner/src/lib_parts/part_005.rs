fn resolve_api_credentials_with_source(
    cfg: &AppConfig,
) -> Result<(ApiCredentials, CredentialSource)> {
    let inline_address = cfg.exchange.api_address.trim();
    let inline_key = cfg.exchange.api_key.trim();
    let inline_secret = cfg.exchange.api_secret.trim();
    let inline_passphrase = cfg.exchange.api_passphrase.trim();
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

        let has_encrypted_inline = [inline_address, inline_key, inline_secret, inline_passphrase]
            .into_iter()
            .any(|value| value.starts_with(CONFIG_ENC_PREFIX));

        let key_material = if has_encrypted_inline {
            Some(load_config_encryption_key()?)
        } else {
            None
        };

        let address =
            decrypt_config_value_if_needed("api_address", inline_address, key_material.as_ref())?;
        let key = decrypt_config_value_if_needed("api_key", inline_key, key_material.as_ref())?;
        let secret =
            decrypt_config_value_if_needed("api_secret", inline_secret, key_material.as_ref())?;
        let passphrase = decrypt_config_value_if_needed(
            "api_passphrase",
            inline_passphrase,
            key_material.as_ref(),
        )?;

        return Ok((
            ApiCredentials {
                address,
                key,
                secret,
                passphrase,
            },
            CredentialSource::Inline,
        ));
    }

    Ok((
        ApiCredentials::from_env(
            &cfg.exchange.api_address_env,
            &cfg.exchange.api_key_env,
            &cfg.exchange.api_secret_env,
            &cfg.exchange.api_passphrase_env,
        )?,
        CredentialSource::Env,
    ))
}

fn decrypt_config_string_if_needed(field_name: &str, raw_value: &str) -> Result<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let key_material = if trimmed.starts_with(CONFIG_ENC_PREFIX) {
        Some(load_config_encryption_key()?)
    } else {
        None
    };

    decrypt_config_value_if_needed(field_name, trimmed, key_material.as_ref())
        .map(|value| value.trim().to_string())
}

fn current_config_dir() -> PathBuf {
    PathBuf::from(env::var("BOT_CONFIG_DIR").unwrap_or_else(|_| "./config".to_string()))
}

pub(crate) async fn load_user_app_config_cached(
    repo: &PostgresRepository,
    user_id: i64,
    cache: &mut HashMap<i64, AppConfig>,
) -> Result<AppConfig> {
    if let Some(cfg) = cache.get(&user_id) {
        return Ok(cfg.clone());
    }

    let settings = repo.load_user_settings_payloads(user_id).await?;
    let cfg = AppConfig::load_from_user_settings(&current_config_dir(), &settings)?;
    cache.insert(user_id, cfg.clone());
    Ok(cfg)
}

fn build_order_executor_from_app_config(cfg: &AppConfig) -> Result<ClobHttpClient> {
    let (creds, _) = resolve_api_credentials_with_source(cfg)?;
    let private_key = cfg
        .exchange
        .resolve_signer_private_key()
        .context("CLOB signer private key")?;
    let wallet = private_key
        .parse::<LocalWallet>()
        .context("parse signer private key")?
        .with_chain_id(cfg.exchange.chain_id);
    let exchange_address: Address = cfg
        .exchange
        .ctf_exchange_address
        .parse()
        .context("parse ctf_exchange_address")?;
    let gnosis_safe: Option<Address> = cfg
        .exchange
        .resolve_gnosis_safe_address()
        .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
        .transpose()?;
    Ok(ClobHttpClient::from_credentials(
        cfg.exchange.clob_base_url.clone(),
        Some(cfg.claim.data_api_base_url.clone()),
        cfg.claim.positions_page_size,
        cfg.claim.positions_max_pages,
        creds,
        wallet,
        exchange_address,
        cfg.exchange.chain_id,
        gnosis_safe,
    ))
}

pub(crate) async fn load_user_order_executor_cached(
    repo: &PostgresRepository,
    user_id: i64,
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
    executor_cache: &mut HashMap<i64, SharedOrderExecutor>,
) -> Result<SharedOrderExecutor> {
    if let Some(client) = executor_cache.get(&user_id) {
        return Ok(Arc::clone(client));
    }

    let cfg = load_user_app_config_cached(repo, user_id, user_cfg_cache).await?;
    let client: SharedOrderExecutor = Arc::new(build_order_executor_from_app_config(&cfg)?);
    executor_cache.insert(user_id, Arc::clone(&client));
    Ok(client)
}

async fn load_user_telegram_config(
    repo: &PostgresRepository,
    user_id: i64,
) -> Result<TelegramConfig> {
    let settings = repo.load_user_settings_payloads(user_id).await?;
    match settings.get("telegram") {
        Some(value) => serde_json::from_value::<TelegramConfig>(value.clone())
            .context("parsing stored telegram config payload"),
        None => Ok(TelegramConfig::default()),
    }
}

fn resolve_telegram_bot_token(telegram: &TelegramConfig, node: &TradeFlowNode) -> Result<String> {
    let configured_token = telegram.bot_token.trim();
    if !configured_token.is_empty() {
        let resolved = decrypt_config_string_if_needed("telegram.bot_token", configured_token)?;
        anyhow::ensure!(
            !resolved.is_empty(),
            "action.telegram_notify requires non-empty telegram.bot_token for the current user"
        );
        return Ok(resolved);
    }

    let has_legacy_inline_token = node_config_string(node, "botToken").is_some();
    anyhow::ensure!(
        !has_legacy_inline_token,
        "action.telegram_notify requires telegram.bot_token for the current user; legacy botToken is no longer used"
    );
    Err(anyhow::anyhow!(
        "action.telegram_notify requires telegram.bot_token for the current user"
    ))
}

fn resolve_telegram_chat_id(telegram: &TelegramConfig, node: &TradeFlowNode) -> Result<String> {
    if let Some(node_chat_id) = node_config_string(node, "chatId") {
        let resolved = node_chat_id.trim().to_string();
        if !resolved.is_empty() {
            return Ok(resolved);
        }
    }

    let default_chat_id = telegram.chat_id.trim();
    anyhow::ensure!(
        !default_chat_id.is_empty(),
        "action.telegram_notify requires chatId or telegram.chat_id for the current user"
    );
    Ok(default_chat_id.to_string())
}

fn masked_prefix(value: &str, take: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let prefix: String = trimmed.chars().take(take).collect();
    format!("{prefix}***")
}

fn extract_http_status_code(raw: &str) -> Option<i32> {
    if raw.contains("401 Unauthorized") {
        return Some(401);
    }
    if raw.contains("403 Forbidden") {
        return Some(403);
    }
    if raw.contains("400 Bad Request") {
        return Some(400);
    }
    if raw.contains("429 Too Many Requests") {
        return Some(429);
    }
    None
}

fn classify_clob_error(err: &anyhow::Error) -> ClobErrorClassification {
    let raw = err.to_string().to_ascii_lowercase();
    if raw.contains("trading restricted in your region") || raw.contains("geoblock") {
        return ClobErrorClassification {
            reason_code: "geoblock_restricted",
            reason_message: "Trading is restricted in this region.",
        };
    }
    if raw.contains("unauthorized/invalid api key") || raw.contains("invalid api key") {
        return ClobErrorClassification {
            reason_code: "invalid_api_key",
            reason_message: "CLOB API key credentials are invalid.",
        };
    }
    if raw.contains("401 unauthorized") {
        return ClobErrorClassification {
            reason_code: "unauthorized",
            reason_message: "Request is unauthorized.",
        };
    }
    if raw.contains("403 forbidden") {
        return ClobErrorClassification {
            reason_code: "forbidden",
            reason_message: "Request is forbidden.",
        };
    }
    if raw.contains("400 bad request") {
        return ClobErrorClassification {
            reason_code: "bad_request",
            reason_message: "Request payload or path is invalid.",
        };
    }
    if raw.contains("timed out") || raw.contains("timeout") {
        return ClobErrorClassification {
            reason_code: "request_timeout",
            reason_message: "Request timed out.",
        };
    }
    ClobErrorClassification {
        reason_code: "clob_request_failed",
        reason_message: "CLOB request failed.",
    }
}

fn log_resolved_api_credentials(run_id: i64, creds: &ApiCredentials, source: CredentialSource) {
    info!(
        run_id,
        credential_source = source.as_str(),
        api_address = %creds.address,
        api_key_prefix = %masked_prefix(&creds.key, 8),
        api_secret_len = creds.secret.trim().len(),
        api_passphrase_len = creds.passphrase.trim().len(),
        "CLOB_CREDENTIALS_RESOLVED"
    );
}

async fn record_clob_auth_preflight_event(
    repo: &PostgresRepository,
    run_id: i64,
    decision: &str,
    reason_code: &str,
    reason_message: &str,
    source: CredentialSource,
    creds: &ApiCredentials,
    status_code: Option<i32>,
    error: Option<&str>,
) {
    let details = json!({
        "run_id": run_id,
        "decision": decision,
        "reason_code": reason_code,
        "reason_message": reason_message,
        "status_code": status_code,
        "credential_source": source.as_str(),
        "api_address": creds.address,
        "api_key_prefix": masked_prefix(&creds.key, 8),
        "api_secret_len": creds.secret.trim().len(),
        "api_passphrase_len": creds.passphrase.trim().len(),
        "error": error
    })
    .to_string();

    if let Err(err) = repo
        .record_risk_event(None, "clob_auth_preflight", decision, &details)
        .await
    {
        warn!(
            run_id,
            error = %err,
            "CLOB_AUTH_PREFLIGHT_EVENT_WRITE_FAILED"
        );
    }
}

async fn run_clob_auth_preflight(
    run_id: i64,
    repo: &PostgresRepository,
    client: &ClobHttpClient,
    creds: &ApiCredentials,
    source: CredentialSource,
) {
    match ClobRestClient::list_fills(client, None).await {
        Ok(fills) => {
            info!(
                run_id,
                status_code = 200,
                reason_code = "allow",
                reason_message = "Auth preflight succeeded.",
                credential_source = source.as_str(),
                api_address = %creds.address,
                api_key_prefix = %masked_prefix(&creds.key, 8),
                fill_count = fills.len(),
                "CLOB_AUTH_PREFLIGHT_OK"
            );
            record_clob_auth_preflight_event(
                repo,
                run_id,
                "allow",
                "allow",
                "Auth preflight succeeded.",
                source,
                creds,
                Some(200),
                None,
            )
            .await;
        }
        Err(err) => {
            let classification = classify_clob_error(&err);
            let error_text = format!("{err:#}");
            let status_code = extract_http_status_code(&error_text);
            warn!(
                run_id,
                status_code = ?status_code,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                credential_source = source.as_str(),
                api_address = %creds.address,
                api_key_prefix = %masked_prefix(&creds.key, 8),
                error = %error_text,
                "CLOB_AUTH_PREFLIGHT_FAILED"
            );
            record_clob_auth_preflight_event(
                repo,
                run_id,
                "block",
                classification.reason_code,
                classification.reason_message,
                source,
                creds,
                status_code,
                Some(error_text.as_str()),
            )
            .await;
        }
    }
}

async fn run_daily_pnl_startup_check(
    run_id: i64,
    repo: &PostgresRepository,
    max_daily_loss_usdc: f64,
) -> Result<()> {
    let daily_pnl = repo.daily_realized_pnl().await?;
    info!(
        run_id,
        daily_pnl_usdc = daily_pnl,
        max_daily_loss_usdc = max_daily_loss_usdc,
        "STARTUP_DAILY_PNL_CHECK"
    );
    if daily_pnl <= -max_daily_loss_usdc {
        anyhow::bail!(
            "Daily loss limit already breached at startup: pnl={:.2} limit={:.2}",
            daily_pnl,
            max_daily_loss_usdc
        );
    }
    Ok(())
}

async fn run_balance_preflight(
    run_id: i64,
    repo: &PostgresRepository,
    client: &ClobHttpClient,
    min_balance_usdc: f64,
) {
    match ClobRestClient::get_balance(client).await {
        Ok(balance) => {
            info!(
                run_id,
                balance_usdc = balance,
                min_balance_usdc = min_balance_usdc,
                "CLOB_BALANCE_PREFLIGHT_OK"
            );
            if balance < min_balance_usdc {
                let _ = repo
                    .record_risk_event(
                        None,
                        "balance_preflight",
                        "halt",
                        &format!("balance={:.2} < min={:.2}", balance, min_balance_usdc),
                    )
                    .await;
                panic!(
                    "Insufficient USDC balance at startup: {:.2} < {:.2}",
                    balance, min_balance_usdc
                );
            }
        }
        Err(err) => {
            warn!(run_id, error = %err, "CLOB_BALANCE_PREFLIGHT_FAILED_SKIP");
        }
    }
}

fn load_config_encryption_key() -> Result<[u8; 32]> {
    let encoded = env::var("CONFIG_ENCRYPTION_KEY")
        .context("CONFIG_ENCRYPTION_KEY is required to decrypt encrypted exchange credentials")?;
    let decoded = BASE64_STANDARD
        .decode(encoded.trim().as_bytes())
        .context("CONFIG_ENCRYPTION_KEY must be valid base64")?;

    anyhow::ensure!(
        decoded.len() == 32,
        "CONFIG_ENCRYPTION_KEY must decode to exactly 32 bytes"
    );

    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn decrypt_config_value_if_needed(
    field_name: &str,
    value: &str,
    key_material: Option<&[u8; 32]>,
) -> Result<String> {
    if !value.starts_with(CONFIG_ENC_PREFIX) {
        return Ok(value.to_string());
    }

    let key = key_material.context(
        "encrypted inline credentials found but CONFIG_ENCRYPTION_KEY was not available",
    )?;

    let payload = value.trim_start_matches(CONFIG_ENC_PREFIX);
    let decoded = BASE64_STANDARD
        .decode(payload.as_bytes())
        .with_context(|| format!("invalid encrypted payload for {field_name}"))?;

    anyhow::ensure!(
        decoded.len() > CONFIG_ENC_NONCE_LEN + CONFIG_ENC_TAG_LEN,
        "encrypted payload for {field_name} is too short"
    );

    let (nonce_bytes, encrypted_and_tag) = decoded.split_at(CONFIG_ENC_NONCE_LEN);
    let split_at = encrypted_and_tag.len() - CONFIG_ENC_TAG_LEN;
    let (ciphertext, auth_tag) = encrypted_and_tag.split_at(split_at);

    let mut combined = Vec::with_capacity(ciphertext.len() + auth_tag.len());
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(auth_tag);

    let cipher = Aes256Gcm::new_from_slice(key).context("invalid config encryption key")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), combined.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to decrypt encrypted value for {field_name}"))?;

    String::from_utf8(plaintext)
        .with_context(|| format!("decrypted value for {field_name} is not valid UTF-8"))
}
