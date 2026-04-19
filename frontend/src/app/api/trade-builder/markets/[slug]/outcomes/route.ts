import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { readConfig } from '@/lib/config';
import { getMarketOutcomesBySlug } from '@/lib/queries/trade-builder';
import type { TradeBuilderOutcome } from '@/lib/types';

export const dynamic = 'force-dynamic';

const DEFAULT_CLOB_BASE_URL = process.env.CLOB_BASE_URL || 'https://clob.polymarket.com';

function parseFeeRateBps(raw: unknown): number | null {
  if (!raw || typeof raw !== 'object') return null;
  const value =
    (raw as Record<string, unknown>).fee_rate_bps ??
    (raw as Record<string, unknown>).feeRateBps;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

async function resolveClobBaseUrl(): Promise<string> {
  const user = await getSessionUser().catch(() => null);
  if (!user) return DEFAULT_CLOB_BASE_URL;
  const exchange = (await readConfig('exchange', user).catch(
    () => ({}) as Record<string, unknown>
  )) as Record<string, unknown>;
  const configured = String(exchange.clob_base_url ?? '').trim();
  return configured || DEFAULT_CLOB_BASE_URL;
}

async function enrichOutcomesWithFeeRates(
  outcomes: TradeBuilderOutcome[]
): Promise<TradeBuilderOutcome[]> {
  const tokenIds = Array.from(
    new Set(outcomes.map((outcome) => outcome.token_id.trim()).filter(Boolean))
  );
  if (tokenIds.length === 0) return outcomes;

  const clobBaseUrl = (await resolveClobBaseUrl()).replace(/\/$/, '');
  const feeRateEntries = await Promise.all(
    tokenIds.map(async (tokenId) => {
      try {
        const res = await fetch(`${clobBaseUrl}/fee-rate?token_id=${encodeURIComponent(tokenId)}`, {
          cache: 'no-store',
        });
        if (!res.ok) return [tokenId, null] as const;
        const raw = (await res.json()) as unknown;
        return [tokenId, parseFeeRateBps(raw)] as const;
      } catch {
        return [tokenId, null] as const;
      }
    })
  );
  const feeRateByTokenId = new Map(feeRateEntries);
  return outcomes.map((outcome) => ({
    ...outcome,
    feeRateBps: feeRateByTokenId.get(outcome.token_id) ?? null,
  }));
}

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ slug: string }> }
) {
  try {
    const { slug } = await params;
    const data = await enrichOutcomesWithFeeRates(await getMarketOutcomesBySlug(slug));
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade builder outcomes error:', err);
    return NextResponse.json({ error: 'Failed to load outcomes' }, { status: 500 });
  }
}
