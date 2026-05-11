import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeFlowRunEvents } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ runId: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { runId: id } = await params;
    const runId = Number(id);
    if (!Number.isFinite(runId) || runId <= 0) {
      return NextResponse.json({ error: 'Invalid run id' }, { status: 400 });
    }

    const { searchParams } = new URL(req.url);
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '50');

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 200) {
      return NextResponse.json({ error: 'limit must be in [1,200]' }, { status: 400 });
    }

    const result = await getTradeFlowRunEvents(user.userId, runId, Math.floor(page), Math.floor(limit));
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow run events list error:', err);
    return NextResponse.json({ error: 'Failed to load flow run events' }, { status: 500 });
  }
}
