import { readDataApiActivityConfigForServer } from '@/lib/config';
import type {
  AutoScopeTradeAnalysisPnlSourceStatus,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSummary,
  AutoScopeTradeAnalysisTimeRange,
} from '@/lib/types';
import { getOfficialMarketPnlSummaryForFilters } from './official-market-pnl';

const WALLET_PNL_CACHE_TTL_MS = 180_000;
const WALLET_PNL_REQUEST_TIMEOUT_MS = 12_000;
const CLOSED_POSITION_PAGE_SIZE = 50;
const CLOSED_POSITION_MAX_PAGES = 100;
const PNL_SOURCE_MISMATCH_TOLERANCE_USDC = 0.02;
const USER_PNL_API_BASE =
  process.env.POLYMARKET_USER_PNL_API_BASE || 'https://user-pnl-api.polymarket.com';

type PolymarketWalletPnlSource =
  | 'polymarket_leaderboard'
  | 'polymarket_user_pnl_history'
  | 'official_activity_window';

type PolymarketPositionSource =
  | 'closed_positions'
  | 'positions'
  | 'positions_redeemable_lost';

interface CacheEntry<T> {
  fetchedAtMs: number;
  value?: T;
  inFlight?: Promise<T>;
}

interface UserPnlPoint {
  t?: unknown;
  p?: unknown;
}

interface TimeRangeUserPnlRequest {
  interval: string;
  fidelity: string;
}

interface NormalizedPosition {
  marketSlug: string;
  tokenId: string | null;
  outcomeLabel: string | null;
  source: PolymarketPositionSource;
  pnlUsdc: number;
  totalBetUsdc: number | null;
  amountReturnedUsdc: number | null;
  realizedPnlUsdc: number | null;
  cashPnlUsdc: number | null;
}

interface PositionStats {
  index: Map<string, NormalizedPosition>;
  marketPnlIndex: Map<string, number>;
  marketCount: number;
  profitCount: number;
  lossCount: number;
  profitUsdc: number;
  lossUsdc: number;
  realizedPnlUsdc: number;
  openPnlUsdc: number;
  costBasisUsdc: number;
  netValueUsdc: number;
  largestLossUsdc: number | null;
}

interface PolymarketWalletPnlSummary {
  source: PolymarketWalletPnlSource;
  marketCount: number;
  profitCount: number;
  lossCount: number;
  totalPnlUsdc: number;
  realizedPnlUsdc: number;
  openPnlUsdc: number;
  lossUsdc: number;
  profitUsdc: number;
  costBasisUsdc: number;
  netValueUsdc: number;
  largestLossUsdc: number | null;
  officialBuyUsdc?: number;
  officialSellUsdc?: number;
  officialRedeemUsdc?: number;
  rootRowsPnlUsdc: number;
  officialDeltaUsdc: number;
  refreshedAt: string;
}

const jsonCache = new Map<string, CacheEntry<unknown>>();

function parseNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function parseString(value: unknown): string | null {
  const text = typeof value === 'string' ? value.trim() : '';
  return text || null;
}

function parseBoolean(value: unknown): boolean {
  return value === true || (typeof value === 'string' && value.toLowerCase() === 'true');
}

function roundCash(value: number): number {
  return Math.round(value * 100_000) / 100_000;
}

function getRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function getRows(body: unknown): Record<string, unknown>[] {
  if (Array.isArray(body)) {
    return body.reduce<Record<string, unknown>[]>((rows, row) => {
      const record = getRecord(row);
      if (record) rows.push(record);
      return rows;
    }, []);
  }
  const record = getRecord(body);
  const candidates = [record?.data, record?.results, record?.leaderboard];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) {
      return candidate.reduce<Record<string, unknown>[]>((rows, row) => {
        const candidateRecord = getRecord(row);
        if (candidateRecord) rows.push(candidateRecord);
        return rows;
      }, []);
    }
  }
  return [];
}

