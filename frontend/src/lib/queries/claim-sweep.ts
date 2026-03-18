import { pool } from '@/lib/db';
import type { ClaimSweepQueueStatus } from '@/lib/types';

export interface ClaimSweepJobSeed {
  ownerAddress: string;
  marketSlug: string | null;
  conditionId: string;
}

export interface ClaimSweepQueueUpsertResult {
  queuedNewCount: number;
  rearmedCount: number;
  alreadyTrackedCount: number;
}

const ACTIVE_AUTO_CLAIM_VALUES = ['true', '1', 'yes', 'on'];

function emptyQueue(): ClaimSweepQueueStatus {
  return {
    pending: 0,
    retry: 0,
    processing: 0,
    submitted: 0,
    failed: 0,
    claimed: 0,
  };
}

export async function hasPublishedAutoClaimEnabledFlow(userId: number): Promise<boolean> {
  const res = await pool.query<{ enabled: boolean }>(
    `SELECT EXISTS (
       SELECT 1
       FROM trade_flow_definitions d
       JOIN trade_flow_versions v ON v.id = d.published_version_id
       WHERE d.user_id = $1
         AND d.status = 'published'
         AND d.published_version_id IS NOT NULL
         AND LOWER(COALESCE(v.graph_json #>> '{context,autoClaimEnabled}', 'false')) = ANY($2::text[])
     ) AS enabled`,
    [userId, ACTIVE_AUTO_CLAIM_VALUES]
  );
  return res.rows[0]?.enabled === true;
}

export async function getClaimSweepQueueStatus(
  ownerAddresses: string[]
): Promise<ClaimSweepQueueStatus> {
  if (ownerAddresses.length === 0) {
    return emptyQueue();
  }

  const res = await pool.query<{ status: string; count: string }>(
    `SELECT status, COUNT(*)::text AS count
     FROM auto_claim_jobs
     WHERE LOWER(owner_address) = ANY($1::text[])
     GROUP BY status`,
    [ownerAddresses]
  );

  const out = emptyQueue();
  for (const row of res.rows) {
    const count = Number.parseInt(row.count, 10) || 0;
    if (row.status === 'pending') out.pending = count;
    if (row.status === 'retry') out.retry = count;
    if (row.status === 'processing') out.processing = count;
    if (row.status === 'submitted') out.submitted = count;
    if (row.status === 'failed') out.failed = count;
    if (row.status === 'claimed') out.claimed = count;
  }
  return out;
}

export async function getLatestClaimSweepError(
  ownerAddresses: string[]
): Promise<string | null> {
  if (ownerAddresses.length === 0) {
    return null;
  }

  const res = await pool.query<{ last_error: string | null }>(
    `SELECT last_error
     FROM auto_claim_jobs
     WHERE LOWER(owner_address) = ANY($1::text[])
       AND COALESCE(last_error, '') <> ''
     ORDER BY updated_at DESC
     LIMIT 1`,
    [ownerAddresses]
  );

  return res.rows[0]?.last_error?.trim() || null;
}

export async function queueClaimSweepJobs(
  jobs: ClaimSweepJobSeed[],
  maxAttempts: number
): Promise<ClaimSweepQueueUpsertResult> {
  if (jobs.length === 0) {
    return {
      queuedNewCount: 0,
      rearmedCount: 0,
      alreadyTrackedCount: 0,
    };
  }

  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    let queuedNewCount = 0;
    let rearmedCount = 0;
    let alreadyTrackedCount = 0;

    const byOwner = new Map<string, ClaimSweepJobSeed[]>();
    for (const job of jobs) {
      const rows = byOwner.get(job.ownerAddress) ?? [];
      rows.push(job);
      byOwner.set(job.ownerAddress, rows);
    }

    for (const [ownerAddress, ownerJobs] of byOwner) {
      const conditionIds = ownerJobs.map((job) => job.conditionId);
      const existing = await client.query<{ condition_id: string; status: string }>(
        `SELECT condition_id, status
         FROM auto_claim_jobs
         WHERE LOWER(owner_address) = $1
           AND condition_id = ANY($2::text[])`,
        [ownerAddress, conditionIds]
      );

      const statusByCondition = new Map(
        existing.rows.map((row) => [row.condition_id.toLowerCase(), row.status])
      );

      for (const job of ownerJobs) {
        const currentStatus = statusByCondition.get(job.conditionId);
        if (!currentStatus) {
          queuedNewCount += 1;
          continue;
        }
        if (currentStatus === 'failed' || currentStatus === 'retry') {
          rearmedCount += 1;
          continue;
        }
        alreadyTrackedCount += 1;
      }

      const marketSlugs = ownerJobs.map((job) => job.marketSlug ?? '');
      await client.query(
        `INSERT INTO auto_claim_jobs
          (owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, submitted_at, last_seen_redeemable_at, created_at, updated_at)
         SELECT
           $1,
           NULLIF(seed.market_slug, ''),
           seed.condition_id,
           'pending',
           0,
           $2,
           NOW(),
           NULL,
           NULL,
           NULL,
           NULL,
           NOW(),
           NOW(),
           NOW()
         FROM UNNEST($3::text[], $4::text[]) AS seed(condition_id, market_slug)
         ON CONFLICT (owner_address, condition_id) DO UPDATE SET
           market_slug = COALESCE(EXCLUDED.market_slug, auto_claim_jobs.market_slug),
           max_attempts = GREATEST(auto_claim_jobs.max_attempts, EXCLUDED.max_attempts),
           last_seen_redeemable_at = NOW(),
           updated_at = NOW(),
           status = CASE
             WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.status
             ELSE 'pending'
           END,
           attempts = CASE
             WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.attempts
             ELSE 0
           END,
           next_attempt_at = CASE
             WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.next_attempt_at
             ELSE NOW()
           END,
           last_error = CASE
             WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.last_error
             ELSE NULL
           END`,
        [ownerAddress, maxAttempts, conditionIds, marketSlugs]
      );
    }

    await client.query('COMMIT');
    return {
      queuedNewCount,
      rearmedCount,
      alreadyTrackedCount,
    };
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}
