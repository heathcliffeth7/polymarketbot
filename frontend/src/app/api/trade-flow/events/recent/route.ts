import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getRecentTradeFlowEvents } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

const ALLOWED_RUN_STATUSES = new Set([
  'queued',
  'running',
  'completed',
  'failed',
  'canceled',
]);

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { searchParams } = new URL(req.url);
    const rawStatus = searchParams.get('status');
    const status = rawStatus && rawStatus.trim().length > 0 ? rawStatus.trim() : 'running';
    const limit = Number(searchParams.get('limit') || '100');

    if (!ALLOWED_RUN_STATUSES.has(status)) {
      return NextResponse.json({ error: 'Invalid run status' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 200) {
      return NextResponse.json({ error: 'limit must be in [1,200]' }, { status: 400 });
    }

    const data = await getRecentTradeFlowEvents(
      user.userId,
      status as 'queued' | 'running' | 'completed' | 'failed' | 'canceled',
      Math.floor(limit)
    );
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow recent events error:', err);
    return NextResponse.json({ error: 'Failed to load recent flow events' }, { status: 500 });
  }
}