async function fetchJsonWithTimeout(url: URL): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), WALLET_PNL_REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(url, { signal: controller.signal });
    if (response.status === 400 && Number(url.searchParams.get('offset') ?? '0') > 0) {
      return [];
    }
    if (!response.ok) {
      throw new Error(`Polymarket API returned ${response.status} for ${url.pathname}`);
    }
    return response.json();
  } finally {
    clearTimeout(timeout);
  }
}

async function fetchCachedJson(url: URL): Promise<unknown> {
  const cacheKey = url.toString();
  const cached = jsonCache.get(cacheKey);
  const now = Date.now();
  if (cached?.value !== undefined && now - cached.fetchedAtMs < WALLET_PNL_CACHE_TTL_MS) {
    return cached.value;
  }
  if (cached?.inFlight) return cached.inFlight;

  const inFlight = fetchJsonWithTimeout(url).then((value) => {
    jsonCache.set(cacheKey, { fetchedAtMs: Date.now(), value });
    return value;
  });
  jsonCache.set(cacheKey, {
    fetchedAtMs: cached?.fetchedAtMs ?? 0,
    value: cached?.value,
    inFlight,
  });

  try {
    return await inFlight;
  } catch (err) {
    jsonCache.delete(cacheKey);
    throw err;
  }
}

function dataApiUrl(baseUrl: string, pathname: string): URL {
  return new URL(pathname, `${baseUrl.replace(/\/+$/, '')}/`);
}

export function mapTimeRangeToUserPnlRequest(
  timeRange: AutoScopeTradeAnalysisTimeRange
): TimeRangeUserPnlRequest | null {
  switch (timeRange) {
    case '3h':
    case '6h':
      return { interval: '6h', fidelity: '1h' };
    case '12h':
      return { interval: '12h', fidelity: '1h' };
    case '24h':
      return { interval: '1d', fidelity: '1h' };
    case '48h':
    case '1w':
      return { interval: '1w', fidelity: '3h' };
    case '1m':
      return { interval: '1m', fidelity: '12h' };
    default:
      return null;
  }
}

export function extractLeaderboardPnl(body: unknown): number | null {
  for (const row of getRows(body)) {
    const pnl = parseNumber(row.pnl);
    if (pnl != null) return roundCash(pnl);
  }
  return null;
}

export function buildUserPnlDelta(points: UserPnlPoint[]): number | null {
  const normalized = points
    .map((point) => ({ t: parseNumber(point.t), p: parseNumber(point.p) }))
    .filter((point): point is { t: number; p: number } => point.t != null && point.p != null)
    .sort((left, right) => left.t - right.t);
  const first = normalized[0];
  const last = normalized[normalized.length - 1];
  return first && last ? roundCash(last.p - first.p) : null;
}

function positionKey(marketSlug: string, tokenId: string | null, outcomeLabel: string | null): string {
  const slug = marketSlug.trim().toLowerCase();
  if (tokenId?.trim()) return `${slug}|asset:${tokenId.trim().toLowerCase()}`;
  return `${slug}|outcome:${(outcomeLabel ?? '').trim().toLowerCase()}`;
}

function marketKey(marketSlug: string): string {
  return marketSlug.trim().toLowerCase();
}

function productOrNull(left: number | null, right: number | null): number | null {
  return left == null || right == null ? null : roundCash(left * right);
}

function normalizeClosedPosition(row: Record<string, unknown>): NormalizedPosition | null {
  const marketSlug = parseString(row.slug ?? row.marketSlug);
  if (!marketSlug) return null;
  const pnl = parseNumber(row.realizedPnl);
  if (pnl == null) return null;
  const totalBet =
    parseNumber(row.totalBet) ??
    parseNumber(row.initialValue) ??
    productOrNull(parseNumber(row.totalBought), parseNumber(row.avgPrice));
  const amountReturned =
    parseNumber(row.amountReturned) ?? (totalBet == null ? null : Math.max(0, totalBet + pnl));
  return {
    marketSlug,
    tokenId: parseString(row.asset ?? row.tokenId),
    outcomeLabel: parseString(row.outcome),
    source: 'closed_positions',
    pnlUsdc: roundCash(pnl),
    totalBetUsdc: totalBet,
    amountReturnedUsdc: amountReturned == null ? null : roundCash(amountReturned),
    realizedPnlUsdc: roundCash(pnl),
    cashPnlUsdc: null,
  };
}

