import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  buildAutoScopeTradeAnalysisCsv,
  getAutoScopeTradeAnalysisRowsForExport,
} from '@/lib/queries/trade-flow';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
} from '@/lib/types';

export const dynamic = 'force-dynamic';

function parseOptionalDate(value: string | null): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  const parsed = new Date(trimmed);
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString();
}

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
    const fromRaw = searchParams.get('from');
    const toRaw = searchParams.get('to');
    const from = parseOptionalDate(fromRaw);
    const to = parseOptionalDate(toRaw);

    if (fromRaw && !from) {
      return NextResponse.json({ error: 'from must be a valid date' }, { status: 400 });
    }
    if (toRaw && !to) {
      return NextResponse.json({ error: 'to must be a valid date' }, { status: 400 });
    }

    const rows = await getAutoScopeTradeAnalysisRowsForExport({
      userId: user.userId,
      sortBy,
      sortDirection,
      pnl,
      position,
      from,
      to,
    });
    const csv = buildAutoScopeTradeAnalysisCsv(rows);
    const date = new Date().toISOString().slice(0, 10);

    return new NextResponse(csv, {
      headers: {
        'Content-Type': 'text/csv; charset=utf-8',
        'Content-Disposition': `attachment; filename="auto-scope-trade-analysis-${date}.csv"`,
        'Cache-Control': 'no-store',
      },
    });
  } catch (err) {
    console.error('Trade flow auto-scope analytics export error:', err);
    return NextResponse.json(
      { error: 'Failed to export auto-scope trade analysis' },
      { status: 500 }
    );
  }
}
