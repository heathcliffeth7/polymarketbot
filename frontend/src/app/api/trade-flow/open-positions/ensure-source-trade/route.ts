import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { ensureSourceTradeForOpenPosition } from '@/lib/queries/trade-flow';
import type { TradeFlowEnsureSourceTradeRequest } from '@/lib/types';

export const dynamic = 'force-dynamic';

function parseNullableNumber(value: unknown): number | null {
  if (value == null || value === '') return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = (await req.json()) as Partial<TradeFlowEnsureSourceTradeRequest> | null;
    const payload: TradeFlowEnsureSourceTradeRequest = {
      marketSlug: String(body?.marketSlug || '').trim(),
      tokenId: String(body?.tokenId || '').trim(),
      outcomeLabel: String(body?.outcomeLabel || '').trim(),
      marketTitle:
        body?.marketTitle == null ? null : String(body.marketTitle),
      size: parseNullableNumber(body?.size),
      avgPrice: parseNullableNumber(body?.avgPrice),
      currentValue: parseNullableNumber(body?.currentValue),
    };

    if (!payload.marketSlug) {
      return NextResponse.json({ error: 'marketSlug is required' }, { status: 400 });
    }
    if (!payload.tokenId) {
      return NextResponse.json({ error: 'tokenId is required' }, { status: 400 });
    }

    const data = await ensureSourceTradeForOpenPosition(user.userId, payload);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow ensure source trade error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Source trade oluşturulamadı' },
      { status: 500 }
    );
  }
}
