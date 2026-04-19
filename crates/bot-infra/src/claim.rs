use crate::claim_relayer::{
    ClaimRelayerAdapter, ClaimRelayerAdapterErrorBody, ClaimRelayerAdapterRequest,
    ClaimRelayerAdapterSuccess, ClaimSubmitFailure, SubmittedRedeemTx,
};
use crate::config::{AppConfig, ClaimExecutionMode};
use crate::db::{AutoClaimJob, PostgresRepository};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use ethers::contract::abigen;
use ethers::middleware::{NonceManagerMiddleware, SignerMiddleware};
use ethers::providers::{Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{Address, BlockNumber, H256, U256};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tracing::{info, warn};

mod batch;
mod support;
use support::{
    apply_gas_price_floor_and_buffer, build_safe_prevalidated_signature, compact_error,
    compact_submit_failure, elapsed_seconds_since, gas_price_gwei, has_receipt_timed_out,
    max_inflight_claim_jobs, meets_min_claim_value, normalize_position_owner_address,
    parse_json_f64, relayer_rate_limit_cooldown_until, submit_failure_indicates_rate_limit,
};

pub(self) const AUTO_CLAIM_INDEX_SETS: [u64; 2] = [1, 2];
const AUTO_CLAIM_MAX_ERROR_LEN: usize = 400;
const RECEIPT_TIMEOUT_SEC: u64 = 120;
const MIN_GAS_PRICE_GWEI: u64 = 30;
const MIN_PRIORITY_FEE_GWEI: u64 = 30;
const RECEIPT_CHECK_BATCH_LIMIT: i64 = 50;

type ClaimSigner = SignerMiddleware<NonceManagerMiddleware<Provider<Http>>, LocalWallet>;

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
    user_id: i64,
    signer_address: String,
    safe_address: Option<String>,
    execution_mode: ClaimExecutionMode,
    relayer_adapter: Option<ClaimRelayerAdapter>,
    positions_base_url: String,
    positions_page_size: i64,
    positions_max_pages: i64,
    process_batch_size: i64,
    max_attempts: i32,
    retry_backoff_ms: u64,
    min_claim_usdc: f64,
    discovery_interval_sec: u64,
    next_discovery_at: DateTime<Utc>,
    next_submission_at: DateTime<Utc>,
    http: Client,
    middleware: Arc<ClaimSigner>,
    ctf_contract: ConditionalTokens<ClaimSigner>,
    safe_contract: Option<GnosisSafe<ClaimSigner>>,
    collateral_token: Address,
    needs_processing_recovery: bool,
}

