use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use bot_core::{ExecutionMode, KillSwitchMode};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, env, fs, path::Path};

mod crypto;
mod defaults;
mod load;
mod models;
mod tests;
mod validate;

pub(crate) use crypto::decrypt_config_string_if_needed;
pub(crate) use defaults::*;
pub(crate) use load::{
    load_json_merged_with_toml, load_json_or_default, load_json_or_toml,
    load_json_or_toml_or_default, load_toml, load_toml_or_default,
};
pub use models::{
    AppConfig, BotConfig, ClaimConfig, ClaimExecutionMode, ExchangeConfig, ExecutionConfig,
    RiskConfig, StrategyConfig, TelegramConfig,
};
pub(crate) use validate::validate;
