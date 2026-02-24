import { NextRequest, NextResponse } from 'next/server';
import { publishTradeFlowDefinition } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function POST(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const data = await publishTradeFlowDefinition(definitionId);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow publish error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to publish flow' },
      { status: 500 }
    );
  }
}
