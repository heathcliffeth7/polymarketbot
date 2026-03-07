import type { PoolClient } from 'pg';
import { pool } from '@/lib/db';
import type {
  PaginatedResponse,
  TradeFlowDefinition,
  TradeFlowDefinitionDetail,
  TradeFlowEdge,
  TradeFlowEnsureDualDcaSourceTradeRequest,
  TradeFlowEnsureDualDcaSourceTradeResult,
  TradeFlowEnsureSourceTradeRequest,
  TradeFlowEnsureSourceTradeResult,
  TradeFlowEvent,
  TradeFlowGraph,
  TradeFlowNode,
  TradeFlowOpenPositionOption,
  TradeFlowOpenPositionsResponse,
  TradeFlowRun,
  TradeFlowValidationIssue,
  TradeFlowValidationResult,
  TradeFlowVersion,
} from '@/lib/types';
import {
  isValidTelegramChatTarget,
  readPositionWalletAddress,
  readTelegramBotTokenForServer,
  readTelegramChatIdForServer,
  type UserConfigContext,
} from '@/lib/config';
import { getMarketOutcomesBySlug, getTradeBuilderWorkflowById } from './trade-builder';

type Queryable = {
  query: PoolClient['query'];
};

const SUPPORTED_NODE_TYPES = new Set([
  'trigger.market_price',
  'trigger.sell_progress',
  'trigger.open_positions',
  'trigger.position_drawdown',
  'trigger.time_window',
  'logic.if',
  'logic.switch',
  'logic.delay',
  'logic.retry',
  'action.resolve_market',
  'action.dual_dca',
  'action.place_order',
  'action.cancel_order',
  'action.update_order',
  'action.set_state',
  'action.notify',
  'action.telegram_notify',
]);

const POLYMARKET_DATA_API_BASE =
  process.env.POLYMARKET_DATA_API_BASE || 'https://data-api.polymarket.com';
const OPEN_POSITIONS_MIN_CURRENT_VALUE_USD = 1;
const OPEN_POSITION_OUTCOME_LABEL_CACHE_TTL_MS = 5 * 60 * 1000;
const OPEN_POSITION_OUTCOME_LABEL_ERROR_CACHE_TTL_MS = 60 * 1000;
const OPEN_POSITION_MARKET_SLUG_KEYS = ['slug', 'market_slug', 'marketSlug'];
const OPEN_POSITION_TOKEN_ID_KEYS = ['asset', 'tokenId', 'token_id', 'clobTokenId'];
const OPEN_POSITION_OUTCOME_LABEL_KEYS = [
  'outcomeLabel',
  'outcome_label',
  'outcomeName',
  'outcome_name',
  'label',
  'outcome',
];
const GENERIC_OPEN_POSITION_OUTCOME_LABELS = new Set([
  '',
  'unknown',
  'yes',
  'no',
  'true',
  'false',
  '1',
  '0',
]);

interface OpenPositionOutcomeLabelCacheEntry {
  expiresAt: number;
  byTokenId: Map<string, string>;
}

const openPositionOutcomeLabelCache = new Map<string, OpenPositionOutcomeLabelCacheEntry>();

const DEFAULT_GRAPH: TradeFlowGraph = {
  context: {},
  nodes: [],
  edges: [],
};

interface TradeFlowListFilters {
  userId: number;
  page?: number;
  limit?: number;
  status?: string;
  autoMigrateLegacy?: boolean;
}

interface TradeFlowRunFilters {
  userId: number;
  page?: number;
  limit?: number;
  definitionId?: number;
  status?: string;
}

interface CreateTradeFlowDefinitionInput {
  userId: number;
  name: string;
  description?: string | null;
  graphJson?: unknown;
  legacyWorkflowId?: number;
}

interface UpdateTradeFlowDefinitionInput {
  name?: string;
  description?: string | null;
  graphJson?: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function isSupportedTriggerCondition(value: unknown): value is 'cross_above' | 'cross_below' {
  return value === 'cross_above' || value === 'cross_below';
}

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function hasProvidedValue(value: unknown): boolean {
  if (typeof value === 'string') return value.trim().length > 0;
  return value != null;
}

function hasValidOptionalMaxPrice(maxPriceCentValue: unknown, legacyMaxPriceValue: unknown): boolean {
  if (hasProvidedValue(maxPriceCentValue)) {
    const maxPriceCent = toFiniteNumber(maxPriceCentValue);
    return maxPriceCent != null && maxPriceCent > 0 && maxPriceCent <= 100;
  }

  if (hasProvidedValue(legacyMaxPriceValue)) {
    const maxPrice = toFiniteNumber(legacyMaxPriceValue);
    return maxPrice != null && maxPrice > 0 && maxPrice <= 1;
  }

  return true;
}

function resolveConfiguredBinaryPrice(
  centValue: unknown,
  rawValue: unknown
): { provided: boolean; value: number | null } {
  if (hasProvidedValue(centValue)) {
    const cent = toFiniteNumber(centValue);
    if (cent != null && cent > 0 && cent <= 100) {
      return { provided: true, value: cent / 100 };
    }
    return { provided: true, value: null };
  }

  if (hasProvidedValue(rawValue)) {
    const raw = toFiniteNumber(rawValue);
    if (raw != null && raw > 0 && raw <= 1) {
      return { provided: true, value: raw };
    }
    return { provided: true, value: null };
  }

  return { provided: false, value: null };
}

function countValidOutcomeConditions(config: Record<string, unknown>): number {
  const raw = config.outcomeConditions;
  if (!Array.isArray(raw)) return 0;

  let validCount = 0;
  for (const item of raw) {
    if (!isRecord(item)) continue;
    const tokenId = toTrimmedString(item.tokenId);
    const outcomeLabel = toTrimmedString(item.outcomeLabel);
    const triggerCondition = toTrimmedString(item.triggerCondition);
    const triggerPriceCent = toFiniteNumber(item.triggerPriceCent);
    const triggerPrice = toFiniteNumber(item.triggerPrice);
    const hasValidTriggerPriceCent =
      triggerPriceCent != null && triggerPriceCent > 0 && triggerPriceCent <= 100;
    const hasValidTriggerPrice =
      triggerPrice != null && triggerPrice > 0 && triggerPrice <= 1;
    const hasValidMaxPrice = hasValidOptionalMaxPrice(item.maxPriceCent, item.maxPrice);
    if (!tokenId || !outcomeLabel) continue;
    if (!isSupportedTriggerCondition(triggerCondition)) continue;
    if (!hasValidTriggerPriceCent && !hasValidTriggerPrice) continue;
    if (!hasValidMaxPrice) continue;
    validCount += 1;
  }

  return validCount;
}

function toBooleanish(value: unknown): boolean | null {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value !== 0;
  if (typeof value !== 'string') return null;

  const normalized = value.trim().toLowerCase();
  if (['true', '1', 'yes', 'y', 'on'].includes(normalized)) return true;
  if (['false', '0', 'no', 'n', 'off'].includes(normalized)) return false;
  return null;
}

const RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME: Record<string, { asset: string; timeframe: string }> = {
  btc_5m_updown: { asset: 'btc', timeframe: '5m' },
  btc_15m_updown: { asset: 'btc', timeframe: '15m' },
  eth_5m_updown: { asset: 'eth', timeframe: '5m' },
  eth_15m_updown: { asset: 'eth', timeframe: '15m' },
  sol_5m_updown: { asset: 'sol', timeframe: '5m' },
  sol_15m_updown: { asset: 'sol', timeframe: '15m' },
  xrp_5m_updown: { asset: 'xrp', timeframe: '5m' },
  xrp_15m_updown: { asset: 'xrp', timeframe: '15m' },
};
const RESOLVE_MARKET_ALLOWED_ASSETS = new Set(['btc', 'eth', 'sol', 'xrp']);
const RESOLVE_MARKET_ALLOWED_TIMEFRAMES = new Set(['5m', '15m']);

function toTrimmedString(value: unknown): string {
  if (typeof value === 'string') return value.trim();
  if (typeof value === 'number' || typeof value === 'boolean') return String(value).trim();
  return '';
}

function normalizeDualDcaAsset(value: unknown): 'btc' | 'eth' | 'sol' | 'xrp' | null {
  const normalized = toTrimmedString(value).toLowerCase();
  if (normalized === 'btc' || normalized === 'eth' || normalized === 'sol' || normalized === 'xrp') {
    return normalized;
  }
  return null;
}

function normalizeDualDcaTimeframe(value: unknown): '5m' | '15m' | null {
  const raw = toTrimmedString(value).toLowerCase();
  if (raw === '5m' || raw === '5min' || raw === '5 min' || raw === '5') return '5m';
  if (raw === '15m' || raw === '15min' || raw === '15 min' || raw === '15') return '15m';
  return null;
}

function toSlugPart(value: unknown, fallback: string): string {
  const normalized = toTrimmedString(value)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return (normalized || fallback).slice(0, 48);
}

function toFinitePositiveInteger(value: unknown): number | null {
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed <= 0) return null;
  return Math.floor(parsed);
}

function pickString(row: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = toTrimmedString(row[key]);
    if (value) return value;
  }
  return '';
}

function pickNumber(row: Record<string, unknown>, keys: string[]): number | null {
  for (const key of keys) {
    const value = toFiniteNumber(row[key]);
    if (value != null) return value;
  }
  return null;
}

function extractOpenPositionOutcomeLabel(row: Record<string, unknown>): string {
  return pickString(row, OPEN_POSITION_OUTCOME_LABEL_KEYS);
}

function normalizeOpenPositionSlug(slug: string): string {
  return slug.trim().toLowerCase();
}

function normalizeOpenPositionTokenId(tokenId: string): string {
  return tokenId.trim().toLowerCase();
}

function openPositionOutcomeLabelIndexKey(marketSlug: string, tokenId: string): string {
  return `${normalizeOpenPositionSlug(marketSlug)}::${normalizeOpenPositionTokenId(tokenId)}`;
}

function isGenericOpenPositionOutcomeLabel(label: string): boolean {
  return GENERIC_OPEN_POSITION_OUTCOME_LABELS.has(label.trim().toLowerCase());
}

async function getMarketOutcomeLabelMapCached(slug: string): Promise<Map<string, string>> {
  const normalizedSlug = normalizeOpenPositionSlug(slug);
  if (!normalizedSlug) return new Map<string, string>();

  const now = Date.now();
  const cached = openPositionOutcomeLabelCache.get(normalizedSlug);
  if (cached && cached.expiresAt > now) {
    return cached.byTokenId;
  }

  try {
    const outcomes = await getMarketOutcomesBySlug(slug);
    const byTokenId = new Map<string, string>();
    for (const outcome of outcomes) {
      const tokenKey = normalizeOpenPositionTokenId(outcome.token_id);
      const label = String(outcome.label || '').trim();
      if (!tokenKey || !label) continue;
      byTokenId.set(tokenKey, label);
    }
    openPositionOutcomeLabelCache.set(normalizedSlug, {
      expiresAt: now + OPEN_POSITION_OUTCOME_LABEL_CACHE_TTL_MS,
      byTokenId,
    });
    return byTokenId;
  } catch (err) {
    console.warn('Open position outcome label resolve error:', slug, err);
    const empty = new Map<string, string>();
    openPositionOutcomeLabelCache.set(normalizedSlug, {
      expiresAt: now + OPEN_POSITION_OUTCOME_LABEL_ERROR_CACHE_TTL_MS,
      byTokenId: empty,
    });
    return empty;
  }
}

