use anyhow::{Context, Result};
use bot_core::{ExecutionMode, KillSwitchMode};
use serde::Deserialize;
use std::{fs, path::Path};

const SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES: [(&str, &str); 8] = [
    ("btc_5m_updown", "btc-updown-5m-"),
    ("btc_15m_updown", "btc-updown-15m-"),
    ("eth_5m_updown", "eth-updown-5m-"),
    ("eth_15m_updown", "eth-updown-15m-"),
    ("sol_5m_updown", "sol-updown-5m-"),
    ("sol_15m_updown", "sol-updown-15m-"),
    ("xrp_5m_updown", "xrp-updown-5m-"),
    ("xrp_15m_updown", "xrp-updown-15m-"),
];

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
    /// Returns the list of market scopes to run.
    /// If `market_scopes` is non-empty, uses it; otherwise falls back to `market_scope`.
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
    pub gamma_base_url: String,
    pub clob_base_url: String,
    pub clob_ws_url: String,
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
    pub ctf_exchange_address: String,
    #[serde(default)]
    pub signer_private_key: String,
    #[serde(default)]
    pub signer_private_key_env: String,
    #[serde(default)]
    pub gnosis_safe_address: String,
    #[serde(default)]
    pub gnosis_safe_address_env: String,
}

impl ExchangeConfig {
    pub fn resolve_signer_private_key(&self) -> Result<String> {
        if !self.signer_private_key.is_empty() {
            return Ok(self.signer_private_key.clone());
        }
        if !self.signer_private_key_env.is_empty() {
            if let Ok(val) = std::env::var(&self.signer_private_key_env) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }
        Err(anyhow::anyhow!("signer_private_key not configured"))
    }

