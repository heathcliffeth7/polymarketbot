import type { TradeBuilderMarketSearchItem, TradeBuilderOutcome } from '@/lib/types'
import type { ActiveUpdownMarketsCacheEntry } from './types'

const GAMMA_BASE_URL = process.env.GAMMA_BASE_URL || 'https://gamma-api.polymarket.com'
const SCOPE_TO_UPDOWN_SLUG_PREFIX: Record<string, string> = {
  btc_5m_updown: 'btc-updown-5m-',
  btc_15m_updown: 'btc-updown-15m-',
  eth_5m_updown: 'eth-updown-5m-',
  eth_15m_updown: 'eth-updown-15m-',
  sol_5m_updown: 'sol-updown-5m-',
  sol_15m_updown: 'sol-updown-15m-',
  xrp_5m_updown: 'xrp-updown-5m-',
  xrp_15m_updown: 'xrp-updown-15m-',
}
const ACTIVE_UPDOWN_MARKETS_CACHE_TTL_MS = 30_000

let activeUpdownMarketsCache: ActiveUpdownMarketsCacheEntry | null = null

export async function searchGammaMarkets(query: string): Promise<TradeBuilderMarketSearchItem[]> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets?active=true&closed=false&limit=200`
  const res = await fetch(url, { cache: 'no-store' })
  if (!res.ok) return []

  const rows = (await res.json()) as Array<Record<string, unknown>>
  const needle = query.trim().toLowerCase()

  return rows
    .map((row) => {
      const slug = String(row.slug || '')
      const question = String(row.question || row.title || slug)
      const endDate = row.endDate ? String(row.endDate) : null
      const active = row.active !== false
      return {
        slug,
        title: question,
        endDate,
        active,
      }
    })
    .filter((item) => item.slug.length > 0)
    .filter((item) => {
      if (!needle) return true
      return item.slug.toLowerCase().includes(needle) || item.title.toLowerCase().includes(needle)
    })
    .slice(0, 40)
}

export async function getMarketOutcomesBySlug(slug: string): Promise<TradeBuilderOutcome[]> {
  const trimmed = slug.trim()
  if (!trimmed) return []
  const normalized = trimmed.toLowerCase()

  const scopeMarket = await resolveMarketFromScope(normalized)
  if (scopeMarket) {
    const eventOutcomes = await tryExtractEventOutcomes(scopeMarket)
    if (eventOutcomes.length > 1) return eventOutcomes
    return extractOutcomes(scopeMarket)
  }

  const market = await fetchMarketBySlug(trimmed)
  if (market) {
    const eventOutcomes = await tryExtractEventOutcomes(market)
    if (eventOutcomes.length > 1) return eventOutcomes
    return extractOutcomes(market)
  }

  const eventData = await fetchEventData(trimmed)
  if (eventData) {
    const markets = Array.isArray(eventData.markets)
      ? (eventData.markets as Array<Record<string, unknown>>)
      : []
    if (markets.length > 1) return extractEventMarketOutcomes(markets)
    if (markets.length === 1) return extractOutcomes(markets[0] as Record<string, unknown>)
  }

  return []
}

async function fetchMarketBySlug(slug: string): Promise<Record<string, unknown> | null> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets/slug/${encodeURIComponent(slug)}`
  const res = await fetch(url, { cache: 'no-store' })
  if (!res.ok) return null
  const data = (await res.json()) as unknown
  if (Array.isArray(data)) return (data[0] as Record<string, unknown>) || null
  if (data && typeof data === 'object') return data as Record<string, unknown>
  return null
}

