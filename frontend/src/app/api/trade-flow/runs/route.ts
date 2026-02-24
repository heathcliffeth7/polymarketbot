import { NextRequest, NextResponse } from 'next/server';
import { getTradeFlowRuns } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '20');
    const definitionIdRaw = searchParams.get('definitionId');
    const status = (searchParams.get('status') || '').trim() || undefined;

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }

    let definitionId: number | undefined;
    if (definitionIdRaw != null && definitionIdRaw.trim() !== '') {
      const parsed = Number(definitionIdRaw);
      if (!Number.isFinite(parsed) || parsed <= 0) {
        return NextResponse.json({ error: 'definitionId must be > 0' }, { status: 400 });
      }
      definitionId = parsed;
    }

    const result = await getTradeFlowRuns({
      page: Math.floor(page),
      limit: Math.floor(limit),
      definitionId,
      status,
    });

    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow run list error:', err);
    return NextResponse.json({ error: 'Failed to load flow runs' }, { status: 500 });
  }
}