    pub fn resolve_gnosis_safe_address(&self) -> Option<String> {
        if !self.gnosis_safe_address.is_empty() {
            return Some(self.gnosis_safe_address.clone());
        }
        if !self.gnosis_safe_address_env.is_empty() {
            if let Ok(val) = std::env::var(&self.gnosis_safe_address_env) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        None
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
            chain_id: default_claim_chain_id(),
            ctf_contract_address: default_claim_ctf_contract_address(),
            collateral_token_address: default_claim_collateral_token_address(),
            discovery_interval_sec: default_claim_discovery_interval_sec(),
            positions_page_size: default_claim_positions_page_size(),
            positions_max_pages: default_claim_positions_max_pages(),
            process_batch_size: default_claim_process_batch_size(),
            max_attempts: default_claim_max_attempts(),
            retry_backoff_ms: default_claim_retry_backoff_ms(),
        }
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
}

impl AppConfig {
    pub fn load(dir: &Path) -> Result<Self> {
        let bot: BotConfig = load_toml(&dir.join("bot.toml"))?;
        let strategy: StrategyConfig = load_toml(&dir.join("strategy.toml"))?;
        let risk: RiskConfig = load_toml(&dir.join("risk.toml"))?;
        let execution: ExecutionConfig = load_toml(&dir.join("execution.toml"))?;
        let exchange: ExchangeConfig = load_toml(&dir.join("exchange.toml"))?;
        let claim: ClaimConfig = load_toml_or_default(&dir.join("claim.toml"))?;

        validate(&bot, &strategy, &risk, &execution, &exchange, &claim)?;

        Ok(Self {
            bot,
            strategy,
            risk,
            execution,
            exchange,
            claim,
        })
    }
}

fn default_min_balance_usdc() -> f64 {
    5.0
}

fn default_entry_price() -> f64 {
    0.60
}

fn default_market_scope() -> String {
    "btc_5m_updown".to_string()
}

fn default_market_slug_override() -> String {
    String::new()
}

fn default_loop_interval_ms() -> u64 {
    1000
}

fn default_market_discovery_retry_interval_ms() -> u64 {
    5000
}

fn default_market_discovery_timeout_sec() -> u64 {
    0
}

fn default_market_selection() -> String {
    "latest_by_slug".to_string()
}

fn default_tp_pct() -> f64 {
    0.12
}

fn default_base_sl_pct() -> f64 {
    0.08
}

fn default_aggressive_sl_pct() -> f64 {
    0.30
}

fn default_entry_window_sec() -> u64 {
    180
}

fn default_max_hold_sec() -> u64 {
    240
}

fn default_sl_renew_interval_ms() -> u64 {
    2000
}

fn default_total_notional_usdc() -> f64 {
    10.0
}

fn default_per_leg_initial_notional_usdc() -> f64 {
    5.0
}

fn default_dca_interval_sec() -> u64 {
    20
}

fn default_dca_step_pct() -> f64 {
    0.02
}

fn default_max_dca_levels_per_leg() -> u32 {
    3
}

fn default_leg_tp_pct() -> f64 {
    0.035
}

fn default_basket_tp_usdc() -> f64 {
    0.35
}

fn default_basket_sl_usdc() -> f64 {
    -0.60
}

fn default_force_flatten_sec_before_close() -> u64 {
    45
}

fn default_claim_rpc_url() -> String {
    "https://polygon-rpc.com".to_string()
}

fn default_claim_data_api_base_url() -> String {
    "https://data-api.polymarket.com".to_string()
}

fn default_claim_rpc_url_env() -> String {
    "CLAIM_RPC_URL".to_string()
}

fn default_claim_user_address_env() -> String {
    "POLYMARKET_ADDRESS".to_string()
}

fn default_claim_private_key_env() -> String {
    "CLAIMER_PRIVATE_KEY".to_string()
}

fn default_claim_chain_id() -> u64 {
    137
}

fn default_claim_ctf_contract_address() -> String {
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_string()
}

fn default_claim_collateral_token_address() -> String {
    "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()
}

fn default_claim_discovery_interval_sec() -> u64 {
    30
}

fn default_claim_positions_page_size() -> i64 {
    200
}

fn default_claim_positions_max_pages() -> i64 {
    5
}

fn default_claim_process_batch_size() -> i64 {
    10
}

fn default_claim_max_attempts() -> i32 {
    5
}

fn default_claim_retry_backoff_ms() -> u64 {
    10_000
}

fn supported_market_scope_names_csv() -> String {
    SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES
        .iter()
        .map(|(scope, _)| *scope)
        .collect::<Vec<_>>()
        .join(", ")
}

fn load_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str::<T>(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn load_toml_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    load_toml(path)
}

fn validate(
    bot: &BotConfig,
    strategy: &StrategyConfig,
    risk: &RiskConfig,
    execution: &ExecutionConfig,
    exchange: &ExchangeConfig,
    claim: &ClaimConfig,
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
    if !claim.user_address.trim().is_empty() {
        anyhow::ensure!(
            is_hex_address(&claim.user_address),
            "claim.user_address must be a valid 0x address when provided"
        );
    }
    if !claim.private_key.trim().is_empty() {
        anyhow::ensure!(
            is_hex_private_key(&claim.private_key),
            "claim.private_key must be a valid 0x private key when provided"
        );
    }
    if claim.enabled {
        anyhow::ensure!(
            !claim.rpc_url_env.trim().is_empty(),
            "claim.rpc_url_env is required when claim.enabled=true"
        );
        anyhow::ensure!(
            !claim.user_address.trim().is_empty() || !claim.user_address_env.trim().is_empty(),
            "claim.user_address or claim.user_address_env is required when claim.enabled=true"
        );
        anyhow::ensure!(
            !claim.private_key.trim().is_empty() || !claim.private_key_env.trim().is_empty(),
            "claim.private_key or claim.private_key_env is required when claim.enabled=true"
        );
    }

    anyhow::ensure!(
        !matches!(risk.kill_switch_mode, KillSwitchMode::Disabled)
            || !risk.manual_kill_switch_active,
        "manual_kill_switch_active cannot be true when kill_switch_mode is disabled"
    );
    Ok(())
}

fn is_hex_address(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with("0x")
        && trimmed.len() == 42
        && trimmed[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_hex_private_key(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with("0x")
        && trimmed.len() == 66
        && trimmed[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}
