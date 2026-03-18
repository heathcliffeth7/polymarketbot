use super::*;

/// Gnosis Safe MultiSendCallOnly contract on Polygon.
const MULTISEND_CALL_ONLY_ADDRESS: &str = "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D";

/// Safe operation: delegatecall (1). MultiSend must be delegatecalled
/// so that msg.sender stays as the Safe inside each inner call.
const SAFE_OPERATION_DELEGATECALL: u8 = 1;

/// Maximum number of redeemPositions calls to pack into a single MultiSend tx.
/// 100 × ~150k gas ≈ 15M gas, well within Polygon's 30M block gas limit.
const BATCH_MAX_CONDITIONS: usize = 100;

abigen!(
    MultiSendCallOnly,
    r#"[function multiSend(bytes transactions)]"#,
);

pub(super) struct BatchRedeemResult {
    pub tx_hash: String,
    pub gas_price: U256,
    pub job_count: usize,
}

// ---------------------------------------------------------------------------
//  Processing orchestration (moved from claim.rs to reduce file size)
// ---------------------------------------------------------------------------

impl AutoClaimService {
    /// Process pending claim jobs. Safe-owned jobs are batched into a single
    /// MultiSend tx when in direct/safe mode. Relayer and direct-EOA jobs are
    /// processed individually.
    pub(super) async fn process_pending_jobs(
        &mut self,
        repo: &PostgresRepository,
        limit: i64,
    ) -> Result<usize> {
        let jobs = repo
            .list_auto_claim_jobs_for_processing(limit.max(1))
            .await?;
        if jobs.is_empty() {
            return Ok(0);
        }

        // In non-relayer mode, try to batch all Safe-owned jobs.
        if !matches!(
            self.execution_mode,
            ClaimExecutionMode::BuilderRelayer | ClaimExecutionMode::RelayerApiKey
        ) {
            let (safe_jobs, other_jobs) = partition_safe_jobs(&jobs, &self.safe_address);
            let mut processed = 0usize;

            if safe_jobs.len() >= 2 {
                processed += self.process_batch_safe_jobs(repo, &safe_jobs).await?;
            } else {
                // Single safe job or none: process individually.
                for job in &safe_jobs {
                    let rate_limited = self.process_single_job(repo, job).await?;
                    processed += 1;
                    if rate_limited {
                        self.schedule_rate_limit_cooldown();
                        return Ok(processed);
                    }
                }
            }

            for job in &other_jobs {
                let rate_limited = self.process_single_job(repo, job).await?;
                processed += 1;
                if rate_limited {
                    self.schedule_rate_limit_cooldown();
                    return Ok(processed);
                }
            }

            return Ok(processed);
        }

        // Relayer mode: process one-by-one (relayer does not support batching).
        let mut processed = 0usize;
        for job in &jobs {
            let rate_limited = self.process_single_job(repo, job).await?;
            processed += 1;
            if rate_limited {
                self.schedule_rate_limit_cooldown();
                break;
            }
        }
        Ok(processed)
    }

