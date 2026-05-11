import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  getTradeBuilderWorkflowById,
  requestCancelTradeBuilderWorkflow,
  updateTradeBuilderWorkflow,
} from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const workflowId = Number(id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) {
      return NextResponse.json({ error: 'Invalid workflow id' }, { status: 400 });
    }
    const data = await getTradeBuilderWorkflowById(user.userId, workflowId);
    if (!data) return NextResponse.json({ error: 'Workflow not found' }, { status: 404 });
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade builder workflow detail error:', err);
    return NextResponse.json({ error: 'Failed to load workflow' }, { status: 500 });
  }
}

export async function PATCH(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const workflowId = Number(id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) {
      return NextResponse.json({ error: 'Invalid workflow id' }, { status: 400 });
    }

    const body = await req.json();
    const updates: {
      buyStartAfterSellProgressPct?: number;
      buyTriggerMode?: 'sell_progress_only' | 'price_only' | 'sell_progress_and_price';
      buyAllocationPct?: number;
      expiresAt?: string | null;
    } = {};

    if (body?.buyStartAfterSellProgressPct !== undefined) {
      const v = Number(body.buyStartAfterSellProgressPct);
      if (!Number.isFinite(v) || v < 0 || v > 100) {
        return NextResponse.json({ error: 'buyStartAfterSellProgressPct must be in [0,100]' }, { status: 400 });
      }
      updates.buyStartAfterSellProgressPct = v;
    }

    if (body?.buyTriggerMode !== undefined) {
      const v = String(body.buyTriggerMode);
      if (!['sell_progress_only', 'price_only', 'sell_progress_and_price'].includes(v)) {
        return NextResponse.json({ error: 'buyTriggerMode invalid' }, { status: 400 });
      }
      updates.buyTriggerMode = v as 'sell_progress_only' | 'price_only' | 'sell_progress_and_price';
    }

    if (body?.buyAllocationPct !== undefined) {
      const v = Number(body.buyAllocationPct);
      if (!Number.isFinite(v) || v <= 0 || v > 100) {
        return NextResponse.json({ error: 'buyAllocationPct must be in (0,100]' }, { status: 400 });
      }
      updates.buyAllocationPct = v;
    }

    if (body?.expiresAt !== undefined) {
      updates.expiresAt = body.expiresAt ? String(body.expiresAt) : null;
    }

    await updateTradeBuilderWorkflow(user.userId, workflowId, updates);
    return NextResponse.json({ success: true });
  } catch (err) {
    if (err instanceof Error && err.message === 'Trade builder workflow not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    console.error('Trade builder workflow patch error:', err);
    return NextResponse.json({ error: 'Failed to update workflow' }, { status: 500 });
  }
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const workflowId = Number(id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) {
      return NextResponse.json({ error: 'Invalid workflow id' }, { status: 400 });
    }
    await requestCancelTradeBuilderWorkflow(user.userId, workflowId);
    return NextResponse.json({ success: true });
  } catch (err) {
    if (err instanceof Error && err.message === 'Trade builder workflow not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    console.error('Trade builder workflow cancel error:', err);
    return NextResponse.json({ error: 'Failed to cancel workflow' }, { status: 500 });
  }
}
