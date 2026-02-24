use crate::config::AppConfig;
use crate::db::{AutoClaimJob, PostgresRepository};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use ethers::contract::abigen;
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, H256, U256};
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

#[derive(Debug, Clone, Deserialize)]
struct DataApiPosition {
    #[serde(rename = "conditionId")]
    condition_id: Option<String>,
    #[serde(rename = "marketSlug")]
    market_slug: Option<String>,
    slug: Option<String>,
    redeemable: Option<bool>,
    size: Option<Value>,
    balance: Option<Value>,
}

pub struct AutoClaimService {
    user_address: String,
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
        let ctf_contract = ConditionalTokens::new(
            parse_address(
                &cfg.claim.ctf_contract_address,
                "claim.ctf_contract_address",
            )?,
            middleware,
        );

        Ok(Some(Self {
            user_address,
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
                    user = %self.user_address,
                    "AUTO_CLAIM_JOBS_DISCOVERED"
                );
            }
        }

        let processed = self.process_pending_jobs(repo).await?;
        if processed > 0 {
            info!(
                processed,
                user = %self.user_address,
                "AUTO_CLAIM_JOBS_PROCESSED"
            );
        }
        Ok(())
    }

    async fn discover_redeemable_jobs(&self, repo: &PostgresRepository) -> Result<usize> {
        let mut by_condition: HashMap<String, Option<String>> = HashMap::new();

        for page in 0..self.positions_max_pages {
            let offset = page * self.positions_page_size;
            let positions = self.fetch_redeemable_positions(offset).await?;
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
                let Some(raw_condition_id) = position.condition_id.as_deref() else {
                    continue;
                };
                let condition_id = match normalize_condition_id(raw_condition_id) {
                    Ok(value) => value,
                    Err(err) => {
                        warn!(
                            user = %self.user_address,
                            condition_id = raw_condition_id,
                            error = %err,
                            "AUTO_CLAIM_CONDITION_ID_INVALID"
                        );
                        continue;
                    }
                };
                let market_slug = position.market_slug.or(position.slug);
                by_condition.entry(condition_id).or_insert(market_slug);
            }

            if positions_len < self.positions_page_size as usize {
                break;
            }
        }

        let mut upserted = 0usize;
        for (condition_id, market_slug) in by_condition {
            if repo
                .upsert_auto_claim_job(
                    &self.user_address,
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

    async fn fetch_redeemable_positions(&self, offset: i64) -> Result<Vec<DataApiPosition>> {
        let url = format!(
            "{}/positions",
            self.positions_base_url.trim_end_matches('/')
        );
        let limit = self.positions_page_size.to_string();
        let offset_str = offset.to_string();

        self.http
            .get(url)
            .query(&[
                ("user", self.user_address.as_str()),
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
                "condition_id": job.condition_id,
                "market_slug": job.market_slug,
                "attempt": job.attempts + 1
            }),
        )
        .await?;

        match self.submit_redeem_tx(&job.condition_id).await {
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

    async fn submit_redeem_tx(&self, condition_id: &str) -> Result<String> {
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

fn compact_error(err: anyhow::Error) -> String {
    let mut out = err.to_string().replace('\n', " ");
    if out.len() > AUTO_CLAIM_MAX_ERROR_LEN {
        out.truncate(AUTO_CLAIM_MAX_ERROR_LEN);
    }
    out
}