    pub(super) async fn process_single_job(
        &self,
        repo: &PostgresRepository,
        job: &AutoClaimJob,
    ) -> Result<bool> {
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
            Ok(submission) => {
                let SubmittedRedeemTx {
                    tx_hash,
                    gas_price,
                    submission_mode,
                } = submission;
                repo.mark_auto_claim_job_submitted(job.id, &tx_hash).await?;
                let mut payload = json!({
                    "job_id": job.id,
                    "condition_id": job.condition_id,
                    "tx_hash": tx_hash,
                    "submission_mode": submission_mode,
                });
                if let Some(gas_price) = gas_price {
                    payload["gas_price_gwei"] = json!(gas_price_gwei(gas_price));
                }
                repo.append_auto_claim_event(job.id, "submitted", &payload)
                    .await?;
                Ok(false)
            }
            Err(err) => {
                let rate_limited = submit_failure_indicates_rate_limit(&err);
                let compact_error = compact_submit_failure(&err);
                let (status, event_type) = if err.retryable {
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
                    (status, event_type)
                } else {
                    repo.mark_auto_claim_job_failed(job.id, &compact_error)
                        .await?;
                    ("failed".to_string(), "claim_failed")
                };
                repo.append_auto_claim_event(
                    job.id,
                    event_type,
                    &json!({
                        "job_id": job.id,
                        "condition_id": job.condition_id,
                        "status": status,
                        "retryable": err.retryable,
                        "error": compact_error,
                        "error_chain": compact_error,
                        "retry_backoff_ms": self.retry_backoff_ms
                    }),
                )
                .await?;
                Ok(rate_limited)
            }
        }
    }

    fn schedule_rate_limit_cooldown(&mut self) {
        self.next_submission_at =
            relayer_rate_limit_cooldown_until(Utc::now(), self.retry_backoff_ms);
        info!(
            user = %self.signer_address,
            next_submission_at = %self.next_submission_at,
            "AUTO_CLAIM_RELAYER_RATE_LIMIT_COOLDOWN_SCHEDULED"
        );
    }

    /// Process a batch of Safe-owned claim jobs in a single MultiSend tx.
    async fn process_batch_safe_jobs(
        &self,
        repo: &PostgresRepository,
        jobs: &[&AutoClaimJob],
    ) -> Result<usize> {
        let batch_size = jobs.len().min(BATCH_MAX_CONDITIONS);
        let batch_jobs = &jobs[..batch_size];

        // Mark all as processing and log events.
        for job in batch_jobs {
            repo.mark_auto_claim_job_processing(job.id).await?;
            repo.append_auto_claim_event(
                job.id,
                "processing_started",
                &json!({
                    "job_id": job.id,
                    "owner_address": job.owner_address,
                    "condition_id": job.condition_id,
                    "market_slug": job.market_slug,
                    "attempt": job.attempts + 1,
                    "batch_mode": true,
                    "batch_size": batch_size
                }),
            )
            .await?;
        }

        let condition_ids: Vec<String> =
            batch_jobs.iter().map(|j| j.condition_id.clone()).collect();

        let gas_price = self.effective_gas_price().await.map_err(|err| {
            anyhow::anyhow!("failed to fetch gas price for batch redeem: {err:#}")
        })?;

        let safe_contract = self
            .safe_contract
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("safe contract not configured for batch redeem"))?;

        match submit_batch_redeem_via_safe(
            &self.middleware,
            safe_contract,
            &self.ctf_contract,
            self.collateral_token,
            &self.signer_address,
            &condition_ids,
            gas_price,
        )
        .await
        {
            Ok(result) => {
                info!(
                    user = %self.signer_address,
                    tx_hash = %result.tx_hash,
                    job_count = result.job_count,
                    gas_price_gwei = gas_price_gwei(result.gas_price),
                    "AUTO_CLAIM_BATCH_SUBMITTED"
                );
                for job in batch_jobs {
                    repo.mark_auto_claim_job_submitted(job.id, &result.tx_hash)
                        .await?;
                    repo.append_auto_claim_event(
                        job.id,
                        "submitted",
                        &json!({
                            "job_id": job.id,
                            "condition_id": job.condition_id,
                            "tx_hash": result.tx_hash,
                            "submission_mode": "safe_batch",
                            "batch_size": result.job_count,
                            "gas_price_gwei": gas_price_gwei(result.gas_price)
                        }),
                    )
                    .await?;
                }
                Ok(batch_size)
            }
            Err(err) => {
                let compact = compact_submit_failure(&err);
                warn!(
                    user = %self.signer_address,
                    error = %compact,
                    job_count = batch_size,
                    "AUTO_CLAIM_BATCH_FAILED"
                );
                for job in batch_jobs {
                    if err.retryable {
                        repo.mark_auto_claim_job_retry_or_fail(
                            job.id,
                            &compact,
                            self.retry_backoff_ms as i64,
                        )
                        .await?;
                    } else {
                        repo.mark_auto_claim_job_failed(job.id, &compact).await?;
                    }
                    repo.append_auto_claim_event(
                        job.id,
                        if err.retryable {
                            "retry_scheduled"
                        } else {
                            "claim_failed"
                        },
                        &json!({
                            "job_id": job.id,
                            "condition_id": job.condition_id,
                            "retryable": err.retryable,
                            "error": compact,
                            "batch_mode": true,
                            "batch_size": batch_size
                        }),
                    )
                    .await?;
                }
                Ok(batch_size)
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  MultiSend submission
// ---------------------------------------------------------------------------

/// Submit a batch of redeemPositions calls via Safe MultiSend in a single tx.
async fn submit_batch_redeem_via_safe(
    middleware: &Arc<ClaimSigner>,
    safe_contract: &GnosisSafe<ClaimSigner>,
    ctf_contract: &ConditionalTokens<ClaimSigner>,
    collateral_token: Address,
    signer_address: &str,
    condition_ids: &[String],
    gas_price: U256,
) -> std::result::Result<BatchRedeemResult, ClaimSubmitFailure> {
    if condition_ids.is_empty() {
        return Err(ClaimSubmitFailure::non_retryable(
            "batch redeem called with empty condition list",
        ));
    }

    let owner = parse_address(signer_address, "claim.user_address").map_err(|err| {
        ClaimSubmitFailure::non_retryable(format!(
            "invalid claim.user_address for batch safe redeem: {err:#}"
        ))
    })?;

    let multisend_address = parse_address(MULTISEND_CALL_ONLY_ADDRESS, "multisend_call_only")
        .map_err(|err| {
            ClaimSubmitFailure::non_retryable(format!("invalid multisend address: {err:#}"))
        })?;

    // Build individual redeemPositions calldata for each condition_id.
    let ctf_address = ctf_contract.address();
    let mut inner_calls: Vec<(Address, Vec<u8>)> = Vec::with_capacity(condition_ids.len());
    for cid_str in condition_ids {
        let condition = parse_condition_id(cid_str).map_err(|err| {
            ClaimSubmitFailure::non_retryable(format!(
                "invalid condition_id in batch: {cid_str}: {err:#}"
            ))
        })?;
        let index_sets = AUTO_CLAIM_INDEX_SETS
            .iter()
            .copied()
            .map(U256::from)
            .collect::<Vec<_>>();
        let calldata = ctf_contract
            .redeem_positions(
                collateral_token,
                [0u8; 32],
                condition.to_fixed_bytes(),
                index_sets,
            )
            .calldata()
            .ok_or_else(|| {
                ClaimSubmitFailure::non_retryable(format!(
                    "failed to encode redeemPositions calldata for {cid_str}"
                ))
            })?;
        inner_calls.push((ctf_address, calldata.to_vec()));
    }

    // Pack into MultiSend encoding.
    let packed = encode_multisend_transactions(&inner_calls);

    // Build multiSend(bytes) calldata.
    let multi_send = MultiSendCallOnly::new(multisend_address, middleware.clone());
    let multi_send_calldata = multi_send
        .multi_send(packed.into())
        .calldata()
        .ok_or_else(|| ClaimSubmitFailure::non_retryable("failed to encode multiSend calldata"))?;

    // Build Safe.execTransaction with delegatecall to MultiSend.
    let signatures = build_safe_prevalidated_signature(owner);
    let safe_call = safe_contract.exec_transaction(
        multisend_address,
        U256::zero(),
        multi_send_calldata,
        SAFE_OPERATION_DELEGATECALL,
        U256::zero(),
        U256::zero(),
        U256::zero(),
        Address::zero(),
        Address::zero(),
        signatures,
    );

    // Explicit gas limit: ~150k per redeemPositions + 100k overhead for MultiSend + Safe.
    let gas_limit = U256::from(condition_ids.len() as u64 * 150_000 + 100_000);
    let safe_call = safe_call.gas(gas_limit);

    // Simulate first.
    let simulation_ok = safe_call.clone().call().await.map_err(|err| {
        ClaimSubmitFailure::retryable(format!(
            "batch safe execTransaction simulation failed ({} conditions): {err:#}",
            condition_ids.len()
        ))
    })?;
    if !simulation_ok {
        return Err(ClaimSubmitFailure::retryable(format!(
            "batch safe execTransaction simulation returned false ({} conditions)",
            condition_ids.len()
        )));
    }

    // Send.
    let safe_call = safe_call.gas_price(gas_price);
    let pending_tx = safe_call.send().await.map_err(|err| {
        ClaimSubmitFailure::retryable(format!(
            "batch safe execTransaction send failed ({} conditions): {err:#}",
            condition_ids.len()
        ))
    })?;

    Ok(BatchRedeemResult {
        tx_hash: format!("{:#x}", pending_tx.tx_hash()),
        gas_price,
        job_count: condition_ids.len(),
    })
}

// ---------------------------------------------------------------------------
//  Helpers
// ---------------------------------------------------------------------------

/// Partition jobs into Safe-owned and others.
fn partition_safe_jobs<'a>(
    jobs: &'a [AutoClaimJob],
    safe_address: &Option<String>,
) -> (Vec<&'a AutoClaimJob>, Vec<&'a AutoClaimJob>) {
    let Some(safe_addr) = safe_address.as_deref() else {
        return (Vec::new(), jobs.iter().collect());
    };
    let mut safe_jobs = Vec::new();
    let mut other_jobs = Vec::new();
    for job in jobs {
        if job.owner_address.eq_ignore_ascii_case(safe_addr) {
            safe_jobs.push(job);
        } else {
            other_jobs.push(job);
        }
    }
    (safe_jobs, other_jobs)
}

/// Encode transactions into the packed format expected by MultiSend.
///
/// Each transaction:
/// - 1 byte:  operation (0 = call)
/// - 20 bytes: to address
/// - 32 bytes: value (always 0 for redeemPositions)
/// - 32 bytes: data length
/// - N bytes:  data
fn encode_multisend_transactions(txs: &[(Address, Vec<u8>)]) -> Vec<u8> {
    let estimated_size: usize = txs.iter().map(|(_, data)| 85 + data.len()).sum();
    let mut packed = Vec::with_capacity(estimated_size);

    for (to, data) in txs {
        packed.push(0u8); // operation = call
        packed.extend_from_slice(to.as_bytes()); // to (20 bytes)
        packed.extend_from_slice(&[0u8; 32]); // value = 0 (32 bytes)
        let mut len_bytes = [0u8; 32]; // data length (32 bytes, big-endian)
        len_bytes[24..32].copy_from_slice(&(data.len() as u64).to_be_bytes());
        packed.extend_from_slice(&len_bytes);
        packed.extend_from_slice(data); // data
    }

    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_multisend_single_call() {
        let to = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
            .parse::<Address>()
            .unwrap();
        let data = vec![0xAA, 0xBB, 0xCC];
        let packed = encode_multisend_transactions(&[(to, data)]);
        // 1 (op) + 20 (to) + 32 (value) + 32 (len) + 3 (data) = 88
        assert_eq!(packed.len(), 88);
        assert_eq!(packed[0], 0u8);
        assert_eq!(&packed[1..21], to.as_bytes());
        assert_eq!(packed[84], 3u8);
        assert_eq!(&packed[85..88], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn encode_multisend_multiple_calls() {
        let to = Address::zero();
        let txs = vec![(to, vec![0x11; 10]), (to, vec![0x22; 20])];
        let packed = encode_multisend_transactions(&txs);
        assert_eq!(packed.len(), (85 + 10) + (85 + 20));
    }

    #[test]
    fn partition_safe_jobs_splits_correctly() {
        let safe_addr = Some("0xaaaa".to_string());
        let now = Utc::now();
        let make_job = |id: i64, owner: &str| AutoClaimJob {
            id,
            owner_address: owner.to_string(),
            condition_id: format!("0x{id:064x}"),
            market_slug: None,
            status: "pending".to_string(),
            attempts: 0,
            max_attempts: 5,
            next_attempt_at: now,
            tx_hash: None,
            last_error: None,
            claimed_at: None,
            submitted_at: None,
            last_seen_redeemable_at: now,
            created_at: now,
            updated_at: now,
        };
        let jobs = vec![make_job(1, "0xaaaa"), make_job(2, "0xbbbb")];
        let (safe, other) = partition_safe_jobs(&jobs, &safe_addr);
        assert_eq!(safe.len(), 1);
        assert_eq!(other.len(), 1);
        assert_eq!(safe[0].id, 1);
        assert_eq!(other[0].id, 2);
    }
}
