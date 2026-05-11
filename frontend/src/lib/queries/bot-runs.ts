import { pool } from '@/lib/db';
import type { BotRun } from '@/lib/types';

export type MarketDiscoveryState = 'ready' | 'waiting_for_market' | 'error';

export interface MarketDiscoveryStatus {
  state: MarketDiscoveryState;
  selectedMarketSlug: string | null;
  message: string | null;
  updatedAt: string | null;
}

export async function getLastBotRun(): Promise<BotRun | null> {
  const { rows } = await pool.query(
    'SELECT id, mode, version, started_at, stopped_at, reason FROM bot_runs ORDER BY started_at DESC LIMIT 1'
  );
  return rows[0] || null;
}

export async function getBotRuns(limit = 20): Promise<BotRun[]> {
  const { rows } = await pool.query(
    'SELECT id, mode, version, started_at, stopped_at, reason FROM bot_runs ORDER BY started_at DESC LIMIT $1',
    [limit]
  );
  return rows;
}

export async function getMarketDiscoveryStatus(
  runStartedAt?: string | Date | null
): Promise<MarketDiscoveryStatus> {
  if (!runStartedAt) {
    return {
      state: 'ready',
      selectedMarketSlug: null,
      message: null,
      updatedAt: null,
    };
  }

  const params: unknown[] = [];
  let query =
    "SELECT decision, details, created_at FROM risk_events WHERE event_type = 'market_discovery'";

  params.push(runStartedAt);
  query += ' AND created_at >= $1';

  query += ' ORDER BY created_at DESC LIMIT 1';
  const { rows } = await pool.query(query, params);
  const row = rows[0] as
    | { decision?: string; details?: string; created_at?: string | Date }
    | undefined;

  if (!row) {
    return {
      state: 'ready',
      selectedMarketSlug: null,
      message: null,
      updatedAt: null,
    };
  }

  const parsed = parseMarketDiscoveryDetails(row.details);
  const state = normalizeMarketState(parsed.state) ?? mapDecisionToState(row.decision);

  return {
    state,
    selectedMarketSlug: parsed.selectedMarketSlug,
    message: parsed.message || row.details || null,
    updatedAt: row.created_at ? new Date(row.created_at).toISOString() : null,
  };
}

function parseMarketDiscoveryDetails(raw?: string): {
  state: string | null;
  selectedMarketSlug: string | null;
  message: string | null;
} {
  if (!raw) {
    return {
      state: null,
      selectedMarketSlug: null,
      message: null,
    };
  }

  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    return {
      state: typeof parsed.state === 'string' ? parsed.state : null,
      selectedMarketSlug:
        typeof parsed.selected_market_slug === 'string' ? parsed.selected_market_slug : null,
      message: typeof parsed.message === 'string' ? parsed.message : null,
    };
  } catch {
    return {
      state: null,
      selectedMarketSlug: null,
      message: raw,
    };
  }
}

function normalizeMarketState(value: string | null): MarketDiscoveryState | null {
  if (value === 'ready' || value === 'waiting_for_market' || value === 'error') {
    return value;
  }
  return null;
}

function mapDecisionToState(decision?: string): MarketDiscoveryState {
  if (decision === 'block') return 'waiting_for_market';
  if (decision === 'halt') return 'error';
  return 'ready';
}