impl AutoClaimService {
    pub fn from_app_config(user_id: i64, cfg: &AppConfig) -> Result<Option<Self>> {
        if !cfg.claim.enabled {
            return Ok(None);
        }

        let user_address_raw = cfg.claim.resolve_user_address()?;
        let user_address = normalize_address(&user_address_raw)?;

        let private_key = cfg.claim.resolve_private_key()?;
        let rpc_url = cfg.claim.resolve_rpc_url()?;

        let provider = Provider::<Http>::try_from(rpc_url.trim())
            .with_context(|| format!("invalid claim rpc url: {rpc_url}"))?;
        let wallet = private_key
            .parse::<LocalWallet>()
            .context("failed to parse claimer private key")?
            .with_chain_id(cfg.claim.chain_id);
        let wallet_address = wallet.address();

        let signer_address = format!("{:#x}", wallet_address);
        anyhow::ensure!(
            signer_address == user_address,
            "claimer signer address ({signer_address}) does not match configured claim user address ({user_address})"
        );

        let nonce_manager = NonceManagerMiddleware::new(provider, wallet_address);
        let middleware = Arc::new(SignerMiddleware::new(nonce_manager, wallet));
        let safe_address = cfg
            .exchange
            .resolve_gnosis_safe_address()
            .map(|raw| parse_address(&raw, "exchange.gnosis_safe_address"))
            .transpose()?;
        let execution_mode = cfg.claim.execution_mode()?;
        let relayer_adapter = if matches!(
            execution_mode,
            ClaimExecutionMode::BuilderRelayer | ClaimExecutionMode::RelayerApiKey
        ) {
            anyhow::ensure!(
                safe_address.is_some(),
                "exchange.gnosis_safe_address is required when claim.execution_mode=builder_relayer or relayer_api_key"
            );
            Some(ClaimRelayerAdapter::from_env()?)
        } else {
            None
        };
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
            user_id,
            signer_address: user_address,
            safe_address: safe_address.map(|address| format!("{:#x}", address)),
            execution_mode,
            relayer_adapter,
            positions_base_url: cfg.claim.data_api_base_url.clone(),
            positions_page_size: cfg.claim.positions_page_size.max(1),
            positions_max_pages: cfg.claim.positions_max_pages.max(1),
            process_batch_size: cfg.claim.process_batch_size.max(1),
            max_attempts: cfg.claim.max_attempts.max(1),
            retry_backoff_ms: cfg.claim.retry_backoff_ms.max(1000),
            min_claim_usdc: cfg.claim.min_claim_usdc.max(0.0),
            discovery_interval_sec: cfg.claim.discovery_interval_sec.max(5),
            next_discovery_at: Utc::now(),
            next_submission_at: Utc::now(),
            http: Client::new(),
            middleware: middleware.clone(),
            ctf_contract,
            safe_contract,
            collateral_token: parse_address(
                &cfg.claim.collateral_token_address,
                "claim.collateral_token_address",
            )?,
            needs_processing_recovery: true,
        }))
    }

    pub async fn maybe_tick(&mut self, repo: &PostgresRepository) -> Result<()> {
        if self.needs_processing_recovery {
            let recovered = repo.recover_stale_processing_auto_claim_jobs(5).await?;
            if recovered > 0 {
                info!(recovered, user = %self.signer_address, "AUTO_CLAIM_STALE_PROCESSING_RECOVERED");
            }
            self.needs_processing_recovery = false;
        }
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

        let confirmed = self.check_submitted_receipts(repo).await?;
        if confirmed > 0 {
            info!(
                confirmed,
                user = %self.signer_address,
                "AUTO_CLAIM_RECEIPTS_CONFIRMED"
            );
        }

        let max_inflight = max_inflight_claim_jobs(self.execution_mode, self.process_batch_size);
        let inflight_count = repo.count_auto_claim_jobs_submitted().await?;
        let available_slots = max_inflight.saturating_sub(inflight_count);
        if available_slots > 0 && now >= self.next_submission_at {
            let processed = self.process_pending_jobs(repo, available_slots).await?;
            if processed > 0 {
                info!(
                    processed,
                    user = %self.signer_address,
                    "AUTO_CLAIM_JOBS_PROCESSED"
                );
            }
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
                    if !meets_min_claim_value(&position, self.min_claim_usdc) {
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

    // process_pending_jobs and process_single_job are in batch.rs

    async fn check_submitted_receipts(&self, repo: &PostgresRepository) -> Result<usize> {
        let jobs = repo
            .list_auto_claim_jobs_for_receipt_check(RECEIPT_CHECK_BATCH_LIMIT)
            .await?;
        let now = Utc::now();
        let mut confirmed = 0usize;

        for job in &jobs {
            let Some(tx_hash_raw) = job.tx_hash.as_deref() else {
                let error_chain =
                    compact_error(anyhow::anyhow!("submitted auto-claim job missing tx_hash"));
                self.handle_submitted_job_failure(
                    repo,
                    job,
                    "receipt_failed",
                    error_chain,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
                continue;
            };

            let tx_hash = match parse_tx_hash(tx_hash_raw) {
                Ok(tx_hash) => tx_hash,
                Err(err) => {
                    let error_chain = compact_error(err);
                    self.handle_submitted_job_failure(
                        repo,
                        job,
                        "receipt_failed",
                        error_chain,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await?;
                    continue;
                }
            };

            let receipt = match self
                .middleware
                .get_transaction_receipt(tx_hash)
                .await
                .with_context(|| format!("transaction receipt lookup failed for {tx_hash_raw}"))
            {
                Ok(receipt) => receipt,
                Err(err) => {
                    let error_chain = compact_error(err);
                    warn!(
                        user = %self.signer_address,
                        job_id = job.id,
                        tx_hash = tx_hash_raw,
                        error = %error_chain,
                        "AUTO_CLAIM_RECEIPT_LOOKUP_FAILED"
                    );
                    continue;
                }
            };

            if let Some(receipt) = receipt {
                let receipt_status = receipt.status.map(|value| value.as_u64());
                let block_number = receipt.block_number.map(|value| value.as_u64());
                if receipt_status == Some(1) {
                    repo.mark_auto_claim_job_receipt_confirmed(job.id).await?;
                    repo.append_auto_claim_event(
                        job.id,
                        "receipt_confirmed",
                        &json!({
                            "job_id": job.id,
                            "condition_id": job.condition_id,
                            "tx_hash": tx_hash_raw,
                            "receipt_status": receipt_status,
                            "block_number": block_number
                        }),
                    )
                    .await?;
                    confirmed += 1;
                } else {
                    let error_chain = compact_error(anyhow::anyhow!(
                        "transaction receipt status was {:?} for {tx_hash_raw}",
                        receipt_status
                    ));
                    self.handle_submitted_job_failure(
                        repo,
                        job,
                        "receipt_failed",
                        error_chain,
                        receipt_status,
                        block_number,
                        None,
                        None,
                    )
                    .await?;
                }
                continue;
            }

            let submitted_at = job.submitted_at.unwrap_or(job.updated_at);
            let elapsed_sec = elapsed_seconds_since(submitted_at, now);
            if has_receipt_timed_out(submitted_at, now) {
                let error_chain = compact_error(anyhow::anyhow!(
                    "transaction receipt not found within {} seconds for {tx_hash_raw}",
                    RECEIPT_TIMEOUT_SEC
                ));
                self.handle_submitted_job_failure(
                    repo,
                    job,
                    "receipt_timeout",
                    error_chain,
                    None,
                    None,
                    Some(elapsed_sec),
                    Some(RECEIPT_TIMEOUT_SEC),
                )
                .await?;
            }
        }

        Ok(confirmed)
    }

    async fn handle_submitted_job_failure(
        &self,
        repo: &PostgresRepository,
        job: &AutoClaimJob,
        event_type: &str,
        error_chain: String,
        receipt_status: Option<u64>,
        block_number: Option<u64>,
        elapsed_sec: Option<i64>,
        timeout_sec: Option<u64>,
    ) -> Result<()> {
        let status = repo
            .mark_auto_claim_job_retry_or_fail(job.id, &error_chain, self.retry_backoff_ms as i64)
            .await?;
        repo.append_auto_claim_event(
            job.id,
            event_type,
            &json!({
                "job_id": job.id,
                "condition_id": job.condition_id,
                "status": status,
                "tx_hash": job.tx_hash,
                "receipt_status": receipt_status,
                "block_number": block_number,
                "timeout_sec": timeout_sec,
                "elapsed_sec": elapsed_sec,
                "error_chain": error_chain,
                "retry_backoff_ms": self.retry_backoff_ms
            }),
        )
        .await?;
        Ok(())
    }

    async fn effective_gas_price(&self) -> Result<U256> {
        let gas_price = self
            .middleware
            .get_gas_price()
            .await
            .context("failed to fetch gas price for claim tx")?;

        // Fetch base fee so gasPrice - baseFee (= priority fee) meets Polygon's minimum.
        // Legacy txs are converted by the node: maxPriorityFeePerGas = gasPrice - baseFee.
        let base_fee = self
            .middleware
            .get_block(BlockNumber::Latest)
            .await
            .ok()
            .flatten()
            .and_then(|block| block.base_fee_per_gas)
            .unwrap_or(U256::zero());

        let min_priority = U256::from(MIN_PRIORITY_FEE_GWEI) * U256::from(1_000_000_000u64);
        let base_plus_priority = base_fee + min_priority;

        Ok(apply_gas_price_floor_and_buffer(
            gas_price.max(base_plus_priority),
        ))
    }

    async fn submit_redeem_tx(
        &self,
        owner_address: &str,
        condition_id: &str,
    ) -> std::result::Result<SubmittedRedeemTx, ClaimSubmitFailure> {
        if matches!(
            self.execution_mode,
            ClaimExecutionMode::BuilderRelayer | ClaimExecutionMode::RelayerApiKey
        ) {
            return self
                .submit_redeem_tx_via_relayer(owner_address, condition_id)
                .await;
        }
        if self.is_safe_owner_address(owner_address) {
            return self.submit_redeem_tx_via_safe(condition_id).await;
        }
        self.submit_redeem_tx_direct(condition_id).await
    }

    async fn submit_redeem_tx_direct(
        &self,
        condition_id: &str,
    ) -> std::result::Result<SubmittedRedeemTx, ClaimSubmitFailure> {
        let condition = parse_condition_id(condition_id).map_err(|err| {
            ClaimSubmitFailure::non_retryable(format!(
                "invalid auto-claim condition id {condition_id}: {err:#}"
            ))
        })?;
        let index_sets = AUTO_CLAIM_INDEX_SETS
            .iter()
            .copied()
            .map(U256::from)
            .collect::<Vec<_>>();
        let gas_price = self.effective_gas_price().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "failed to fetch gas price for redeemPositions {condition_id}: {err:#}"
            ))
        })?;

        let call = self.ctf_contract.redeem_positions(
            self.collateral_token,
            [0u8; 32],
            condition.to_fixed_bytes(),
            index_sets,
        );
        let call = call.gas_price(gas_price);
        let pending_tx = call.send().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "redeemPositions send failed for {condition_id}: {err:#}"
            ))
        })?;

        Ok(SubmittedRedeemTx {
            tx_hash: format!("{:#x}", pending_tx.tx_hash()),
            gas_price: Some(gas_price),
            submission_mode: "direct",
        })
    }

    async fn submit_redeem_tx_via_safe(
        &self,
        condition_id: &str,
    ) -> std::result::Result<SubmittedRedeemTx, ClaimSubmitFailure> {
        let safe_contract = self.safe_contract.as_ref().ok_or_else(|| {
            ClaimSubmitFailure::non_retryable("safe contract not configured for auto-claim")
        })?;
        let signer_address =
            parse_address(&self.signer_address, "claim.user_address").map_err(|err| {
                ClaimSubmitFailure::non_retryable(format!(
                    "invalid claim.user_address for safe auto-claim: {err:#}"
                ))
            })?;
        let condition = parse_condition_id(condition_id).map_err(|err| {
            ClaimSubmitFailure::non_retryable(format!(
                "invalid auto-claim condition id {condition_id}: {err:#}"
            ))
        })?;
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
        let redeem_calldata = redeem_call.calldata().ok_or_else(|| {
            ClaimSubmitFailure::non_retryable(format!(
                "failed to build redeemPositions calldata for {condition_id}"
            ))
        })?;
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
        let simulation_ok = safe_call.clone().call().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "safe execTransaction simulation failed for {condition_id}: {err:#}"
            ))
        })?;
        if !simulation_ok {
            return Err(ClaimSubmitFailure::retryable(format!(
                "safe execTransaction simulation returned false for {condition_id}"
            )));
        }
        let gas_price = self.effective_gas_price().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "failed to fetch gas price for safe redeem {condition_id}: {err:#}"
            ))
        })?;
        let safe_call = safe_call.gas_price(gas_price);
        let pending_tx = safe_call.send().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "safe execTransaction send failed for {condition_id}: {err:#}"
            ))
        })?;

        Ok(SubmittedRedeemTx {
            tx_hash: format!("{:#x}", pending_tx.tx_hash()),
            gas_price: Some(gas_price),
            submission_mode: "safe",
        })
    }

    async fn submit_redeem_tx_via_relayer(
        &self,
        owner_address: &str,
        condition_id: &str,
    ) -> std::result::Result<SubmittedRedeemTx, ClaimSubmitFailure> {
        let safe_address = self.safe_address.as_deref().ok_or_else(|| {
            ClaimSubmitFailure::non_retryable(
                "builder_relayer requires exchange.gnosis_safe_address",
            )
        })?;
        if !safe_address.eq_ignore_ascii_case(owner_address) {
            return Err(ClaimSubmitFailure::non_retryable(format!(
                "builder_relayer only supports the configured safe owner address ({safe_address}), got {owner_address}"
            )));
        }
        let adapter = self.relayer_adapter.as_ref().ok_or_else(|| {
            ClaimSubmitFailure::non_retryable(
                "claim relayer adapter not configured for builder_relayer mode",
            )
        })?;

        let request = ClaimRelayerAdapterRequest {
            user_id: self.user_id,
            owner_address: safe_address.to_string(),
            condition_id: normalize_condition_id(condition_id).map_err(|err| {
                ClaimSubmitFailure::non_retryable(format!(
                    "invalid auto-claim condition id {condition_id}: {err:#}"
                ))
            })?,
            collateral_token: format!("{:#x}", self.collateral_token),
            index_sets: AUTO_CLAIM_INDEX_SETS.to_vec(),
        };

        let response = self
            .http
            .post(&adapter.url)
            .bearer_auth(&adapter.token)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                ClaimSubmitFailure::retryable(format!(
                    "claim relayer adapter request failed for {condition_id}: {err:#}"
                ))
            })?;
        let status = response.status();
        let body = response.text().await.map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "failed reading claim relayer adapter response for {condition_id}: {err:#}"
            ))
        })?;

        if !status.is_success() {
            let parsed = serde_json::from_str::<ClaimRelayerAdapterErrorBody>(&body).ok();
            let (retryable, code, message) =
                claim_relayer_adapter_error_details(status, parsed.as_ref(), &body);
            return Err(if retryable {
                ClaimSubmitFailure::retryable(format!("{code}: {message}"))
            } else {
                ClaimSubmitFailure::non_retryable(format!("{code}: {message}"))
            });
        }

        let payload = serde_json::from_str::<ClaimRelayerAdapterSuccess>(&body).map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "failed to parse claim relayer adapter success response for {condition_id}: {err:#}"
            ))
        })?;
        let tx_hash = normalize_tx_hash(&payload.tx_hash).map_err(|err| {
            ClaimSubmitFailure::retryable(format!(
                "claim relayer adapter returned invalid tx hash for {condition_id}: {err:#}"
            ))
        })?;

        Ok(SubmittedRedeemTx {
            tx_hash,
            gas_price: None,
            submission_mode: if matches!(self.execution_mode, ClaimExecutionMode::RelayerApiKey) {
                "relayer_api_key"
            } else {
                "builder_relayer"
            },
        })
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

