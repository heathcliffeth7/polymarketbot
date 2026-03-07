import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  createTradeBuilderOrder,
  getTradeBuilderOrders,
} from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { searchParams } = new URL(req.url);
    const result = await getTradeBuilderOrders({
      userId: user.userId,
      page: parseInt(searchParams.get('page') || '1', 10),
      limit: parseInt(searchParams.get('limit') || '20', 10),
      status: searchParams.get('status') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade builder order list error:', err);
    return NextResponse.json({ error: 'Failed to load trade builder orders' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = await req.json();

    const kind = String(body?.kind || '');
    const marketSlug = String(body?.marketSlug || '').trim();
    const tokenId = String(body?.tokenId || '').trim();
    const outcomeLabel = String(body?.outcomeLabel || '').trim();
    const side = String(body?.side || '').toLowerCase();
    const executionMode = String(body?.executionMode || '').trim().toLowerCase();
    const sizeUsdc = Number(body?.sizeUsdc);
    const minPriceDistanceCent = Number(body?.minPriceDistanceCent);
    const triggerCondition = body?.triggerCondition ? String(body.triggerCondition) : undefined;
    const triggerPriceCent = body?.triggerPriceCent != null ? Number(body.triggerPriceCent) : undefined;
    const expiresAt = body?.expiresAt ? String(body.expiresAt) : undefined;
    const maxTriggers = body?.maxTriggers != null ? Number(body.maxTriggers) : undefined;

    if (!['immediate', 'conditional'].includes(kind)) {
      return NextResponse.json({ error: 'kind must be immediate or conditional' }, { status: 400 });
    }
    if (!marketSlug) return NextResponse.json({ error: 'marketSlug is required' }, { status: 400 });
    if (!tokenId) return NextResponse.json({ error: 'tokenId is required' }, { status: 400 });
    if (!outcomeLabel) return NextResponse.json({ error: 'outcomeLabel is required' }, { status: 400 });
    if (!['buy', 'sell'].includes(side)) {
      return NextResponse.json({ error: 'side must be buy or sell' }, { status: 400 });
    }
    if (executionMode && !['limit', 'market'].includes(executionMode)) {
      return NextResponse.json({ error: 'executionMode must be limit or market' }, { status: 400 });
    }
    if (!Number.isFinite(sizeUsdc) || sizeUsdc <= 0) {
      return NextResponse.json({ error: 'sizeUsdc must be > 0' }, { status: 400 });
    }
    if (!Number.isFinite(minPriceDistanceCent) || minPriceDistanceCent <= 0) {
      return NextResponse.json({ error: 'minPriceDistanceCent must be > 0' }, { status: 400 });
    }

    if (kind === 'conditional') {
      if (!['cross_above', 'cross_below'].includes(triggerCondition || '')) {
        return NextResponse.json({ error: 'triggerCondition must be cross_above or cross_below' }, { status: 400 });
      }
      if (!Number.isFinite(triggerPriceCent) || (triggerPriceCent as number) <= 0 || (triggerPriceCent as number) > 100) {
        return NextResponse.json({ error: 'triggerPriceCent must be in (0, 100]' }, { status: 400 });
      }
      if (!expiresAt) {
        return NextResponse.json({ error: 'expiresAt is required for conditional orders' }, { status: 400 });
      }
    }

    const order = await createTradeBuilderOrder({
      userId: user.userId,
      kind: kind as 'immediate' | 'conditional',
      marketSlug,
      tokenId,
      outcomeLabel,
      side: side as 'buy' | 'sell',
      executionMode: (executionMode || 'limit') as 'limit' | 'market',
      sizeUsdc,
      minPriceDistanceCent,
      triggerCondition: triggerCondition as 'cross_above' | 'cross_below' | undefined,
      triggerPriceCent,
      expiresAt,
      maxTriggers,
    });

    return NextResponse.json({ data: order }, { status: 201 });
  } catch (err) {
    console.error('Trade builder order create error:', err);
    return NextResponse.json({ error: 'Failed to create trade builder order' }, { status: 500 });
  }
}
