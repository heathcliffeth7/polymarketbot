#![cfg(test)]

use super::defaults::{CONFIG_ENC_NONCE_LEN, CONFIG_ENC_PREFIX};
use super::models::{AppConfig, ClaimConfig};
use super::validate::validate_claim_private_key;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScopedConfigEncryptionKey {
    previous: Option<String>,
}

impl ScopedConfigEncryptionKey {
    fn set(encoded_key: &str) -> Self {
        let previous = env::var("CONFIG_ENCRYPTION_KEY").ok();
        env::set_var("CONFIG_ENCRYPTION_KEY", encoded_key);
        Self { previous }
    }
}

impl Drop for ScopedConfigEncryptionKey {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_deref() {
            env::set_var("CONFIG_ENCRYPTION_KEY", previous);
        } else {
            env::remove_var("CONFIG_ENCRYPTION_KEY");
        }
    }
}

fn encrypt_config_string_for_test(raw: &str) -> Result<(String, String)> {
    let key_material = [17_u8; 32];
    let encoded_key = BASE64_STANDARD.encode(key_material);
    let cipher = Aes256Gcm::new_from_slice(&key_material)?;
    let nonce_bytes = [9_u8; CONFIG_ENC_NONCE_LEN];
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, raw.as_bytes())
        .map_err(|_| anyhow::anyhow!("test encryption failed"))?;
    let encrypted = format!(
        "{CONFIG_ENC_PREFIX}{}",
        BASE64_STANDARD.encode([nonce_bytes.as_slice(), ciphertext.as_slice()].concat())
    );
    Ok((encoded_key, encrypted))
}

struct TempConfigDir {
    path: PathBuf,
}

