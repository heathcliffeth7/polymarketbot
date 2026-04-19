import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeFlowPtbState } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { searchParams } = new URL(req.url);
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '50');
    const runIdRaw = searchParams.get('runId');
    const runId = runIdRaw == null || runIdRaw.trim() === '' ? null : Number(runIdRaw);

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }
    if (runIdRaw != null && (!Number.isFinite(runId) || (runId as number) <= 0)) {
      return NextResponse.json({ error: 'runId must be > 0' }, { status: 400 });
    }

    const result = await getTradeFlowPtbState({
      userId: user.userId,
      runId,
      page: Math.floor(page),
      limit: Math.floor(limit),
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow PTB state analytics error:', err);
    return NextResponse.json(
      { error: 'Failed to load PTB state analytics' },
      { status: 500 }
    );
  }
}
