import { Pool } from 'pg';

const globalForPg = globalThis as unknown as { pgPool: Pool };
const TELEMETRY_ERROR_MAX_LENGTH = 120;
const createdPool = !globalForPg.pgPool;

export const pool =
  globalForPg.pgPool ??
  new Pool({
    connectionString: process.env.DATABASE_URL,
    max: 10,
    idleTimeoutMillis: 30000,
    connectionTimeoutMillis: 5000,
    statement_timeout: 20000,
    idle_in_transaction_session_timeout: 30000,
  });

export function isFlowTelemetryEnabled(): boolean {
  return process.env.TRADE_FLOW_TELEMETRY === '1';
}

export function getPoolTelemetrySnapshot(): string {
  return `${pool.totalCount}/${pool.idleCount}/${pool.waitingCount}`;
}

export function compactTelemetryError(err: unknown): string {
  const rawMessage = err instanceof Error ? err.message : String(err ?? 'unknown error');
  const compact = rawMessage.replace(/\s+/g, ' ').trim();
  if (!compact) return 'unknown error';
  if (compact.length <= TELEMETRY_ERROR_MAX_LENGTH) return compact;
  return `${compact.slice(0, TELEMETRY_ERROR_MAX_LENGTH)}...`;
}

if (createdPool) {
  pool.on('error', (err) => {
    console.error('[pg-pool] idle client error:', compactTelemetryError(err));
  });

  if (isFlowTelemetryEnabled()) {
    pool.on('connect', () => {
      console.log(`[pg-pool] new-conn pool=${getPoolTelemetrySnapshot()}`);
    });

    pool.on('acquire', () => {
      if (pool.waitingCount > 0) {
        console.warn(`[pg-pool] acquire-queued pool=${getPoolTelemetrySnapshot()}`);
      }
    });
  }
}

if (process.env.NODE_ENV !== 'production') {
  globalForPg.pgPool = pool;
}
