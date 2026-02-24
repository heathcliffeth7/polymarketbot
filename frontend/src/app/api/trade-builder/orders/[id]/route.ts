import { NextRequest, NextResponse } from 'next/server';
import {
  requestCancelTradeBuilderOrder,
  updateTradeBuilderOrder,
} from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function PATCH(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const orderId = Number(id);
    if (!Number.isFinite(orderId) || orderId <= 0) {
      return NextResponse.json({ error: 'Invalid order id' }, { status: 400 });
    }

    const body = await req.json();
    const updates: { minPriceDistanceCent?: number; maxTriggers?: number; expiresAt?: string | null } = {};

    if (body?.minPriceDistanceCent !== undefined) {
      const v = Number(body.minPriceDistanceCent);
      if (!Number.isFinite(v) || v <= 0) {
        return NextResponse.json({ error: 'minPriceDistanceCent must be > 0' }, { status: 400 });
      }
      updates.minPriceDistanceCent = v;
    }

    if (body?.maxTriggers !== undefined) {
      const v = Number(body.maxTriggers);
      if (!Number.isFinite(v) || v < 1 || v > 20) {
        return NextResponse.json({ error: 'maxTriggers must be in [1,20]' }, { status: 400 });
      }
      updates.maxTriggers = v;
    }

    if (body?.expiresAt !== undefined) {
      updates.expiresAt = body.expiresAt ? String(body.expiresAt) : null;
    }

    await updateTradeBuilderOrder(orderId, updates);
    return NextResponse.json({ success: true });
  } catch (err) {
    console.error('Trade builder order patch error:', err);
    return NextResponse.json({ error: 'Failed to update trade builder order' }, { status: 500 });
  }
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const orderId = Number(id);
    if (!Number.isFinite(orderId) || orderId <= 0) {
      return NextResponse.json({ error: 'Invalid order id' }, { status: 400 });
    }

    await requestCancelTradeBuilderOrder(orderId);
    return NextResponse.json({ success: true });
  } catch (err) {
    console.error('Trade builder order cancel error:', err);
    return NextResponse.json({ error: 'Failed to cancel trade builder order' }, { status: 500 });
  }
}
