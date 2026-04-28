import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveAutoScopeTradeAnalysisDateFilters } from '@/lib/queries/trade-flow/analysis-time-range';
import {
  buildAutoScopeTradeAnalysisForensicCsv,
  getAutoScopeTradeAnalysisRowsForExport,
} from '@/lib/queries/trade-flow';
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

    if (dateFilters.error) {
      return NextResponse.json({ error: dateFilters.error }, { status: 400 });
    }

    const rows = await getAutoScopeTradeAnalysisRowsForExport({
      userId: user.userId,
      username: user.username,
      sortBy,
      sortDirection,
      pnl,
      position,
      timeRange: dateFilters.timeRange,
      from: dateFilters.from,
      to: dateFilters.to,
    });
    const csv = buildAutoScopeTradeAnalysisForensicCsv(rows);
    const date = new Date().toISOString().slice(0, 10);

    return new NextResponse(csv, {
      headers: {
        'Content-Type': 'text/csv; charset=utf-8',
        'Content-Disposition': `attachment; filename="trade_analysis_forensic_full-${date}.csv"`,
        'Cache-Control': 'no-store',
      },
    });
  } catch (err) {
    console.error('Trade flow auto-scope forensic export error:', err);
    return NextResponse.json(
      { error: 'Failed to export auto-scope forensic trade analysis' },
      { status: 500 }
    );
  }
}
