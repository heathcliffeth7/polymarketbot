import type {
  TradeFlowEnsureDualDcaSourceTradeResult,
  TradeFlowEnsureSourceTradeResult,
  TradeFlowOpenPositionOption,
  TradeFlowOpenPositionsResponse,
} from '@/lib/types';
import { pool } from '@/lib/db';
import { readPositionWalletAddress, type UserConfigContext } from '@/lib/config';
import {
  OPEN_POSITIONS_MIN_CURRENT_VALUE_USD,
  OPEN_POSITION_MARKET_SLUG_KEYS,
  OPEN_POSITION_TOKEN_ID_KEYS,
  buildOpenPositionOutcomeLabelIndex,
  estimateSourceTradeValues,
  extractOpenPositionOutcomeLabel,
  fetchPolymarketOpenPositions,
  loadOpenTradeMatchCandidates,
  matchOpenTradePosition,
  normalizeDualDcaAsset,
  normalizeDualDcaTimeframe,
  normalizeOutcomeToLegSide,
  resolveMarketOutcomeLegSideCached,
  pickNumber,
  pickString,
  resolveOpenPositionOutcomeLabel,
  toFinitePositiveInteger,
  toSlugPart,
  toTrimmedString,
  type TradeFlowEnsureDualDcaSourceTradeRequest,
  type TradeFlowEnsureSourceTradeRequest,
} from './shared';

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
  const legSide =
    (await resolveMarketOutcomeLegSideCached(marketSlug, tokenId)) ??
    normalizeOutcomeToLegSide(outcomeLabel);

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
      [tradeId, legSide, tokenId, qty, entryPrice]
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
