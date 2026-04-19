use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimExecutionMode {
    Direct,
    BuilderRelayer,
    RelayerApiKey,
}

impl ClaimExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::BuilderRelayer => "builder_relayer",
            Self::RelayerApiKey => "relayer_api_key",
        }
    }

    pub fn from_config_value(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "direct" => Ok(Self::Direct),
            "builder_relayer" => Ok(Self::BuilderRelayer),
            "relayer_api_key" => Ok(Self::RelayerApiKey),
            other => Err(anyhow::anyhow!(
                "claim.execution_mode must be one of: direct, builder_relayer, relayer_api_key (got {other})"
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    pub mode: ExecutionMode,
    #[serde(default = "default_market_scope")]
    pub market_scope: String,
    #[serde(default)]
    pub market_scopes: Vec<String>,
    #[serde(default = "default_market_slug_override")]
    pub market_slug_override: String,
    #[serde(default = "default_loop_interval_ms")]
    pub loop_interval_ms: u64,
    #[serde(default = "default_market_discovery_retry_interval_ms")]
    pub market_discovery_retry_interval_ms: u64,
    #[serde(default = "default_market_discovery_timeout_sec")]
    pub market_discovery_timeout_sec: u64,
    #[serde(default = "default_market_selection")]
    pub market_selection: String,
}

impl BotConfig {
    pub fn resolve_scopes(&self) -> Vec<String> {
        if !self.market_scopes.is_empty() {
            self.market_scopes.clone()
        } else {
            vec![self.market_scope.clone()]
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfig {
    #[serde(default = "default_entry_price")]
    pub entry_price: f64,
    #[serde(default = "default_tp_pct")]
    pub tp_pct: f64,
    #[serde(default = "default_base_sl_pct")]
    pub base_sl_pct: f64,
    #[serde(default = "default_aggressive_sl_pct")]
    pub aggressive_sl_pct: f64,
    #[serde(default = "default_entry_window_sec")]
    pub entry_window_sec: u64,
    #[serde(default = "default_max_hold_sec")]
    pub max_hold_sec: u64,
    #[serde(default = "default_sl_renew_interval_ms")]
    pub sl_renew_interval_ms: u64,
    #[serde(default)]
    pub flow_only: bool,
    #[serde(default)]
    pub dual_side_enabled: bool,
    #[serde(default = "default_total_notional_usdc")]
    pub total_notional_usdc: f64,
    #[serde(default = "default_per_leg_initial_notional_usdc")]
    pub per_leg_initial_notional_usdc: f64,
    #[serde(default = "default_dca_interval_sec")]
    pub dca_interval_sec: u64,
    #[serde(default = "default_dca_step_pct")]
    pub dca_step_pct: f64,
    #[serde(default = "default_max_dca_levels_per_leg")]
    pub max_dca_levels_per_leg: u32,
    #[serde(default = "default_leg_tp_pct")]
    pub leg_tp_pct: f64,
    #[serde(default = "default_basket_tp_usdc")]
    pub basket_tp_usdc: f64,
    #[serde(default = "default_basket_sl_usdc")]
    pub basket_sl_usdc: f64,
    #[serde(default = "default_force_flatten_sec_before_close")]
    pub force_flatten_sec_before_close: u64,
    #[serde(default = "default_sl_bid_confirm_timeout_ms")]
    pub sl_bid_confirm_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    pub max_daily_loss_usdc: f64,
    pub max_consecutive_losses: u32,
    pub max_notional_per_market_usdc: f64,
    pub max_open_orders: u32,
    pub max_stale_data_ms: u64,
    pub kill_switch_mode: KillSwitchMode,
    pub manual_kill_switch_active: bool,
    #[serde(default = "default_min_balance_usdc")]
    pub min_balance_usdc: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    pub order_type: String,
    pub time_in_force: String,
    pub retry_count: u32,
    pub retry_backoff_ms: u64,
    pub reconcile_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeConfig {
    #[serde(default = "default_exchange_gamma_base_url")]
    pub gamma_base_url: String,
    #[serde(default = "default_exchange_clob_base_url")]
    pub clob_base_url: String,
    #[serde(default = "default_exchange_clob_ws_url")]
    pub clob_ws_url: String,
    #[serde(default = "default_exchange_chain_id")]
    pub chain_id: u64,
    #[serde(default)]
    pub api_address: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
    #[serde(default)]
    pub api_passphrase: String,
    pub api_address_env: String,
    pub api_key_env: String,
    pub api_secret_env: String,
    pub api_passphrase_env: String,
    #[serde(default)]
    pub builder_api_key_env: String,
    #[serde(default)]
    pub builder_api_secret_env: String,
    #[serde(default)]
    pub builder_api_passphrase_env: String,
    #[serde(default)]
    pub ctf_exchange_address: String,
    #[serde(default = "default_neg_risk_ctf_exchange_address")]
    pub neg_risk_ctf_exchange_address: String,
    #[serde(default)]
    pub signer_private_key: String,
    #[serde(default)]
    pub signer_private_key_env: String,
    #[serde(default)]
    pub gnosis_safe_address: String,
    #[serde(default)]
    pub gnosis_safe_address_env: String,
    #[serde(default)]
    pub builder_api_key: String,
    #[serde(default)]
    pub builder_api_secret: String,
    #[serde(default)]
    pub builder_api_passphrase: String,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            gamma_base_url: default_exchange_gamma_base_url(),
            clob_base_url: default_exchange_clob_base_url(),
            clob_ws_url: default_exchange_clob_ws_url(),
            chain_id: default_exchange_chain_id(),
            api_address: String::new(),
            api_key: String::new(),
            api_secret: String::new(),
            api_passphrase: String::new(),
            api_address_env: String::new(),
            api_key_env: String::new(),
            api_secret_env: String::new(),
            api_passphrase_env: String::new(),
            builder_api_key_env: String::new(),
            builder_api_secret_env: String::new(),
            builder_api_passphrase_env: String::new(),
            ctf_exchange_address: default_exchange_ctf_exchange_address(),
            neg_risk_ctf_exchange_address: default_neg_risk_ctf_exchange_address(),
            signer_private_key: String::new(),
            signer_private_key_env: String::new(),
            gnosis_safe_address: String::new(),
            gnosis_safe_address_env: String::new(),
            builder_api_key: String::new(),
            builder_api_secret: String::new(),
            builder_api_passphrase: String::new(),
        }
    }
}

impl ExchangeConfig {
    pub fn resolve_signer_private_key(&self) -> Result<String> {
        if !self.signer_private_key.is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.signer_private_key",
                &self.signer_private_key,
            );
        }
        if !self.signer_private_key_env.is_empty() {
            if let Ok(val) = env::var(&self.signer_private_key_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        Err(anyhow::anyhow!("signer_private_key not configured"))
    }

    pub fn resolve_gnosis_safe_address(&self) -> Option<String> {
        if !self.gnosis_safe_address.is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.gnosis_safe_address",
                &self.gnosis_safe_address,
            )
            .ok()
            .filter(|value| !value.is_empty());
        }
        if !self.gnosis_safe_address_env.is_empty() {
            if let Ok(val) = env::var(&self.gnosis_safe_address_env) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        None
    }

    pub fn resolve_builder_api_key(&self) -> Result<String> {
        if !self.builder_api_key.trim().is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.builder_api_key",
                &self.builder_api_key,
            );
        }
        if !self.builder_api_key_env.is_empty() {
            if let Ok(val) = env::var(&self.builder_api_key_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        if !self.api_key.trim().is_empty() {
            return decrypt_config_string_if_needed("exchange.api_key", &self.api_key);
        }
        if !self.api_key_env.is_empty() {
            if let Ok(val) = env::var(&self.api_key_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        Err(anyhow::anyhow!("builder_api_key not configured"))
    }

    pub fn resolve_builder_api_secret(&self) -> Result<String> {
        if !self.builder_api_secret.trim().is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.builder_api_secret",
                &self.builder_api_secret,
            );
        }
        if !self.builder_api_secret_env.is_empty() {
            if let Ok(val) = env::var(&self.builder_api_secret_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        if !self.api_secret.trim().is_empty() {
            return decrypt_config_string_if_needed("exchange.api_secret", &self.api_secret);
        }
        if !self.api_secret_env.is_empty() {
            if let Ok(val) = env::var(&self.api_secret_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        Err(anyhow::anyhow!("builder_api_secret not configured"))
    }

    pub fn resolve_builder_api_passphrase(&self) -> Result<String> {
        if !self.builder_api_passphrase.trim().is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.builder_api_passphrase",
                &self.builder_api_passphrase,
            );
        }
        if !self.builder_api_passphrase_env.is_empty() {
            if let Ok(val) = env::var(&self.builder_api_passphrase_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        if !self.api_passphrase.trim().is_empty() {
            return decrypt_config_string_if_needed(
                "exchange.api_passphrase",
                &self.api_passphrase,
            );
        }
        if !self.api_passphrase_env.is_empty() {
            if let Ok(val) = env::var(&self.api_passphrase_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        Err(anyhow::anyhow!("builder_api_passphrase not configured"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClaimConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_claim_rpc_url")]
    pub rpc_url: String,
    #[serde(default = "default_claim_rpc_url_env")]
    pub rpc_url_env: String,
    #[serde(default = "default_claim_data_api_base_url")]
    pub data_api_base_url: String,
    #[serde(default)]
    pub user_address: String,
    #[serde(default = "default_claim_user_address_env")]
    pub user_address_env: String,
    #[serde(default)]
    pub private_key: String,
    #[serde(default = "default_claim_private_key_env")]
    pub private_key_env: String,
    #[serde(default = "default_claim_execution_mode")]
    pub execution_mode: String,
    #[serde(default = "default_claim_chain_id")]
    pub chain_id: u64,
    #[serde(default = "default_claim_ctf_contract_address")]
    pub ctf_contract_address: String,
    #[serde(default = "default_claim_collateral_token_address")]
    pub collateral_token_address: String,
    #[serde(default = "default_claim_discovery_interval_sec")]
    pub discovery_interval_sec: u64,
    #[serde(default = "default_claim_positions_page_size")]
    pub positions_page_size: i64,
    #[serde(default = "default_claim_positions_max_pages")]
    pub positions_max_pages: i64,
    #[serde(default = "default_claim_process_batch_size")]
    pub process_batch_size: i64,
    #[serde(default = "default_claim_max_attempts")]
    pub max_attempts: i32,
    #[serde(default = "default_claim_retry_backoff_ms")]
    pub retry_backoff_ms: u64,
    #[serde(default = "default_claim_min_claim_usdc")]
    pub min_claim_usdc: f64,
}

impl Default for ClaimConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rpc_url: default_claim_rpc_url(),
            rpc_url_env: default_claim_rpc_url_env(),
            data_api_base_url: default_claim_data_api_base_url(),
            user_address: String::new(),
            user_address_env: default_claim_user_address_env(),
            private_key: String::new(),
            private_key_env: default_claim_private_key_env(),
            execution_mode: default_claim_execution_mode(),
            chain_id: default_claim_chain_id(),
            ctf_contract_address: default_claim_ctf_contract_address(),
            collateral_token_address: default_claim_collateral_token_address(),
            discovery_interval_sec: default_claim_discovery_interval_sec(),
            positions_page_size: default_claim_positions_page_size(),
            positions_max_pages: default_claim_positions_max_pages(),
            process_batch_size: default_claim_process_batch_size(),
            max_attempts: default_claim_max_attempts(),
            retry_backoff_ms: default_claim_retry_backoff_ms(),
            min_claim_usdc: default_claim_min_claim_usdc(),
        }
    }
}

impl ClaimConfig {
    pub fn execution_mode(&self) -> Result<ClaimExecutionMode> {
        ClaimExecutionMode::from_config_value(&self.execution_mode)
    }

    pub fn resolve_user_address(&self) -> Result<String> {
        if !self.user_address.trim().is_empty() {
            return decrypt_config_string_if_needed("claim.user_address", &self.user_address);
        }
        if !self.user_address_env.trim().is_empty() {
            return env::var(&self.user_address_env).with_context(|| {
                format!(
                    "missing env {} required for auto-claim user address",
                    self.user_address_env
                )
            });
        }
        Err(anyhow::anyhow!("claim.user_address not configured"))
    }

    pub fn resolve_private_key(&self) -> Result<String> {
        if !self.private_key.trim().is_empty() {
            return decrypt_config_string_if_needed("claim.private_key", &self.private_key);
        }
        if !self.private_key_env.trim().is_empty() {
            return env::var(&self.private_key_env).with_context(|| {
                format!(
                    "missing env {} required for auto-claim signer private key",
                    self.private_key_env
                )
            });
        }
        Err(anyhow::anyhow!("claim.private_key not configured"))
    }

    pub fn resolve_rpc_url(&self) -> Result<String> {
        if !self.rpc_url.trim().is_empty() {
            return Ok(self.rpc_url.trim().to_string());
        }
        if !self.rpc_url_env.trim().is_empty() {
            return env::var(&self.rpc_url_env).with_context(|| {
                format!(
                    "missing env {} required for claim rpc url",
                    self.rpc_url_env
                )
            });
        }
        Err(anyhow::anyhow!("claim.rpc_url not configured"))
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TelegramConfig {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub chat_id: String,
}

impl TelegramConfig {
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        load_toml_or_default(&dir.join("telegram.toml"))
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bot: BotConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub execution: ExecutionConfig,
    pub exchange: ExchangeConfig,
    pub claim: ClaimConfig,
    pub telegram: TelegramConfig,
}

impl AppConfig {
    pub fn load(dir: &Path) -> Result<Self> {
        let bot: BotConfig = load_toml(&dir.join("bot.toml"))?;
        let strategy: StrategyConfig = load_toml(&dir.join("strategy.toml"))?;
        let risk: RiskConfig = load_toml(&dir.join("risk.toml"))?;
        let execution: ExecutionConfig = load_toml(&dir.join("execution.toml"))?;
        let exchange: ExchangeConfig = load_toml(&dir.join("exchange.toml"))?;
        let claim: ClaimConfig = load_toml_or_default(&dir.join("claim.toml"))?;
        let telegram = TelegramConfig::load_from_dir(dir)?;

        validate(
            &bot, &strategy, &risk, &execution, &exchange, &claim, &telegram,
        )?;

        Ok(Self {
            bot,
            strategy,
            risk,
            execution,
            exchange,
            claim,
            telegram,
        })
    }

    pub fn load_from_user_settings(dir: &Path, settings: &HashMap<String, Value>) -> Result<Self> {
        let bot: BotConfig = load_json_or_toml(settings.get("bot"), &dir.join("bot.toml"))?;
        let strategy: StrategyConfig =
            load_json_or_toml(settings.get("strategy"), &dir.join("strategy.toml"))?;
        let risk: RiskConfig = load_json_or_toml(settings.get("risk"), &dir.join("risk.toml"))?;
        let execution: ExecutionConfig =
            load_json_or_toml(settings.get("execution"), &dir.join("execution.toml"))?;
        let exchange: ExchangeConfig =
            load_json_merged_with_toml(settings.get("exchange"), &dir.join("exchange.toml"))?;
        let claim: ClaimConfig =
            load_json_merged_with_toml(settings.get("claim"), &dir.join("claim.toml"))?;
        let telegram: TelegramConfig = load_json_or_default(settings.get("telegram"))?;

        validate(
            &bot, &strategy, &risk, &execution, &exchange, &claim, &telegram,
        )?;

        Ok(Self {
            bot,
            strategy,
            risk,
            execution,
            exchange,
            claim,
            telegram,
        })
    }
}