async function buildOpenPositionOutcomeLabelIndex(
  rows: Array<Record<string, unknown>>
): Promise<Map<string, string>> {
  const slugsNeedingLookup = new Set<string>();

  for (const row of rows) {
    const marketSlug = pickString(row, OPEN_POSITION_MARKET_SLUG_KEYS);
    const tokenId = pickString(row, OPEN_POSITION_TOKEN_ID_KEYS);
    if (!marketSlug || !tokenId) continue;
    if (!isGenericOpenPositionOutcomeLabel(extractOpenPositionOutcomeLabel(row))) continue;
    slugsNeedingLookup.add(marketSlug);
  }

  if (slugsNeedingLookup.size === 0) {
    return new Map<string, string>();
  }

  const bySlug = new Map<string, Map<string, string>>();
  const loaded = await Promise.all(
    Array.from(slugsNeedingLookup).map(async (slug) => [slug, await getMarketOutcomeLabelMapCached(slug)] as const)
  );
  for (const [slug, tokenMap] of loaded) {
    bySlug.set(normalizeOpenPositionSlug(slug), tokenMap);
  }

  const out = new Map<string, string>();
  for (const row of rows) {
    const marketSlug = pickString(row, OPEN_POSITION_MARKET_SLUG_KEYS);
    const tokenId = pickString(row, OPEN_POSITION_TOKEN_ID_KEYS);
    if (!marketSlug || !tokenId) continue;
    if (!isGenericOpenPositionOutcomeLabel(extractOpenPositionOutcomeLabel(row))) continue;
    const tokenMap = bySlug.get(normalizeOpenPositionSlug(marketSlug));
    if (!tokenMap) continue;
    const mapped = tokenMap.get(normalizeOpenPositionTokenId(tokenId));
    if (!mapped) continue;
    out.set(openPositionOutcomeLabelIndexKey(marketSlug, tokenId), mapped);
  }

  return out;
}

function resolveOpenPositionOutcomeLabel(
  rawOutcomeLabel: string,
  marketSlug: string,
  tokenId: string,
  labelIndex: Map<string, string>
): string {
  const normalizedRaw = rawOutcomeLabel.trim();
  if (marketSlug && tokenId && isGenericOpenPositionOutcomeLabel(normalizedRaw)) {
    const mapped = labelIndex.get(openPositionOutcomeLabelIndexKey(marketSlug, tokenId));
    if (mapped) return mapped;
  }
  return normalizedRaw || 'unknown';
}

interface OpenTradeMatchCandidate {
  tradeId: number;
  marketSlug: string;
  tokenId: string;
}

async function loadOpenTradeMatchCandidates(userId: number): Promise<OpenTradeMatchCandidate[]> {
  const res = await pool.query(
    `SELECT t.id AS trade_id, m.market_slug, lp.token_id
     FROM trades t
     JOIN markets m ON m.id = t.market_id
     LEFT JOIN leg_positions lp ON lp.trade_id = t.id
     WHERE t.user_id = $1
       AND (
         t.state NOT IN ('Settled', 'Halted', 'Idle')
         OR (t.state = 'Idle' AND COALESCE(t.strategy_mode, '') = 'manual_trade_builder')
       )
     ORDER BY t.opened_at DESC NULLS LAST, t.id DESC`
    ,
    [userId]
  );

  const out: OpenTradeMatchCandidate[] = [];
  for (const row of res.rows) {
    const tradeId = Number(row.trade_id);
    if (!Number.isFinite(tradeId) || tradeId <= 0) continue;
    out.push({
      tradeId,
      marketSlug: String(row.market_slug || '').trim(),
      tokenId: String(row.token_id || '').trim(),
    });
  }
  return out;
}

function matchOpenTradePosition(
  marketSlug: string,
  tokenId: string,
  candidates: OpenTradeMatchCandidate[]
): { matchedTradeId: number | null; matchConfidence: TradeFlowOpenPositionOption['matchConfidence'] } {
  const normalizedMarket = marketSlug.toLowerCase();
  const normalizedToken = tokenId.toLowerCase();

  if (normalizedMarket && normalizedToken) {
    const exact = candidates.find(
      (candidate) =>
        candidate.marketSlug.toLowerCase() === normalizedMarket &&
        candidate.tokenId.toLowerCase() === normalizedToken
    );
    if (exact) {
      return {
        matchedTradeId: exact.tradeId,
        matchConfidence: 'exact',
      };
    }
  }

  if (normalizedToken) {
    const byToken = candidates.find(
      (candidate) => candidate.tokenId.toLowerCase() === normalizedToken
    );
    if (byToken) {
      return {
        matchedTradeId: byToken.tradeId,
        matchConfidence: 'market_token',
      };
    }
  }

  if (normalizedMarket) {
    const byMarket = candidates.find(
      (candidate) => candidate.marketSlug.toLowerCase() === normalizedMarket
    );
    if (byMarket) {
      return {
        matchedTradeId: byMarket.tradeId,
        matchConfidence: 'market_token',
      };
    }
  }

  return {
    matchedTradeId: null,
    matchConfidence: 'none',
  };
}

async function fetchPolymarketOpenPositions(
  walletAddress: string
): Promise<Array<Record<string, unknown>>> {
  const base = POLYMARKET_DATA_API_BASE.replace(/\/$/, '');
  const query = new URLSearchParams({
    user: walletAddress.toLowerCase(),
    sizeThreshold: '0',
    limit: '500',
    offset: '0',
  });
  const url = `${base}/positions?${query.toString()}`;

  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) {
    throw new Error(`Polymarket open positions isteği başarısız (HTTP ${res.status})`);
  }

  const payload = (await res.json()) as unknown;
  if (Array.isArray(payload)) {
    return payload.filter((item): item is Record<string, unknown> => isRecord(item));
  }
  if (isRecord(payload)) {
    if (Array.isArray(payload.data)) {
      return payload.data.filter((item): item is Record<string, unknown> => isRecord(item));
    }
    if (Array.isArray(payload.positions)) {
      return payload.positions.filter((item): item is Record<string, unknown> => isRecord(item));
    }
  }
  return [];
}

function normalizeOutcomeToLegSide(outcomeLabel: string): 'yes' | 'no' {
  const normalized = outcomeLabel.trim().toLowerCase();
  if (normalized === 'no' || normalized === 'false' || normalized === '0') return 'no';
  return 'yes';
}

function estimateSourceTradeValues(input: TradeFlowEnsureSourceTradeRequest): {
  entryPrice: number;
  qty: number;
  notionalUsdc: number;
} {
  const rawAvgPrice = toFiniteNumber(input.avgPrice);
  const normalizedPrice =
    rawAvgPrice != null && rawAvgPrice > 0
      ? rawAvgPrice > 1 && rawAvgPrice <= 100
        ? rawAvgPrice / 100
        : rawAvgPrice
      : 0.5;
  const entryPrice = Math.max(0.01, Math.min(0.99, normalizedPrice));

  const sizeAbs = Math.abs(toFiniteNumber(input.size) ?? 0);
  const currentValueAbs = Math.abs(toFiniteNumber(input.currentValue) ?? 0);

  const qty = sizeAbs > 0 ? sizeAbs : Math.max(0.0001, currentValueAbs / entryPrice);
  const notionalCandidate = currentValueAbs > 0 ? currentValueAbs : qty * entryPrice;
  const notionalUsdc = Math.max(1, Number.isFinite(notionalCandidate) ? notionalCandidate : 5);

  return {
    entryPrice,
    qty,
    notionalUsdc,
  };
}

