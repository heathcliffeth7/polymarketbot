use crate::config::AppConfig;
use crate::db::{AutoClaimJob, PostgresRepository};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use ethers::contract::abigen;
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, Bytes, H256, U256};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, env, str::FromStr, sync::Arc};
use tracing::{info, warn};

const AUTO_CLAIM_INDEX_SETS: [u64; 2] = [1, 2];
const AUTO_CLAIM_MAX_ERROR_LEN: usize = 400;

type ClaimSigner = SignerMiddleware<Provider<Http>, LocalWallet>;

abigen!(
    ConditionalTokens,
    r#"[
        function redeemPositions(address collateralToken, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets)
    ]"#,
);

abigen!(
    GnosisSafe,
    r#"[
        function execTransaction(address to, uint256 value, bytes data, uint8 operation, uint256 safeTxGas, uint256 baseGas, uint256 gasPrice, address gasToken, address refundReceiver, bytes signatures) returns (bool success)
    ]"#,
);

#[derive(Debug, Clone, Deserialize)]
struct DataApiPosition {
    #[serde(rename = "proxyWallet")]
    proxy_wallet: Option<String>,
    #[serde(rename = "conditionId")]
    condition_id: Option<String>,
    #[serde(rename = "marketSlug")]
    market_slug: Option<String>,
    slug: Option<String>,
    #[serde(rename = "currentValue")]
    current_value: Option<Value>,
    #[serde(rename = "curPrice")]
    cur_price: Option<Value>,
    redeemable: Option<bool>,
    size: Option<Value>,
    balance: Option<Value>,
}

pub struct AutoClaimService {
    signer_address: String,
    safe_address: Option<String>,
    positions_base_url: String,
    positions_page_size: i64,
    positions_max_pages: i64,
    process_batch_size: i64,
    max_attempts: i32,
    retry_backoff_ms: u64,
    discovery_interval_sec: u64,
    next_discovery_at: DateTime<Utc>,
    http: Client,
    ctf_contract: ConditionalTokens<ClaimSigner>,
    safe_contract: Option<GnosisSafe<ClaimSigner>>,
    collateral_token: Address,
}

impl AutoClaimService {
    pub fn from_app_config(cfg: &AppConfig) -> Result<Option<Self>> {
        if !cfg.claim.enabled {
            return Ok(None);
        }

        let user_address_raw = if cfg.claim.user_address.trim().is_empty() {
            env::var(&cfg.claim.user_address_env).with_context(|| {
                format!(
                    "missing env {} required for auto-claim user address",
                    cfg.claim.user_address_env
                )
            })?
        } else {
            cfg.claim.user_address.clone()
        };
        let user_address = normalize_address(&user_address_raw)?;

        let private_key = if cfg.claim.private_key.trim().is_empty() {
            env::var(&cfg.claim.private_key_env).with_context(|| {
                format!(
                    "missing env {} required for auto-claim signer private key",
                    cfg.claim.private_key_env
                )
            })?
        } else {
            cfg.claim.private_key.clone()
        };

        let rpc_url = env::var(&cfg.claim.rpc_url_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| cfg.claim.rpc_url.clone());

        let provider = Provider::<Http>::try_from(rpc_url.trim())
            .with_context(|| format!("invalid claim rpc url: {rpc_url}"))?;
        let wallet = private_key
            .parse::<LocalWallet>()
            .context("failed to parse claimer private key")?
            .with_chain_id(cfg.claim.chain_id);

        let signer_address = format!("{:#x}", wallet.address());
        anyhow::ensure!(
            signer_address == user_address,
            "claimer signer address ({signer_address}) does not match configured claim user address ({user_address})"
        );

        let middleware = Arc::new(SignerMiddleware::new(provider, wallet));
        let safe_address = cfg
            .exchange
            .resolve_gnosis_safe_address()
            .map(|raw| parse_address(&raw, "exchange.gnosis_safe_address"))
            .transpose()?;
        let ctf_contract = ConditionalTokens::new(
            parse_address(
                &cfg.claim.ctf_contract_address,
                "claim.ctf_contract_address",
            )?,
            middleware.clone(),
        );
        let safe_contract =
            safe_address.map(|address| GnosisSafe::new(address, middleware.clone()));

        Ok(Some(Self {
            signer_address: user_address,
            safe_address: safe_address.map(|address| format!("{:#x}", address)),
            positions_base_url: cfg.claim.data_api_base_url.clone(),
            positions_page_size: cfg.claim.positions_page_size.max(1),
            positions_max_pages: cfg.claim.positions_max_pages.max(1),
            process_batch_size: cfg.claim.process_batch_size.max(1),
            max_attempts: cfg.claim.max_attempts.max(1),
            retry_backoff_ms: cfg.claim.retry_backoff_ms.max(1000),
            discovery_interval_sec: cfg.claim.discovery_interval_sec.max(5),
            next_discovery_at: Utc::now(),
            http: Client::new(),
            ctf_contract,
            safe_contract,
            collateral_token: parse_address(
                &cfg.claim.collateral_token_address,
                "claim.collateral_token_address",
            )?,
        }))
    }

