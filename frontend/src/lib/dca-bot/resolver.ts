import type { DcaResolvedMarket, DcaResolvedOutcome } from './schema';

const GAMMA_BASE_URL = process.env.GAMMA_BASE_URL || 'https://gamma-api.polymarket.com';
const CLOB_BASE_URL = process.env.CLOB_BASE_URL || 'https://clob.polymarket.com';

interface QuoteSnapshot {
  bestBid: number | null;
  bestAsk: number | null;
  mid: number | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function toNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function toBoolean(value: unknown, fallback: boolean): boolean {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return value !== 0;
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    if (['true', '1', 'yes', 'on'].includes(normalized)) return true;
    if (['false', '0', 'no', 'off'].includes(normalized)) return false;
  }
  return fallback;
}

function parseStringArray(value: unknown): string[] {
  if (Array.isArray(value)) return value.map((item) => String(item));
  if (typeof value !== 'string') return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.map((item) => String(item)) : [];
  } catch {
    return [];
  }
}

function parseDateIso(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  const parsed = Date.parse(String(value));
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : null;
}

function parseSlugInput(input: string): string {
  const trimmed = input.trim();
  if (!trimmed) return '';
  try {
    const url = new URL(trimmed);
    const parts = url.pathname.split('/').map((part) => part.trim()).filter(Boolean);
    const markerIndex = parts.findIndex((part) => part === 'event' || part === 'market');
    if (markerIndex >= 0 && parts[markerIndex + 1]) return decodeURIComponent(parts[markerIndex + 1]);
    return decodeURIComponent(parts.at(-1) || trimmed);
  } catch {
    return trimmed.replace(/^\/+|\/+$/g, '');
  }
}

async function fetchGammaPath(path: string): Promise<unknown | null> {
  const res = await fetch(`${GAMMA_BASE_URL.replace(/\/$/, '')}${path}`, { cache: 'no-store' });
  if (!res.ok) return null;
  return res.json() as Promise<unknown>;
}

async function fetchMarketBySlug(slug: string): Promise<Record<string, unknown> | null> {
  const data = await fetchGammaPath(`/markets/slug/${encodeURIComponent(slug)}`);
  if (Array.isArray(data)) return isRecord(data[0]) ? data[0] : null;
  return isRecord(data) ? data : null;
}

async function fetchEventBySlug(slug: string): Promise<Record<string, unknown> | null> {
  const data = await fetchGammaPath(`/events/slug/${encodeURIComponent(slug)}`);
  return isRecord(data) ? data : null;
}

function parseOrderPrice(item: unknown): number | null {
  if (!isRecord(item)) return null;
  return toNumber(item.price ?? item.p);
}

async function fetchClobQuote(tokenId: string): Promise<QuoteSnapshot> {
  try {
    const url = `${CLOB_BASE_URL.replace(/\/$/, '')}/book?token_id=${encodeURIComponent(tokenId)}`;
    const res = await fetch(url, { cache: 'no-store' });
    if (!res.ok) return { bestBid: null, bestAsk: null, mid: null };
    const data = (await res.json()) as unknown;
    if (!isRecord(data)) return { bestBid: null, bestAsk: null, mid: null };
    const bids = Array.isArray(data.bids) ? data.bids.map(parseOrderPrice).filter((v): v is number => v != null) : [];
    const asks = Array.isArray(data.asks) ? data.asks.map(parseOrderPrice).filter((v): v is number => v != null) : [];
    const bestBid = bids.length > 0 ? Math.max(...bids) : null;
    const bestAsk = asks.length > 0 ? Math.min(...asks) : null;
    const mid = bestBid != null && bestAsk != null ? (bestBid + bestAsk) / 2 : null;
    return { bestBid, bestAsk, mid };
  } catch {
    return { bestBid: null, bestAsk: null, mid: null };
  }
}