export async function ensureSourceTradeForOpenPosition(
  userId: number,
  input: TradeFlowEnsureSourceTradeRequest
): Promise<TradeFlowEnsureSourceTradeResult> {
  const marketSlug = toTrimmedString(input.marketSlug);
  const tokenId = toTrimmedString(input.tokenId);
  const outcomeLabel = toTrimmedString(input.outcomeLabel) || tokenId || 'unknown';

  if (!marketSlug) {
    throw new Error('marketSlug zorunlu.');
  }
  if (!tokenId) {
    throw new Error('tokenId zorunlu.');
  }

  const exactRes = await pool.query(
    `SELECT t.id
     FROM trades t
     JOIN markets m ON m.id = t.market_id
     LEFT JOIN leg_positions lp ON lp.trade_id = t.id
     WHERE LOWER(m.market_slug) = LOWER($1)
       AND LOWER(COALESCE(lp.token_id, '')) = LOWER($2)
       AND t.user_id = $3
       AND t.state NOT IN ('Settled', 'Halted')
     ORDER BY t.opened_at DESC NULLS LAST, t.id DESC
     LIMIT 1`,
    [marketSlug, tokenId, userId]
  );
  const existingTradeId = Number(exactRes.rows[0]?.id);
  if (Number.isFinite(existingTradeId) && existingTradeId > 0) {
    return {
      sourceTradeId: existingTradeId,
      created: false,
    };
  }

  const { entryPrice, qty, notionalUsdc } = estimateSourceTradeValues(input);

  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const now = new Date();
    const startsAt = new Date(now.getTime() - 60 * 60 * 1000);
    const endsAt = new Date(now.getTime() + 30 * 24 * 60 * 60 * 1000);

    const marketRes = await client.query(
      `INSERT INTO markets (market_slug, starts_at, ends_at, status)
       VALUES ($1, $2, $3, 'open')
       ON CONFLICT (market_slug) DO UPDATE SET
         starts_at = LEAST(markets.starts_at, EXCLUDED.starts_at),
         ends_at = GREATEST(markets.ends_at, EXCLUDED.ends_at),
         status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE 'open' END
       RETURNING id`,
      [marketSlug, startsAt, endsAt]
    );
    const marketId = Number(marketRes.rows[0]?.id);
    if (!Number.isFinite(marketId) || marketId <= 0) {
      throw new Error('Market oluşturulamadı.');
    }

    const tradeRes = await client.query(
      `INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, strategy_mode, opened_at)
       VALUES ($1, $2, 'Idle', $3, $4, 'manual_trade_builder', NOW())
       RETURNING id`,
      [marketId, userId, entryPrice, notionalUsdc]
    );
    const tradeId = Number(tradeRes.rows[0]?.id);
    if (!Number.isFinite(tradeId) || tradeId <= 0) {
      throw new Error('Source trade oluşturulamadı.');
    }

    await client.query(
      `INSERT INTO leg_positions
         (trade_id, leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price, updated_at)
       VALUES
         ($1, $2, $3, $4, $5, 1, $5, NOW())
       ON CONFLICT (trade_id, leg_side) DO UPDATE SET
         token_id = EXCLUDED.token_id,
         qty = EXCLUDED.qty,
         avg_entry = EXCLUDED.avg_entry,
         levels_filled = GREATEST(leg_positions.levels_filled, EXCLUDED.levels_filled),
         last_fill_price = EXCLUDED.last_fill_price,
         updated_at = NOW()`,
      [tradeId, normalizeOutcomeToLegSide(outcomeLabel), tokenId, qty, entryPrice]
    );

    await client.query('COMMIT');
    return {
      sourceTradeId: tradeId,
      created: true,
    };
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function ensureDualDcaSourceTrade(
  userId: number,
  input: TradeFlowEnsureDualDcaSourceTradeRequest
): Promise<TradeFlowEnsureDualDcaSourceTradeResult> {
  const asset = normalizeDualDcaAsset(input.asset);
  if (!asset) {
    throw new Error('asset must be one of: btc, eth, sol, xrp.');
  }
  const timeframe = normalizeDualDcaTimeframe(input.timeframe);
  if (!timeframe) {
    throw new Error('timeframe must be one of: 5m, 15m.');
  }

  const definitionId = toFinitePositiveInteger(input.definitionId);
  const nodeKeyPart = toSlugPart(input.nodeKey, 'root');
  const definitionPart = definitionId == null ? 'd0' : `d${definitionId}`;
  const marketSlug = `dual-dca-source-${definitionPart}-${asset}-${timeframe}-${nodeKeyPart}`;
  const tokenId = `dual-dca-seed-${asset}-${timeframe}`;
  const marketTitle = `Dual DCA Source ${asset.toUpperCase()} ${timeframe}`;

  return ensureSourceTradeForOpenPosition(userId, {
    marketSlug,
    tokenId,
    outcomeLabel: 'yes',
    marketTitle,
    size: 1,
    avgPrice: 0.5,
    currentValue: 1,
  });
}

export async function getTradeFlowOpenPositions(
  context: UserConfigContext
): Promise<TradeFlowOpenPositionsResponse> {
  const walletAddress = (await readPositionWalletAddress(context)).trim();
  if (!walletAddress) {
    throw new Error(
      'Open positions için cüzdan adresi bulunamadı. Settings -> Exchange ekranindan Wallet Address veya Gnosis Safe Address tanimlayin.'
    );
  }

  const [openRows, candidates] = await Promise.all([
    fetchPolymarketOpenPositions(walletAddress),
    loadOpenTradeMatchCandidates(context.userId),
  ]);
  const outcomeLabelIndex = await buildOpenPositionOutcomeLabelIndex(openRows);

  const positions = openRows
    .map((row, idx): TradeFlowOpenPositionOption | null => {
      const marketTitle =
        pickString(row, ['title', 'question', 'marketTitle', 'market_title', 'name']) ||
        'Untitled market';
      const marketSlug = pickString(row, OPEN_POSITION_MARKET_SLUG_KEYS);
      const tokenId = pickString(row, OPEN_POSITION_TOKEN_ID_KEYS);
      const rawOutcomeLabel = extractOpenPositionOutcomeLabel(row);
      const outcomeLabel = resolveOpenPositionOutcomeLabel(
        rawOutcomeLabel,
        marketSlug,
        tokenId,
        outcomeLabelIndex
      );
      const size = pickNumber(row, ['size', 'amount', 'positionSize', 'balance']) ?? 0;
      const avgPrice = pickNumber(row, ['avgPrice', 'avg_price', 'averagePrice', 'entryPrice']);
      const currentValue = pickNumber(row, ['currentValue', 'current_value', 'value']);
      const unrealizedPnl = pickNumber(row, ['cashPnl', 'unrealizedPnl', 'pnl']);

      if (!marketSlug && !tokenId) {
        return null;
      }

      const positionId = pickString(row, ['positionId', 'position_id', 'id']);
      const positionKey =
        positionId || `${marketSlug || 'market'}:${tokenId || 'token'}:${outcomeLabel}:${idx}`;
      const matched = matchOpenTradePosition(marketSlug, tokenId, candidates);

      return {
        positionKey,
        marketTitle,
        marketSlug,
        tokenId,
        outcomeLabel,
        size,
        avgPrice,
        currentValue,
        unrealizedPnl,
        walletAddress,
        matchedTradeId: matched.matchedTradeId,
        matchConfidence: matched.matchConfidence,
      };
    })
    .filter((item): item is TradeFlowOpenPositionOption => !!item)
    .filter(
      (item) =>
        item.currentValue != null &&
        Number.isFinite(item.currentValue) &&
        item.currentValue >= OPEN_POSITIONS_MIN_CURRENT_VALUE_USD
    )
    .sort((a, b) => Math.abs(b.size) - Math.abs(a.size));

  return {
    data: positions,
    meta: {
      walletAddressUsed: walletAddress,
      count: positions.length,
      minCurrentValueUsd: OPEN_POSITIONS_MIN_CURRENT_VALUE_USD,
      fetchedAt: new Date().toISOString(),
    },
  };
}

function pushNodeError(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  code: string,
  message: string
) {
  issues.push({
    severity: 'error',
    code,
    message,
    nodeKey: node.key,
  });
}

function pushNodeWarning(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  code: string,
  message: string
) {
  issues.push({
    severity: 'warning',
    code,
    message,
    nodeKey: node.key,
  });
}

function toNode(raw: unknown, idx: number): TradeFlowNode | null {
  if (!isRecord(raw)) return null;
  const keyRaw = String(raw.key ?? '').trim();
  const typeRaw = String(raw.type ?? '').trim();
  if (!keyRaw || !typeRaw) return null;

  const positionX = Number(raw.positionX);
  const positionY = Number(raw.positionY);

  return {
    key: keyRaw,
    type: typeRaw,
    positionX: Number.isFinite(positionX) ? positionX : idx * 220,
    positionY: Number.isFinite(positionY) ? positionY : 80,
    config: isRecord(raw.config) ? raw.config : {},
  };
}

function toEdge(raw: unknown, idx: number): TradeFlowEdge | null {
  if (!isRecord(raw)) return null;
  const keyRaw = String(raw.key ?? '').trim() || `edge_${idx + 1}`;
  const sourceRaw = String(raw.source ?? '').trim();
  const targetRaw = String(raw.target ?? '').trim();
  if (!sourceRaw || !targetRaw) return null;

  return {
    key: keyRaw,
    source: sourceRaw,
    target: targetRaw,
    type: String(raw.type ?? 'default').trim() || 'default',
    condition: isRecord(raw.condition) ? raw.condition : null,
  };
}

export function normalizeTradeFlowGraph(graphJson: unknown): TradeFlowGraph {
  if (!isRecord(graphJson)) return DEFAULT_GRAPH;
  const contextRaw = isRecord(graphJson.context) ? graphJson.context : {};
  const nodesRaw = Array.isArray(graphJson.nodes) ? graphJson.nodes : [];
  const edgesRaw = Array.isArray(graphJson.edges) ? graphJson.edges : [];

  const nodes = nodesRaw
    .map((row, idx) => toNode(row, idx))
    .filter((row): row is TradeFlowNode => !!row);

  const edges = edgesRaw
    .map((row, idx) => toEdge(row, idx))
    .filter((row): row is TradeFlowEdge => !!row);

  return {
    context: contextRaw,
    nodes,
    edges,
  };
}

function detectCycles(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): boolean {
  const adjacency = new Map<string, string[]>();
  for (const node of nodes) adjacency.set(node.key, []);
  for (const edge of edges) {
    const list = adjacency.get(edge.source);
    if (list) list.push(edge.target);
  }

  const visited = new Set<string>();
  const stack = new Set<string>();

  const dfs = (nodeKey: string): boolean => {
    if (stack.has(nodeKey)) return true;
    if (visited.has(nodeKey)) return false;
    visited.add(nodeKey);
    stack.add(nodeKey);

    for (const next of adjacency.get(nodeKey) || []) {
      if (dfs(next)) return true;
    }

    stack.delete(nodeKey);
    return false;
  };

  for (const node of nodes) {
    if (dfs(node.key)) return true;
  }

  return false;
}

function collectRootNodeKeys(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): Set<string> {
  const incoming = new Set(edges.map((edge) => edge.target));
  return new Set(nodes.filter((node) => !incoming.has(node.key)).map((node) => node.key));
}

function collectReachableFromTriggers(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): Set<string> {
  const adjacency = new Map<string, string[]>();
  for (const node of nodes) adjacency.set(node.key, []);
  for (const edge of edges) {
    const list = adjacency.get(edge.source);
    if (list) list.push(edge.target);
  }

  const triggerStarts = nodes
    .filter((node) => node.type.startsWith('trigger.'))
    .map((node) => node.key);
  const rootNodeKeys = collectRootNodeKeys(nodes, edges);
  const dualDcaRootStarts = nodes
    .filter((node) => node.type === 'action.dual_dca' && rootNodeKeys.has(node.key))
    .map((node) => node.key);
  const queue = triggerStarts.length > 0 ? triggerStarts : dualDcaRootStarts;
  const reachable = new Set<string>(queue);

  while (queue.length > 0) {
    const current = queue.shift() as string;
    for (const next of adjacency.get(current) || []) {
      if (reachable.has(next)) continue;
      reachable.add(next);
      queue.push(next);
    }
  }

  return reachable;
}

function hasUpstreamAutoScopeMarketTrigger(nodeKey: string, graph: TradeFlowGraph): boolean {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }

  const visited = new Set<string>();
  const queue = [nodeKey];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      if (
        sourceNode.type === 'trigger.market_price' &&
        toTrimmedString((isRecord(sourceNode.config) ? sourceNode.config : {}).marketMode).toLowerCase() === 'auto_scope'
      ) {
        return true;
      }
      queue.push(sourceKey);
    }
  }

  return false;
}

function validateNodeConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph
) {
  const config = isRecord(node.config) ? node.config : {};
  const graphSourceTradeId = toFiniteNumber(graph.context.sourceTradeId);
  const graphMarketSlug = String(graph.context.marketSlug ?? '').trim();
  const graphTokenId = String(graph.context.tokenId ?? '').trim();
  const graphOutcomeLabel = String(graph.context.outcomeLabel ?? '').trim();
  const hasResolveMarketNode = graph.nodes.some((candidate) => candidate.type === 'action.resolve_market');
  const hasUpstreamMarketPriceAutoScope = hasUpstreamAutoScopeMarketTrigger(node.key, graph);

  if (node.type === 'trigger.market_price') {
    const marketMode = toTrimmedString(config.marketMode).toLowerCase();
    const autoScope = marketMode === 'auto_scope';
    const protectionMode = toTrimmedString(config.protectionMode).toLowerCase();
    const protectionPreset = toTrimmedString(config.protectionPreset).toLowerCase();
    if (autoScope) {
      const marketScope = toTrimmedString(config.marketScope).toLowerCase();
      if (!marketScope) {
        pushNodeError(
          issues,
          node,
          'missing_market_scope',
          'trigger.market_price auto_scope requires marketScope.'
        );
      } else if (!RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]) {
        pushNodeError(
          issues,
          node,
          'invalid_market_scope',
          'trigger.market_price marketScope is unsupported.'
        );
      }
      const marketSelection = toTrimmedString(config.marketSelection).toLowerCase();
      if (marketSelection && marketSelection !== 'latest_by_slug') {
        pushNodeError(
          issues,
          node,
          'invalid_market_selection',
          'trigger.market_price marketSelection must be latest_by_slug.'
        );
      }
      if (protectionMode && protectionMode !== 'off' && protectionMode !== 'underlying_confirm') {
        pushNodeError(
          issues,
          node,
          'invalid_protection_mode',
          'trigger.market_price protectionMode must be off or underlying_confirm.'
        );
      }
      if (protectionMode === 'underlying_confirm') {
        if (!marketScope || !RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]) {
          pushNodeError(
            issues,
            node,
            'invalid_protection_scope',
            'trigger.market_price underlying_confirm requires a supported auto_scope marketScope.'
          );
        }
        if (
          protectionPreset &&
          protectionPreset !== 'loose' &&
          protectionPreset !== 'balanced' &&
          protectionPreset !== 'strict'
        ) {
          pushNodeError(
            issues,
            node,
            'invalid_protection_preset',
            'trigger.market_price protectionPreset must be loose, balanced, or strict.'
          );
        }
      }
    } else if (!String(config.marketSlug ?? graphMarketSlug).trim()) {
      pushNodeError(
        issues,
        node,
        'missing_market_slug',
        'trigger.market_price requires marketSlug in node config or graph context.'
      );
    }
    if (!autoScope && protectionMode === 'underlying_confirm') {
      pushNodeError(
        issues,
        node,
        'invalid_protection_mode_scope',
        'trigger.market_price underlying_confirm is only valid when marketMode is auto_scope.'
      );
    }

    if (config.confirmationMs != null && toTrimmedString(config.confirmationMs).length > 0) {
      const confirmationMs = toFiniteNumber(config.confirmationMs);
      if (
        confirmationMs == null ||
        !Number.isInteger(confirmationMs) ||
        confirmationMs < 0
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_confirmation_ms',
          'trigger.market_price confirmationMs must be an integer >= 0.'
        );
      }
    }

    const priceMode = toTrimmedString(config.priceMode).toLowerCase();
    const validPriceModes = ['midpoint', 'raw', 'best_bid', 'best_ask'];
    if (priceMode && !validPriceModes.includes(priceMode)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_mode',
        'trigger.market_price priceMode must be midpoint, raw, best_bid, or best_ask.'
      );
    }

    if (countValidOutcomeConditions(config) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_conditions',
        'trigger.market_price requires at least one valid outcome condition.'
      );
    }
  }

  if (node.type === 'trigger.sell_progress') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'trigger.sell_progress requires sourceTradeId in node config or graph context.'
      );
    }
  }

  if (node.type === 'trigger.open_positions') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'trigger.open_positions requires sourceTradeId in node config or graph context.'
      );
    }
    if (countValidOutcomeConditions(config) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_conditions',
        'trigger.open_positions requires at least one valid outcome condition.'
      );
    }

    const minPositionQty = toFiniteNumber(config.minPositionQty);
    if (minPositionQty != null && minPositionQty < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_min_position_qty',
        'trigger.open_positions minPositionQty must be >= 0.'
      );
    }

    const triggerConditionRaw = config.triggerCondition;
    const hasTriggerCondition = String(triggerConditionRaw ?? '').trim().length > 0;
    if (hasTriggerCondition && !isSupportedTriggerCondition(triggerConditionRaw)) {
      pushNodeError(
        issues,
        node,
        'invalid_trigger_condition',
        'trigger.open_positions triggerCondition must be cross_above or cross_below.'
      );
    }
    if (isSupportedTriggerCondition(triggerConditionRaw)) {
      const triggerPriceCent = toFiniteNumber(config.triggerPriceCent);
      const triggerPrice = toFiniteNumber(config.triggerPrice);
      const maxPriceCentProvided = hasProvidedValue(config.maxPriceCent);
      const maxPriceProvided = !maxPriceCentProvided && hasProvidedValue(config.maxPrice);
      if (triggerPriceCent == null && triggerPrice == null) {
        pushNodeError(
          issues,
          node,
          'missing_trigger_price',
          'trigger.open_positions triggerCondition requires triggerPriceCent (or legacy triggerPrice).'
        );
      }
      if (triggerPriceCent != null && (triggerPriceCent <= 0 || triggerPriceCent > 100)) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_price_cent',
          'trigger.open_positions triggerPriceCent must be in (0, 100].'
        );
      }
      if (triggerPrice != null && (triggerPrice <= 0 || triggerPrice > 1)) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_price',
          'trigger.open_positions triggerPrice must be in (0, 1].'
        );
      }
      if (maxPriceCentProvided) {
        const maxPriceCent = toFiniteNumber(config.maxPriceCent);
        if (maxPriceCent == null || maxPriceCent <= 0 || maxPriceCent > 100) {
          pushNodeError(
            issues,
            node,
            'invalid_max_price_cent',
            'trigger.open_positions maxPriceCent must be in (0, 100].'
          );
        }
      } else if (maxPriceProvided) {
        const maxPrice = toFiniteNumber(config.maxPrice);
        if (maxPrice == null || maxPrice <= 0 || maxPrice > 1) {
          pushNodeError(
            issues,
            node,
            'invalid_max_price',
            'trigger.open_positions maxPrice must be in (0, 1].'
          );
        }
      }
      const marketSlug = String(config.marketSlug ?? graph.context.marketSlug ?? '').trim();
      if (!marketSlug) {
        pushNodeError(
          issues,
          node,
          'missing_market_slug',
          'trigger.open_positions with triggerCondition requires marketSlug.'
        );
      }
      const tokenId = String(config.tokenId ?? graph.context.tokenId ?? '').trim();
      if (!tokenId) {
        pushNodeError(
          issues,
          node,
          'missing_token_id',
          'trigger.open_positions with triggerCondition requires tokenId.'
        );
      }
    }

    const minIntervalMs = toFiniteNumber(config.minIntervalMs);
    if (minIntervalMs != null && minIntervalMs < 250) {
      pushNodeError(
        issues,
        node,
        'invalid_min_interval',
        'trigger.open_positions minIntervalMs must be >= 250.'
      );
    }
  }

  if (node.type === 'trigger.position_drawdown') {
    const marketSlug = String(config.marketSlug ?? graphMarketSlug).trim();
    if (!marketSlug) {
      pushNodeError(
        issues,
        node,
        'missing_market_slug',
        'trigger.position_drawdown requires marketSlug in node config or graph context.'
      );
    }

    const tokenId = String(config.tokenId ?? graphTokenId).trim();
    if (!tokenId) {
      pushNodeError(
        issues,
        node,
        'missing_token_id',
        'trigger.position_drawdown requires tokenId in node config or graph context.'
      );
    }
    const outcomeLabel = String(config.outcomeLabel ?? graphOutcomeLabel).trim();
    if (!outcomeLabel) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_label',
        'trigger.position_drawdown requires outcomeLabel in node config or graph context.'
      );
    }

    const entryPriceCent = toFiniteNumber(config.entryPriceCent);
    const entryPrice = toFiniteNumber(config.entryPrice);
    if (entryPriceCent == null && entryPrice == null) {
      pushNodeError(
        issues,
        node,
        'missing_entry_price',
        'trigger.position_drawdown requires entryPriceCent (or legacy entryPrice).'
      );
    }
    if (entryPriceCent != null && (entryPriceCent <= 0 || entryPriceCent > 100)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_price_cent',
        'trigger.position_drawdown entryPriceCent must be in (0, 100].'
      );
    }
    if (entryPrice != null && (entryPrice <= 0 || entryPrice > 1)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_price',
        'trigger.position_drawdown entryPrice must be in (0, 1].'
      );
    }

    const minIntervalMs = toFiniteNumber(config.minIntervalMs);
    if (minIntervalMs != null && minIntervalMs < 250) {
      pushNodeError(
        issues,
        node,
        'invalid_min_interval',
        'trigger.position_drawdown minIntervalMs must be >= 250.'
      );
    }

    const combineMode = toTrimmedString(config.combineMode).toLowerCase();
    if (combineMode && combineMode !== 'and' && combineMode !== 'or') {
      pushNodeError(
        issues,
        node,
        'invalid_combine_mode',
        'trigger.position_drawdown combineMode must be and, or, or empty.'
      );
    }

    let validRuleCount = 0;
    let invalidDirectionFound = false;
    const hasDeprecatedWindowSec =
      Object.prototype.hasOwnProperty.call(config, 'windowSec') ||
      (Array.isArray(config.lossRules) &&
        config.lossRules.some(
          (item) => isRecord(item) && Object.prototype.hasOwnProperty.call(item, 'windowSec')
        ));
    if (Array.isArray(config.lossRules)) {
      for (const item of config.lossRules) {
        if (!isRecord(item)) continue;
        const direction = toTrimmedString(item.direction).toLowerCase();
        if (direction && direction !== 'down' && direction !== 'up') {
          invalidDirectionFound = true;
          continue;
        }
        const lossPct = toFiniteNumber(item.lossPct);
        if (lossPct == null || lossPct <= 0 || lossPct > 100) {
          continue;
        }
        const windowMs = toFiniteNumber(item.windowMs);
        if (windowMs != null && windowMs <= 0) {
          continue;
        }
        validRuleCount += 1;
      }
    } else {
      const legacyLossPct = toFiniteNumber(config.lossPct);
      const legacyWindowMs = toFiniteNumber(config.windowMs);
      if (
        legacyLossPct != null &&
        legacyLossPct > 0 &&
        legacyLossPct <= 100 &&
        (legacyWindowMs == null || legacyWindowMs > 0)
      ) {
        validRuleCount += 1;
      }
    }

    if (invalidDirectionFound) {
      pushNodeError(
        issues,
        node,
        'invalid_rule_direction',
        'trigger.position_drawdown lossRules[].direction must be down, up, or empty.'
      );
    }
    if (hasDeprecatedWindowSec) {
      pushNodeError(
        issues,
        node,
        'invalid_deprecated_window_sec',
        'trigger.position_drawdown windowSec is deprecated; use windowMs.'
      );
    }

    if (validRuleCount <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_loss_rules',
        'trigger.position_drawdown requires at least one valid loss rule (lossPct in (0,100], optional windowMs > 0).'
      );
    }
  }

  if (node.type === 'trigger.time_window') {
    const startAt = config.startAt == null ? null : String(config.startAt);
    const endAt = config.endAt == null ? null : String(config.endAt);
    if (startAt && Number.isNaN(new Date(startAt).getTime())) {
      pushNodeError(issues, node, 'invalid_start_at', 'trigger.time_window startAt must be RFC3339 datetime.');
    }
    if (endAt && Number.isNaN(new Date(endAt).getTime())) {
      pushNodeError(issues, node, 'invalid_end_at', 'trigger.time_window endAt must be RFC3339 datetime.');
    }
  }

  if (node.type === 'logic.if' && !isRecord(config.expression)) {
    pushNodeError(issues, node, 'missing_expression', 'logic.if requires expression object (JSONLogic).');
  }

  if (node.type === 'logic.switch' && config.expression === undefined) {
    pushNodeError(issues, node, 'missing_expression', 'logic.switch requires expression.');
  }

  if (node.type === 'logic.delay') {
    const delayMs = toFiniteNumber(config.delayMs ?? config.ms);
    if (delayMs != null && delayMs < 0) {
      pushNodeError(issues, node, 'invalid_delay', 'logic.delay delayMs must be >= 0.');
    }
  }

  if (node.type === 'logic.retry') {
    const maxAttempts = toFiniteNumber(config.maxAttempts);
    if (maxAttempts != null && maxAttempts < 1) {
      pushNodeError(issues, node, 'invalid_max_attempts', 'logic.retry maxAttempts must be >= 1.');
    }
  }

  if (node.type === 'action.resolve_market') {
    const marketScope = String(config.marketScope ?? '').trim().toLowerCase();
    const marketScopeResolved = marketScope
      ? RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope] || null
      : null;
    const asset = String(config.asset ?? marketScopeResolved?.asset ?? '').trim().toLowerCase();
    const timeframe = String(config.timeframe ?? marketScopeResolved?.timeframe ?? '').trim().toLowerCase();

    if (marketScope && !marketScopeResolved) {
      pushNodeWarning(
        issues,
        node,
        'legacy_scope_unknown',
        'action.resolve_market marketScope is unknown; asset/timeframe should be used.'
      );
    }
    if (asset && !RESOLVE_MARKET_ALLOWED_ASSETS.has(asset)) {
      pushNodeError(
        issues,
        node,
        'invalid_asset',
        'action.resolve_market asset must be one of: btc, eth, sol, xrp.'
      );
    }
    if (timeframe && !RESOLVE_MARKET_ALLOWED_TIMEFRAMES.has(timeframe)) {
      pushNodeError(
        issues,
        node,
        'invalid_timeframe',
        'action.resolve_market timeframe must be one of: 5m, 15m.'
      );
    }
    if ((!asset || !timeframe) && !marketScopeResolved) {
      pushNodeWarning(
        issues,
        node,
        'missing_asset_timeframe',
        'action.resolve_market missing asset/timeframe; runtime falls back to bot market_scope.'
      );
    }

    const selection = String(config.selection ?? '').trim();
    if (selection && selection !== 'latest_by_slug') {
      pushNodeError(
        issues,
        node,
        'invalid_selection',
        'action.resolve_market selection must be latest_by_slug.'
      );
    }

    const outcomeLabel = String(config.outcomeLabel ?? graphOutcomeLabel).trim().toLowerCase();
    if (outcomeLabel && outcomeLabel !== 'yes' && outcomeLabel !== 'no') {
      pushNodeError(
        issues,
        node,
        'invalid_outcome_label',
        'action.resolve_market outcomeLabel must be yes or no.'
      );
    }

    for (const boolKey of ['failOnMissingMarket', 'requireYesNoTokens', 'requireTokenId']) {
      if (config[boolKey] != null && toBooleanish(config[boolKey]) == null) {
        pushNodeError(
          issues,
          node,
          `invalid_${boolKey.toLowerCase()}`,
          `action.resolve_market ${boolKey} must be boolean (true/false).`
        );
      }
    }
  }

  if (node.type === 'action.place_order') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    const side = String(config.side ?? '').trim().toLowerCase();
    const effectiveSourceTradeId = sourceTradeId ?? graphSourceTradeId ?? 0;
    const allowBuyAutoScopeSourceTrade =
      side === 'buy' && hasUpstreamMarketPriceAutoScope;
    if (effectiveSourceTradeId <= 0 && !allowBuyAutoScopeSourceTrade) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'action.place_order requires sourceTradeId in node config or graph context.'
      );
    }
    if (
      !String(config.marketSlug ?? graphMarketSlug).trim() &&
      !hasResolveMarketNode &&
      !(side === 'buy' && hasUpstreamMarketPriceAutoScope)
    ) {
      pushNodeError(
        issues,
        node,
        'missing_market_slug',
        'action.place_order requires marketSlug in node config/graph context. Buy auto_scope zincirinde runtime tetikten de cozulebilir.'
      );
    }
    if (
      !String(config.tokenId ?? graphTokenId).trim() &&
      !hasResolveMarketNode &&
      !(side === 'buy' && hasUpstreamMarketPriceAutoScope)
    ) {
      pushNodeError(
        issues,
        node,
        'missing_token_id',
        'action.place_order requires tokenId in node config/graph context. Buy auto_scope zincirinde runtime tetikten de cozulebilir.'
      );
    }
    if (!side) {
      pushNodeError(issues, node, 'missing_side', 'action.place_order side is required (buy or sell).');
    } else if (side !== 'buy' && side !== 'sell') {
      pushNodeError(issues, node, 'invalid_side', 'action.place_order side must be buy or sell.');
    }

    const executionMode = String(config.executionMode ?? '').trim().toLowerCase();
    if (!executionMode) {
      pushNodeError(
        issues,
        node,
        'missing_execution_mode',
        'action.place_order executionMode is required (market or limit).'
      );
    } else if (executionMode !== 'market' && executionMode !== 'limit') {
      pushNodeError(
        issues,
        node,
        'invalid_execution_mode',
        'action.place_order executionMode must be market or limit.'
      );
    }
    const maxTriggers = toFiniteNumber(config.maxTriggers);
    if (maxTriggers != null && (maxTriggers < 1 || maxTriggers > 20)) {
      pushNodeError(
        issues,
        node,
        'invalid_max_triggers',
        'action.place_order maxTriggers must be in [1, 20].'
      );
    }
    const sizeModeRaw = String(config.sizeMode ?? '').trim().toLowerCase();
    if (sizeModeRaw && sizeModeRaw !== 'usdc' && sizeModeRaw !== 'pct') {
      pushNodeError(
        issues,
        node,
        'invalid_size_mode',
        'action.place_order sizeMode must be usdc or pct.'
      );
    }
    const triggerSizesRaw = config.triggerSizes;
    const triggerSizes: number[] = [];
    let triggerSizesInvalid = false;
    if (triggerSizesRaw != null) {
      if (!Array.isArray(triggerSizesRaw)) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_sizes',
          'action.place_order triggerSizes must be an array.'
        );
      } else {
        for (const item of triggerSizesRaw) {
          const value = toFiniteNumber(item);
          if (value == null || value <= 0) {
            triggerSizesInvalid = true;
            continue;
          }
          triggerSizes.push(value);
        }
        if (triggerSizesRaw.length > 0 && triggerSizes.length === 0) {
          triggerSizesInvalid = true;
        }
        if (triggerSizesInvalid) {
          pushNodeError(
            issues,
            node,
            'invalid_trigger_sizes',
            'action.place_order triggerSizes entries must be finite numbers > 0.'
          );
        }
        if (
          maxTriggers != null &&
          maxTriggers >= 1 &&
          triggerSizes.length > 0 &&
          triggerSizes.length > Math.floor(maxTriggers)
        ) {
          pushNodeError(
            issues,
            node,
            'invalid_trigger_sizes_length',
            'action.place_order triggerSizes length cannot exceed maxTriggers.'
          );
        }
        if (sizeModeRaw === 'pct' && triggerSizes.length > 0) {
          const triggerSizesSum = triggerSizes.reduce((sum, value) => sum + value, 0);
          if (triggerSizesSum > 100.000001) {
            pushNodeError(
              issues,
              node,
              'invalid_trigger_sizes_sum_pct',
              'action.place_order pct triggerSizes total must be <= 100.'
            );
          }
        }
      }
    }
    const sizeUsdc = toFiniteNumber(config.sizeUsdc ?? config.targetNotionalUsdc);
    const sizePct = toFiniteNumber(config.sizePct ?? config.sizePercent);
    const hasTriggerSizes = triggerSizes.length > 0;
    const usesPctSizing =
      sizeModeRaw === 'pct' || (!hasTriggerSizes && sizeUsdc == null && sizePct != null);
    if (!hasTriggerSizes) {
      const usePct = usesPctSizing;
      if (usePct) {
        if (sizePct == null || sizePct <= 0 || sizePct > 100) {
          pushNodeError(
            issues,
            node,
            'invalid_size_pct',
            'action.place_order sizePct must be in (0, 100].'
          );
        }
      } else if (sizeUsdc == null || sizeUsdc <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_size',
          'action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode).'
        );
      }
    }
    if (side === 'buy' && usesPctSizing && effectiveSourceTradeId <= 0 && hasUpstreamMarketPriceAutoScope) {
      pushNodeError(
        issues,
        node,
        'pct_buy_requires_source_trade',
        'action.place_order buy + auto_scope zincirinde pct sizing icin sourceTradeId gerekir. Buy auto source trade yalnizca usdc sizing ile desteklenir.'
      );
    }
    const minDistance = toFiniteNumber(config.minPriceDistanceCent);
    if (minDistance != null && minDistance <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_min_price_distance',
        'action.place_order minPriceDistanceCent must be > 0.'
      );
    }
    const triggerCondition = config.triggerCondition;
    if (triggerCondition != null && !isSupportedTriggerCondition(triggerCondition)) {
      pushNodeError(
        issues,
        node,
        'invalid_trigger_condition',
        'action.place_order triggerCondition must be cross_above or cross_below.'
      );
    }

    const tpEnabled = toBooleanish(config.tpEnabled);
    const slEnabled = toBooleanish(config.slEnabled);
    const tpPrice = resolveConfiguredBinaryPrice(config.tpPriceCent, config.tpPrice);
    const slPrice = resolveConfiguredBinaryPrice(config.slPriceCent, config.slPrice);

    if (config.tpEnabled != null && tpEnabled == null) {
      pushNodeError(
        issues,
        node,
        'invalid_tp_enabled',
        'action.place_order tpEnabled must be boolean (true/false).'
      );
    }
    if (config.slEnabled != null && slEnabled == null) {
      pushNodeError(
        issues,
        node,
        'invalid_sl_enabled',
        'action.place_order slEnabled must be boolean (true/false).'
      );
    }
    if (tpEnabled === true && side !== 'buy') {
      pushNodeError(
        issues,
        node,
        'invalid_tp_side',
        'action.place_order tpEnabled is only valid for side=buy.'
      );
    }
    if (slEnabled === true && side !== 'buy') {
      pushNodeError(
        issues,
        node,
        'invalid_sl_side',
        'action.place_order slEnabled is only valid for side=buy.'
      );
    }
    if (tpEnabled === true && !tpPrice.provided) {
      pushNodeError(
        issues,
        node,
        'missing_tp_price',
        'action.place_order tpEnabled requires tpPriceCent (or legacy tpPrice).'
      );
    } else if (tpPrice.provided && tpPrice.value == null) {
      pushNodeError(
        issues,
        node,
        'invalid_tp_price',
        'action.place_order tpPriceCent must be in (0, 100] or legacy tpPrice must be in (0, 1].'
      );
    }
    if (slEnabled === true && !slPrice.provided) {
      pushNodeError(
        issues,
        node,
        'missing_sl_price',
        'action.place_order slEnabled requires slPriceCent (or legacy slPrice).'
      );
    } else if (slPrice.provided && slPrice.value == null) {
      pushNodeError(
        issues,
        node,
        'invalid_sl_price',
        'action.place_order slPriceCent must be in (0, 100] or legacy slPrice must be in (0, 1].'
      );
    }
    if (
      tpEnabled === true &&
      slEnabled === true &&
      tpPrice.value != null &&
      slPrice.value != null &&
      slPrice.value >= tpPrice.value
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_sl_tp_band',
        'action.place_order requires slPrice < tpPrice when both stop loss and take profit are enabled.'
      );
    }
  }

  if (node.type === 'action.dual_dca') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'action.dual_dca requires sourceTradeId in node config or graph context. Publish sirasinda otomatik olusturulmasi icin asset/timeframe alanlari dolu olmali.'
      );
    }

    const asset = String(config.asset ?? config.coin ?? '').trim().toLowerCase();
    if (!asset) {
      pushNodeError(
        issues,
        node,
        'missing_asset',
        'action.dual_dca requires asset (btc, eth, sol, xrp).'
      );
    } else if (!RESOLVE_MARKET_ALLOWED_ASSETS.has(asset)) {
      pushNodeError(
        issues,
        node,
        'invalid_asset',
        'action.dual_dca asset must be one of: btc, eth, sol, xrp.'
      );
    }

    const timeframeRaw = String(config.timeframe ?? config.marketPeriod ?? '').trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    if (!timeframe) {
      pushNodeError(
        issues,
        node,
        'missing_timeframe',
        'action.dual_dca requires timeframe (5m or 15m).'
      );
    } else if (!RESOLVE_MARKET_ALLOWED_TIMEFRAMES.has(timeframe)) {
      pushNodeError(
        issues,
        node,
        'invalid_timeframe',
        'action.dual_dca timeframe must be one of: 5m, 15m.'
      );
    }

    const sideMode = String(config.sideMode ?? config.side ?? '').trim().toLowerCase();
    if (!sideMode) {
      pushNodeError(
        issues,
        node,
        'missing_side_mode',
        'action.dual_dca requires sideMode (up/down/all).'
      );
    } else if (sideMode !== 'up' && sideMode !== 'down' && sideMode !== 'all') {
      pushNodeError(
        issues,
        node,
        'invalid_side_mode',
        'action.dual_dca sideMode must be up, down or all.'
      );
    }

    const baseSizing = String(config.baseSizing ?? config.baseSizeMode ?? '').trim().toLowerCase();
    if (!baseSizing) {
      pushNodeError(
        issues,
        node,
        'missing_base_sizing',
        'action.dual_dca requires baseSizing (shares/usdc).'
      );
    } else if (baseSizing !== 'shares' && baseSizing !== 'usdc') {
      pushNodeError(
        issues,
        node,
        'invalid_base_sizing',
        'action.dual_dca baseSizing must be shares or usdc.'
      );
    }
    const baseShares = toFiniteNumber(config.baseShares);
    const baseUsdc = toFiniteNumber(config.baseUsdc);
    if (baseSizing === 'shares') {
      if (baseShares == null || baseShares <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_base_shares',
          'action.dual_dca baseShares must be > 0 when baseSizing is shares.'
        );
      }
    } else if (baseSizing === 'usdc' && (baseUsdc == null || baseUsdc <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_base_usdc',
        'action.dual_dca baseUsdc must be > 0 when baseSizing is usdc.'
      );
    }
    const basePrice = toFiniteNumber(config.basePriceUsdc ?? config.basePrice);
    if (basePrice != null && (basePrice <= 0 || basePrice > 1)) {
      pushNodeError(
        issues,
        node,
        'invalid_base_price',
        'action.dual_dca basePriceUsdc must be in (0, 1].'
      );
    }

    const dcaLevels = toFiniteNumber(config.dcaLevels);
    if (dcaLevels == null) {
      pushNodeError(
        issues,
        node,
        'missing_dca_levels',
        'action.dual_dca requires dcaLevels.'
      );
    } else if (dcaLevels < 1 || dcaLevels > 20) {
      pushNodeError(
        issues,
        node,
        'invalid_dca_levels',
        'action.dual_dca dcaLevels must be in [1, 20].'
      );
    }

    const nearStep = toFiniteNumber(config.nearStep);
    if (nearStep == null) {
      pushNodeError(
        issues,
        node,
        'missing_near_step',
        'action.dual_dca requires nearStep.'
      );
    } else if (nearStep <= 0 || nearStep >= 1) {
      pushNodeError(
        issues,
        node,
        'invalid_near_step',
        'action.dual_dca nearStep must be in (0, 1).'
      );
    }

    const stepMult = toFiniteNumber(config.stepMult);
    if (stepMult == null) {
      pushNodeError(
        issues,
        node,
        'missing_step_mult',
        'action.dual_dca requires stepMult.'
      );
    } else if (stepMult < 1) {
      pushNodeError(
        issues,
        node,
        'invalid_step_mult',
        'action.dual_dca stepMult must be >= 1.'
      );
    }

    const sizeMult = toFiniteNumber(config.sizeMult);
    if (sizeMult == null) {
      pushNodeError(
        issues,
        node,
        'missing_size_mult',
        'action.dual_dca requires sizeMult.'
      );
    } else if (sizeMult <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_size_mult',
        'action.dual_dca sizeMult must be > 0.'
      );
    }

    const minDistance = toFiniteNumber(config.minPriceDistanceCent);
    if (minDistance == null) {
      pushNodeError(
        issues,
        node,
        'missing_min_price_distance',
        'action.dual_dca requires minPriceDistanceCent.'
      );
    } else if (minDistance <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_min_price_distance',
        'action.dual_dca minPriceDistanceCent must be > 0.'
      );
    }

    const cutoffMin = toFiniteNumber(config.cutoffMin);
    if (cutoffMin == null) {
      pushNodeError(
        issues,
        node,
        'missing_cutoff_min',
        'action.dual_dca requires cutoffMin.'
      );
    } else if (cutoffMin < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_cutoff_min',
        'action.dual_dca cutoffMin must be >= 0.'
      );
    }

    for (const riskKey of ['tpProfitPct', 'slLossPct', 'slSpreadPct']) {
      const value = toFiniteNumber(config[riskKey]);
      if (value == null) {
        pushNodeError(
          issues,
          node,
          `missing_${riskKey.toLowerCase()}`,
          `action.dual_dca requires ${riskKey}.`
        );
      } else if (value < 0) {
        pushNodeError(
          issues,
          node,
          `invalid_${riskKey.toLowerCase()}`,
          `action.dual_dca ${riskKey} must be >= 0.`
        );
      }
    }
  }

  if (node.type === 'action.cancel_order' || node.type === 'action.update_order') {
    const hasId = toFiniteNumber(config.builderOrderId) != null;
    const hasRef = String(config.targetRef ?? '').trim().length > 0;
    if (!hasId && !hasRef) {
      pushNodeError(
        issues,
        node,
        'missing_target_ref',
        `${node.type} requires builderOrderId or targetRef.`
      );
    }
  }

  if (node.type === 'action.set_state') {
    const patch = config.statePatch ?? config.state;
    if (patch !== undefined && !isRecord(patch)) {
      pushNodeError(
        issues,
        node,
        'invalid_state_patch',
        'action.set_state statePatch/state must be an object.'
      );
    }
  }

}