async function fetchEventData(slug: string): Promise<Record<string, unknown> | null> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/events/slug/${encodeURIComponent(slug)}`
  const res = await fetch(url, { cache: 'no-store' })
  if (!res.ok) return null
  const data = (await res.json()) as unknown
  if (data && typeof data === 'object' && !Array.isArray(data)) return data as Record<string, unknown>
  return null
}

function isTruthyValue(value: unknown): boolean {
  if (typeof value === 'boolean') return value
  if (typeof value === 'number') return value !== 0
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase()
    return normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on'
  }
  return false
}

function isMarketActive(row: Record<string, unknown>): boolean {
  const activeRaw = row.active
  const closedRaw = row.closed
  const active = activeRaw == null ? true : isTruthyValue(activeRaw)
  const closed = closedRaw == null ? false : isTruthyValue(closedRaw)
  return active && !closed
}

function parseDateMs(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) {
    if (value > 10_000_000_000) return Math.floor(value)
    if (value > 100_000_000) return Math.floor(value * 1000)
    return null
  }
  if (typeof value !== 'string') return null
  const trimmed = value.trim()
  if (!trimmed) return null
  const parsedNumber = Number(trimmed)
  if (Number.isFinite(parsedNumber)) {
    if (parsedNumber > 10_000_000_000) return Math.floor(parsedNumber)
    if (parsedNumber > 100_000_000) return Math.floor(parsedNumber * 1000)
  }
  const parsedDate = Date.parse(trimmed)
  return Number.isFinite(parsedDate) ? parsedDate : null
}

function scopeWindowSeconds(scope: string): number {
  return scope.includes('_15m_') ? 900 : 300
}

function inferWindowFromScopeMarket(
  market: Record<string, unknown>,
  slugPrefix: string,
  windowSec: number
): { startsAtMs: number | null; endsAtMs: number | null } {
  const slug = String(market.slug || '').trim().toLowerCase()
  let startsAtMs: number | null = null
  if (slug.startsWith(slugPrefix)) {
    const suffix = slug.slice(slugPrefix.length)
    const match = suffix.match(/^(\d{9,13})$/)
    if (match) {
      const rawTs = Number(match[1])
      if (Number.isFinite(rawTs) && rawTs > 0) {
        startsAtMs = rawTs > 10_000_000_000 ? Math.floor(rawTs) : Math.floor(rawTs * 1000)
      }
    }
  }

  const endsAtMs =
    parseDateMs(market.endDate) ??
    parseDateMs(market.end_date) ??
    parseDateMs(market.endDateIso) ??
    parseDateMs(market.end_date_iso)

  if (startsAtMs != null && endsAtMs == null) {
    return { startsAtMs, endsAtMs: startsAtMs + windowSec * 1000 }
  }
  if (startsAtMs == null && endsAtMs != null) {
    return { startsAtMs: endsAtMs - windowSec * 1000, endsAtMs }
  }
  return { startsAtMs, endsAtMs }
}

function selectPreferredScopeMarket(
  markets: Array<Record<string, unknown>>,
  slugPrefix: string,
  windowSec: number
): Record<string, unknown> | null {
  if (markets.length === 0) return null
  const nowMs = Date.now()
  const withWindow = markets.map((market) => {
    const slug = String(market.slug || '').trim().toLowerCase()
    const window = inferWindowFromScopeMarket(market, slugPrefix, windowSec)
    return { market, slug, ...window }
  })

  const inWindow = withWindow
    .filter((row) => row.startsAtMs != null && row.endsAtMs != null && row.startsAtMs <= nowMs && nowMs < row.endsAtMs)
    .sort((a, b) => {
      const startA = a.startsAtMs ?? 0
      const startB = b.startsAtMs ?? 0
      if (startA !== startB) return startB - startA
      return b.slug.localeCompare(a.slug)
    })
  if (inWindow.length > 0) return inWindow[0].market

  const nearestFuture = withWindow
    .filter((row) => row.startsAtMs != null && row.startsAtMs >= nowMs)
    .sort((a, b) => {
      const startA = a.startsAtMs ?? Number.MAX_SAFE_INTEGER
      const startB = b.startsAtMs ?? Number.MAX_SAFE_INTEGER
      if (startA !== startB) return startA - startB
      return a.slug.localeCompare(b.slug)
    })
  if (nearestFuture.length > 0) return nearestFuture[0].market

  return withWindow.sort((a, b) => b.slug.localeCompare(a.slug)).at(0)?.market || null
}

function buildScopeCandidateSlugs(slugPrefix: string, windowSec: number, nowMs: number): string[] {
  const nowSec = Math.floor(nowMs / 1000)
  const base = nowSec - (nowSec % windowSec)
  return [base - windowSec, base, base + windowSec, base + 2 * windowSec]
    .filter((value) => value > 0)
    .map((value) => `${slugPrefix}${value}`)
}

async function fetchActiveUpdownMarkets(): Promise<Array<Record<string, unknown>>> {
  const now = Date.now()
  if (activeUpdownMarketsCache && activeUpdownMarketsCache.expiresAt > now) {
    return activeUpdownMarketsCache.markets
  }

  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets?active=true&closed=false&limit=1000`
  const res = await fetch(url, { cache: 'no-store' })
  if (!res.ok) return []
  const payload = (await res.json()) as unknown
  const rows = Array.isArray(payload)
    ? payload.filter((item): item is Record<string, unknown> => !!item && typeof item === 'object')
    : []
  const prefixes = new Set(Object.values(SCOPE_TO_UPDOWN_SLUG_PREFIX))
  const markets = rows.filter((row) => {
    if (!isMarketActive(row)) return false
    const marketSlug = String(row.slug || '').trim().toLowerCase()
    if (!marketSlug) return false
    for (const prefix of prefixes) {
      if (marketSlug.startsWith(prefix)) return true
    }
    return false
  })
  activeUpdownMarketsCache = {
    expiresAt: now + ACTIVE_UPDOWN_MARKETS_CACHE_TTL_MS,
    markets,
  }
  return markets
}