function normalizeOutcomeLabel(label: string): string {
  const normalized = label.trim().toLowerCase();
  if (['yes', 'true', '1'].includes(normalized)) return 'YES';
  if (['no', 'false', '0'].includes(normalized)) return 'NO';
  if (normalized === 'up') return 'UP';
  if (normalized === 'down') return 'DOWN';
  return label.trim().toUpperCase();
}

function areComplementaryBinaryOutcomes(outcomes: DcaResolvedOutcome[]): boolean {
  if (outcomes.length !== 2) return false;
  const labels = new Set(outcomes.map((outcome) => outcome.normalizedLabel));
  return (
    (labels.has('YES') && labels.has('NO')) ||
    (labels.has('UP') && labels.has('DOWN'))
  );
}

function extractMarketOutcomes(market: Record<string, unknown>): Array<Omit<DcaResolvedOutcome, 'bestBid' | 'bestAsk' | 'mid'>> {
  const tokenRows = Array.isArray(market.tokens)
    ? (market.tokens as unknown[]).filter(isRecord)
    : [];
  if (tokenRows.length > 0) {
    return tokenRows
      .map((token) => {
        const tokenId = String(token.token_id ?? token.tokenId ?? token.clobTokenId ?? token.id ?? '').trim();
        const label = String(token.outcome ?? token.name ?? token.title ?? '').trim();
        if (!tokenId || !label) return null;
        return {
          label,
          normalizedLabel: normalizeOutcomeLabel(label),
          tokenId,
          liquidity: toNumber(token.liquidity ?? market.liquidity),
        };
      })
      .filter((item): item is Omit<DcaResolvedOutcome, 'bestBid' | 'bestAsk' | 'mid'> => item != null);
  }

  const labels = parseStringArray(market.outcomes);
  const tokenIds = parseStringArray(market.clobTokenIds ?? market.clob_token_ids);
  const prices = parseStringArray(market.outcomePrices ?? market.outcome_prices);
  const len = Math.min(labels.length, tokenIds.length);
  const outcomes: Array<Omit<DcaResolvedOutcome, 'bestBid' | 'bestAsk' | 'mid'>> = [];
  for (let index = 0; index < len; index += 1) {
    const label = labels[index]?.trim() || '';
    const tokenId = tokenIds[index]?.trim() || '';
    if (!label || !tokenId) continue;
    outcomes.push({
      label,
      normalizedLabel: normalizeOutcomeLabel(label),
      tokenId,
      liquidity: toNumber(prices[index]) ?? toNumber(market.liquidity),
    });
  }
  return outcomes;
}

function eventMarketLabel(market: Record<string, unknown>): string {
  return String(
    market.groupItemTitle ||
      market.title ||
      market.question ||
      market.slug ||
      'Market'
  ).trim();
}

async function enrichOutcomes(rawOutcomes: Array<Omit<DcaResolvedOutcome, 'bestBid' | 'bestAsk' | 'mid'>>): Promise<DcaResolvedOutcome[]> {
  const quotes = await Promise.all(rawOutcomes.map((outcome) => fetchClobQuote(outcome.tokenId)));
  return rawOutcomes.map((outcome, index) => ({
    ...outcome,
    bestBid: quotes[index]?.bestBid ?? null,
    bestAsk: quotes[index]?.bestAsk ?? null,
    mid: quotes[index]?.mid ?? null,
  }));
}

function pairEligibility(outcomes: DcaResolvedOutcome[], eventMarketCount: number): Pick<DcaResolvedMarket, 'isBinary' | 'pairEligible' | 'pairEligibilityReason'> {
  if (eventMarketCount > 1) {
    return { isBinary: false, pairEligible: false, pairEligibilityReason: 'event_has_multiple_markets' };
  }
  if (outcomes.length < 2) {
    return { isBinary: false, pairEligible: false, pairEligibilityReason: 'market_has_less_than_two_outcomes' };
  }
  if (outcomes.length > 2) {
    return { isBinary: false, pairEligible: false, pairEligibilityReason: 'market_has_more_than_two_outcomes' };
  }
  if (!areComplementaryBinaryOutcomes(outcomes)) {
    return { isBinary: true, pairEligible: false, pairEligibilityReason: 'binary_outcomes_are_not_standard_complements' };
  }
  return { isBinary: true, pairEligible: true, pairEligibilityReason: 'binary_two_outcome_market' };
}