fn parse_tx_hash(raw: &str) -> Result<H256> {
    let trimmed = raw.trim();
    let prefixed = if trimmed.starts_with("0x") {
        trimmed.to_string()
    } else {
        format!("0x{trimmed}")
    };
    H256::from_str(&prefixed).with_context(|| format!("invalid tx_hash: {trimmed}"))
}

fn normalize_tx_hash(raw: &str) -> Result<String> {
    Ok(format!("{:#x}", parse_tx_hash(raw)?))
}

fn claim_relayer_adapter_error_details(
    status: StatusCode,
    parsed: Option<&ClaimRelayerAdapterErrorBody>,
    body: &str,
) -> (bool, String, String) {
    let retryable = parsed
        .and_then(|value| value.retryable)
        .unwrap_or_else(|| status.as_u16() == 429 || status.is_server_error());
    let message = parsed
        .map(|value| value.message.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_relayer_adapter_message(status, body));
    let code = parsed
        .map(|value| value.code.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if looks_like_html_document(body) {
                "claim_relayer_adapter_invalid_html".to_string()
            } else {
                "claim_relayer_adapter_error".to_string()
            }
        });
    (retryable, code, message)
}

fn fallback_relayer_adapter_message(status: StatusCode, body: &str) -> String {
    if looks_like_html_document(body) {
        return format!("HTTP {} from internal adapter", status.as_u16());
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return format!("HTTP {} from claim relayer adapter", status.as_u16());
    }

    compact_error(anyhow::anyhow!(trimmed.to_string()))
}

fn looks_like_html_document(body: &str) -> bool {
    let trimmed = body.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    let prefix = trimmed
        .chars()
        .take(256)
        .collect::<String>()
        .to_ascii_lowercase();
    prefix.starts_with("<!doctype html") || prefix.starts_with("<html") || prefix.contains("<html")
}

#[cfg(test)]
mod tests;
