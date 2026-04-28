import { readDataApiActivityConfigForServer } from '@/lib/config';
import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisSummary,
} from '@/lib/types';

const ACTIVITY_CACHE_TTL_MS = 180_000;
const ACTIVITY_REQUEST_TIMEOUT_MS = 12_000;

interface RawDataApiActivity {
  type?: unknown;
  side?: unknown;
  slug?: unknown;
  size?: unknown;
  usdcSize?: unknown;
  timestamp?: unknown;
}

export interface OfficialMarketActivity {
  activityType: string;
  side: string | null;
  slug: string;
  size: number;
  usdcSize: number;
  timestamp: number | null;
}

export interface OfficialMarketLedger {
  marketSlug: string;
  buyUsdc: number;
  sellUsdc: number;
  redeemUsdc: number;
  pnlUsdc: number;
  activityCount: number;
}

export interface OfficialMarketPnlSummary {
  source: 'official_market_activity';
  marketCount: number;
  profitCount: number;
  lossCount: number;
  totalPnlUsdc: number;
  realizedPnlUsdc: number;
  lossUsdc: number;
  profitUsdc: number;
  costBasisUsdc: number;
  netValueUsdc: number;
  largestLossUsdc: number | null;
  officialBuyUsdc: number;
  officialSellUsdc: number;
  officialRedeemUsdc: number;
  rootRowsPnlUsdc: number;
  officialDeltaUsdc: number;
  refreshedAt: string;
}

interface ActivityCacheEntry {
  fetchedAtMs: number;
  rows: OfficialMarketActivity[];
  inFlight?: Promise<OfficialMarketActivity[]>;
}

const activityCache = new Map<string, ActivityCacheEntry>();

function parseNumber(value: unknown): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function parseTimestamp(value: unknown): number | null {
  const parsed = parseNumber(value);
  return parsed > 0 ? parsed : null;
}

function normalizeActivity(row: RawDataApiActivity): OfficialMarketActivity | null {
  const slug = String(row.slug ?? '').trim();
  if (!slug) return null;
  return {
    activityType: String(row.type ?? '').trim().toUpperCase(),
    side: String(row.side ?? '').trim().toUpperCase() || null,
    slug,
    size: parseNumber(row.size),
    usdcSize: parseNumber(row.usdcSize),
    timestamp: parseTimestamp(row.timestamp),
  };
}

function roundCash(value: number): number {
  return Math.round(value * 100_000) / 100_000;
}

function activityTimeMs(row: OfficialMarketActivity): number | null {
  return row.timestamp == null ? null : row.timestamp * 1000;
}

function isInsideTimeWindow(
  row: OfficialMarketActivity,
  from: string | null | undefined,
  to: string | null | undefined
): boolean {
  if (!from && !to) return true;
  const timeMs = activityTimeMs(row);
  if (timeMs == null) return false;
  if (from && timeMs < new Date(from).getTime()) return false;
  if (to && timeMs > new Date(to).getTime()) return false;
  return true;
}

function shouldKeepByPnl(ledger: OfficialMarketLedger, pnlFilter: AutoScopeTradeAnalysisPnlFilter) {
  if (pnlFilter === 'loss') return ledger.pnlUsdc < 0;
  if (pnlFilter === 'profit') return ledger.pnlUsdc > 0;
  return true;
}

export function buildOfficialMarketLedgersFromActivity({
  activity,
  marketSlugs,
  from,
  to,
  pnlFilter,
}: {
  activity: OfficialMarketActivity[];
  marketSlugs: Set<string>;
  from?: string | null;
  to?: string | null;
  pnlFilter: AutoScopeTradeAnalysisPnlFilter;
}): OfficialMarketLedger[] {
  const ledgers = new Map<string, OfficialMarketLedger>();

  for (const row of activity) {
    const normalizedSlug = row.slug.trim().toLowerCase();
    if (!normalizedSlug || !marketSlugs.has(normalizedSlug)) continue;
    if (!isInsideTimeWindow(row, from, to)) continue;

    const ledger =
      ledgers.get(normalizedSlug) ??
      {
        marketSlug: row.slug,
        buyUsdc: 0,
        sellUsdc: 0,
        redeemUsdc: 0,
        pnlUsdc: 0,
        activityCount: 0,
      };

    if (row.activityType === 'REDEEM') {
      ledger.redeemUsdc += Math.max(row.usdcSize, row.size, 0);
      ledger.activityCount += 1;
    } else if (row.activityType === 'TRADE' && row.side === 'BUY') {
      ledger.buyUsdc += Math.max(row.usdcSize, 0);
      ledger.activityCount += 1;
    } else if (row.activityType === 'TRADE' && row.side === 'SELL') {
      ledger.sellUsdc += Math.max(row.usdcSize, 0);
      ledger.activityCount += 1;
    }

    ledger.buyUsdc = roundCash(ledger.buyUsdc);
    ledger.sellUsdc = roundCash(ledger.sellUsdc);
    ledger.redeemUsdc = roundCash(ledger.redeemUsdc);
    ledger.pnlUsdc = roundCash(ledger.sellUsdc + ledger.redeemUsdc - ledger.buyUsdc);
    ledgers.set(normalizedSlug, ledger);
  }

  return [...ledgers.values()]
    .filter((ledger) => ledger.activityCount > 0)
    .filter((ledger) => shouldKeepByPnl(ledger, pnlFilter));
}

