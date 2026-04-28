import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveAutoScopeTradeAnalysisDateFilters } from '@/lib/queries/trade-flow/analysis-time-range';
import { getAutoScopeTradeAnalysis } from '@/lib/queries/trade-flow';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
} from '@/lib/types';

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
    const sortBy =
      ((searchParams.get('sortBy') || '').trim() || 'default') as AutoScopeTradeAnalysisSortBy;
    const sortDirection =
      ((searchParams.get('sortDirection') || '').trim() ||
        'desc') as AutoScopeTradeAnalysisSortDirection;
    const pnl =
      ((searchParams.get('pnl') || '').trim() || 'all') as AutoScopeTradeAnalysisPnlFilter;
    const position =
      ((searchParams.get('position') || '').trim() ||
        'all') as AutoScopeTradeAnalysisPositionFilter;
    const dateFilters = resolveAutoScopeTradeAnalysisDateFilters({
      timeRangeRaw: searchParams.get('timeRange'),
      fromRaw: searchParams.get('from'),
      toRaw: searchParams.get('to'),
    });

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }
    if (dateFilters.error) {
      return NextResponse.json({ error: dateFilters.error }, { status: 400 });
    }

    const result = await getAutoScopeTradeAnalysis({
      userId: user.userId,
      username: user.username,
      page: Math.floor(page),
      limit: Math.floor(limit),
      sortBy,
      sortDirection,
      pnl,
      position,
      timeRange: dateFilters.timeRange,
      from: dateFilters.from,
      to: dateFilters.to,
    });

    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow auto-scope analytics error:', err);
    return NextResponse.json(
      { error: 'Failed to load auto-scope trade analysis' },
      { status: 500 }
    );
  }
}
