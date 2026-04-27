import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveAutoScopeTradeAnalysisDateFilters } from '@/lib/queries/trade-flow/analysis-time-range';
import { buildDecisionLogsRawCsv } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { searchParams } = new URL(req.url);
    const dateFilters = resolveAutoScopeTradeAnalysisDateFilters({
      timeRangeRaw: searchParams.get('timeRange'),
      fromRaw: searchParams.get('from'),
      toRaw: searchParams.get('to'),
    });

    if (dateFilters.error) {
      return NextResponse.json({ error: dateFilters.error }, { status: 400 });
    }

    const csv = await buildDecisionLogsRawCsv({
      userId: user.userId,
      from: dateFilters.from,
      to: dateFilters.to,
    });
    const date = new Date().toISOString().slice(0, 10);

    return new NextResponse(csv, {
      headers: {
        'Content-Type': 'text/csv; charset=utf-8',
        'Content-Disposition': `attachment; filename="decision_logs_raw-${date}.csv"`,
        'Cache-Control': 'no-store',
      },
    });
  } catch (err) {
    console.error('Decision log raw export error:', err);
    return NextResponse.json({ error: 'Failed to export decision logs' }, { status: 500 });
  }
}