impl TempConfigDir {
    fn new() -> Result<Self> {
        let path = env::temp_dir().join(format!("bot-infra-config-tests-{}", Uuid::new_v4()));
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempConfigDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_config(path: &Path, name: &str, contents: &str) -> Result<()> {
    fs::write(path.join(name), contents)?;
    Ok(())
}

fn write_base_app_config(dir: &Path) -> Result<()> {
    write_config(
        dir,
        "bot.toml",
        r#"
mode = "live"
market_scope = "btc_15m_updown"
market_scopes = []
loop_interval_ms = 250
market_discovery_retry_interval_ms = 5000
market_discovery_timeout_sec = 0
market_selection = "latest_by_slug"
"#,
    )?;
    write_config(
        dir,
        "strategy.toml",
        r#"
entry_price = 0.6
tp_pct = 0.12
base_sl_pct = 0.08
aggressive_sl_pct = 0.3
entry_window_sec = 180
max_hold_sec = 240
sl_renew_interval_ms = 2000
flow_only = true
dual_side_enabled = false
total_notional_usdc = 10
per_leg_initial_notional_usdc = 5
dca_interval_sec = 20
dca_step_pct = 0.02
max_dca_levels_per_leg = 3
leg_tp_pct = 0.035
basket_tp_usdc = 0.35
basket_sl_usdc = -0.6
force_flatten_sec_before_close = 45
"#,
    )?;
    write_config(
        dir,
        "risk.toml",
        r#"
max_daily_loss_usdc = 10.0
max_consecutive_losses = 2
max_notional_per_market_usdc = 20.0
max_open_orders = 20
max_stale_data_ms = 2000
kill_switch_mode = "disabled"
manual_kill_switch_active = false
min_balance_usdc = 1.0
"#,
    )?;
    write_config(
        dir,
        "execution.toml",
        r#"
order_type = "limit"
time_in_force = "GTC"
retry_count = 3
retry_backoff_ms = 400
reconcile_interval_ms = 1500
"#,
    )?;
    Ok(())
}

fn base_exchange_payload() -> serde_json::Value {
    json!({
        "gamma_base_url": "https://gamma-api.polymarket.com",
        "clob_base_url": "https://clob.polymarket.com",
        "clob_ws_url": "wss://ws-subscriptions-clob.polymarket.com/ws/",
        "chain_id": 137,
        "api_address": "0x1111111111111111111111111111111111111111",
        "api_key": "key",
        "api_secret": "secret",
        "api_passphrase": "passphrase",
        "builder_api_key": "",
        "builder_api_secret": "",
        "builder_api_passphrase": "",
        "ctf_exchange_address": "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E",
        "signer_private_key": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "signer_private_key_env": "",
        "api_address_env": "",
        "api_key_env": "",
        "api_secret_env": "",
        "api_passphrase_env": "",
        "builder_api_key_env": "",
        "builder_api_secret_env": "",
        "builder_api_passphrase_env": "",
        "gnosis_safe_address": "",
        "gnosis_safe_address_env": ""
    })
}

#[test]
fn claim_private_key_validation_ignores_disabled_encrypted_value() {
    let claim = ClaimConfig {
        enabled: false,
        private_key: "enc:v1:not-a-real-key".to_string(),
        ..ClaimConfig::default()
    };

    assert!(validate_claim_private_key(&claim).is_ok());
}

#[test]
fn claim_private_key_validation_accepts_enabled_encrypted_value() {
    let _guard = test_env_lock().lock().unwrap();
    let (encoded_key, encrypted_key) = encrypt_config_string_for_test(
        "0x1111111111111111111111111111111111111111111111111111111111111111",
    )
    .unwrap();
    let _env_guard = ScopedConfigEncryptionKey::set(&encoded_key);
    let claim = ClaimConfig {
        enabled: true,
        private_key: encrypted_key,
        ..ClaimConfig::default()
    };

    assert!(validate_claim_private_key(&claim).is_ok());
}

#[test]
fn claim_private_key_validation_rejects_invalid_decrypted_value() {
    let _guard = test_env_lock().lock().unwrap();
    let (encoded_key, encrypted_key) = encrypt_config_string_for_test("not-a-private-key").unwrap();
    let _env_guard = ScopedConfigEncryptionKey::set(&encoded_key);
    let claim = ClaimConfig {
        enabled: true,
        private_key: encrypted_key,
        ..ClaimConfig::default()
    };

    let err = validate_claim_private_key(&claim).unwrap_err();
    assert!(err
        .to_string()
        .contains("claim.private_key must be a valid 0x private key when provided"));
}

#[test]
fn app_config_load_from_user_settings_uses_claim_toml_fallback_when_payload_missing() {
    let dir = TempConfigDir::new().unwrap();
    write_base_app_config(dir.path()).unwrap();
    write_config(
        dir.path(),
        "claim.toml",
        r#"
enabled = true
execution_mode = "direct"
rpc_url = "https://polygon-rpc.com"
rpc_url_env = ""
data_api_base_url = "https://data-api.polymarket.com"
user_address = "0x2222222222222222222222222222222222222222"
user_address_env = ""
private_key = "0x2222222222222222222222222222222222222222222222222222222222222222"
private_key_env = ""
chain_id = 137
ctf_contract_address = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
discovery_interval_sec = 30
positions_page_size = 200
positions_max_pages = 5
process_batch_size = 10
max_attempts = 5
retry_backoff_ms = 10000
"#,
    )
    .unwrap();

    let settings = HashMap::from([("exchange".to_string(), base_exchange_payload())]);
    let cfg = AppConfig::load_from_user_settings(dir.path(), &settings).unwrap();

    assert!(cfg.claim.enabled);
    assert_eq!(cfg.claim.rpc_url, "https://polygon-rpc.com");
    assert_eq!(
        cfg.claim.user_address,
        "0x2222222222222222222222222222222222222222"
    );
    assert_eq!(cfg.claim.execution_mode.as_str(), "direct");
}

#[test]
fn app_config_load_from_user_settings_prefers_stored_claim_payload_over_toml() {
    let dir = TempConfigDir::new().unwrap();
    write_base_app_config(dir.path()).unwrap();
    write_config(
        dir.path(),
        "claim.toml",
        r#"
enabled = true
execution_mode = "direct"
rpc_url = "https://polygon-rpc.com"
rpc_url_env = ""
data_api_base_url = "https://data-api.polymarket.com"
user_address = "0x2222222222222222222222222222222222222222"
user_address_env = ""
private_key = "0x2222222222222222222222222222222222222222222222222222222222222222"
private_key_env = ""
chain_id = 137
ctf_contract_address = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
discovery_interval_sec = 30
positions_page_size = 200
positions_max_pages = 5
process_batch_size = 10
max_attempts = 5
retry_backoff_ms = 10000
"#,
    )
    .unwrap();

    let settings = HashMap::from([
        ("exchange".to_string(), base_exchange_payload()),
        (
            "claim".to_string(),
            json!({
                "enabled": false,
                "rpc_url": "https://db-rpc.example",
                "rpc_url_env": "",
                "data_api_base_url": "https://data-api.polymarket.com",
                "user_address": "",
                "user_address_env": "",
                "private_key": "",
                "private_key_env": "",
                "chain_id": 137,
                "ctf_contract_address": "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045",
                "collateral_token_address": "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174",
                "discovery_interval_sec": 30,
                "positions_page_size": 200,
                "positions_max_pages": 5,
                "process_batch_size": 10,
                "max_attempts": 5,
                "retry_backoff_ms": 10000
            }),
        ),
    ]);
    let cfg = AppConfig::load_from_user_settings(dir.path(), &settings).unwrap();

    assert!(!cfg.claim.enabled);
    assert_eq!(cfg.claim.rpc_url, "https://db-rpc.example");
}

#[test]
fn app_config_builder_relayer_requires_safe_address() {
    let dir = TempConfigDir::new().unwrap();
    write_base_app_config(dir.path()).unwrap();
    write_config(
        dir.path(),
        "claim.toml",
        r#"
enabled = true
execution_mode = "builder_relayer"
rpc_url = "https://polygon-rpc.com"
rpc_url_env = ""
data_api_base_url = "https://data-api.polymarket.com"
user_address = "0x2222222222222222222222222222222222222222"
user_address_env = ""
private_key = "0x2222222222222222222222222222222222222222222222222222222222222222"
private_key_env = ""
chain_id = 137
ctf_contract_address = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
discovery_interval_sec = 30
positions_page_size = 200
positions_max_pages = 5
process_batch_size = 10
max_attempts = 5
retry_backoff_ms = 10000
"#,
    )
    .unwrap();

    let settings = HashMap::from([("exchange".to_string(), base_exchange_payload())]);

    let err = AppConfig::load_from_user_settings(dir.path(), &settings).unwrap_err();
    assert!(err.to_string().contains(
        "exchange.gnosis_safe_address is required when claim.execution_mode=builder_relayer"
    ));
}

#[test]
fn app_config_builder_relayer_accepts_safe_and_builder_credentials() {
    let dir = TempConfigDir::new().unwrap();
    write_base_app_config(dir.path()).unwrap();
    write_config(
        dir.path(),
        "claim.toml",
        r#"
enabled = true
execution_mode = "builder_relayer"
rpc_url = "https://polygon-rpc.com"
rpc_url_env = ""
data_api_base_url = "https://data-api.polymarket.com"
user_address = "0x2222222222222222222222222222222222222222"
user_address_env = ""
private_key = "0x2222222222222222222222222222222222222222222222222222222222222222"
private_key_env = ""
chain_id = 137
ctf_contract_address = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
discovery_interval_sec = 30
positions_page_size = 200
positions_max_pages = 5
process_batch_size = 10
max_attempts = 5
retry_backoff_ms = 10000
"#,
    )
    .unwrap();

    let mut exchange = base_exchange_payload();
    exchange["gnosis_safe_address"] = json!("0x3333333333333333333333333333333333333333");
    exchange["builder_api_key"] = json!("builder-key");
    exchange["builder_api_secret"] = json!("builder-secret");
    exchange["builder_api_passphrase"] = json!("builder-passphrase");
    let settings = HashMap::from([("exchange".to_string(), exchange)]);

    let cfg = AppConfig::load_from_user_settings(dir.path(), &settings).unwrap();
    assert_eq!(
        cfg.claim.execution_mode().unwrap(),
        super::models::ClaimExecutionMode::BuilderRelayer
    );
    assert_eq!(
        cfg.exchange.resolve_gnosis_safe_address().as_deref(),
        Some("0x3333333333333333333333333333333333333333")
    );
}
