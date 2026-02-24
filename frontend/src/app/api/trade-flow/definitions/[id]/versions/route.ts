import { NextRequest, NextResponse } from 'next/server';
import { getTradeFlowVersions } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const data = await getTradeFlowVersions(definitionId);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow versions list error:', err);
    return NextResponse.json({ error: 'Failed to load flow versions' }, { status: 500 });
  }
}