export function buildOfficialMarketPnlSummaryFromLedgers({
  ledgers,
  rootRowsPnlUsdc,
}: {
  ledgers: OfficialMarketLedger[];
  rootRowsPnlUsdc: number;
}): OfficialMarketPnlSummary {
  const totalPnlUsdc = roundCash(ledgers.reduce((sum, row) => sum + row.pnlUsdc, 0));
  const profitUsdc = roundCash(
    ledgers.reduce((sum, row) => sum + Math.max(row.pnlUsdc, 0), 0)
  );
  const lossUsdc = roundCash(
    Math.abs(ledgers.reduce((sum, row) => sum + Math.min(row.pnlUsdc, 0), 0))
  );
  const largestLoss = ledgers
    .filter((row) => row.pnlUsdc < 0)
    .map((row) => Math.abs(row.pnlUsdc))
    .sort((left, right) => right - left)[0];
  const officialBuyUsdc = roundCash(ledgers.reduce((sum, row) => sum + row.buyUsdc, 0));
  const officialSellUsdc = roundCash(ledgers.reduce((sum, row) => sum + row.sellUsdc, 0));
  const officialRedeemUsdc = roundCash(
    ledgers.reduce((sum, row) => sum + row.redeemUsdc, 0)
  );

  return {
    source: 'official_market_activity',
    marketCount: ledgers.length,
    profitCount: ledgers.filter((row) => row.pnlUsdc > 0).length,
    lossCount: ledgers.filter((row) => row.pnlUsdc < 0).length,
    totalPnlUsdc,
    realizedPnlUsdc: totalPnlUsdc,
    lossUsdc,
    profitUsdc,
    costBasisUsdc: officialBuyUsdc,
    netValueUsdc: roundCash(officialSellUsdc + officialRedeemUsdc),
    largestLossUsdc: largestLoss == null ? null : roundCash(largestLoss),
    officialBuyUsdc,
    officialSellUsdc,
    officialRedeemUsdc,
    rootRowsPnlUsdc: roundCash(rootRowsPnlUsdc),
    officialDeltaUsdc: roundCash(totalPnlUsdc - rootRowsPnlUsdc),
    refreshedAt: new Date().toISOString(),
  };
}

export function applyOfficialMarketPnlSummary(
  rowSummary: AutoScopeTradeAnalysisSummary,
  official: OfficialMarketPnlSummary
): AutoScopeTradeAnalysisSummary {
  const marketCount = official.marketCount;
  const profitCount = official.profitCount;
  const lossCount = official.lossCount;
  return {
    ...rowSummary,
    pnlSource: official.source,
    marketCount,
    profitCount,
    lossCount,
    totalPnlUsdc: official.totalPnlUsdc,
    realizedPnlUsdc: official.realizedPnlUsdc,
    lossUsdc: official.lossUsdc,
    profitUsdc: official.profitUsdc,
    costBasisUsdc: official.costBasisUsdc,
    netValueUsdc: official.netValueUsdc,
    profitFactor: official.lossUsdc > 0 ? official.profitUsdc / official.lossUsdc : null,
    winRatePct: marketCount > 0 ? (profitCount / marketCount) * 100 : null,
    avgWinUsdc: profitCount > 0 ? official.profitUsdc / profitCount : null,
    avgLossUsdc: lossCount > 0 ? official.lossUsdc / lossCount : null,
    largestLossUsdc: official.largestLossUsdc,
    officialBuyUsdc: official.officialBuyUsdc,
    officialSellUsdc: official.officialSellUsdc,
    officialRedeemUsdc: official.officialRedeemUsdc,
    rootRowsPnlUsdc: official.rootRowsPnlUsdc,
    officialDeltaUsdc: official.officialDeltaUsdc,
  };
}