    pub async fn maybe_tick(&mut self, repo: &PostgresRepository) -> Result<()> {
        let now = Utc::now();
        if now >= self.next_discovery_at {
            let discovered = self.discover_redeemable_jobs(repo).await?;
            self.next_discovery_at =
                now + ChronoDuration::seconds(self.discovery_interval_sec as i64);
            if discovered > 0 {
                info!(
                    discovered,
                    user = %self.signer_address,
                    "AUTO_CLAIM_JOBS_DISCOVERED"
                );
            }
        }

        let processed = self.process_pending_jobs(repo).await?;
        if processed > 0 {
            info!(
                processed,
                user = %self.signer_address,
                "AUTO_CLAIM_JOBS_PROCESSED"
            );
        }
        Ok(())
    }

    async fn discover_redeemable_jobs(&self, repo: &PostgresRepository) -> Result<usize> {
        let mut by_condition: HashMap<(String, String), Option<String>> = HashMap::new();
        let discovery_addresses = self.discovery_addresses();

        for discovery_address in discovery_addresses {
            for page in 0..self.positions_max_pages {
                let offset = page * self.positions_page_size;
                let positions = self
                    .fetch_redeemable_positions(&discovery_address, offset)
                    .await?;
                let positions_len = positions.len();
                if positions_len == 0 {
                    break;
                }

                for position in positions {
                    if position.redeemable == Some(false) {
                        continue;
                    }
                    if parse_json_f64(position.size.as_ref())
                        .or_else(|| parse_json_f64(position.balance.as_ref()))
                        .unwrap_or(0.0)
                        <= 0.0
                    {
                        continue;
                    }
                    if !has_positive_claim_value(&position) {
                        continue;
                    }
                    let Some(raw_condition_id) = position.condition_id.as_deref() else {
                        continue;
                    };
                    let condition_id = match normalize_condition_id(raw_condition_id) {
                        Ok(value) => value,
                        Err(err) => {
                            warn!(
                                user = %self.signer_address,
                                condition_id = raw_condition_id,
                                error = %err,
                                "AUTO_CLAIM_CONDITION_ID_INVALID"
                            );
                            continue;
                        }
                    };
                    let owner_address = match normalize_position_owner_address(
                        position.proxy_wallet.as_deref(),
                        &discovery_address,
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            warn!(
                                user = %self.signer_address,
                                queried_address = discovery_address,
                                error = %err,
                                "AUTO_CLAIM_OWNER_ADDRESS_INVALID"
                            );
                            continue;
                        }
                    };
                    let market_slug = position.market_slug.or(position.slug);
                    by_condition
                        .entry((owner_address, condition_id))
                        .or_insert(market_slug);
                }

                if positions_len < self.positions_page_size as usize {
                    break;
                }
            }
        }

        let mut upserted = 0usize;
        for ((owner_address, condition_id), market_slug) in by_condition {
            if repo
                .upsert_auto_claim_job(
                    &owner_address,
                    market_slug.as_deref(),
                    &condition_id,
                    self.max_attempts,
                )
                .await?
            {
                upserted += 1;
            }
        }