function openPositionPnl(row: Record<string, unknown>): number | null {
  const cashPnl = parseNumber(row.cashPnl);
  const realizedPnl = parseNumber(row.realizedPnl);
  if (cashPnl == null && realizedPnl == null) return null;
  return roundCash((cashPnl ?? 0) + (realizedPnl ?? 0));
}

function isRedeemableLostPosition(row: Record<string, unknown>): boolean {
  if (!parseBoolean(row.redeemable)) return false;
  const currentValue = parseNumber(row.currentValue);
  const curPrice = parseNumber(row.curPrice);
  const percentPnl = parseNumber(row.percentPnl);
  const percentRealizedPnl = parseNumber(row.percentRealizedPnl);
  return (
    (currentValue != null && currentValue <= 0) ||
    (curPrice != null && curPrice <= 0) ||
    (percentPnl != null && percentPnl <= -99.9) ||
    (percentRealizedPnl != null && percentRealizedPnl <= -99.9)
  );
}

function normalizeOpenPosition(
  row: Record<string, unknown>,
  source: PolymarketPositionSource
): NormalizedPosition | null {
  const marketSlug = parseString(row.slug ?? row.marketSlug);
  if (!marketSlug) return null;
  const totalBet =
    parseNumber(row.initialValue) ??
    parseNumber(row.totalBet) ??
    productOrNull(parseNumber(row.size), parseNumber(row.avgPrice));
  const pnl =
    source === 'positions_redeemable_lost'
      ? parseNumber(row.cashPnl) ?? parseNumber(row.realizedPnl) ?? (totalBet == null ? null : -totalBet)
      : openPositionPnl(row);
  if (pnl == null) return null;
  return {
    marketSlug,
    tokenId: parseString(row.asset ?? row.tokenId),
    outcomeLabel: parseString(row.outcome),
    source,
    pnlUsdc: roundCash(pnl),
    totalBetUsdc: totalBet,
    amountReturnedUsdc:
      source === 'positions_redeemable_lost' ? 0 : parseNumber(row.currentValue),
    realizedPnlUsdc: parseNumber(row.realizedPnl),
    cashPnlUsdc: parseNumber(row.cashPnl),
  };
}

function addPosition(index: Map<string, NormalizedPosition>, position: NormalizedPosition) {
  index.set(positionKey(position.marketSlug, position.tokenId, position.outcomeLabel), position);
}

