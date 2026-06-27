import type { PoolClient } from 'pg';
import { pool } from '@/lib/db';
import type { TradeFlowEnsureSourceTradeRequest, TradeFlowGraph, TradeFlowOpenPositionOption } from '@/lib/types';
import { getMarketOutcomesBySlug } from '../trade-builder';

export type {
  TradeFlowEnsureDualDcaSourceTradeRequest,
  TradeFlowEnsureSourceTradeRequest,
} from '@/lib/types';

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

interface MarketOutcomeInfo {
  label: string;
  legSide: 'yes' | 'no';
}

interface OpenPositionOutcomeCacheEntry {
  expiresAt: number;
  byTokenId: Map<string, MarketOutcomeInfo>;
}

const openPositionOutcomeCache = new Map<string, OpenPositionOutcomeCacheEntry>();

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
  syncNormalizedTables?: boolean;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function isSupportedTriggerCondition(value: unknown): value is 'cross_above' | 'cross_below' {
  return value === 'cross_above' || value === 'cross_below';
}

function isSupportedMarketPriceTriggerCondition(
  value: unknown
): value is 'cross_above' | 'cross_below' | 'level_above' | 'level_below' {
  return (
    value === 'cross_above' ||
    value === 'cross_below' ||
    value === 'level_above' ||
    value === 'level_below'
  );
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

function countValidMarketPriceOutcomeConditions(config: Record<string, unknown>): number {
  const raw = config.outcomeConditions;
  if (!Array.isArray(raw)) return 0;

  let validCount = 0;
  for (const item of raw) {
    if (!isValidMarketPriceOutcomeCondition(item, config)) continue;
    validCount += 1;
  }

  return validCount;
}

function isValidMarketPriceOutcomeCondition(
  row: unknown,
  config: Record<string, unknown>
): row is Record<string, unknown> {
  if (!isRecord(row)) return false;

  const tokenId = toTrimmedString(row.tokenId);
  const outcomeLabel = toTrimmedString(row.outcomeLabel);
  if (!tokenId || !outcomeLabel) return false;

  const ptbEnabled = toBooleanish(config.priceToBeatTriggerEnabled) === true;
  const triggerCondition = toTrimmedString(row.triggerCondition);
  const triggerPriceCentProvided = hasProvidedValue(row.triggerPriceCent);
  const triggerPriceProvided = !triggerPriceCentProvided && hasProvidedValue(row.triggerPrice);
  const triggerPriceCent = toFiniteNumber(row.triggerPriceCent);
  const triggerPrice = toFiniteNumber(row.triggerPrice);
  const hasValidTriggerPriceCent =
    triggerPriceCent != null && triggerPriceCent > 0 && triggerPriceCent <= 100;
  const hasValidTriggerPrice =
    triggerPrice != null && triggerPrice > 0 && triggerPrice <= 1;
  const hasStandardTrigger =
    isSupportedMarketPriceTriggerCondition(triggerCondition) &&
    (hasValidTriggerPriceCent || hasValidTriggerPrice);
  const isPtbOnly =
    ptbEnabled &&
    !triggerCondition &&
    !triggerPriceCentProvided &&
    !triggerPriceProvided;

  if (!hasStandardTrigger && !isPtbOnly) return false;
  if (hasStandardTrigger && !hasValidOptionalMaxPrice(row.maxPriceCent, row.maxPrice)) {
    return false;
  }
  return true;
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
  doge_5m_updown: { asset: 'doge', timeframe: '5m' },
  bnb_5m_updown: { asset: 'bnb', timeframe: '5m' },
  hype_5m_updown: { asset: 'hype', timeframe: '5m' },
};
const RESOLVE_MARKET_ALLOWED_ASSETS = new Set(['btc', 'eth', 'sol', 'xrp', 'doge', 'bnb', 'hype']);
const RESOLVE_MARKET_ALLOWED_TIMEFRAMES = new Set(['5m', '15m']);

function toTrimmedString(value: unknown): string {
  if (typeof value === 'string') return value.trim();
  if (typeof value === 'number' || typeof value === 'boolean') return String(value).trim();
  return '';
}

function normalizeDualDcaAsset(value: unknown): 'btc' | 'eth' | 'sol' | 'xrp' | 'doge' | 'bnb' | 'hype' | null {
  const normalized = toTrimmedString(value).toLowerCase();
  if (
    normalized === 'btc' ||
    normalized === 'eth' ||
    normalized === 'sol' ||
    normalized === 'xrp' ||
    normalized === 'doge' ||
    normalized === 'bnb' ||
    normalized === 'hype'
  ) {
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
  const byTokenId = await getMarketOutcomeInfoMapCached(slug);
  const byTokenLabel = new Map<string, string>();
  for (const [tokenKey, info] of byTokenId.entries()) {
    byTokenLabel.set(tokenKey, info.label);
  }
  return byTokenLabel;
}

async function getMarketOutcomeInfoMapCached(slug: string): Promise<Map<string, MarketOutcomeInfo>> {
  const normalizedSlug = normalizeOpenPositionSlug(slug);
  if (!normalizedSlug) return new Map<string, MarketOutcomeInfo>();

  const now = Date.now();
  const cached = openPositionOutcomeCache.get(normalizedSlug);
  if (cached && cached.expiresAt > now) {
    return cached.byTokenId;
  }

  try {
    const outcomes = await getMarketOutcomesBySlug(slug);
    const byTokenId = new Map<string, MarketOutcomeInfo>();
    for (const outcome of outcomes) {
      const tokenKey = normalizeOpenPositionTokenId(outcome.token_id);
      const label = String(outcome.label || '').trim();
      if (!tokenKey || !label) continue;
      byTokenId.set(tokenKey, { label, legSide: outcome.legSide });
    }
    openPositionOutcomeCache.set(normalizedSlug, {
      expiresAt: now + OPEN_POSITION_OUTCOME_LABEL_CACHE_TTL_MS,
      byTokenId,
    });
    return byTokenId;
  } catch (err) {
    console.warn('Open position outcome label resolve error:', slug, err);
    const empty = new Map<string, MarketOutcomeInfo>();
    openPositionOutcomeCache.set(normalizedSlug, {
      expiresAt: now + OPEN_POSITION_OUTCOME_LABEL_ERROR_CACHE_TTL_MS,
      byTokenId: empty,
    });
    return empty;
  }
}

async function resolveMarketOutcomeLegSideCached(
  slug: string,
  tokenId: string
): Promise<'yes' | 'no' | null> {
  const normalizedTokenId = normalizeOpenPositionTokenId(tokenId);
  if (!normalizedTokenId) return null;
  const byTokenId = await getMarketOutcomeInfoMapCached(slug);
  return byTokenId.get(normalizedTokenId)?.legSide ?? null;
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


export type {
  Queryable,
  TradeFlowListFilters,
  TradeFlowRunFilters,
  CreateTradeFlowDefinitionInput,
  UpdateTradeFlowDefinitionInput,
  OpenTradeMatchCandidate,
};

export {
  SUPPORTED_NODE_TYPES,
  DEFAULT_GRAPH,
  OPEN_POSITIONS_MIN_CURRENT_VALUE_USD,
  OPEN_POSITION_MARKET_SLUG_KEYS,
  OPEN_POSITION_TOKEN_ID_KEYS,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  RESOLVE_MARKET_ALLOWED_ASSETS,
  RESOLVE_MARKET_ALLOWED_TIMEFRAMES,
  isRecord,
  isSupportedMarketPriceTriggerCondition,
  isSupportedTriggerCondition,
  toFiniteNumber,
  hasProvidedValue,
  hasValidOptionalMaxPrice,
  resolveConfiguredBinaryPrice,
  countValidOutcomeConditions,
  countValidMarketPriceOutcomeConditions,
  isValidMarketPriceOutcomeCondition,
  toBooleanish,
  toTrimmedString,
  normalizeDualDcaAsset,
  normalizeDualDcaTimeframe,
  toSlugPart,
  toFinitePositiveInteger,
  pickString,
  pickNumber,
  extractOpenPositionOutcomeLabel,
  normalizeOpenPositionSlug,
  normalizeOpenPositionTokenId,
  openPositionOutcomeLabelIndexKey,
  isGenericOpenPositionOutcomeLabel,
  getMarketOutcomeLabelMapCached,
  getMarketOutcomeInfoMapCached,
  buildOpenPositionOutcomeLabelIndex,
  resolveOpenPositionOutcomeLabel,
  resolveMarketOutcomeLegSideCached,
  loadOpenTradeMatchCandidates,
  matchOpenTradePosition,
  fetchPolymarketOpenPositions,
  normalizeOutcomeToLegSide,
  estimateSourceTradeValues,
};