async function resolveMarketRecord(market: Record<string, unknown>, eventMarketCount = 1): Promise<DcaResolvedMarket> {
  const rawOutcomes = extractMarketOutcomes(market);
  const outcomes = await enrichOutcomes(rawOutcomes);
  const closed = toBoolean(market.closed, false);
  const resolved = toBoolean(market.resolved ?? market.archived, false);
  const active = toBoolean(market.active, !closed);
  const eligibility = pairEligibility(outcomes, eventMarketCount);
  return {
    slug: String(market.slug || '').trim(),
    title: String(market.question || market.title || market.slug || '').trim(),
    status: closed ? 'closed' : resolved ? 'resolved' : active ? 'active' : 'inactive',
    isClosed: closed,
    isResolved: resolved,
    ...eligibility,
    endTime: parseDateIso(market.endDate ?? market.end_date ?? market.endTime ?? market.end_time),
    volume: toNumber(market.volume ?? market.volumeNum),
    liquidity: toNumber(market.liquidity ?? market.liquidityNum),
    outcomes,
  };
}

async function resolveEventRecord(event: Record<string, unknown>): Promise<DcaResolvedMarket | null> {
  const markets = Array.isArray(event.markets)
    ? (event.markets as unknown[]).filter(isRecord)
    : [];
  if (markets.length === 0) return null;
  if (markets.length === 1) return resolveMarketRecord(markets[0], 1);

  const rawOutcomes = markets.flatMap((market) =>
    extractMarketOutcomes(market).map((outcome) => ({
      ...outcome,
      label: `${eventMarketLabel(market)}: ${outcome.label}`,
      normalizedLabel: normalizeOutcomeLabel(`${eventMarketLabel(market)}: ${outcome.label}`),
    }))
  );
  const outcomes = await enrichOutcomes(rawOutcomes);
  return {
    slug: String(event.slug || '').trim(),
    title: String(event.title || event.question || event.slug || '').trim(),
    status: 'active',
    isClosed: false,
    isResolved: false,
    isBinary: false,
    endTime: parseDateIso(event.endDate ?? event.end_date ?? event.endTime ?? event.end_time),
    volume: toNumber(event.volume ?? event.volumeNum),
    liquidity: toNumber(event.liquidity ?? event.liquidityNum),
    outcomes,
    pairEligible: false,
    pairEligibilityReason: 'event_has_multiple_markets',
  };
}

export async function resolveDcaMarketInput(input: string): Promise<DcaResolvedMarket | null> {
  const slug = parseSlugInput(input);
  if (!slug) return null;

  const market = await fetchMarketBySlug(slug);
  if (market) {
    const events = Array.isArray(market.events) ? market.events.filter(isRecord) : [];
    if (events.length > 0) {
      const eventSlug = String(events[0].slug || '').trim();
      if (eventSlug) {
        const event = await fetchEventBySlug(eventSlug);
        const eventMarkets = Array.isArray(event?.markets) ? event.markets.filter(isRecord) : [];
        if (eventMarkets.length > 1) return resolveEventRecord(event as Record<string, unknown>);
      }
    }
    return resolveMarketRecord(market, 1);
  }

  const event = await fetchEventBySlug(slug);
  return event ? resolveEventRecord(event) : null;
}

export async function resolveDcaMarketInputs(inputs: string[]): Promise<DcaResolvedMarket[]> {
  const uniqueInputs = Array.from(new Set(inputs.map((input) => input.trim()).filter(Boolean)));
  const markets = await Promise.all(uniqueInputs.map((input) => resolveDcaMarketInput(input)));
  return markets.filter((market): market is DcaResolvedMarket => market != null);
}