export function buildPolymarketPositionStats({
  closedRows,
  openRows,
}: {
  closedRows: Record<string, unknown>[];
  openRows: Record<string, unknown>[];
}): PositionStats {
  const index = new Map<string, NormalizedPosition>();
  const positions: NormalizedPosition[] = [];
  const closedKeys = new Set<string>();

  for (const row of closedRows) {
    const position = normalizeClosedPosition(row);
    if (!position) continue;
    addPosition(index, position);
    closedKeys.add(positionKey(position.marketSlug, position.tokenId, position.outcomeLabel));
    positions.push(position);
  }

  for (const row of openRows) {
    const lost = isRedeemableLostPosition(row);
    const position = normalizeOpenPosition(row, lost ? 'positions_redeemable_lost' : 'positions');
    if (!position) continue;
    const key = positionKey(position.marketSlug, position.tokenId, position.outcomeLabel);
    if (lost && closedKeys.has(key)) continue;
    if (!index.has(key)) addPosition(index, position);
    positions.push(position);
  }

  const profitUsdc = roundCash(positions.reduce((sum, row) => sum + Math.max(row.pnlUsdc, 0), 0));
  const lossUsdc = roundCash(Math.abs(positions.reduce((sum, row) => sum + Math.min(row.pnlUsdc, 0), 0)));
  const marketPnlIndex = positions.reduce<Map<string, number>>((index, position) => {
    const key = marketKey(position.marketSlug);
    index.set(key, roundCash((index.get(key) ?? 0) + position.pnlUsdc));
    return index;
  }, new Map());
  const largestLoss = positions
    .filter((row) => row.pnlUsdc < 0)
    .map((row) => Math.abs(row.pnlUsdc))
    .sort((left, right) => right - left)[0];

  return {
    index,
    marketPnlIndex,
    marketCount: positions.length,
    profitCount: positions.filter((row) => row.pnlUsdc > 0).length,
    lossCount: positions.filter((row) => row.pnlUsdc < 0).length,
    profitUsdc,
    lossUsdc,
    realizedPnlUsdc: roundCash(
      positions
        .filter((row) => row.source !== 'positions')
        .reduce((sum, row) => sum + row.pnlUsdc, 0)
    ),
    openPnlUsdc: roundCash(
      positions
        .filter((row) => row.source === 'positions')
        .reduce((sum, row) => sum + row.pnlUsdc, 0)
    ),
    costBasisUsdc: roundCash(positions.reduce((sum, row) => sum + (row.totalBetUsdc ?? 0), 0)),
    netValueUsdc: roundCash(
      positions.reduce((sum, row) => sum + (row.amountReturnedUsdc ?? 0), 0)
    ),
    largestLossUsdc: largestLoss == null ? null : roundCash(largestLoss),
  };
}

async function fetchLeaderboardPnl(baseUrl: string, walletAddress: string): Promise<number | null> {
  const url = dataApiUrl(baseUrl, '/v1/leaderboard');
  url.searchParams.set('timePeriod', 'all');
  url.searchParams.set('orderBy', 'VOL');
  url.searchParams.set('limit', '1');
  url.searchParams.set('offset', '0');
  url.searchParams.set('category', 'overall');
  url.searchParams.set('user', walletAddress.trim().toLowerCase());
  return extractLeaderboardPnl(await fetchCachedJson(url));
}

async function fetchUserPnlDelta(
  walletAddress: string,
  request: TimeRangeUserPnlRequest
): Promise<number | null> {
  const url = new URL('/user-pnl', USER_PNL_API_BASE);
  url.searchParams.set('user_address', walletAddress.trim().toLowerCase());
  url.searchParams.set('interval', request.interval);
  url.searchParams.set('fidelity', request.fidelity);
  const body = await fetchCachedJson(url);
  return Array.isArray(body) ? buildUserPnlDelta(body as UserPnlPoint[]) : null;
}

async function fetchPositionPages({
  baseUrl,
  walletAddress,
  pathname,
  pageSize,
  maxPages,
}: {
  baseUrl: string;
  walletAddress: string;
  pathname: '/positions' | '/closed-positions';
  pageSize: number;
  maxPages: number;
}): Promise<Record<string, unknown>[]> {
  const rows: Record<string, unknown>[] = [];
  for (let page = 0; page < maxPages; page += 1) {
    const url = dataApiUrl(baseUrl, pathname);
    url.searchParams.set('user', walletAddress.trim().toLowerCase());
    url.searchParams.set('limit', String(pageSize));
    url.searchParams.set('offset', String(page * pageSize));
    const pageRows = getRows(await fetchCachedJson(url));
    rows.push(...pageRows);
    if (pageRows.length < pageSize) break;
  }
  return rows;
}