async function resolveMarketFromScope(scope: string): Promise<Record<string, unknown> | null> {
  const normalizedScope = scope.trim().toLowerCase()
  const slugPrefix = SCOPE_TO_UPDOWN_SLUG_PREFIX[normalizedScope]
  if (!slugPrefix) return null

  const windowSec = scopeWindowSeconds(normalizedScope)
  const activeMarkets = await fetchActiveUpdownMarkets()
  const scopedMarkets = activeMarkets.filter((market) =>
    String(market.slug || '').trim().toLowerCase().startsWith(slugPrefix)
  )
  const selected = selectPreferredScopeMarket(scopedMarkets, slugPrefix, windowSec)
  if (selected) return selected

  const candidates = buildScopeCandidateSlugs(slugPrefix, windowSec, Date.now())
  const fetched = await Promise.all(candidates.map((candidateSlug) => fetchMarketBySlug(candidateSlug)))
  const fallbackScoped = fetched
    .filter((market): market is Record<string, unknown> => !!market)
    .filter((market) => isMarketActive(market))
    .filter((market) => String(market.slug || '').trim().toLowerCase().startsWith(slugPrefix))
  return selectPreferredScopeMarket(fallbackScoped, slugPrefix, windowSec)
}

async function tryExtractEventOutcomes(market: Record<string, unknown>): Promise<TradeBuilderOutcome[]> {
  const events = Array.isArray(market.events) ? (market.events as Array<Record<string, unknown>>) : []
  const eventSlug = events.length > 0 ? String(events[0].slug || '').trim() : ''
  if (!eventSlug) return []
  const eventData = await fetchEventData(eventSlug)
  if (!eventData) return []
  const markets = Array.isArray(eventData.markets) ? (eventData.markets as Array<Record<string, unknown>>) : []
  if (markets.length <= 1) return []
  return extractEventMarketOutcomes(markets)
}