async function fetchDataApiActivityPage({
  baseUrl,
  walletAddress,
  limit,
  offset,
}: {
  baseUrl: string;
  walletAddress: string;
  limit: number;
  offset: number;
}): Promise<OfficialMarketActivity[]> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), ACTIVITY_REQUEST_TIMEOUT_MS);
  try {
    const url = new URL(`${baseUrl.replace(/\/+$/, '')}/activity`);
    url.searchParams.set('user', walletAddress.trim().toLowerCase());
    url.searchParams.set('limit', String(limit));
    url.searchParams.set('offset', String(offset));
    const response = await fetch(url, { signal: controller.signal });
    if (response.status === 400 && offset > 0) {
      return [];
    }
    if (!response.ok) {
      throw new Error(`Data API activity returned ${response.status}`);
    }
    const body = (await response.json()) as RawDataApiActivity[];
    if (!Array.isArray(body)) return [];
    return body.flatMap((row) => {
      const normalized = normalizeActivity(row);
      return normalized ? [normalized] : [];
    });
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchDataApiActivity({
  baseUrl,
  walletAddress,
  pageSize,
  maxPages,
}: {
  baseUrl: string;
  walletAddress: string;
  pageSize: number;
  maxPages: number;
}): Promise<OfficialMarketActivity[]> {
  const cacheKey = `${baseUrl}|${walletAddress.toLowerCase()}|${pageSize}|${maxPages}`;
  const cached = activityCache.get(cacheKey);
  const now = Date.now();
  if (cached?.rows.length && now - cached.fetchedAtMs < ACTIVITY_CACHE_TTL_MS) {
    return cached.rows;
  }
  if (cached?.inFlight) {
    return cached.inFlight;
  }

  const inFlight = (async () => {
    const rows: OfficialMarketActivity[] = [];
    for (let page = 0; page < maxPages; page += 1) {
      const pageRows = await fetchDataApiActivityPage({
        baseUrl,
        walletAddress,
        limit: pageSize,
        offset: page * pageSize,
      });
      rows.push(...pageRows);
      if (pageRows.length < pageSize) break;
    }
    activityCache.set(cacheKey, { fetchedAtMs: Date.now(), rows });
    return rows;
  })();

  activityCache.set(cacheKey, {
    fetchedAtMs: cached?.fetchedAtMs ?? 0,
    rows: cached?.rows ?? [],
    inFlight,
  });

  try {
    return await inFlight;
  } catch (err) {
    activityCache.delete(cacheKey);
    throw err;
  }
}

async function listBotMarketSlugs(userId: number): Promise<Set<string>> {
  const result = await pool.query<{ market_slug: string }>(
    `SELECT DISTINCT LOWER(TRIM(o.market_slug)) AS market_slug
     FROM trade_builder_orders o
     WHERE o.user_id = $1
       AND o.origin_flow_run_id IS NOT NULL
       AND TRIM(o.market_slug) <> ''
       AND (
         COALESCE(o.filled_qty, 0) > 0
         OR EXISTS (
           SELECT 1
           FROM trade_builder_order_events e
           WHERE e.builder_order_id = o.id
             AND e.event_type = 'filled'
         )
       )`,
    [userId]
  );
  return new Set(result.rows.map((row) => row.market_slug).filter(Boolean));
}

export async function getOfficialMarketPnlSummaryForFilters({
  userId,
  username,
  from,
  to,
  pnlFilter,
  positionFilter,
  rootRowsPnlUsdc,
}: {
  userId: number;
  username: string;
  from?: string | null;
  to?: string | null;
  pnlFilter: AutoScopeTradeAnalysisPnlFilter;
  positionFilter: AutoScopeTradeAnalysisPositionFilter;
  rootRowsPnlUsdc: number;
}): Promise<OfficialMarketPnlSummary | null> {
  if (positionFilter === 'open') return null;

  const [activityConfig, marketSlugs] = await Promise.all([
    readDataApiActivityConfigForServer({ userId, username }),
    listBotMarketSlugs(userId),
  ]);
  if (!activityConfig.walletAddress || marketSlugs.size === 0) return null;

  const activity = await fetchDataApiActivity(activityConfig);
  const ledgers = buildOfficialMarketLedgersFromActivity({
    activity,
    marketSlugs,
    from,
    to,
    pnlFilter,
  });

  return buildOfficialMarketPnlSummaryFromLedgers({ ledgers, rootRowsPnlUsdc });
}
