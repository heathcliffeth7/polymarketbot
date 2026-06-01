use anyhow::{Context, Result};
use ethers::types::U256;
use serde::{Deserialize, Serialize};
use std::env;

const CLAIM_RELAYER_ADAPTER_URL_ENV: &str = "CLAIM_RELAYER_ADAPTER_URL";
const CLAIM_MERGE_ADAPTER_URL_ENV: &str = "CLAIM_MERGE_ADAPTER_URL";
const CLAIM_FUNDS_ACTIVATION_ADAPTER_URL_ENV: &str = "CLAIM_FUNDS_ACTIVATION_ADAPTER_URL";
const CLAIM_RELAYER_ADAPTER_TOKEN_ENV: &str = "CLAIM_RELAYER_ADAPTER_TOKEN";
const DEFAULT_CLAIM_RELAYER_ADAPTER_URL: &str = "http://127.0.0.1:3000/api/internal/claim/redeem";
const DEFAULT_CLAIM_MERGE_ADAPTER_URL: &str = "http://127.0.0.1:3000/api/internal/claim/merge";
const DEFAULT_CLAIM_FUNDS_ACTIVATION_ADAPTER_URL: &str =
    "http://127.0.0.1:3000/api/internal/claim/activate-funds";

#[derive(Debug, Clone)]
pub(crate) struct ClaimRelayerAdapter {
    pub(crate) redeem_url: String,
    pub(crate) merge_url: String,
    pub(crate) activate_funds_url: String,
    pub(crate) token: String,
}

#[derive(Debug)]
pub(crate) struct SubmittedRedeemTx {
    pub(crate) tx_hash: String,
    pub(crate) gas_price: Option<U256>,
    pub(crate) submission_mode: &'static str,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ClaimSubmitFailure {
    pub(crate) retryable: bool,
    pub(crate) message: String,
}

impl ClaimSubmitFailure {
    pub(crate) fn retryable(message: impl Into<String>) -> Self {
        Self {
            retryable: true,
            message: message.into(),
        }
    }

    pub(crate) fn non_retryable(message: impl Into<String>) -> Self {
        Self {
            retryable: false,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ClaimRelayerAdapterRequest {
    #[serde(rename = "userId")]
    pub(crate) user_id: i64,
    #[serde(rename = "ownerAddress")]
    pub(crate) owner_address: String,
    #[serde(rename = "conditionId")]
    pub(crate) condition_id: String,
    #[serde(rename = "collateralToken")]
    pub(crate) collateral_token: String,
    #[serde(rename = "indexSets")]
    pub(crate) index_sets: Vec<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClaimMergeRelayerAdapterRequest {
    #[serde(rename = "userId")]
    pub(crate) user_id: i64,
    #[serde(rename = "ownerAddress")]
    pub(crate) owner_address: String,
    #[serde(rename = "conditionId")]
    pub(crate) condition_id: String,
    #[serde(rename = "collateralToken")]
    pub(crate) collateral_token: String,
    pub(crate) partition: Vec<u64>,
    #[serde(rename = "amountRaw")]
    pub(crate) amount_raw: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClaimFundsActivationAdapterRequest {
    #[serde(rename = "userId")]
    pub(crate) user_id: i64,
    #[serde(rename = "ownerAddress")]
    pub(crate) owner_address: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaimRelayerAdapterSuccess {
    #[serde(rename = "txHash")]
    pub(crate) tx_hash: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClaimFundsActivationAdapterSuccess {
    pub(crate) status: String,
    #[serde(rename = "activatedAmountUsdc")]
    pub(crate) activated_amount_usdc: f64,
    #[serde(rename = "approveTxHash")]
    pub(crate) approve_tx_hash: Option<String>,
    #[serde(rename = "wrapTxHash")]
    pub(crate) wrap_tx_hash: Option<String>,
    #[serde(rename = "usdcEBalance")]
    pub(crate) usdce_balance: f64,
    #[serde(rename = "pUsdBalance")]
    pub(crate) pusd_balance: f64,
    pub(crate) message: String,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ClaimRelayerAdapterErrorBody {
    #[serde(default)]
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) retryable: Option<bool>,
    #[serde(default)]
    pub(crate) message: String,
}

impl ClaimRelayerAdapter {
    pub(crate) fn from_env() -> Result<Self> {
        let token = env::var(CLAIM_RELAYER_ADAPTER_TOKEN_ENV)
            .with_context(|| format!("missing env {CLAIM_RELAYER_ADAPTER_TOKEN_ENV}"))?;
        anyhow::ensure!(
            !token.trim().is_empty(),
            "{CLAIM_RELAYER_ADAPTER_TOKEN_ENV} cannot be empty"
        );

        let redeem_url = env::var(CLAIM_RELAYER_ADAPTER_URL_ENV)
            .unwrap_or_else(|_| DEFAULT_CLAIM_RELAYER_ADAPTER_URL.to_string());
        anyhow::ensure!(
            redeem_url.starts_with("http://") || redeem_url.starts_with("https://"),
            "{CLAIM_RELAYER_ADAPTER_URL_ENV} must start with http:// or https://"
        );
        let merge_url = env::var(CLAIM_MERGE_ADAPTER_URL_ENV)
            .unwrap_or_else(|_| DEFAULT_CLAIM_MERGE_ADAPTER_URL.to_string());
        anyhow::ensure!(
            merge_url.starts_with("http://") || merge_url.starts_with("https://"),
            "{CLAIM_MERGE_ADAPTER_URL_ENV} must start with http:// or https://"
        );
        let activate_funds_url = env::var(CLAIM_FUNDS_ACTIVATION_ADAPTER_URL_ENV)
            .unwrap_or_else(|_| DEFAULT_CLAIM_FUNDS_ACTIVATION_ADAPTER_URL.to_string());
        anyhow::ensure!(
            activate_funds_url.starts_with("http://") || activate_funds_url.starts_with("https://"),
            "{CLAIM_FUNDS_ACTIVATION_ADAPTER_URL_ENV} must start with http:// or https://"
        );

        Ok(Self {
            redeem_url,
            merge_url,
            activate_funds_url,
            token: token.trim().to_string(),
        })
    }
}