export function validateTradeFlowGraph(graphJson: unknown): TradeFlowValidationResult {
  const graph = normalizeTradeFlowGraph(graphJson);
  const issues: TradeFlowValidationIssue[] = [];

  if (!isRecord(graph.context)) {
    issues.push({
      severity: 'error',
      code: 'invalid_context',
      message: 'Graph context must be an object.',
    });
  } else if (
    graph.context.autoClaimEnabled != null &&
    toBooleanish(graph.context.autoClaimEnabled) == null
  ) {
    issues.push({
      severity: 'error',
      code: 'invalid_auto_claim_enabled',
      message: 'Graph context autoClaimEnabled must be boolean (true/false).',
    });
  }

  const nodeKeySet = new Set<string>();
  for (const node of graph.nodes) {
    if (nodeKeySet.has(node.key)) {
      issues.push({
        severity: 'error',
        code: 'duplicate_node_key',
        message: `Node key already exists: ${node.key}`,
        nodeKey: node.key,
      });
      continue;
    }
    nodeKeySet.add(node.key);

    if (!SUPPORTED_NODE_TYPES.has(node.type)) {
      issues.push({
        severity: 'warning',
        code: 'unknown_node_type',
        message: `Unsupported/unknown node type: ${node.type}`,
        nodeKey: node.key,
      });
    }

    validateNodeConfig(issues, node, graph);
  }

  const edgeKeySet = new Set<string>();
  for (const edge of graph.edges) {
    if (edgeKeySet.has(edge.key)) {
      issues.push({
        severity: 'error',
        code: 'duplicate_edge_key',
        message: `Edge key already exists: ${edge.key}`,
        edgeKey: edge.key,
      });
    }
    edgeKeySet.add(edge.key);

    if (!nodeKeySet.has(edge.source)) {
      issues.push({
        severity: 'error',
        code: 'edge_source_missing',
        message: `Edge source node not found: ${edge.source}`,
        edgeKey: edge.key,
      });
    }
    if (!nodeKeySet.has(edge.target)) {
      issues.push({
        severity: 'error',
        code: 'edge_target_missing',
        message: `Edge target node not found: ${edge.target}`,
        edgeKey: edge.key,
      });
    }
  }

  const triggerCount = graph.nodes.filter((node) => node.type.startsWith('trigger.')).length;
  const actionCount = graph.nodes.filter((node) => node.type.startsWith('action.')).length;
  const rootNodeKeys = collectRootNodeKeys(graph.nodes, graph.edges);

  if (triggerCount === 0) {
    const hasDualDcaNode = graph.nodes.some((node) => node.type === 'action.dual_dca');
    if (!hasDualDcaNode) {
      issues.push({
        severity: 'error',
        code: 'missing_trigger',
        message: 'At least one trigger node is required.',
      });
    } else {
      const invalidRootNodes = graph.nodes
        .filter((node) => rootNodeKeys.has(node.key) && node.type !== 'action.dual_dca')
        .map((node) => node.key);
      if (invalidRootNodes.length > 0) {
        issues.push({
          severity: 'error',
          code: 'missing_trigger_invalid_roots_for_dual_dca',
          message: `Trigger yoksa root node'lar sadece action.dual_dca olabilir: ${invalidRootNodes.join(', ')}`,
        });
      }
    }
  }
  if (actionCount === 0) {
    issues.push({
      severity: 'error',
      code: 'missing_action',
      message: 'At least one action node is required.',
    });
  }

  for (const node of graph.nodes) {
    if (node.type !== 'logic.if') continue;
    const outgoing = graph.edges.filter((edge) => edge.source === node.key);
    const hasTrue = outgoing.some((edge) => edge.type === 'on_true');
    const hasFalse = outgoing.some((edge) => edge.type === 'on_false');
    if (!hasTrue || !hasFalse) {
      issues.push({
        severity: 'warning',
        code: 'if_missing_branch',
        message: `If node should include both on_true and on_false branches: ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  for (const node of graph.nodes) {
    if (node.type !== 'logic.switch') continue;
    const outgoing = graph.edges.filter((edge) => edge.source === node.key);
    const hasDefault = outgoing.some((edge) => edge.type === 'default');
    if (!hasDefault) {
      issues.push({
        severity: 'warning',
        code: 'switch_missing_default',
        message: `Switch node should include default branch: ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  if (graph.nodes.length > 0 && detectCycles(graph.nodes, graph.edges)) {
    issues.push({
      severity: 'error',
      code: 'cycle_detected',
      message: 'Graph contains cycle(s). In this version, cyclic flow is not allowed.',
    });
  }

  const reachable = collectReachableFromTriggers(graph.nodes, graph.edges);
  for (const node of graph.nodes) {
    if (!reachable.has(node.key)) {
      issues.push({
        severity: 'warning',
        code: 'unreachable_node',
        message: `Node is unreachable from start node(s): ${node.key}`,
        nodeKey: node.key,
      });
    }
  }

  const valid = !issues.some((issue) => issue.severity === 'error');
  return {
    valid,
    issues,
    stats: {
      nodes: graph.nodes.length,
      edges: graph.edges.length,
      triggers: triggerCount,
      actions: actionCount,
    },
  };
}

export async function validateTradeFlowGraphWithRuntimeConfig(
  graphJson: unknown,
  context: UserConfigContext
): Promise<TradeFlowValidationResult> {
  const graph = normalizeTradeFlowGraph(graphJson);
  const baseValidation = validateTradeFlowGraph(graph);
  const issues = [...baseValidation.issues];
  const telegramNodes = graph.nodes.filter((node) => node.type === 'action.telegram_notify');

  if (telegramNodes.length > 0) {
    let userTelegramBotToken = '';
    let userTelegramDefaultChatId = '';
    let userTelegramReadError: string | null = null;
    try {
      userTelegramBotToken = (await readTelegramBotTokenForServer(context)).trim();
      userTelegramDefaultChatId = (await readTelegramChatIdForServer(context)).trim();
    } catch (err) {
      userTelegramReadError =
        err instanceof Error ? err.message : 'Failed to read telegram config';
    }

    for (const node of telegramNodes) {
      const config = isRecord(node.config) ? node.config : {};
      const nodeChatId = String(config.chatId ?? '').trim();

      if (userTelegramReadError) {
        issues.push({
          severity: 'error',
          code: 'telegram_config_invalid',
          message: `Telegram config okunamadi: ${userTelegramReadError}`,
          nodeKey: node.key,
        });
        continue;
      }

      if (!userTelegramBotToken) {
        issues.push({
          severity: 'error',
          code: 'missing_telegram_bot_token',
          message: 'action.telegram_notify requires a Telegram bot token in Settings -> Telegram for the current user.',
          nodeKey: node.key,
        });
      }

      if (!nodeChatId && !userTelegramDefaultChatId) {
        issues.push({
          severity: 'error',
          code: 'missing_telegram_chat_id',
          message:
            'action.telegram_notify requires chatId in node config or a default Telegram chat_id in Settings -> Telegram for the current user.',
          nodeKey: node.key,
        });
      }

      if (nodeChatId && !isValidTelegramChatTarget(nodeChatId)) {
        issues.push({
          severity: 'error',
          code: 'invalid_telegram_chat_id',
          message:
            'action.telegram_notify chatId must be a Telegram chat ID like -1001234567890 or a @channelusername.',
          nodeKey: node.key,
        });
      }

      if (!nodeChatId && userTelegramDefaultChatId && !isValidTelegramChatTarget(userTelegramDefaultChatId)) {
        issues.push({
          severity: 'error',
          code: 'invalid_default_telegram_chat_id',
          message:
            'Settings -> Telegram chat_id must be a Telegram chat ID like -1001234567890 or a @channelusername.',
          nodeKey: node.key,
        });
      }

      if (String(config.botToken ?? '').trim()) {
        issues.push({
          severity: 'warning',
          code: 'legacy_inline_telegram_bot_token',
          message:
            'Bu node eski inline botToken tasiyor, fakat artik kullanilmaz. Settings -> Telegram ekraninda kullanici tokenini tanimlayip node’u kaydederek yeni modele gec.',
          nodeKey: node.key,
        });
      }
    }
  }

  return {
    ...baseValidation,
    valid: !issues.some((issue) => issue.severity === 'error'),
    issues,
  };
}

function mapDefinitionRow(row: Record<string, unknown>): TradeFlowDefinition {
  return {
    id: Number(row.id),
    name: String(row.name || ''),
    description: row.description == null ? null : String(row.description),
    status: String(row.status || 'draft') as TradeFlowDefinition['status'],
    draft_version_id: row.draft_version_id == null ? null : Number(row.draft_version_id),
    published_version_id: row.published_version_id == null ? null : Number(row.published_version_id),
    last_error: row.last_error == null ? null : String(row.last_error),
    created_at: String(row.created_at),
    updated_at: String(row.updated_at),
    legacy_workflow_id: row.legacy_workflow_id == null ? null : Number(row.legacy_workflow_id),
  };
}

function mapVersionRow(row: Record<string, unknown>): TradeFlowVersion {
  return {
    id: Number(row.id),
    definition_id: Number(row.definition_id),
    version_no: Number(row.version_no),
    status: String(row.status || 'draft') as TradeFlowVersion['status'],
    graph_json: normalizeTradeFlowGraph(row.graph_json),
    published_at: row.published_at == null ? null : String(row.published_at),
    created_at: String(row.created_at),
  };
}

function buildLegacyFlowGraph(workflow: NonNullable<Awaited<ReturnType<typeof getTradeBuilderWorkflowById>>>): TradeFlowGraph {
  const sellLeg = workflow.legs.find((leg) => leg.leg_type === 'sell');
  const buyLeg = workflow.legs.find((leg) => leg.leg_type === 'buy');
  if (!sellLeg || !buyLeg) {
    return DEFAULT_GRAPH;
  }

  const sellProgressExpr = {
    '>=': [{ var: 'sell_progress_pct' }, workflow.workflow.buy_start_after_sell_progress_pct],
  };

  const buyPriceExpr =
    buyLeg.trigger_condition === 'cross_above'
      ? { '>=': [{ var: 'market_price' }, (buyLeg.trigger_price || 0) * 100] }
      : buyLeg.trigger_condition === 'cross_below'
        ? { '<=': [{ var: 'market_price' }, (buyLeg.trigger_price || 0) * 100] }
        : null;

  let gateExpression: Record<string, unknown>;
  if (workflow.workflow.buy_trigger_mode === 'sell_progress_only') {
    gateExpression = sellProgressExpr;
  } else if (workflow.workflow.buy_trigger_mode === 'price_only') {
    gateExpression = buyPriceExpr || { '==': [1, 1] };
  } else {
    gateExpression = buyPriceExpr
      ? { and: [sellProgressExpr, buyPriceExpr] }
      : sellProgressExpr;
  }

  return {
    context: {
      sourceTradeId: workflow.workflow.source_trade_id,
      marketSlug: sellLeg.market_slug,
      tokenId: sellLeg.token_id,
      outcomeLabel: sellLeg.outcome_label,
    },
    nodes: [
      {
        key: 'trigger_market_tick',
        type: 'trigger.market_price',
        positionX: 100,
        positionY: 150,
        config: {
          marketSlug: sellLeg.market_slug,
          tokenId: sellLeg.token_id,
        },
      },
      {
        key: 'action_sell',
        type: 'action.place_order',
        positionX: 420,
        positionY: 80,
        config: {
          side: sellLeg.side,
          executionMode: 'market',
          marketSlug: sellLeg.market_slug,
          tokenId: sellLeg.token_id,
          outcomeLabel: sellLeg.outcome_label,
          minPriceDistanceCent: sellLeg.min_price_distance_cent,
          triggerCondition: sellLeg.trigger_condition,
          triggerPriceCent:
            sellLeg.trigger_price == null ? null : Math.round(sellLeg.trigger_price * 100),
          targetNotionalUsdc: sellLeg.target_notional_usdc,
        },
      },
      {
        key: 'if_buy_gate',
        type: 'logic.if',
        positionX: 720,
        positionY: 150,
        config: {
          expression: gateExpression,
          mode: workflow.workflow.buy_trigger_mode,
        },
      },
      {
        key: 'action_buy',
        type: 'action.place_order',
        positionX: 1020,
        positionY: 80,
        config: {
          side: buyLeg.side,
          executionMode: 'market',
          marketSlug: buyLeg.market_slug,
          tokenId: buyLeg.token_id,
          outcomeLabel: buyLeg.outcome_label,
          minPriceDistanceCent: buyLeg.min_price_distance_cent,
          triggerCondition: buyLeg.trigger_condition,
          triggerPriceCent: buyLeg.trigger_price == null ? null : Math.round(buyLeg.trigger_price * 100),
          targetNotionalUsdc: buyLeg.target_notional_usdc,
        },
      },
      {
        key: 'action_wait',
        type: 'action.set_state',
        positionX: 1020,
        positionY: 250,
        config: {
          statePatch: {
            state: 'waiting_sell_progress',
            reason: 'buy_gate_not_satisfied',
          },
        },
      },
    ],
    edges: [
      {
        key: 'e1',
        source: 'trigger_market_tick',
        target: 'action_sell',
        type: 'default',
        condition: null,
      },
      {
        key: 'e2',
        source: 'action_sell',
        target: 'if_buy_gate',
        type: 'on_success',
        condition: null,
      },
      {
        key: 'e3',
        source: 'if_buy_gate',
        target: 'action_buy',
        type: 'on_true',
        condition: null,
      },
      {
        key: 'e4',
        source: 'if_buy_gate',
        target: 'action_wait',
        type: 'on_false',
        condition: null,
      },
    ],
  };
}

async function fetchVersionById(queryable: Queryable, versionId: number | null): Promise<TradeFlowVersion | null> {
  if (!versionId) return null;
  const res = await queryable.query('SELECT * FROM trade_flow_versions WHERE id = $1 LIMIT 1', [versionId]);
  if ((res.rowCount ?? 0) === 0) return null;
  return mapVersionRow(res.rows[0] as Record<string, unknown>);
}

async function replaceVersionGraph(queryable: Queryable, versionId: number, graph: TradeFlowGraph): Promise<void> {
  await queryable.query('DELETE FROM trade_flow_nodes WHERE version_id = $1', [versionId]);
  await queryable.query('DELETE FROM trade_flow_edges WHERE version_id = $1', [versionId]);

  for (const node of graph.nodes) {
    await queryable.query(
      `INSERT INTO trade_flow_nodes (version_id, node_key, node_type, position_x, position_y, config_json, created_at)
       VALUES ($1, $2, $3, $4, $5, $6::jsonb, NOW())`,
      [
        versionId,
        node.key,
        node.type,
        node.positionX,
        node.positionY,
        JSON.stringify(node.config || {}),
      ]
    );
  }

  for (const edge of graph.edges) {
    await queryable.query(
      `INSERT INTO trade_flow_edges
         (version_id, edge_key, source_node_key, target_node_key, edge_type, condition_json, created_at)
       VALUES ($1, $2, $3, $4, $5, $6::jsonb, NOW())`,
      [
        versionId,
        edge.key,
        edge.source,
        edge.target,
        edge.type,
        edge.condition ? JSON.stringify(edge.condition) : null,
      ]
    );
  }
}

export async function createTradeFlowDefinition(
  input: CreateTradeFlowDefinitionInput
): Promise<TradeFlowDefinitionDetail> {
  const name = input.name.trim();
  if (!name) {
    throw new Error('Flow name is required');
  }

  const graph = normalizeTradeFlowGraph(input.graphJson);

  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    if (input.legacyWorkflowId) {
      const legacyWorkflowRes = await client.query(
        `SELECT id
         FROM trade_builder_workflows
         WHERE id = $1 AND user_id = $2
         LIMIT 1`,
        [input.legacyWorkflowId, input.userId]
      );
      if ((legacyWorkflowRes.rowCount ?? 0) === 0) {
        throw new Error('Legacy workflow not found');
      }
    }

    const defRes = await client.query(
      `INSERT INTO trade_flow_definitions (user_id, name, description, status, created_at, updated_at)
       VALUES ($1, $2, $3, 'draft', NOW(), NOW())
       RETURNING *`,
      [input.userId, name, input.description ?? null]
    );
    const definition = defRes.rows[0] as Record<string, unknown>;

    const versionRes = await client.query(
      `INSERT INTO trade_flow_versions (definition_id, version_no, status, graph_json, created_at)
       VALUES ($1, 1, 'draft', $2::jsonb, NOW())
       RETURNING *`,
      [definition.id, JSON.stringify(graph)]
    );
    const draftVersion = versionRes.rows[0] as Record<string, unknown>;

    await replaceVersionGraph(client, Number(draftVersion.id), graph);

    await client.query(
      `UPDATE trade_flow_definitions
       SET draft_version_id = $2, updated_at = NOW()
       WHERE id = $1`,
      [definition.id, draftVersion.id]
    );

    if (input.legacyWorkflowId) {
      await client.query(
        `INSERT INTO trade_flow_legacy_mappings (legacy_workflow_id, definition_id, version_id, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())
         ON CONFLICT (legacy_workflow_id) DO UPDATE
         SET definition_id = EXCLUDED.definition_id,
             version_id = EXCLUDED.version_id,
             updated_at = NOW()`,
        [input.legacyWorkflowId, definition.id, draftVersion.id]
      );
    }

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(input.userId, Number(definition.id))) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function updateTradeFlowDefinitionDraft(
  userId: number,
  definitionId: number,
  updates: UpdateTradeFlowDefinitionInput
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT *
       FROM trade_flow_definitions
       WHERE id = $1
         AND user_id = $2
       LIMIT 1
       FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }
    const definition = defRes.rows[0] as Record<string, unknown>;

    let draftVersionId = definition.draft_version_id == null ? null : Number(definition.draft_version_id);
    if (!draftVersionId) {
      const maxVersionRes = await client.query(
        `SELECT COALESCE(MAX(version_no), 0)::int AS max_version
         FROM trade_flow_versions
         WHERE definition_id = $1`,
        [definitionId]
      );
      const maxVersion = Number(maxVersionRes.rows[0]?.max_version || 0);
      const fallbackGraph =
        (await fetchVersionById(
          client,
          definition.published_version_id == null ? null : Number(definition.published_version_id)
        ))?.graph_json || DEFAULT_GRAPH;

      const insertDraftRes = await client.query(
        `INSERT INTO trade_flow_versions (definition_id, version_no, status, graph_json, created_at)
         VALUES ($1, $2, 'draft', $3::jsonb, NOW())
         RETURNING id`,
        [definitionId, maxVersion + 1, JSON.stringify(fallbackGraph)]
      );
      draftVersionId = Number(insertDraftRes.rows[0].id);
      await replaceVersionGraph(client, draftVersionId, fallbackGraph);
    }

    if (updates.graphJson !== undefined) {
      const normalizedGraph = normalizeTradeFlowGraph(updates.graphJson);

      await client.query(
        `UPDATE trade_flow_versions
         SET graph_json = $2::jsonb
         WHERE id = $1`,
        [draftVersionId, JSON.stringify(normalizedGraph)]
      );
      await replaceVersionGraph(client, draftVersionId, normalizedGraph);
    }

    const fields: string[] = ['updated_at = NOW()'];
    const params: unknown[] = [definitionId, userId];
    let idx = 3;

    if (updates.name !== undefined) {
      const nextName = updates.name.trim();
      if (!nextName) throw new Error('Flow name cannot be empty');
      fields.push(`name = $${idx++}`);
      params.push(nextName);
    }

    if (updates.description !== undefined) {
      fields.push(`description = $${idx++}`);
      params.push(updates.description ?? null);
    }

    if (draftVersionId !== (definition.draft_version_id == null ? null : Number(definition.draft_version_id))) {
      fields.push(`draft_version_id = $${idx++}`);
      params.push(draftVersionId);
    }

    await client.query(
      `UPDATE trade_flow_definitions
       SET ${fields.join(', ')}
       WHERE id = $1 AND user_id = $2`,
      params
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function publishTradeFlowDefinition(
  context: { userId: number; username: string },
  definitionId: number
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT * FROM trade_flow_definitions WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE`,
      [definitionId, context.userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const def = defRes.rows[0] as Record<string, unknown>;
    const draftVersionId = def.draft_version_id == null ? null : Number(def.draft_version_id);
    if (!draftVersionId) {
      throw new Error('Draft version not found');
    }

    const draftVersion = await fetchVersionById(client, draftVersionId);
    if (!draftVersion) {
      throw new Error('Draft version payload not found');
    }

    const validation = await validateTradeFlowGraphWithRuntimeConfig(draftVersion.graph_json, context);
    if (!validation.valid) {
      throw new Error(
        validation.issues
          .filter((issue) => issue.severity === 'error')
          .map((issue) => issue.message)
          .join(' | ')
      );
    }

    const maxVersionRes = await client.query(
      `SELECT COALESCE(MAX(version_no), 0)::int AS max_version
       FROM trade_flow_versions
       WHERE definition_id = $1`,
      [definitionId]
    );
    const maxVersion = Number(maxVersionRes.rows[0]?.max_version || 0);

    await client.query(
      `UPDATE trade_flow_versions
       SET status = 'archived'
       WHERE definition_id = $1 AND status = 'published'`,
      [definitionId]
    );

    const publishedRes = await client.query(
      `INSERT INTO trade_flow_versions
         (definition_id, version_no, status, graph_json, published_at, created_at)
       VALUES
         ($1, $2, 'published', $3::jsonb, NOW(), NOW())
       RETURNING *`,
      [definitionId, maxVersion + 1, JSON.stringify(draftVersion.graph_json)]
    );
    const publishedVersionId = Number(publishedRes.rows[0].id);
    await replaceVersionGraph(client, publishedVersionId, draftVersion.graph_json);

    const newDraftRes = await client.query(
      `INSERT INTO trade_flow_versions
         (definition_id, version_no, status, graph_json, created_at)
       VALUES
         ($1, $2, 'draft', $3::jsonb, NOW())
       RETURNING id`,
      [definitionId, maxVersion + 2, JSON.stringify(draftVersion.graph_json)]
    );
    const newDraftVersionId = Number(newDraftRes.rows[0].id);
    await replaceVersionGraph(client, newDraftVersionId, draftVersion.graph_json);

    await client.query(
      `UPDATE trade_flow_definitions
       SET status = 'published',
           published_version_id = $2,
           draft_version_id = $3,
           updated_at = NOW(),
           last_error = NULL
       WHERE id = $1`,
      [definitionId, publishedVersionId, newDraftVersionId]
    );

    await client.query(
      `INSERT INTO trade_flow_events (run_id, definition_id, version_id, event_type, payload_json, created_at)
       VALUES (NULL, $1, $2, 'flow_published', $3::jsonb, NOW())`,
      [
        definitionId,
        publishedVersionId,
        JSON.stringify({
          publishedVersionId,
          draftVersionId: newDraftVersionId,
        }),
      ]
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(context.userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function archiveTradeFlowDefinition(
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT * FROM trade_flow_definitions WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const current = defRes.rows[0] as Record<string, unknown>;
    const currentStatus = String(current.status || '');
    if (currentStatus === 'archived') {
      await client.query('COMMIT');
      return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
    }

    await client.query(
      `UPDATE trade_flow_runs
       SET status = 'canceled',
           ended_at = NOW(),
           updated_at = NOW(),
           last_error = COALESCE(last_error, 'definition_archived')
       WHERE definition_id = $1
         AND user_id = $2
         AND status = 'running'`,
      [definitionId, userId]
    );

    await client.query(
      `UPDATE trade_flow_definitions
       SET status = 'archived',
           updated_at = NOW()
       WHERE id = $1 AND user_id = $2`,
      [definitionId, userId]
    );

    await client.query(
      `INSERT INTO trade_flow_events
        (run_id, definition_id, version_id, event_type, payload_json, created_at)
       VALUES
        (NULL, $1, $2, 'flow_archived', $3::jsonb, NOW())`,
      [
        definitionId,
        current.published_version_id == null ? null : Number(current.published_version_id),
        JSON.stringify({ definitionId, archivedAt: new Date().toISOString() }),
      ]
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function getTradeFlowDefinitionById(
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail | null> {
  const defRes = await pool.query(
    `SELECT d.*, m.legacy_workflow_id
     FROM trade_flow_definitions d
     LEFT JOIN trade_flow_legacy_mappings m ON m.definition_id = d.id
     WHERE d.id = $1
       AND d.user_id = $2
     LIMIT 1`,
    [definitionId, userId]
  );
  if ((defRes.rowCount ?? 0) === 0) return null;

  const definition = mapDefinitionRow(defRes.rows[0] as Record<string, unknown>);
  const [draftVersion, publishedVersion] = await Promise.all([
    fetchVersionById(pool, definition.draft_version_id),
    fetchVersionById(pool, definition.published_version_id),
  ]);

  return {
    definition,
    draftVersion,
    publishedVersion,
  };
}

export async function getTradeFlowDefinitions(
  filters: TradeFlowListFilters
): Promise<PaginatedResponse<TradeFlowDefinition>> {
  if (filters.autoMigrateLegacy !== false) {
    await migrateLegacyWorkflowsToFlows(filters.userId, 25);
  }

  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['d.user_id = $1'];
  const params: unknown[] = [filters.userId];
  let idx = 2;

  if (filters.status) {
    whereParts.push(`d.status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_flow_definitions d ${where}`, params),
    pool.query(
      `SELECT d.*, m.legacy_workflow_id
       FROM trade_flow_definitions d
       LEFT JOIN trade_flow_legacy_mappings m ON m.definition_id = d.id
       ${where}
       ORDER BY d.updated_at DESC, d.id DESC
       LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows.map((row) => mapDefinitionRow(row as Record<string, unknown>)),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeFlowVersions(userId: number, definitionId: number): Promise<TradeFlowVersion[]> {
  const res = await pool.query(
    `SELECT v.*
     FROM trade_flow_versions v
     JOIN trade_flow_definitions d ON d.id = v.definition_id
     WHERE v.definition_id = $1 AND d.user_id = $2
     ORDER BY v.version_no DESC`,
    [definitionId, userId]
  );
  return res.rows.map((row) => mapVersionRow(row as Record<string, unknown>));
}

export async function getTradeFlowRuns(
  filters: TradeFlowRunFilters
): Promise<PaginatedResponse<TradeFlowRun>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['user_id = $1'];
  const params: unknown[] = [filters.userId];
  let idx = 2;

  if (filters.definitionId) {
    whereParts.push(`definition_id = $${idx++}`);
    params.push(filters.definitionId);
  }
  if (filters.status) {
    whereParts.push(`status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_flow_runs ${where}`, params),
    pool.query(
      `SELECT * FROM trade_flow_runs ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows as TradeFlowRun[],
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeFlowRunEvents(
  userId: number,
  runId: number,
  page = 1,
  limit = 50
): Promise<PaginatedResponse<TradeFlowEvent>> {
  const safeLimit = Math.min(Math.max(1, limit), 200);
  const safePage = Math.max(1, page);
  const offset = (safePage - 1) * safeLimit;

  const [countRes, dataRes] = await Promise.all([
    pool.query(
      `SELECT COUNT(*)::int AS total
       FROM trade_flow_events e
       JOIN trade_flow_runs r ON r.id = e.run_id
       WHERE e.run_id = $1 AND r.user_id = $2`,
      [runId, userId]
    ),
    pool.query(
      `SELECT e.*
       FROM trade_flow_events e
       JOIN trade_flow_runs r ON r.id = e.run_id
       WHERE e.run_id = $1 AND r.user_id = $2
       ORDER BY e.created_at DESC
       LIMIT $3 OFFSET $4`,
      [runId, userId, safeLimit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows as TradeFlowEvent[],
    total,
    page: safePage,
    limit: safeLimit,
    totalPages: Math.ceil(total / safeLimit),
  };
}

export async function migrateLegacyWorkflowsToFlows(userId: number, limit = 50): Promise<number> {
  const pendingRes = await pool.query(
    `SELECT w.id
     FROM trade_builder_workflows w
     LEFT JOIN trade_flow_legacy_mappings m ON m.legacy_workflow_id = w.id
     WHERE m.legacy_workflow_id IS NULL
       AND w.user_id = $1
     ORDER BY w.id ASC
     LIMIT $2`,
    [userId, Math.max(1, limit)]
  );

  let migrated = 0;
  for (const row of pendingRes.rows) {
    const workflowId = Number(row.id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) continue;

    try {
      const created = await createFlowFromLegacyWorkflow(userId, workflowId);
      if (created) migrated += 1;
    } catch (err) {
      console.error('Legacy workflow migration error:', workflowId, err);
    }
  }

  return migrated;
}

export async function createFlowFromLegacyWorkflow(userId: number, workflowId: number): Promise<boolean> {
  const existingMapRes = await pool.query(
    'SELECT definition_id FROM trade_flow_legacy_mappings WHERE legacy_workflow_id = $1 LIMIT 1',
    [workflowId]
  );
  if ((existingMapRes.rowCount ?? 0) > 0) {
    return false;
  }

  const legacy = await getTradeBuilderWorkflowById(userId, workflowId);
  if (!legacy) {
    throw new Error(`Legacy workflow not found: ${workflowId}`);
  }

  const graph = buildLegacyFlowGraph(legacy);
  const validation = validateTradeFlowGraph(graph);
  if (!validation.valid) {
    throw new Error(
      `Cannot migrate legacy workflow ${workflowId}: ${validation.issues
        .filter((issue) => issue.severity === 'error')
        .map((issue) => issue.message)
        .join(' | ')}`
    );
  }

  await createTradeFlowDefinition({
    userId,
    name: `Legacy ${legacy.workflow.name} (#${legacy.workflow.id})`,
    description: 'Migrated from trade_builder_workflows',
    graphJson: graph,
    legacyWorkflowId: legacy.workflow.id,
  });

  return true;
}
