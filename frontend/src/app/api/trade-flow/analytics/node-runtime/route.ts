import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeFlowNodeRuntime } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { searchParams } = new URL(req.url);
    const runId = Number(searchParams.get('runId') || '');
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '50');
    const nodeKey = (searchParams.get('nodeKey') || '').trim() || undefined;
    const nodeType = (searchParams.get('nodeType') || '').trim() || undefined;

    if (!Number.isFinite(runId) || runId <= 0) {
      return NextResponse.json({ error: 'runId must be > 0' }, { status: 400 });
    }
    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }

    const result = await getTradeFlowNodeRuntime({
      userId: user.userId,
      runId: Math.floor(runId),
      nodeKey,
      nodeType,
      page: Math.floor(page),
      limit: Math.floor(limit),
    });

    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow node runtime analytics error:', err);
    return NextResponse.json(
      { error: 'Failed to load node runtime analytics' },
      { status: 500 }
    );
  }
}