async function fetchPolymarketPositionStats({
  baseUrl,
  walletAddress,
  positionsPageSize,
  positionsMaxPages,
}: {
  baseUrl: string;
  walletAddress: string;
  positionsPageSize: number;
  positionsMaxPages: number;
}): Promise<PositionStats> {
  const [closedRows, openRows] = await Promise.all([
    fetchPositionPages({
      baseUrl,
      walletAddress,
      pathname: '/closed-positions',
      pageSize: CLOSED_POSITION_PAGE_SIZE,
      maxPages: CLOSED_POSITION_MAX_PAGES,
    }),
    fetchPositionPages({
      baseUrl,
      walletAddress,
      pathname: '/positions',
      pageSize: positionsPageSize,
      maxPages: positionsMaxPages,
    }),
  ]);
  return buildPolymarketPositionStats({ closedRows, openRows });
}

function resolvePnlSourceStatus({
  baseStatus,
  activityMarketPnlUsdc,
  positionMarketPnlUsdc,
}: {
  baseStatus: AutoScopeTradeAnalysisPnlSourceStatus | null;
  activityMarketPnlUsdc: number | null;
  positionMarketPnlUsdc: number | null;
}): AutoScopeTradeAnalysisPnlSourceStatus | null {
  if (
    activityMarketPnlUsdc != null &&
    positionMarketPnlUsdc != null &&
    Math.abs(activityMarketPnlUsdc - positionMarketPnlUsdc) >
      PNL_SOURCE_MISMATCH_TOLERANCE_USDC
  ) {
    return 'pnl_source_mismatch';
  }
  return baseStatus;
}

function applyPositionToRow(
  row: AutoScopeTradeAnalysisRow,
  stats: Pick<PositionStats, 'index' | 'marketPnlIndex'>
): AutoScopeTradeAnalysisRow {
  const position = stats.index.get(positionKey(row.marketSlug, row.tokenId, row.outcomeLabel));
  const positionMarketPnlUsdc = stats.marketPnlIndex.get(marketKey(row.marketSlug)) ?? null;
  const pnlSourceStatus = resolvePnlSourceStatus({
    baseStatus: row.pnlSourceStatus,
    activityMarketPnlUsdc: row.activityMarketPnlUsdc,
    positionMarketPnlUsdc,
  });
  if (!position) {
    return {
      ...row,
      positionMarketPnlUsdc,
      pnlSourceStatus,
      polymarketPositionPnlUsdc: null,
      polymarketPositionSource: null,
      polymarketTotalBetUsdc: null,
      polymarketAmountReturnedUsdc: null,
      polymarketRealizedPnlUsdc: null,
      polymarketCashPnlUsdc: null,
    };
  }
  return {
    ...row,
    positionMarketPnlUsdc,
    pnlSourceStatus,
    polymarketPositionPnlUsdc: position.pnlUsdc,
    polymarketPositionSource: position.source,
    polymarketTotalBetUsdc: position.totalBetUsdc,
    polymarketAmountReturnedUsdc: position.amountReturnedUsdc,
    polymarketRealizedPnlUsdc: position.realizedPnlUsdc,
    polymarketCashPnlUsdc: position.cashPnlUsdc,
  };
}

export async function enrichRowsWithPolymarketPositionPnl({
  userId,
  username,
  rows,
}: {
  userId: number;
  username: string;
  rows: AutoScopeTradeAnalysisRow[];
}): Promise<AutoScopeTradeAnalysisRow[]> {
  if (rows.length === 0) return rows;
  try {
    const config = await readDataApiActivityConfigForServer({ userId, username });
    const emptyStats = { index: new Map<string, NormalizedPosition>(), marketPnlIndex: new Map<string, number>() };
    if (!config.walletAddress) return rows.map((row) => applyPositionToRow(row, emptyStats));
    const stats = await fetchPolymarketPositionStats({
      baseUrl: config.baseUrl,
      walletAddress: config.walletAddress,
      positionsPageSize: config.pageSize,
      positionsMaxPages: config.maxPages,
    });
    return rows.map((row) => applyPositionToRow(row, stats));
  } catch (err) {
    console.error('Polymarket position PnL enrichment failed:', err);
    const emptyStats = { index: new Map<string, NormalizedPosition>(), marketPnlIndex: new Map<string, number>() };
    return rows.map((row) => applyPositionToRow(row, emptyStats));
  }
}

