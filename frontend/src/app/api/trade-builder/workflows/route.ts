import { NextRequest, NextResponse } from 'next/server';
import {
  createTradeBuilderWorkflow,
  getTradeBuilderWorkflows,
} from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const result = await getTradeBuilderWorkflows({
      page: parseInt(searchParams.get('page') || '1', 10),
      limit: parseInt(searchParams.get('limit') || '20', 10),
      status: searchParams.get('status') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade builder workflow list error:', err);
    return NextResponse.json({ error: 'Failed to load workflows' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();

    const sourceTradeId = Number(body?.sourceTradeId);
    const sellTargetPct = Number(body?.sellTargetPct);
    const buyStartAfterSellProgressPct = Number(body?.buyStartAfterSellProgressPct);
    const buyAllocationPct = Number(body?.buyAllocationPct);
    const buyTriggerMode = String(body?.buyTriggerMode || '');
    const expiresAt = body?.expiresAt ? String(body.expiresAt) : null;

    const sellLeg = body?.sellLeg;
    const buyLeg = body?.buyLeg;

    if (!Number.isFinite(sourceTradeId) || sourceTradeId <= 0) {
      return NextResponse.json({ error: 'sourceTradeId must be > 0' }, { status: 400 });
    }
    if (!Number.isFinite(sellTargetPct) || sellTargetPct <= 0 || sellTargetPct > 100) {
      return NextResponse.json({ error: 'sellTargetPct must be in (0, 100]' }, { status: 400 });
    }
    if (!Number.isFinite(buyStartAfterSellProgressPct) || buyStartAfterSellProgressPct < 0 || buyStartAfterSellProgressPct > 100) {
      return NextResponse.json({ error: 'buyStartAfterSellProgressPct must be in [0, 100]' }, { status: 400 });
    }
    if (!Number.isFinite(buyAllocationPct) || buyAllocationPct <= 0 || buyAllocationPct > 100) {
      return NextResponse.json({ error: 'buyAllocationPct must be in (0, 100]' }, { status: 400 });
    }
    if (!['sell_progress_only', 'price_only', 'sell_progress_and_price'].includes(buyTriggerMode)) {
      return NextResponse.json({ error: 'buyTriggerMode invalid' }, { status: 400 });
    }

    const normalizedSellLeg = normalizeLegPayload(sellLeg, 'sell');
    if ('error' in normalizedSellLeg) return NextResponse.json({ error: normalizedSellLeg.error }, { status: 400 });
    const normalizedBuyLeg = normalizeLegPayload(buyLeg, 'buy');
    if ('error' in normalizedBuyLeg) return NextResponse.json({ error: normalizedBuyLeg.error }, { status: 400 });

    const data = await createTradeBuilderWorkflow({
      name: body?.name ? String(body.name) : undefined,
      sourceTradeId,
      sellTargetPct,
      buyStartAfterSellProgressPct,
      buyTriggerMode: buyTriggerMode as 'sell_progress_only' | 'price_only' | 'sell_progress_and_price',
      buyAllocationPct,
      expiresAt,
      sellLeg: normalizedSellLeg,
      buyLeg: normalizedBuyLeg,
    });

    return NextResponse.json({ data }, { status: 201 });
  } catch (err) {
    console.error('Trade builder workflow create error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to create workflow' },
      { status: 500 }
    );
  }
}

function normalizeLegPayload(
  leg: unknown,
  expectedSide: 'sell' | 'buy'
):
  | {
      marketSlug: string;
      tokenId: string;
      outcomeLabel: string;
      side: 'buy' | 'sell';
      triggerCondition?: 'cross_above' | 'cross_below';
      triggerPriceCent?: number;
      minPriceDistanceCent: number;
    }
  | { error: string } {
  const payload = leg as Record<string, unknown> | null;
  const marketSlug = String(payload?.marketSlug || '').trim();
  const tokenId = String(payload?.tokenId || '').trim();
  const outcomeLabel = String(payload?.outcomeLabel || '').trim();
  const side = String(payload?.side || '').toLowerCase();
  const minPriceDistanceCent = Number(payload?.minPriceDistanceCent);
  const triggerCondition = payload?.triggerCondition ? String(payload.triggerCondition) : undefined;
  const triggerPriceCent = payload?.triggerPriceCent != null ? Number(payload.triggerPriceCent) : undefined;

  if (!marketSlug) return { error: `${expectedSide} leg marketSlug is required` };
  if (!tokenId) return { error: `${expectedSide} leg tokenId is required` };
  if (!outcomeLabel) return { error: `${expectedSide} leg outcomeLabel is required` };
  if (!['buy', 'sell'].includes(side)) return { error: `${expectedSide} leg side must be buy/sell` };
  if (!Number.isFinite(minPriceDistanceCent) || minPriceDistanceCent <= 0) {
    return { error: `${expectedSide} leg minPriceDistanceCent must be > 0` };
  }
  if (triggerCondition && !['cross_above', 'cross_below'].includes(triggerCondition)) {
    return { error: `${expectedSide} leg triggerCondition invalid` };
  }
  if (triggerCondition && (!Number.isFinite(triggerPriceCent) || (triggerPriceCent as number) <= 0 || (triggerPriceCent as number) > 100)) {
    return { error: `${expectedSide} leg triggerPriceCent must be in (0, 100]` };
  }

  return {
    marketSlug,
    tokenId,
    outcomeLabel,
    side: side as 'buy' | 'sell',
    triggerCondition: triggerCondition as 'cross_above' | 'cross_below' | undefined,
    triggerPriceCent,
    minPriceDistanceCent,
  };
}
