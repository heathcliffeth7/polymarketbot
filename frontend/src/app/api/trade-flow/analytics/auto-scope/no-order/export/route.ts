import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveAutoScopeTradeAnalysisDateFilters } from '@/lib/queries/trade-flow/analysis-time-range';
import {
  buildAutoScopeNoOrderSignalsCsv,
  getAutoScopeNoOrderSignalsForExport,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { searchParams } = new URL(req.url);
    const runIdRaw = searchParams.get('runId');
    const runId = runIdRaw ? Number(runIdRaw) : null;
    const dateFilters = resolveAutoScopeTradeAnalysisDateFilters({
      timeRangeRaw: searchParams.get('timeRange'),
      fromRaw: searchParams.get('from'),
      toRaw: searchParams.get('to'),
    });
    if (runId != null && (!Number.isFinite(runId) || runId < 1)) {
      return NextResponse.json({ error: 'runId must be >= 1' }, { status: 400 });
    }
    if (dateFilters.error) {
      return NextResponse.json({ error: dateFilters.error }, { status: 400 });
    }

    const signals = await getAutoScopeNoOrderSignalsForExport({
      userId: user.userId,
      runId: runId == null ? null : Math.floor(runId),
      from: dateFilters.from,
      to: dateFilters.to,
      limit: 2000,
    });
    const csv = buildAutoScopeNoOrderSignalsCsv(signals);
    const date = new Date().toISOString().slice(0, 10);
    const scope = runId == null ? 'all' : String(Math.floor(runId));

    return new NextResponse(csv, {
      headers: {
        'Content-Type': 'text/csv; charset=utf-8',
        'Content-Disposition': `attachment; filename="auto-scope-no-order-${scope}-${date}.csv"`,
        'Cache-Control': 'no-store',
      },
    });
  } catch (err) {
    console.error('Trade flow auto-scope no-order export error:', err);
    return NextResponse.json(
      { error: 'Failed to export auto-scope no-order signals' },
      { status: 500 }
    );
  }
}