export function applyPolymarketWalletPnlSummary(
  rowSummary: AutoScopeTradeAnalysisSummary,
  official: PolymarketWalletPnlSummary
): AutoScopeTradeAnalysisSummary {
  return {
    ...rowSummary,
    referencePnlUsdc: official.totalPnlUsdc,
    referencePnlSource: official.source,
    referenceDeltaUsdc: roundCash(official.totalPnlUsdc - rowSummary.totalPnlUsdc),
    officialBuyUsdc: official.officialBuyUsdc,
    officialSellUsdc: official.officialSellUsdc,
    officialRedeemUsdc: official.officialRedeemUsdc,
    rootRowsPnlUsdc: official.rootRowsPnlUsdc,
    officialDeltaUsdc: roundCash(official.totalPnlUsdc - rowSummary.totalPnlUsdc),
  };
}

export async function getPolymarketWalletPnlSummaryForFilters({
  userId,
  username,
  timeRange,
  from,
  to,
  rootRowsPnlUsdc,
}: {
  userId: number;
  username: string;
  timeRange: AutoScopeTradeAnalysisTimeRange;
  from?: string | null;
  to?: string | null;
  rootRowsPnlUsdc: number;
}): Promise<PolymarketWalletPnlSummary | null> {
  if (timeRange === 'custom' || from || to) {
    const activity = await getOfficialMarketPnlSummaryForFilters({
      userId,
      username,
      from,
      to,
      pnlFilter: 'all',
      positionFilter: 'all',
      rootRowsPnlUsdc,
    });
    return activity
      ? { ...activity, source: 'official_activity_window', openPnlUsdc: 0 }
      : null;
  }

  const config = await readDataApiActivityConfigForServer({ userId, username });
  if (!config.walletAddress) return null;

  const userPnlRequest = mapTimeRangeToUserPnlRequest(timeRange);
  const [totalPnlUsdc, positionStats] = await Promise.all([
    userPnlRequest
      ? fetchUserPnlDelta(config.walletAddress, userPnlRequest)
      : fetchLeaderboardPnl(config.baseUrl, config.walletAddress),
    fetchPolymarketPositionStats({
      baseUrl: config.baseUrl,
      walletAddress: config.walletAddress,
      positionsPageSize: config.pageSize,
      positionsMaxPages: config.maxPages,
    }),
  ]);
  if (totalPnlUsdc == null) return null;

  return {
    source: userPnlRequest ? 'polymarket_user_pnl_history' : 'polymarket_leaderboard',
    marketCount: positionStats.marketCount,
    profitCount: positionStats.profitCount,
    lossCount: positionStats.lossCount,
    totalPnlUsdc,
    realizedPnlUsdc: positionStats.realizedPnlUsdc,
    openPnlUsdc: positionStats.openPnlUsdc,
    lossUsdc: positionStats.lossUsdc,
    profitUsdc: positionStats.profitUsdc,
    costBasisUsdc: positionStats.costBasisUsdc,
    netValueUsdc: positionStats.netValueUsdc,
    largestLossUsdc: positionStats.largestLossUsdc,
    rootRowsPnlUsdc: roundCash(rootRowsPnlUsdc),
    officialDeltaUsdc: roundCash(totalPnlUsdc - rootRowsPnlUsdc),
    refreshedAt: new Date().toISOString(),
  };
}

export const __polymarketWalletPnlTestUtils = {
  buildUserPnlDelta,
  extractLeaderboardPnl,
  mapTimeRangeToUserPnlRequest,
  buildPolymarketPositionStats,
  resolvePnlSourceStatus,
};