        Ok(upserted)
    }

    async fn fetch_redeemable_positions(
        &self,
        discovery_address: &str,
        offset: i64,
    ) -> Result<Vec<DataApiPosition>> {
        let url = format!(
            "{}/positions",
            self.positions_base_url.trim_end_matches('/')
        );
        let limit = self.positions_page_size.to_string();
        let offset_str = offset.to_string();

        self.http
            .get(url)
            .query(&[
                ("user", discovery_address),
                ("redeemable", "true"),
                ("sizeThreshold", "0"),
                ("limit", limit.as_str()),
                ("offset", offset_str.as_str()),
            ])
            .send()
            .await
            .context("auto-claim positions request failed")?
            .error_for_status()
            .context("auto-claim positions endpoint returned error status")?
            .json::<Vec<DataApiPosition>>()
            .await
            .context("failed to parse auto-claim positions response")
    }

    async fn process_pending_jobs(&self, repo: &PostgresRepository) -> Result<usize> {
        let jobs = repo
            .list_auto_claim_jobs_for_processing(self.process_batch_size)
            .await?;
        for job in &jobs {
            self.process_single_job(repo, job).await?;
        }
        Ok(jobs.len())
    }

    async fn process_single_job(
        &self,
        repo: &PostgresRepository,
        job: &AutoClaimJob,
    ) -> Result<()> {
        repo.mark_auto_claim_job_processing(job.id).await?;
        repo.append_auto_claim_event(
            job.id,
            "processing_started",
            &json!({
                "job_id": job.id,
                "owner_address": job.owner_address,
                "condition_id": job.condition_id,
                "market_slug": job.market_slug,
                "attempt": job.attempts + 1
            }),
        )
        .await?;

        match self
            .submit_redeem_tx(&job.owner_address, &job.condition_id)
            .await
        {
            Ok(tx_hash) => {
                repo.mark_auto_claim_job_claimed(job.id, &tx_hash).await?;
                repo.append_auto_claim_event(
                    job.id,
                    "claimed",
                    &json!({
                        "job_id": job.id,
                        "condition_id": job.condition_id,
                        "tx_hash": tx_hash
                    }),
                )
                .await?;
            }
            Err(err) => {
                let compact_error = compact_error(err);
                let status = repo
                    .mark_auto_claim_job_retry_or_fail(
                        job.id,
                        &compact_error,
                        self.retry_backoff_ms as i64,
                    )
                    .await?;
                let event_type = if status == "failed" {
                    "claim_failed"
                } else {
                    "retry_scheduled"
                };
                repo.append_auto_claim_event(
                    job.id,
                    event_type,
                    &json!({
                        "job_id": job.id,
                        "condition_id": job.condition_id,
                        "status": status,
                        "error": compact_error,
                        "retry_backoff_ms": self.retry_backoff_ms
                    }),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn submit_redeem_tx(&self, owner_address: &str, condition_id: &str) -> Result<String> {
        if self.is_safe_owner_address(owner_address) {
            return self.submit_redeem_tx_via_safe(condition_id).await;
        }
        self.submit_redeem_tx_direct(condition_id).await
    }

    async fn submit_redeem_tx_direct(&self, condition_id: &str) -> Result<String> {
        let condition = parse_condition_id(condition_id)?;
        let index_sets = AUTO_CLAIM_INDEX_SETS
            .iter()
            .copied()
            .map(U256::from)
            .collect::<Vec<_>>();

        let call = self.ctf_contract.redeem_positions(
            self.collateral_token,
            [0u8; 32],
            condition.to_fixed_bytes(),
            index_sets,
        );
        let pending_tx = call
            .send()
            .await
            .with_context(|| format!("redeemPositions send failed for {condition_id}"))?;

        Ok(format!("{:#x}", pending_tx.tx_hash()))
    }

    async fn submit_redeem_tx_via_safe(&self, condition_id: &str) -> Result<String> {
        let safe_contract = self
            .safe_contract
            .as_ref()
            .context("safe contract not configured for auto-claim")?;
        let signer_address = parse_address(&self.signer_address, "claim.user_address")?;
        let condition = parse_condition_id(condition_id)?;
        let index_sets = AUTO_CLAIM_INDEX_SETS
            .iter()
            .copied()
            .map(U256::from)
            .collect::<Vec<_>>();
        let redeem_call = self.ctf_contract.redeem_positions(
            self.collateral_token,
            [0u8; 32],
            condition.to_fixed_bytes(),
            index_sets,
        );
        let redeem_calldata = redeem_call
            .calldata()
            .context("failed to build redeemPositions calldata")?;
        let signatures = build_safe_prevalidated_signature(signer_address);
        let safe_call = safe_contract.exec_transaction(
            self.ctf_contract.address(),
            U256::zero(),
            redeem_calldata,
            0u8,
            U256::zero(),
            U256::zero(),
            U256::zero(),
            Address::zero(),
            Address::zero(),
            signatures,
        );
        let simulation_ok = safe_call.clone().call().await.with_context(|| {
            format!("safe execTransaction simulation failed for {condition_id}")
        })?;
        anyhow::ensure!(
            simulation_ok,
            "safe execTransaction simulation returned false for {condition_id}"
        );
        let pending_tx = safe_call
            .send()
            .await
            .with_context(|| format!("safe execTransaction send failed for {condition_id}"))?;

        Ok(format!("{:#x}", pending_tx.tx_hash()))
    }

    fn discovery_addresses(&self) -> Vec<String> {
        let mut out = vec![self.signer_address.clone()];
        if let Some(safe_address) = &self.safe_address {
            if safe_address != &self.signer_address {
                out.push(safe_address.clone());
            }
        }
        out
    }

    fn is_safe_owner_address(&self, owner_address: &str) -> bool {
        self.safe_address
            .as_deref()
            .map(|safe| safe.eq_ignore_ascii_case(owner_address))
            .unwrap_or(false)
    }
}

fn parse_address(raw: &str, field: &str) -> Result<Address> {
    Address::from_str(raw.trim())
        .with_context(|| format!("{field} must be a valid EVM address (0x...)"))
}

fn normalize_address(raw: &str) -> Result<String> {
    let address = Address::from_str(raw.trim()).context("invalid user address for auto-claim")?;
    Ok(format!("{:#x}", address))
}

fn normalize_condition_id(raw: &str) -> Result<String> {
    let hash = parse_condition_id(raw)?;
    Ok(format!("{:#x}", hash))
}

fn parse_condition_id(raw: &str) -> Result<H256> {
    let trimmed = raw.trim();
    let prefixed = if trimmed.starts_with("0x") {
        trimmed.to_string()
    } else {
        format!("0x{trimmed}")
    };
    H256::from_str(&prefixed).with_context(|| format!("invalid condition_id: {trimmed}"))
}

fn parse_json_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(v)) => v.as_f64(),
        Some(Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}

fn normalize_position_owner_address(
    proxy_wallet: Option<&str>,
    fallback_address: &str,
) -> Result<String> {
    if let Some(proxy_wallet) = proxy_wallet {
        let trimmed = proxy_wallet.trim();
        if !trimmed.is_empty() {
            return normalize_address(trimmed);
        }
    }
    normalize_address(fallback_address)
}

fn has_positive_claim_value(position: &DataApiPosition) -> bool {
    if let Some(current_value) = parse_json_f64(position.current_value.as_ref()) {
        return current_value > 0.0;
    }
    if let Some(cur_price) = parse_json_f64(position.cur_price.as_ref()) {
        return cur_price > 0.0;
    }
    true
}

fn build_safe_prevalidated_signature(owner: Address) -> Bytes {
    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&[0u8; 12]);
    signature.extend_from_slice(owner.as_bytes());
    signature.extend_from_slice(&[0u8; 32]);
    signature.push(1u8);
    signature.into()
}

fn compact_error(err: anyhow::Error) -> String {
    let mut out = err.to_string().replace('\n', " ");
    if out.len() > AUTO_CLAIM_MAX_ERROR_LEN {
        out.truncate(AUTO_CLAIM_MAX_ERROR_LEN);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn positive_claim_value_requires_positive_current_value_or_price() {
        let winner = DataApiPosition {
            proxy_wallet: None,
            condition_id: None,
            market_slug: None,
            slug: None,
            current_value: Some(json!(5.32)),
            cur_price: Some(json!(1)),
            redeemable: Some(true),
            size: Some(json!(5.32)),
            balance: None,
        };
        assert!(has_positive_claim_value(&winner));

        let loser = DataApiPosition {
            current_value: Some(json!(0)),
            cur_price: Some(json!(0)),
            ..winner.clone()
        };
        assert!(!has_positive_claim_value(&loser));
    }

    #[test]
    fn safe_prevalidated_signature_embeds_owner_and_marker() {
        let owner = Address::from_str("0x38562e48f0e8ce1c1c7931b482d6e2145937e452").unwrap();
        let signature = build_safe_prevalidated_signature(owner);
        assert_eq!(signature.len(), 65);
        assert_eq!(&signature[12..32], owner.as_bytes());
        assert_eq!(signature[64], 1u8);
    }
}