function extractEventMarketOutcomes(markets: Array<Record<string, unknown>>): TradeBuilderOutcome[] {
  const out: TradeBuilderOutcome[] = []
  for (const m of markets) {
    const clobIds = parseStringArray(m.clobTokenIds || m.clob_token_ids)
    const outcomes = parseStringArray(m.outcomes)
    const outcomePrices = parseStringArray(m.outcomePrices)
    const rawMarketLabel = resolveEventOutcomeLabel(m)
    const marketLabel = trimOutcomeLabel(rawMarketLabel)
    if (!marketLabel) continue

    const len = Math.min(clobIds.length, outcomes.length, 2)
    for (let index = 0; index < len; index += 1) {
      const tokenId = clobIds[index]?.trim()
      const outcomeLabel = outcomes[index]?.trim()
      const legSide = legSideForIndex(index)
      if (!tokenId || !outcomeLabel || !legSide) continue
      const priceStr = outcomePrices[index] ?? null
      const price = priceStr ? parseFloat(priceStr) : null
      out.push({
        token_id: tokenId,
        label: `${marketLabel}: ${outcomeLabel}`,
        price: Number.isFinite(price as number) ? (price as number) : null,
        legSide,
      })
    }
  }
  return out
}

function resolveEventOutcomeLabel(market: Record<string, unknown>): string {
  const groupItemTitle = String(market.groupItemTitle || '').trim()
  if (groupItemTitle) return groupItemTitle

  const title = String(market.title || '').trim()
  if (title) return title

  const sportsMarketType = String(market.sportsMarketType || '').trim().toLowerCase()
  if (sportsMarketType === 'moneyline') return 'Moneyline'
  if (sportsMarketType === 'first_half_moneyline') return '1H Moneyline'

  return String(market.question || '').trim()
}

function trimOutcomeLabel(label: string): string {
  const trimmed = label.trim()
  return trimmed.includes('(') ? trimmed.slice(0, trimmed.indexOf('(')).trim() : trimmed
}

function legSideForIndex(index: number): 'yes' | 'no' | null {
  if (index === 0) return 'yes'
  if (index === 1) return 'no'
  return null
}

function extractOutcomes(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const fromTokens = extractOutcomesFromTokens(market)
  if (fromTokens.length > 0) return fromTokens
  return extractOutcomesFromArrays(market)
}

function extractOutcomesFromTokens(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const tokens = Array.isArray(market.tokens) ? (market.tokens as Array<Record<string, unknown>>) : []
  return tokens
    .slice(0, 2)
    .map((token, index) => {
      const legSide = legSideForIndex(index)
      const tokenId = String(token.token_id || token.tokenId || token.clobTokenId || token.id || '').trim()
      const label = String(token.outcome || token.name || token.title || '').trim()
      const priceValue = token.price ?? token.lastPrice ?? null
      const price = typeof priceValue === 'number' ? priceValue : typeof priceValue === 'string' ? parseFloat(priceValue) : null
      if (!tokenId || !label || !legSide) return null
      return {
        token_id: tokenId,
        label,
        price: Number.isFinite(price as number) ? (price as number) : null,
        legSide,
      }
    })
    .filter((item): item is TradeBuilderOutcome => !!item)
}

function extractOutcomesFromArrays(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const outcomesRaw = market.outcomes
  const tokenIdsRaw = market.clobTokenIds || market.clob_token_ids

  const outcomes = parseStringArray(outcomesRaw)
  const tokenIds = parseStringArray(tokenIdsRaw)
  const outcomePrices = parseStringArray(market.outcomePrices)

  if (outcomes.length === 0 || tokenIds.length === 0) return []

  const len = Math.min(outcomes.length, tokenIds.length, 2)
  const out: TradeBuilderOutcome[] = []
  for (let i = 0; i < len; i += 1) {
    const legSide = legSideForIndex(i)
    const tokenId = tokenIds[i]?.trim()
    const label = outcomes[i]?.trim()
    const priceStr = outcomePrices[i] ?? null
    const price = priceStr ? parseFloat(priceStr) : null
    if (!tokenId || !label || !legSide) continue
    out.push({
      token_id: tokenId,
      label,
      price: Number.isFinite(price as number) ? (price as number) : null,
      legSide,
    })
  }
  return out
}

function parseStringArray(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map((x) => String(x))
  }
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value)
      if (Array.isArray(parsed)) return parsed.map((x) => String(x))
    } catch {
      return []
    }
  }
  return []
}
